//! P09 one bounded, operator-triggered US ETF retrieval. Shares P03 ownership,
//! P06 provenance/snapshots and P08 semantics. No scheduler or alternate engine.
use super::{
    canonical_json,
    http::{FetchError, HttpFetcher, UreqFetcher},
    provenance::{self, RawObservation},
    registry::{self, InstrumentDraft},
    sha256_hex,
    snapshot::{self, SnapshotKind, SnapshotOutcome, SnapshotRequest, SnapshotRow},
    sources::tiingo::{self, EodRow},
    tiingo_credentials,
};
use crate::{
    db::{
        self,
        ownership::HolderKind,
        repositories::{self, Dataset},
    },
    error::{AppError, AppResult},
    research::artifacts::ArtifactStore,
};
use alpha_factor_forge::discovery_core::{
    etf::{
        self, ActionEvidence, DividendAction, EtfCostProfile, EtfMarket, NativeCurrency,
        SplitAction,
    },
    market_foundation::{
        audit_coverage, parse_instrument_id, utc_date_to_ms, AssetType, CoverageRequest, Market,
        PriceBasis, SeriesRole,
    },
};
use chrono::{NaiveDate, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

const DAY: i64 = 86_400_000;
pub const SETTINGS_VERSION: &str = "tiingo-settings-v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentSettings {
    pub market: EtfMarket,
    pub exchange_code: String,
    pub listed_from: String,
    pub listing_evidence: String,
    pub calendar_evidence: String,
    /// Operator confirms the source's split/dividend completeness for this
    /// calendar evidence interval. HTTP 200 alone is not that confirmation.
    pub corporate_actions_confirmed: bool,
    pub costs: Option<EtfCostProfile>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub version: String,
    pub instruments: Vec<InstrumentSettings>,
}

impl Settings {
    pub fn validate(&self) -> AppResult<()> {
        if self.version != SETTINGS_VERSION || self.instruments.len() > 5 {
            return Err(error("invalid_settings_version_or_count"));
        }
        let mut seen = HashSet::new();
        for s in &self.instruments {
            etf::validate_etf_market(&s.market).map_err(error)?;
            let id = parse_instrument_id(&s.market.instrument_id)
                .map_err(|_| error("invalid_instrument"))?;
            if id.market != Market::UsEtf
                || !tiingo::SYMBOLS.contains(&id.symbol.as_str())
                || !seen.insert(id.symbol.clone())
            {
                return Err(error("unsupported_or_duplicate_instrument"));
            }
            // Explicit source-code-to-venue mapping prevents a source response
            // from silently changing the registered market identity.
            if !matches!(
                (id.venue.as_str(), s.exchange_code.as_str()),
                ("nyse-arca", "NYSE ARCA") | ("nasdaq", "NASDAQ")
            ) {
                return Err(error("unsupported_exchange_mapping"));
            }
            if s.calendar_evidence.trim().is_empty()
                || s.listing_evidence.trim().is_empty()
                || utc_date_to_ms(&s.listed_from).is_none()
            {
                return Err(error("market_evidence_missing"));
            }
            if let Some(c) = &s.costs {
                etf::validate_etf_costs(c, NativeCurrency::USD).map_err(error)?;
            }
        }
        Ok(())
    }
}
fn error(code: &str) -> AppError {
    AppError::Other(code.into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub settings: PathBuf,
    pub data_dir: Option<PathBuf>,
    pub from: NaiveDate,
    pub to: NaiveDate,
}
pub fn parse_args<S: AsRef<str>>(args: &[S]) -> Result<Options, String> {
    let mut flags = std::collections::HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let text = args[i].as_ref();
        i += 1;
        let (flag, value) = if let Some(pair) = text.split_once('=') {
            pair
        } else {
            let value = args.get(i).ok_or("Tiingo option needs a value")?.as_ref();
            i += 1;
            (text, value)
        };
        if !["--settings", "--data-dir", "--from", "--to"].contains(&flag)
            || value.is_empty()
            || flags.insert(flag, value).is_some()
        {
            return Err("unknown, empty or duplicate Tiingo option (credentials never belong on the command line)".into());
        }
    }
    Ok(Options {
        settings: PathBuf::from(
            *flags
                .get("--settings")
                .ok_or("fetch-tiingo needs --settings")?,
        ),
        data_dir: flags.get("--data-dir").map(PathBuf::from),
        from: super::fetch::parse_date(flags.get("--from").ok_or("fetch-tiingo needs --from")?)?,
        to: super::fetch::parse_date(flags.get("--to").ok_or("fetch-tiingo needs --to")?)?,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: &'static str,
    pub symbol: String,
    pub instrument_id: Option<String>,
    pub from: String,
    pub to_exclusive: String,
    pub status: &'static str,
    /// Data eligibility only. Never strategy/Test/paper approval.
    pub qualification_eligible: bool,
    pub reasons: Vec<String>,
    pub provenance_ids: Vec<i64>,
    pub bars: usize,
    pub coverage: Value,
    pub dividends: Vec<DividendAction>,
    pub splits: Vec<SplitAction>,
    pub dataset_id: Option<i64>,
    pub snapshot: Option<SnapshotRow>,
}
impl Report {
    fn new(symbol: &str, from: &str, to: &str) -> Self {
        Self {
            version: tiingo::VERSION,
            symbol: symbol.into(),
            instrument_id: None,
            from: from.into(),
            to_exclusive: to.into(),
            status: "BLOCKED",
            qualification_eligible: false,
            reasons: vec![],
            provenance_ids: vec![],
            bars: 0,
            coverage: Value::Null,
            dividends: vec![],
            splits: vec![],
            dataset_id: None,
            snapshot: None,
        }
    }
    fn reason(&mut self, code: &str) {
        if !self.reasons.iter().any(|r| r == code) {
            self.reasons.push(code.into());
        }
    }
}

pub fn run(data_dir: &Path, options: &Options) -> AppResult<Vec<Report>> {
    // Read bounded, non-secret configuration BEFORE opening/migrating a DB.
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(&options.settings)?
        .take(262_145)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 262_144 {
        return Err(error("settings_too_large"));
    }
    let settings: Settings =
        serde_json::from_slice(&bytes).map_err(|_| error("invalid_tiingo_settings"))?;
    settings.validate()?;
    let from = options.from.to_string();
    let to = options.to.to_string();
    let as_of = Utc::now().timestamp_millis();
    validate_range(&from, &to, as_of)?;
    let token = tiingo_credentials::read();
    let (fetcher, credential_issue) = match token {
        Ok(t) => (Some(UreqFetcher::for_tiingo(t)), None),
        Err(code) => (None, Some(code)),
    };
    let workspace =
        crate::runtime::open_workspace(&data_dir.join(db::DB_FILE_NAME), HolderKind::Service)?;
    let store = ArtifactStore::in_data_dir(data_dir);
    let mut conn = workspace.db.lock().map_err(|_| error("db lock poisoned"))?;
    ingest(
        &mut conn,
        &store,
        fetcher.as_ref().map(|f| f as &dyn HttpFetcher),
        credential_issue,
        &settings,
        &from,
        &to,
        as_of,
    )
}

fn validate_range(from: &str, to: &str, as_of: i64) -> AppResult<(i64, i64)> {
    let start = utc_date_to_ms(from).ok_or_else(|| error("invalid_from_date"))?;
    let end = utc_date_to_ms(to).ok_or_else(|| error("invalid_to_date"))?;
    // Daily-only bounded history, conservatively finalized at next-day 06:00
    // UTC (after the documented 20:00 US Eastern correction window in either
    // DST season). No present-day bar or silently clipped future request.
    if end <= start || end - start > 366 * DAY {
        return Err(error("range_must_be_1_to_366_days"));
    }
    if end + 6 * 3_600_000 > as_of {
        return Err(error("range_not_finalized"));
    }
    Ok((start, end))
}

#[allow(clippy::too_many_arguments)]
pub fn ingest(
    conn: &mut Connection,
    store: &ArtifactStore,
    fetcher: Option<&dyn HttpFetcher>,
    credential_issue: Option<&str>,
    settings: &Settings,
    from: &str,
    to: &str,
    as_of: i64,
) -> AppResult<Vec<Report>> {
    settings.validate()?;
    let (start, end) = validate_range(from, to, as_of)?;
    let mut reports = Vec::new();
    let mut batch_stop: Option<&str> = None;
    for symbol in tiingo::SYMBOLS {
        let mut report = Report::new(symbol, from, to);
        let configured = settings
            .instruments
            .iter()
            .find(|s| s.market.instrument_id.ends_with(&format!(":{symbol}")));
        if let Some(s) = configured {
            report.instrument_id = Some(s.market.instrument_id.clone());
            if let Some(reason) = credential_issue {
                report.reason(reason);
            } else if let Some(reason) = batch_stop {
                report.reason(reason);
            } else if let Some(f) = fetcher {
                ingest_symbol(
                    conn,
                    store,
                    f,
                    s,
                    &mut report,
                    start,
                    end,
                    as_of,
                    &mut batch_stop,
                )?;
            } else {
                report.reason("credential_missing");
            }
            // Derived result is immutable and queryable through the existing
            // provenance store, including failures and its complete settings.
            let body = canonical_json(&serde_json::to_value(&report)?)?;
            let observed = Utc::now().to_rfc3339();
            provenance::record_raw(
                conn,
                store,
                &RawObservation {
                    instrument_id: s.market.instrument_id.clone(),
                    interval: "1d".into(),
                    source: "tiingo-report".into(),
                    role: SeriesRole::Comparison,
                    request_scope: json!({"version":tiingo::VERSION,"from":from,"toExclusive":to,"settings":s}),
                    retrieved_at: observed.clone(),
                    available_at: Some(observed),
                    accepted: true,
                    rejection_reason: None,
                    revision_of: None,
                    media_type: "application/json".into(),
                },
                &body,
            )?;
        } else {
            report.reason("source_settings_missing");
        }
        reports.push(report);
    }
    Ok(reports)
}

fn fetch_code(error: &FetchError) -> &'static str {
    match error {
        FetchError::Status { status: 401, .. } => "authentication_required",
        FetchError::Status { status: 403, .. } => "entitlement_denied",
        FetchError::Status { status: 429, .. } => "quota_exhausted",
        FetchError::NotFound(_) => "source_not_found",
        FetchError::TooLarge { .. } => "response_too_large",
        FetchError::Refused(_) => "source_request_refused",
        _ => "source_unavailable",
    }
}

// Record original bytes before returning any parse outcome. Revision links
// are scoped to the exact endpoint/range; re-fetch never overwrites history.
#[allow(clippy::too_many_arguments)]
fn retrieve<T>(
    conn: &Connection,
    store: &ArtifactStore,
    f: &dyn HttpFetcher,
    s: &InstrumentSettings,
    url: &str,
    report: &mut Report,
    batch_stop: &mut Option<&'static str>,
    parse: impl FnOnce(&[u8]) -> Result<T, &'static str>,
) -> AppResult<Option<T>> {
    let (bytes, result, transport) = match f.get(url) {
        Ok(bytes) => {
            let result = parse(&bytes);
            (bytes, result, false)
        }
        Err(e) => {
            let code = fetch_code(&e);
            if code == "quota_exhausted" || code == "authentication_required" {
                *batch_stop = Some(code);
            }
            // Never persist a provider error body/header or an HTTP debug error:
            // they may contain credentials. This is a local failure receipt.
            (
                serde_json::to_vec(&json!({"localFailureReceipt":code}))?,
                Err(code),
                true,
            )
        }
    };
    let previous = provenance::list_provenance(conn, &s.market.instrument_id, "1d")?
        .into_iter()
        .rev()
        .find(|p| {
            p.source == tiingo::SOURCE
                && p.request_scope["endpoint"] == url
                && !p.request_scope["localFailureReceipt"]
                    .as_bool()
                    .unwrap_or(false)
        });
    // Chain even byte-identical observations: a later changed response must
    // supersede ALL prior equivalent observations, not just the last duplicate.
    let revision = if transport {
        None
    } else {
        previous.map(|p| p.id)
    };
    let now = Utc::now().to_rfc3339();
    let (id, _) = provenance::record_raw(
        conn,
        store,
        &RawObservation {
            instrument_id: s.market.instrument_id.clone(),
            interval: "1d".into(),
            source: tiingo::SOURCE.into(),
            role: SeriesRole::Primary,
            request_scope: json!({"version":tiingo::VERSION,"endpoint":url,"settings":s,"localFailureReceipt":transport,"availabilityBasis":"response-completed"}),
            retrieved_at: now.clone(),
            available_at: if transport { None } else { Some(now) },
            accepted: result.is_ok(),
            rejection_reason: result.as_ref().err().map(|code| (*code).into()),
            revision_of: revision,
            media_type: "application/json".into(),
        },
        &bytes,
    )?;
    report.provenance_ids.push(id);
    match result {
        Ok(v) => Ok(Some(v)),
        Err(code) => {
            report.reason(code);
            Ok(None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn ingest_symbol(
    conn: &mut Connection,
    store: &ArtifactStore,
    f: &dyn HttpFetcher,
    s: &InstrumentSettings,
    report: &mut Report,
    start: i64,
    end: i64,
    as_of: i64,
    batch_stop: &mut Option<&'static str>,
) -> AppResult<()> {
    let expected = match etf::etf_session_dates(&s.market, &report.from, &report.to_exclusive, &[])
    {
        Ok(v) => v,
        Err(code) => {
            report.reason(code);
            return Ok(());
        }
    };
    if start < utc_date_to_ms(&s.listed_from).expect("validated listing") {
        report.reason("before_listing");
        return Ok(());
    }
    let Some(&first) = expected.first() else {
        report.reason("no_sessions");
        return Ok(());
    };
    let last = *expected.last().expect("nonempty sessions");
    let final_date = chrono::DateTime::from_timestamp_millis(end - DAY)
        .expect("validated date")
        .format("%Y-%m-%d")
        .to_string();
    let symbol = report.symbol.clone();
    let urls = tiingo::urls(&symbol, &report.from, &final_date);
    if retrieve(conn, store, f, s, &urls[0], report, batch_stop, |b| {
        tiingo::parse_metadata(b, &symbol, &s.exchange_code, first, last)
    })?
    .is_none()
    {
        return Ok(());
    }
    let Some(rows) = retrieve(conn, store, f, s, &urls[1], report, batch_stop, |b| {
        tiingo::parse_eod(b, start, end)
    })?
    else {
        return Ok(());
    };
    let observed: Vec<i64> = rows.iter().map(|r| r.candle().timestamp).collect();
    let coverage = audit_coverage(&CoverageRequest {
        interval: "1d",
        expected: &expected,
        observed: &observed,
        as_of_ms: as_of,
    });
    report.bars = rows.len();
    report.coverage = serde_json::to_value(&coverage)?;
    if coverage.blocking || observed != expected {
        report.reason("requested_range_incomplete_or_off_session");
        return Ok(());
    }
    let events = retrieve(conn, store, f, s, &urls[2], report, batch_stop, |b| {
        tiingo::reconcile_dividends(b, &symbol, &s.market.instrument_id, &rows, start, end)
    })?;
    let events_ok = events.is_some();
    report.dividends = events.unwrap_or_else(|| {
        rows.iter()
            .filter(|r| r.div_cash > 0.0)
            .map(|r| r.dividend(&s.market.instrument_id))
            .collect()
    });
    // Observed at response completion, never at the historical ex/split date.
    let observed_at = Utc::now().timestamp_millis();
    report.splits = rows
        .iter()
        .filter(|r| r.split_factor != 1.0)
        .map(|r| r.split(&s.market.instrument_id, observed_at))
        .collect();
    let unconfirmed = EtfCostProfile {
        version: "unconfirmed-v1".into(),
        currency: NativeCurrency::USD,
        confirmed: false,
        commission_rate: 0.0,
        minimum_commission: 0.0,
        slippage_rate: 0.0,
        buy_tax_rate: 0.0,
        sell_tax_rate: 0.0,
    };
    let costs = s.costs.as_ref().unwrap_or(&unconfirmed);
    let evidence = ActionEvidence {
        version: tiingo::VERSION.into(),
        complete: events_ok && s.corporate_actions_confirmed,
        from: report.from.clone(),
        to_exclusive: report.to_exclusive.clone(),
        dividends: report.dividends.clone(),
    };
    let verdict = etf::assess_etf_semantics(
        &s.market,
        costs,
        &evidence,
        &report.from,
        &report.to_exclusive,
    )
    .map_err(error)?;
    for reason in &verdict.reasons {
        report.reason(reason);
    }

    registry::register_calendar(conn, &s.market.calendar)?;
    let parsed =
        parse_instrument_id(&s.market.instrument_id).map_err(|_| error("invalid_instrument"))?;
    let existing = registry::latest_instrument(conn, &s.market.instrument_id)?;
    // Preserve richer trading specifications. A changed listing/calendar must
    // be explicitly registered elsewhere, never silently erased by a download.
    if let Some(ref old) = existing {
        if old.session_calendar_id != s.market.calendar.calendar_id
            || old.quote != "USD"
            || old.base != parsed.symbol
            || old.timezone != s.market.calendar.timezone
            || old.listed_from != utc_date_to_ms(&s.listed_from)
            || old.delisted_at.is_some()
            || !old.suspensions.is_empty()
        {
            report.reason("registered_market_requires_reconciliation");
            return Ok(());
        }
    } else {
        registry::register_instrument(
            conn,
            &InstrumentDraft {
                instrument_id: s.market.instrument_id.clone(),
                base: parsed.symbol.clone(),
                quote: "USD".into(),
                asset_type: AssetType::Etf,
                session_calendar_id: s.market.calendar.calendar_id.clone(),
                timezone: s.market.calendar.timezone.clone(),
                lot_size: None,
                price_step: None,
                min_notional: None,
                listed_from: utc_date_to_ms(&s.listed_from),
                delisted_at: None,
                suspensions: vec![],
                source_capabilities: json!({"source":tiingo::SOURCE,"raw":true,"adjusted":"audit-only","dividends":events_ok,"split":true,"forwardObserved":false}),
            },
        )?;
    }
    let candles: Vec<_> = rows.iter().map(EodRow::candle).collect();
    let mut dataset = Dataset {
        id: None,
        exchange: parsed.venue,
        symbol,
        interval: "1d".into(),
        start_time: first,
        end_time: last,
        candle_count: candles.len() as i64,
        source: "exchange".into(),
        dataset_hash: String::new(),
    };
    dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, &candles)?;
    let dataset_id = repositories::import_dataset_with_candles(conn, &dataset, &candles)?;
    report.dataset_id = Some(dataset_id);
    let mut accepted = Vec::new();
    for id in &report.provenance_ids {
        let row =
            provenance::get_provenance(conn, *id)?.ok_or_else(|| error("missing_provenance"))?;
        if row.accepted {
            accepted.push(*id);
        }
    }
    // Include content hashes of the complete settings and action observations:
    // merely reusing a cost version label cannot alias different assumptions.
    let action_hash = sha256_hex(&canonical_json(
        &json!({"evidence":evidence,"splits":report.splits,"sources":report.provenance_ids,"settings":s}),
    )?);
    let cost_hash = sha256_hex(&canonical_json(&serde_json::to_value(costs)?)?);
    let action_ok = events_ok
        && s.corporate_actions_confirmed
        && !verdict.reasons.contains(&"unknown_payment_date");
    let outcome = snapshot::build_snapshot(
        conn,
        &SnapshotRequest {
            instrument_id: s.market.instrument_id.clone(),
            interval: "1d".into(),
            dataset_id,
            price_basis: PriceBasis::Raw,
            corporate_action_version: action_ok.then(|| format!("tiingo-actions-v1:{action_hash}")),
            cost_profile_version: costs
                .confirmed
                .then(|| format!("{}:{cost_hash}", costs.version)),
            kind: SnapshotKind::Historical,
            provenance_ids: accepted,
            as_of_ms: as_of,
        },
    )?;
    match outcome {
        SnapshotOutcome::Created(row) | SnapshotOutcome::Existing(row) => {
            report.qualification_eligible =
                row.qualification_eligible() && report.reasons.is_empty();
            report.status = if report.qualification_eligible {
                "OK"
            } else {
                "DEGRADED"
            };
            report.snapshot = Some(row);
        }
        SnapshotOutcome::Blocked { .. } => report.reason("snapshot_blocked"),
    }
    Ok(())
}

#[cfg(test)]
mod tests;
