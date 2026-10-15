//! Bounded, operator-reviewed crypto listing evidence registration.
//!
//! Source captures are checked before workspace acquisition. Registration
//! appends an instrument revision; frozen snapshots keep their original row.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use alpha_factor_forge::discovery_core::market_data::{
    MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE, MIN_MARKET_TIMESTAMP_MS,
};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{ownership, DB_FILE_NAME};
use crate::error::{AppError, AppResult};
use crate::runtime;

use super::registry::{self, InstrumentDraft, InstrumentRow};
use super::sha256_hex;

pub const VERSION: &str = "crypto-listing-registration-v1";
pub const MAX_SETTINGS_BYTES: u64 = 32 * 1024;
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024;
const EVIDENCE_KEY: &str = "listingRegistration";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub settings: PathBuf,
    pub data_dir: Option<PathBuf>,
}

pub fn parse_args<S: AsRef<str>>(args: &[S]) -> Result<Options, String> {
    let mut settings = None;
    let mut data_dir = None;
    let mut index = 0;
    while index < args.len() {
        let text = args[index].as_ref();
        index += 1;
        let (flag, value) = if let Some(pair) = text.split_once('=') {
            pair
        } else {
            let value = args
                .get(index)
                .ok_or("listing option needs a value")?
                .as_ref();
            index += 1;
            (text, value)
        };
        if value.is_empty() || value.starts_with("--") {
            return Err("listing option needs a nonempty value".into());
        }
        let destination = match flag {
            "--settings" => &mut settings,
            "--data-dir" => &mut data_dir,
            _ => return Err(format!("unknown listing option: {flag}")),
        };
        if destination.replace(PathBuf::from(value)).is_some() {
            return Err(format!("duplicate listing option: {flag}"));
        }
    }
    Ok(Options {
        settings: settings.ok_or("register-crypto-listing needs --settings")?,
        data_dir,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    version: String,
    instrument_id: String,
    listed_from: String,
    source_url: String,
    retrieved_at: String,
    raw_file: String,
    raw_sha256: String,
    media_type: String,
    operator_confirmed: bool,
    evidence_excerpt: String,
}

/// Inline originals avoid a second metadata table or a fake candle series.
/// Every field is covered by the instrument's existing canonical hash.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListingEvidence {
    version: String,
    instrument_id: String,
    listed_from: String,
    source_url: String,
    retrieved_at: String,
    raw_sha256: String,
    raw_byte_len: u64,
    raw_content: String,
    media_type: String,
    operator_confirmed: bool,
    evidence_excerpt: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub version: &'static str,
    pub instrument_id: String,
    pub row_id: i64,
    pub revision: i64,
    pub content_hash: String,
    pub listed_from: i64,
    pub raw_sha256: String,
    pub created: bool,
}

fn error(message: impl Into<String>) -> AppError {
    AppError::Other(format!("crypto listing registration: {}", message.into()))
}

fn read_bounded(path: &Path, limit: u64) -> AppResult<Vec<u8>> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(error("input must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(error(format!("input exceeds {limit} bytes")));
    }
    Ok(bytes)
}

fn utc_millis(name: &str, value: &str) -> AppResult<i64> {
    let instant = DateTime::parse_from_rfc3339(value)
        .map_err(|_| error(format!("{name} must be a canonical UTC RFC3339 time")))?;
    if instant.timestamp_subsec_nanos() >= 1_000_000_000
        || instant.timestamp_subsec_nanos() % 1_000_000 != 0
    {
        return Err(error(format!(
            "{name} must have integer millisecond precision"
        )));
    }
    let precision = if instant.timestamp_subsec_millis() == 0 {
        SecondsFormat::Secs
    } else {
        SecondsFormat::Millis
    };
    if instant.with_timezone(&Utc).to_rfc3339_opts(precision, true) != value {
        return Err(error(format!("{name} must use canonical UTC Z notation")));
    }
    Ok(instant.timestamp_millis())
}

fn official_announcement_url(value: &str) -> AppResult<()> {
    if !value.is_ascii()
        || value.bytes().any(|b| b.is_ascii_whitespace())
        || value.contains(['?', '#', '%', '\\'])
    {
        return Err(error(
            "sourceUrl must be an official HTTPS Binance announcement URL",
        ));
    }
    super::http::check_url(value, &["binance.com".into(), "www.binance.com".into()])
        .map_err(|_| error("sourceUrl must be an official HTTPS Binance announcement URL"))?;
    let path = value
        .strip_prefix("https://")
        .expect("checked HTTPS")
        .split_once('/')
        .map(|(_, path)| path)
        .unwrap_or("");
    let components: Vec<_> = path.split('/').collect();
    let announcement = match components.as_slice() {
        [locale, "support", "announcement", "detail", id] if valid_locale(locale) => Some(*id),
        [locale, "support", "announcement", id] if valid_locale(locale) => Some(*id),
        ["support", "announcement", "detail", id] => Some(*id),
        ["support", "announcement", id] => Some(*id),
        _ => None,
    };
    if !announcement.is_some_and(|id| {
        !id.is_empty()
            && id != "detail"
            && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return Err(error(
            "sourceUrl must identify a Binance support announcement",
        ));
    }
    Ok(())
}

fn valid_locale(value: &str) -> bool {
    (2..=5).contains(&value.len())
        && value
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphabetic()))
}

fn supported_draft(instrument_id: &str) -> AppResult<InstrumentDraft> {
    registry::default_crypto_instruments()
        .into_iter()
        .find(|draft| draft.instrument_id == instrument_id)
        .ok_or_else(|| {
            error("instrumentId must be crypto:binance:BTCUSDT or crypto:binance:ETHUSDT")
        })
}

fn names_pair(excerpt: &str, base: &str) -> bool {
    [format!("{base}USDT"), format!("{base}/USDT")]
        .iter()
        .any(|pair| {
            excerpt.match_indices(pair).any(|(start, _)| {
                let end = start + pair.len();
                excerpt[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_ascii_alphanumeric())
                    && excerpt[end..]
                        .chars()
                        .next()
                        .is_none_or(|c| !c.is_ascii_alphanumeric())
            })
        })
}

impl ListingEvidence {
    fn validate(&self, now_ms: i64) -> AppResult<i64> {
        if self.version != VERSION {
            return Err(error("unsupported settings/evidence version"));
        }
        let draft = supported_draft(&self.instrument_id)?;
        let listed = utc_millis("listedFrom", &self.listed_from)?;
        let retrieved = utc_millis("retrievedAt", &self.retrieved_at)?;
        if !(MIN_MARKET_TIMESTAMP_MS..MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE).contains(&listed) {
            return Err(error("listedFrom is outside the market timestamp range"));
        }
        if listed > now_ms || retrieved > now_ms || retrieved < listed {
            return Err(error(
                "listing/retrieval cannot be future and retrieval cannot precede listing",
            ));
        }
        official_announcement_url(&self.source_url)?;
        if !["text/plain", "text/html", "application/json"].contains(&self.media_type.as_str()) {
            return Err(error("unsupported source mediaType"));
        }
        if !self.operator_confirmed {
            return Err(error("operatorConfirmed must be true"));
        }
        if self.raw_content.is_empty()
            || self.raw_content.len() as u64 > MAX_SOURCE_BYTES
            || self.raw_byte_len != self.raw_content.len() as u64
            || self.raw_sha256.len() != 64
            || !self
                .raw_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || sha256_hex(self.raw_content.as_bytes()) != self.raw_sha256
        {
            return Err(error(
                "source bytes, length and lower-case SHA-256 must match",
            ));
        }
        if self.evidence_excerpt.trim().is_empty()
            || !self.raw_content.contains(&self.evidence_excerpt)
            || !names_pair(&self.evidence_excerpt, &draft.base)
        {
            return Err(error(
                "evidenceExcerpt must occur verbatim in the source and identify the declared pair",
            ));
        }
        Ok(listed)
    }
}

fn load_evidence(settings_path: &Path, now_ms: i64) -> AppResult<ListingEvidence> {
    let bytes = read_bounded(settings_path, MAX_SETTINGS_BYTES)?;
    let settings: Settings = serde_json::from_slice(&bytes)?;
    if settings.raw_file.trim().is_empty() || settings.raw_file.contains('\0') {
        return Err(error("rawFile must name the original source capture"));
    }
    let raw_path = Path::new(&settings.raw_file);
    let raw_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        settings_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(raw_path)
    };
    let raw = read_bounded(&raw_path, MAX_SOURCE_BYTES)?;
    let evidence = ListingEvidence {
        version: settings.version,
        instrument_id: settings.instrument_id,
        listed_from: settings.listed_from,
        source_url: settings.source_url,
        retrieved_at: settings.retrieved_at,
        raw_sha256: settings.raw_sha256,
        raw_byte_len: raw.len() as u64,
        raw_content: String::from_utf8(raw).map_err(|_| error("source capture must be UTF-8"))?,
        media_type: settings.media_type,
        operator_confirmed: settings.operator_confirmed,
        evidence_excerpt: settings.evidence_excerpt,
    };
    evidence.validate(now_ms)?;
    Ok(evidence)
}

fn draft_from_row(row: &InstrumentRow) -> InstrumentDraft {
    InstrumentDraft {
        instrument_id: row.instrument_id.clone(),
        base: row.base.clone(),
        quote: row.quote.clone(),
        asset_type: row.asset_type,
        session_calendar_id: row.session_calendar_id.clone(),
        timezone: row.timezone.clone(),
        lot_size: row.lot_size,
        price_step: row.price_step,
        min_notional: row.min_notional,
        listed_from: row.listed_from,
        delisted_at: row.delisted_at,
        suspensions: row.suspensions.clone(),
        source_capabilities: row.source_capabilities.clone(),
    }
}

fn register_evidence(
    conn: &Connection,
    epoch: Option<i64>,
    evidence: &ListingEvidence,
    now_ms: i64,
) -> AppResult<Receipt> {
    let listed_from = evidence.validate(now_ms)?;
    let tx = ownership::write_transaction_quiet(conn, epoch)?;
    let latest = registry::latest_instrument(&tx, &evidence.instrument_id)?;
    let mut draft = match latest {
        Some(ref row) => draft_from_row(row),
        None => supported_draft(&evidence.instrument_id)?,
    };
    if draft
        .listed_from
        .is_some_and(|existing| existing != listed_from)
    {
        return Err(error(
            "known listing differs; use a separately reviewed correction contract",
        ));
    }
    let capabilities = draft
        .source_capabilities
        .as_object_mut()
        .ok_or_else(|| error("existing sourceCapabilities must be an object"))?;
    if let Some(previous) = capabilities.get(EVIDENCE_KEY) {
        let previous: ListingEvidence = serde_json::from_value(previous.clone())
            .map_err(|_| error("existing listingRegistration evidence is invalid"))?;
        let previous_ms = previous
            .validate(now_ms)
            .map_err(|_| error("existing listingRegistration evidence is invalid"))?;
        if previous.instrument_id != draft.instrument_id || Some(previous_ms) != draft.listed_from {
            return Err(error(
                "existing listingRegistration evidence differs from the instrument",
            ));
        }
    }
    capabilities.insert(EVIDENCE_KEY.into(), serde_json::to_value(evidence)?);
    draft.listed_from = Some(listed_from);
    let (row_id, created) = registry::register_instrument(&tx, &draft)?;
    if created {
        tx.execute(
            "UPDATE runtime_state SET mutation_seq = mutation_seq + 1 WHERE id = 1",
            [],
        )?;
    }
    let row = registry::get_instrument(&tx, row_id)?
        .ok_or_else(|| error("registered instrument is missing"))?;
    // Registry deduplication includes historical revisions. Returning a
    // superseded row would imply that a subsequent fetch uses this evidence,
    // even though the latest revision remains a different one.
    if !created && latest.as_ref().is_some_and(|latest| latest.id != row_id) {
        return Err(error(
            "evidence belongs to a superseded revision; historical evidence cannot be reactivated",
        ));
    }
    let receipt = Receipt {
        version: VERSION,
        instrument_id: row.instrument_id,
        row_id,
        revision: row.revision,
        content_hash: row.content_hash,
        listed_from,
        raw_sha256: evidence.raw_sha256.clone(),
        created,
    };
    tx.commit()?;
    Ok(receipt)
}

pub fn run(data_dir: &Path, options: &Options) -> AppResult<Receipt> {
    let now_ms = Utc::now().timestamp_millis();
    let evidence = load_evidence(&options.settings, now_ms)?;
    let workspace =
        runtime::open_workspace(&data_dir.join(DB_FILE_NAME), ownership::HolderKind::Service)?;
    let conn = workspace.db.lock().map_err(|_| error("db lock poisoned"))?;
    let receipt = register_evidence(&conn, Some(workspace.ownership.epoch), &evidence, now_ms);
    drop(conn);
    drop(workspace);
    receipt
}

#[cfg(test)]
mod tests;
