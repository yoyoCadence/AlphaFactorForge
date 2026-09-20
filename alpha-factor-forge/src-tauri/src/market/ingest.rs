//! P07 — one bounded retrieval: plan the units a range needs, fetch each
//! one from the same exchange, record what came back (accepted or refused),
//! assemble what was accepted, and let P06 decide whether the result is a
//! snapshot research may read.
//!
//! The shape of a retrieval (`docs/market-contract.md` §6):
//!
//! 1. the calendar and the requested range say which units should exist;
//! 2. each unit is fetched, checksum-verified, and parsed, or refused with
//!    a stable code that is *kept* as a rejected provenance record;
//! 3. a unit the archive has not published is information, not a failure —
//!    the REST source of the same exchange covers the tail;
//! 4. what was accepted is assembled, conflicts between units are refused
//!    rather than resolved, and the result goes through the existing
//!    dataset import (identity + candle plausibility) unchanged;
//! 5. the coverage audit decides. A gap is reported with its range and its
//!    action; it does not become a snapshot.
//!
//! Nothing here widens what a snapshot means: a gap still blocks, and an
//! unconfirmed cost profile still leaves it degraded.

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::json;

use alpha_factor_forge::discovery_core::market_foundation::{
    audit_coverage, expected_bar_starts, CoverageReport, CoverageRequest, ExpectedRangeRequest,
    interval_ms, PriceBasis, SeriesRole, Severity, TimeUnit,
};

use crate::db::repositories::{self, Candle, Dataset};
use crate::error::{AppError, AppResult};
use crate::research::artifacts::ArtifactStore;

use super::http::{FetchError, HttpFetcher};
use super::provenance::{self, ProvenanceRow, QualityEventRow, RawObservation};
use super::registry;
use super::snapshot::{self, SnapshotKind, SnapshotOutcome, SnapshotRequest, SnapshotRow};
use super::sources::binance::{self, Kline, SourceError};
use super::QualityEvent;

/// What `datasets.source` records for anything this adapter imports. The
/// precise source of every byte is in `market_provenance`; this column keeps
/// the vocabulary 0001 documents (`csv | exchange | import`) and must stay
/// stable, because a re-ingest of the same bars has to land on the same row.
const DATASET_SOURCE: &str = "exchange";

/// How many times one unit's bytes are re-fetched when the published
/// checksum does not match what arrived. Bounded on purpose: a mismatch
/// that survives a retry is evidence about the source, not a hiccup.
const CHECKSUM_ATTEMPTS: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestRequest {
    pub instrument_id: String,
    pub interval: String,
    /// Inclusive, in milliseconds.
    pub from_ms: i64,
    /// Exclusive, in milliseconds.
    pub to_ms_exclusive: i64,
    /// The data cut: a bar that has not closed by this instant is not
    /// retrieved at all, rather than retrieved and then flagged.
    pub as_of_ms: i64,
    /// `None` leaves every snapshot built from this degraded, which is the
    /// contract's default until a user confirms the cost model.
    pub cost_profile_version: Option<String>,
    /// Whether the exchange's REST source may cover what the archive has
    /// not published yet.
    pub allow_rest: bool,
}

/// What became of one unit of retrieval.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UnitStatus {
    /// Retrieved now and accepted.
    Fetched {
        bars: usize,
        provenance_id: i64,
        source_time_unit: String,
        converted_to_milliseconds: bool,
    },
    /// Already retrieved by an earlier run; the stored bytes were reused.
    Cached { bars: usize, provenance_id: i64 },
    /// The source has not published it. Nothing was recorded.
    NotPublished,
    /// Retrieved and refused; the bytes and the reason are kept.
    Rejected { code: String, detail: String, provenance_id: Option<i64> },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitOutcome {
    /// `monthly:2024-07`, `daily:2024-07-15`, `rest:<from>-<to>`.
    pub unit: String,
    pub source: String,
    pub url: String,
    #[serde(flatten)]
    pub status: UnitStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum IngestOutcome {
    /// The range is complete under its calendar and became a snapshot.
    Snapshot(Box<SnapshotRow>),
    /// What was retrieved is a complete dataset in its own right — it may
    /// even be a snapshot — but the REQUESTED range is not covered.
    ///
    /// The distinction matters and is not a technicality: a snapshot says
    /// "these bars are everything the calendar expects BETWEEN THIS
    /// DATASET'S OWN BOUNDS" (`market-snapshot-v1`), so a range that simply
    /// stops early would otherwise look complete. The range audit is the
    /// question the requester actually asked, and a gap in it refuses the
    /// retrieval as a whole.
    RangeIncomplete {
        snapshot: Option<Box<SnapshotRow>>,
        events: Vec<QualityEventRow>,
    },
    /// Everything that was retrieved was imported, but the dataset is not
    /// admissible; the events say which bars and what to do.
    Blocked { events: Vec<QualityEventRow> },
    /// Two units disagree about the same bar. Nothing was imported.
    SourceConflict { events: Vec<QualityEventRow> },
    /// Nothing was accepted, so there is no dataset to speak of.
    NothingRetrieved,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestReport {
    pub instrument_id: String,
    pub interval: String,
    pub requested_from_ms: i64,
    pub requested_to_ms_exclusive: i64,
    pub as_of_ms: i64,
    pub units: Vec<UnitOutcome>,
    /// Bars accepted, after clipping to the range and dropping bars that
    /// have not closed at the cut.
    pub bars: usize,
    /// Bars the source published that had not closed at the cut. They are
    /// not retrieved into the dataset; the next run picks them up.
    pub unclosed_dropped: usize,
    pub dataset_id: Option<i64>,
    /// The coverage audit of the **requested** range, whatever became of
    /// the dataset: what the calendar expected, what arrived, and what is
    /// still missing.
    pub range_coverage: serde_json::Value,
    #[serde(flatten)]
    pub outcome: IngestOutcome,
}

impl IngestReport {
    /// The one-line answer: is this range usable for research?
    pub fn headline(&self) -> String {
        match &self.outcome {
            IngestOutcome::Snapshot(snapshot) => format!(
                "snapshot {} ({}), {} bars, qualification {}",
                &snapshot.snapshot_id[..12],
                snapshot.status.as_str(),
                self.bars,
                if snapshot.qualification_eligible() { "eligible" } else { "not eligible" }
            ),
            IngestOutcome::RangeIncomplete { snapshot, events } => format!(
                "{} bars imported{}, but the requested range is missing {} range(s)",
                self.bars,
                match snapshot {
                    Some(snapshot) => format!(" as snapshot {}", &snapshot.snapshot_id[..12]),
                    None => String::new(),
                },
                events.len()
            ),
            IngestOutcome::Blocked { events } => {
                format!("blocked by {} event(s), {} bars imported", events.len(), self.bars)
            }
            IngestOutcome::SourceConflict { events } => {
                format!("sources disagree ({} event(s)); nothing imported", events.len())
            }
            IngestOutcome::NothingRetrieved => "nothing was retrieved".to_string(),
        }
    }
}

// ------------------------------------------------------------- unit plan

#[derive(Clone, Debug, PartialEq, Eq)]
enum Unit {
    Monthly { year: i32, month: u32 },
    Daily { date: NaiveDate },
}

impl Unit {
    fn label(&self) -> String {
        match self {
            Unit::Monthly { year, month } => format!("monthly:{year:04}-{month:02}"),
            Unit::Daily { date } => format!("daily:{date}"),
        }
    }

    fn url(&self, symbol: &str, interval: &str) -> String {
        match self {
            Unit::Monthly { year, month } => binance::monthly_url(symbol, interval, *year, *month),
            Unit::Daily { date } => binance::daily_url(symbol, interval, *date),
        }
    }

    fn file_name(&self, symbol: &str, interval: &str) -> String {
        match self {
            Unit::Monthly { year, month } => binance::monthly_file_name(symbol, interval, *year, *month),
            Unit::Daily { date } => binance::daily_file_name(symbol, interval, *date),
        }
    }
}

fn date_of(ms: i64) -> AppResult<NaiveDate> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|moment| moment.date_naive())
        .ok_or_else(|| AppError::Other(format!("{ms} is not a representable instant")))
}

fn start_of_day_ms(date: NaiveDate) -> i64 {
    date.and_hms_opt(0, 0, 0).expect("midnight exists").and_utc().timestamp_millis()
}

fn first_of_month(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).expect("the first of a month exists")
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 { (year + 1, 1) } else { (year, month + 1) }
}

/// The months the range touches, oldest first.
fn months_in_range(from_ms: i64, to_ms_exclusive: i64) -> AppResult<Vec<(i32, u32)>> {
    let first = date_of(from_ms)?;
    let last = date_of(to_ms_exclusive - 1)?;
    let mut months = Vec::new();
    let (mut year, mut month) = (first.year(), first.month());
    while (year, month) <= (last.year(), last.month()) {
        months.push((year, month));
        let (next_year, next_month_number) = next_month(year, month);
        year = next_year;
        month = next_month_number;
    }
    Ok(months)
}

/// The days of one month that the range touches, oldest first.
fn days_of_month_in_range(year: i32, month: u32, from_ms: i64, to_ms_exclusive: i64) -> Vec<NaiveDate> {
    let mut days = Vec::new();
    let mut date = first_of_month(year, month);
    while date.month() == month && date.year() == year {
        let start = start_of_day_ms(date);
        let end = start + 86_400_000;
        if end > from_ms && start < to_ms_exclusive {
            days.push(date);
        }
        date = date.succ_opt().expect("the next day exists");
    }
    days
}

// ------------------------------------------------------------- retrieval

struct Accepted {
    provenance_id: i64,
    rows: Vec<Kline>,
}

/// Fetch, verify, parse, and record one archive unit.
fn retrieve_archive_unit(
    conn: &Connection,
    store: &ArtifactStore,
    fetcher: &dyn HttpFetcher,
    request: &IngestRequest,
    symbol: &str,
    unit: &Unit,
) -> AppResult<(UnitOutcome, Option<Accepted>)> {
    let url = unit.url(symbol, &request.interval);
    let file_name = unit.file_name(symbol, &request.interval);
    let scope = json!({
        "unit": unit.label(),
        "url": url,
        "fileName": file_name,
        "interval": request.interval,
    });
    let outcome = |status: UnitStatus| UnitOutcome {
        unit: unit.label(),
        source: binance::SOURCE_ARCHIVE.to_string(),
        url: url.clone(),
        status,
    };

    // Already retrieved and accepted by an earlier run? Then the bytes are
    // in the artifact store and the source does not get asked again.
    if let Some((row, bytes)) = cached_bytes(conn, store, &request.instrument_id, &request.interval, &url)? {
        return match parse_archive_bytes(&bytes) {
            Ok(parsed) => Ok((
                outcome(UnitStatus::Cached { bars: parsed.rows.len(), provenance_id: row.id }),
                Some(Accepted { provenance_id: row.id, rows: parsed.rows }),
            )),
            // Stored bytes that no longer parse are a defect worth showing,
            // not a reason to silently re-download.
            Err(error) => Ok((
                outcome(UnitStatus::Rejected {
                    code: error.code.to_string(),
                    detail: format!("stored bytes for {url}: {}", error.detail),
                    provenance_id: Some(row.id),
                }),
                None,
            )),
        };
    }

    let checksum_url = binance::checksum_url(&url);
    for attempt in 1..=CHECKSUM_ATTEMPTS {
        let digest_bytes = match fetcher.get(&checksum_url) {
            Ok(bytes) => bytes,
            Err(FetchError::NotFound(_)) => return Ok((outcome(UnitStatus::NotPublished), None)),
            Err(error) => return Err(error.into()),
        };
        // Observe each response after it arrives, including retries. Request
        // start time cannot prove that either response was available yet.
        let checksum_time = Utc::now().to_rfc3339();
        let parsed = binance::parse_checksum(&digest_bytes, &file_name);
        let checksum_scope = json!({
            "unit": unit.label(),
            "url": checksum_url,
            "fileName": format!("{file_name}.CHECKSUM"),
            "vouchesFor": file_name,
            "interval": request.interval,
            "kind": "checksum",
            "availabilityBasis": binance::AVAILABILITY_BASIS,
            "periodEndsAt": unit_period_end(unit),
        });
        // Preserve the publisher's original response, not just its parsed
        // digest. Every ZIP attempt below links to this exact observation.
        let checksum_id = record_observation(
            conn, store, request, binance::SOURCE_ARCHIVE, checksum_scope,
            &checksum_time, Some(checksum_time.clone()),
            parsed.as_ref().map(|_| ()), &digest_bytes, "text/plain",
        )?;
        let digest = match parsed {
            Ok(digest) => digest,
            Err(error) => return Ok((
                outcome(UnitStatus::Rejected {
                    code: error.code.to_string(), detail: error.detail,
                    provenance_id: Some(checksum_id),
                }),
                None,
            )),
        };
        let bytes = match fetcher.get(&url) {
            Ok(bytes) => bytes,
            Err(FetchError::NotFound(_)) => return Ok((outcome(UnitStatus::NotPublished), None)),
            Err(error) => return Err(error.into()),
        };
        let archive_time = Utc::now().to_rfc3339();
        let mut scope = scope.clone();
        scope["availabilityBasis"] = json!(binance::AVAILABILITY_BASIS);
        scope["periodEndsAt"] = json!(unit_period_end(unit));
        scope["checksumProvenanceId"] = json!(checksum_id);
        scope["publishedSha256"] = json!(digest);
        let parsed = binance::verify_checksum(&bytes, &digest)
            .and_then(|()| parse_archive_bytes(&bytes));
        if let Ok(parsed) = &parsed {
            scope["sourceTimeUnit"] = json!(parsed.unit.as_str());
            scope["convertedToMilliseconds"] = json!(parsed.converted);
        }
        // Store failures before retrying: a successful second attempt must
        // not erase the first response or the checksum it was tested against.
        let provenance_id = record_observation(
            conn, store, request, binance::SOURCE_ARCHIVE, scope,
            &archive_time, Some(archive_time.clone()),
            parsed.as_ref().map(|_| ()), &bytes, "application/zip",
        )?;
        match parsed {
            Ok(parsed) => return Ok((
                outcome(UnitStatus::Fetched {
                    bars: parsed.rows.len(), provenance_id,
                    source_time_unit: parsed.unit.as_str().to_string(),
                    converted_to_milliseconds: parsed.converted,
                }),
                Some(Accepted { provenance_id, rows: parsed.rows }),
            )),
            Err(error) if error.code == "checksum_mismatch" && attempt < CHECKSUM_ATTEMPTS => continue,
            Err(error) => return Ok((
                outcome(UnitStatus::Rejected {
                    code: error.code.to_string(), detail: error.detail,
                    provenance_id: Some(provenance_id),
                }),
                None,
            )),
        }
    }
    unreachable!("at least one checksum attempt is configured")
}

struct ParsedUnit {
    rows: Vec<Kline>,
    unit: TimeUnit,
    converted: bool,
}

fn parse_archive_bytes(bytes: &[u8]) -> Result<ParsedUnit, SourceError> {
    let entry = binance::read_single_zip_entry(bytes)?;
    let parsed = binance::parse_archive_csv(&entry.content)?;
    Ok(ParsedUnit {
        rows: parsed.rows,
        unit: parsed.source_time_unit,
        converted: parsed.converted_to_milliseconds,
    })
}

/// End of the archive period, retained as descriptive metadata only.
/// Publication/observation may be later, and a historical re-download may
/// include subsequent revisions. This is never an availability claim.
fn unit_period_end(unit: &Unit) -> String {
    let end = match unit {
        Unit::Monthly { year, month } => {
            let (next_year, next_month_number) = next_month(*year, *month);
            start_of_day_ms(first_of_month(next_year, next_month_number))
        }
        Unit::Daily { date } => start_of_day_ms(*date) + 86_400_000,
    };
    Utc.timestamp_millis_opt(end)
        .single()
        .expect("an archive unit ends at a representable instant")
        .to_rfc3339()
}

/// One REST page. Unlike the archive there is no published checksum, so the
/// evidence is the response itself: the exact request and the exact bytes.
fn retrieve_rest_page(
    conn: &Connection,
    store: &ArtifactStore,
    fetcher: &dyn HttpFetcher,
    request: &IngestRequest,
    symbol: &str,
    from_ms: i64,
    to_ms_exclusive: i64,
) -> AppResult<(UnitOutcome, Option<Accepted>)> {
    let url = binance::rest_klines_url(symbol, &request.interval, from_ms, to_ms_exclusive, binance::REST_MAX_LIMIT);
    let label = format!("rest:{from_ms}-{to_ms_exclusive}");
    let scope = json!({
        "unit": label,
        "url": url,
        "interval": request.interval,
        "startTime": from_ms,
        "endTimeExclusive": to_ms_exclusive,
    });
    let outcome = |status: UnitStatus| UnitOutcome {
        unit: label.clone(),
        source: binance::SOURCE_REST.to_string(),
        url: url.clone(),
        status,
    };
    let bytes = match fetcher.get(&url) {
        Ok(bytes) => bytes,
        Err(FetchError::NotFound(_)) => return Ok((outcome(UnitStatus::NotPublished), None)),
        Err(error) => return Err(error.into()),
    };
    let now = Utc::now().to_rfc3339();
    match binance::parse_rest_klines(&bytes) {
        Ok(parsed) => {
            let mut scope = scope;
            scope["sourceTimeUnit"] = json!(parsed.source_time_unit.as_str());
            scope["convertedToMilliseconds"] = json!(parsed.converted_to_milliseconds);
            let provenance_id = record_observation(
                conn,
                store,
                request,
                binance::SOURCE_REST,
                scope,
                &now,
                // A REST answer is observable when it is answered.
                Some(now.clone()),
                Ok(()),
                &bytes,
                "application/json",
            )?;
            Ok((
                outcome(UnitStatus::Fetched {
                    bars: parsed.rows.len(),
                    provenance_id,
                    source_time_unit: parsed.source_time_unit.as_str().to_string(),
                    converted_to_milliseconds: parsed.converted_to_milliseconds,
                }),
                Some(Accepted { provenance_id, rows: parsed.rows }),
            ))
        }
        Err(error) => {
            let provenance_id = record_observation(
                conn,
                store,
                request,
                binance::SOURCE_REST,
                scope,
                &now,
                Some(now.clone()),
                Err(&error),
                &bytes,
                "application/json",
            )?;
            Ok((
                outcome(UnitStatus::Rejected {
                    code: error.code.to_string(),
                    detail: error.detail,
                    provenance_id: Some(provenance_id),
                }),
                None,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_observation(
    conn: &Connection,
    store: &ArtifactStore,
    request: &IngestRequest,
    source: &str,
    request_scope: serde_json::Value,
    retrieved_at: &str,
    available_at: Option<String>,
    verdict: Result<(), &SourceError>,
    bytes: &[u8],
    media_type: &str,
) -> AppResult<i64> {
    let observation = RawObservation {
        instrument_id: request.instrument_id.clone(),
        interval: request.interval.clone(),
        source: source.to_string(),
        role: SeriesRole::Primary,
        request_scope,
        retrieved_at: retrieved_at.to_string(),
        available_at,
        accepted: verdict.is_ok(),
        rejection_reason: verdict.err().map(|error| error.to_string()),
        revision_of: None,
        media_type: media_type.to_string(),
    };
    Ok(provenance::record_raw(conn, store, &observation, bytes)?.0)
}

/// An accepted record of the same URL from an earlier run, with its bytes.
fn cached_bytes(
    conn: &Connection,
    store: &ArtifactStore,
    instrument_id: &str,
    interval: &str,
    url: &str,
) -> AppResult<Option<(ProvenanceRow, Vec<u8>)>> {
    let rows = provenance::list_provenance(conn, instrument_id, interval)?;
    // Do not fall back to an older record when the newest evidence is
    // incomplete or superseded. Legacy records remain immutable history;
    // a fresh retrieval supplies evidence the old adapter never retained.
    let Some(row) = rows.into_iter().rfind(|row| {
        row.accepted && row.source == binance::SOURCE_ARCHIVE && row.request_scope["url"] == url
    }) else {
        return Ok(None);
    };
    if row.raw_media_type != "application/zip"
        || !row.has_observed_archive_availability()
        || provenance::revised_by(conn, row.id)?.is_some()
    {
        return Ok(None);
    }
    let Some(checksum_id) = row.request_scope["checksumProvenanceId"].as_i64() else {
        return Ok(None);
    };
    let Some(checksum) = provenance::get_provenance(conn, checksum_id)? else {
        return Ok(None);
    };
    let Some(file_name) = row.request_scope["fileName"].as_str() else {
        return Ok(None);
    };
    if !checksum.accepted
        || checksum.source != row.source
        || checksum.instrument_id != row.instrument_id
        || checksum.interval != row.interval
        || checksum.raw_media_type != "text/plain"
        || checksum.request_scope["kind"] != "checksum"
        || checksum.request_scope["url"] != binance::checksum_url(url)
        || checksum.request_scope["vouchesFor"] != file_name
        || !checksum.has_observed_archive_availability()
        || provenance::revised_by(conn, checksum.id)?.is_some()
    {
        return Ok(None);
    }
    let (Ok(bytes), Ok(digest_bytes)) = (
        provenance::read_raw(store, &row), provenance::read_raw(store, &checksum),
    ) else {
        return Ok(None);
    };
    let Ok(digest) = binance::parse_checksum(&digest_bytes, file_name) else {
        return Ok(None);
    };
    if row.request_scope["publishedSha256"] != digest
        || binance::verify_checksum(&bytes, &digest).is_err()
    {
        return Ok(None);
    }
    Ok(Some((row, bytes)))
}

// -------------------------------------------------------------- assembly

/// Merge the units' rows, refusing rather than resolving a disagreement.
fn merge_rows(accepted: &[Accepted]) -> Result<Vec<Kline>, (i64, Vec<i64>)> {
    let mut merged: Vec<(Kline, i64)> = Vec::new();
    for unit in accepted {
        for row in &unit.rows {
            merged.push((*row, unit.provenance_id));
        }
    }
    merged.sort_by_key(|(row, _)| row.open_time_ms);
    let mut out: Vec<Kline> = Vec::with_capacity(merged.len());
    let mut sources: Vec<i64> = Vec::new();
    for (row, provenance_id) in merged {
        match out.last() {
            Some(previous) if previous.open_time_ms == row.open_time_ms => {
                let same = previous.open.to_bits() == row.open.to_bits()
                    && previous.high.to_bits() == row.high.to_bits()
                    && previous.low.to_bits() == row.low.to_bits()
                    && previous.close.to_bits() == row.close.to_bits()
                    && previous.volume.to_bits() == row.volume.to_bits();
                if !same {
                    let mut conflicting = sources.clone();
                    conflicting.push(provenance_id);
                    conflicting.sort_unstable();
                    conflicting.dedup();
                    return Err((row.open_time_ms, conflicting));
                }
            }
            _ => out.push(row),
        }
        if !sources.contains(&provenance_id) {
            sources.push(provenance_id);
        }
    }
    Ok(out)
}

// ----------------------------------------------------------------- entry

/// Retrieve `request`'s range for one instrument.
pub fn ingest(
    conn: &mut Connection,
    store: &ArtifactStore,
    fetcher: &dyn HttpFetcher,
    request: &IngestRequest,
) -> AppResult<IngestReport> {
    let cadence = interval_ms(&request.interval).ok_or_else(|| {
        AppError::Other(format!("interval {:?} is not one this contract knows", request.interval))
    })?;
    if request.to_ms_exclusive <= request.from_ms {
        return Err(AppError::Other("the requested range ends before it starts".into()));
    }
    let instrument = registry::latest_instrument(conn, &request.instrument_id)?.ok_or_else(|| {
        AppError::Other(format!(
            "instrument {} is not registered; register it before retrieving its data",
            request.instrument_id
        ))
    })?;
    if instrument.venue != binance::VENUE {
        return Err(AppError::Other(format!(
            "{} is a {} instrument; this adapter only serves {}",
            instrument.instrument_id, instrument.venue, binance::VENUE
        )));
    }
    let symbol = instrument.symbol.clone();

    let mut units: Vec<UnitOutcome> = Vec::new();
    let mut accepted: Vec<Accepted> = Vec::new();
    for (year, month) in months_in_range(request.from_ms, request.to_ms_exclusive)? {
        let monthly = Unit::Monthly { year, month };
        let (outcome, rows) =
            retrieve_archive_unit(conn, store, fetcher, request, &symbol, &monthly)?;
        let published = !matches!(outcome.status, UnitStatus::NotPublished);
        units.push(outcome);
        if let Some(rows) = rows {
            accepted.push(rows);
        }
        if published {
            continue;
        }
        // The month is not published as one file: ask for its days, and for
        // whatever the daily archive has not published either.
        for date in days_of_month_in_range(year, month, request.from_ms, request.to_ms_exclusive) {
            let daily = Unit::Daily { date };
            let (outcome, rows) =
                retrieve_archive_unit(conn, store, fetcher, request, &symbol, &daily)?;
            let published = !matches!(outcome.status, UnitStatus::NotPublished);
            units.push(outcome);
            if let Some(rows) = rows {
                accepted.push(rows);
            }
            if published || !request.allow_rest {
                continue;
            }
            let day_start = start_of_day_ms(date).max(request.from_ms);
            let day_end = (start_of_day_ms(date) + 86_400_000).min(request.to_ms_exclusive);
            // Only closed bars are asked for at all.
            let closed_end = day_end.min(request.as_of_ms - (request.as_of_ms % cadence));
            let mut cursor = day_start;
            while cursor < closed_end {
                let page_end =
                    (cursor + cadence * binance::REST_MAX_LIMIT as i64).min(closed_end);
                let (outcome, rows) =
                    retrieve_rest_page(conn, store, fetcher, request, &symbol, cursor, page_end)?;
                let progressed = match &rows {
                    Some(rows) if !rows.rows.is_empty() => {
                        rows.rows.last().map(|last| last.open_time_ms + cadence).unwrap_or(page_end)
                    }
                    _ => page_end,
                };
                units.push(outcome);
                if let Some(rows) = rows {
                    accepted.push(rows);
                }
                cursor = progressed.max(cursor + cadence);
            }
        }
    }

    let mut report = IngestReport {
        instrument_id: request.instrument_id.clone(),
        interval: request.interval.clone(),
        requested_from_ms: request.from_ms,
        requested_to_ms_exclusive: request.to_ms_exclusive,
        as_of_ms: request.as_of_ms,
        units,
        bars: 0,
        unclosed_dropped: 0,
        dataset_id: None,
        range_coverage: serde_json::Value::Null,
        outcome: IngestOutcome::NothingRetrieved,
    };

    let merged = match merge_rows(&accepted) {
        Ok(rows) => rows,
        Err((timestamp, provenance_ids)) => {
            let event = QualityEvent::new(
                &request.instrument_id,
                &request.interval,
                "source_conflict",
                Severity::Blocking,
                "separate_sources",
            )
            .with_detail(json!({
                "openTimeMs": timestamp,
                "provenanceIds": provenance_ids,
                "reason": "two retrievals describe the same bar differently; both are kept",
            }))
            .with_range(timestamp, timestamp, 1);
            let id = provenance::record_quality_event(conn, &event)?;
            let events = provenance::list_quality_events(conn, &request.instrument_id, &request.interval, 1)?
                .into_iter()
                .filter(|row| row.id == id)
                .collect();
            report.outcome = IngestOutcome::SourceConflict { events };
            return Ok(report);
        }
    };

    // Clip to the request, and never import a bar that had not closed at
    // the cut: an unfinished bar is not data yet.
    let mut unclosed = 0usize;
    let candles: Vec<Candle> = merged
        .into_iter()
        .filter(|row| row.open_time_ms >= request.from_ms && row.open_time_ms < request.to_ms_exclusive)
        .filter(|row| {
            let closed = row.open_time_ms + cadence <= request.as_of_ms;
            if !closed {
                unclosed += 1;
            }
            closed
        })
        .map(|row| Candle {
            timestamp: row.open_time_ms,
            open: row.open,
            high: row.high,
            low: row.low,
            close: row.close,
            volume: row.volume,
        })
        .collect();
    report.unclosed_dropped = unclosed;
    report.bars = candles.len();

    // The question the requester asked: is the RANGE covered? A snapshot
    // only ever answers for the dataset's own bounds, so a retrieval that
    // stopped early would look complete without this.
    let observed: Vec<i64> = candles.iter().map(|candle| candle.timestamp).collect();
    let range = audit_requested_range(conn, &instrument, request, &observed)?;
    report.range_coverage = serde_json::to_value(&range)?;
    let range_events = if range.blocking {
        record_range_events(conn, request, &range, None)?
    } else {
        Vec::new()
    };

    if candles.is_empty() {
        return Ok(report);
    }

    let mut dataset = Dataset {
        id: None,
        exchange: instrument.venue.clone(),
        symbol: instrument.symbol.clone(),
        interval: request.interval.clone(),
        start_time: candles[0].timestamp,
        end_time: candles[candles.len() - 1].timestamp,
        candle_count: candles.len() as i64,
        source: DATASET_SOURCE.to_string(),
        dataset_hash: String::new(),
    };
    dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, &candles)?;
    let dataset_id = repositories::import_dataset_with_candles(conn, &dataset, &candles)?;
    report.dataset_id = Some(dataset_id);

    let mut provenance_ids: Vec<i64> = accepted.iter().map(|unit| unit.provenance_id).collect();
    provenance_ids.sort_unstable();
    provenance_ids.dedup();
    let built = snapshot::build_snapshot(
        conn,
        &SnapshotRequest {
            instrument_id: request.instrument_id.clone(),
            interval: request.interval.clone(),
            dataset_id,
            price_basis: PriceBasis::Raw,
            // Spot crypto has no corporate actions; the ETF phases fill this.
            corporate_action_version: None,
            cost_profile_version: request.cost_profile_version.clone(),
            kind: SnapshotKind::Historical,
            provenance_ids,
            as_of_ms: request.as_of_ms,
        },
    )?;
    report.outcome = match built {
        SnapshotOutcome::Created(row) | SnapshotOutcome::Existing(row) if range_events.is_empty() => {
            IngestOutcome::Snapshot(Box::new(row))
        }
        SnapshotOutcome::Created(row) | SnapshotOutcome::Existing(row) => {
            IngestOutcome::RangeIncomplete {
                snapshot: Some(Box::new(row)),
                events: range_events,
            }
        }
        // A dataset that cannot be a snapshot is the stronger statement, so
        // it is what the outcome reports; the range evidence is recorded
        // either way and both sets are queryable by dataset.
        SnapshotOutcome::Blocked { events } => IngestOutcome::Blocked { events },
    };
    Ok(report)
}

/// Audit the requested range against the instrument's calendar, whatever
/// the dataset's own bounds turned out to be.
fn audit_requested_range(
    conn: &Connection,
    instrument: &registry::InstrumentRow,
    request: &IngestRequest,
    observed: &[i64],
) -> AppResult<CoverageReport> {
    let calendar = registry::get_calendar(conn, &instrument.session_calendar_id)?.ok_or_else(|| {
        AppError::Other(format!(
            "calendar {} is missing for instrument {}",
            instrument.session_calendar_id, instrument.instrument_id
        ))
    })?;
    let expectation = expected_bar_starts(&ExpectedRangeRequest {
        calendar: &calendar,
        interval: &request.interval,
        from_ms: request.from_ms,
        to_ms_exclusive: request.to_ms_exclusive,
        listed_from_ms: instrument.listed_from,
        delisted_at_ms: instrument.delisted_at,
        suspensions: &instrument.suspensions,
    });
    if let Some(issue) = expectation.issue {
        return Err(AppError::Other(format!(
            "the requested range has no expectation under {}: {}",
            calendar.calendar_id,
            issue.as_str()
        )));
    }
    Ok(audit_coverage(&CoverageRequest {
        interval: &request.interval,
        expected: &expectation.timestamps,
        observed,
        as_of_ms: request.as_of_ms,
    }))
}

/// Persist the range audit's blocking events, so a blocked retrieval leaves
/// the same queryable evidence a blocked snapshot does.
fn record_range_events(
    conn: &Connection,
    request: &IngestRequest,
    coverage: &CoverageReport,
    dataset_id: Option<i64>,
) -> AppResult<Vec<QualityEventRow>> {
    let mut recorded = Vec::new();
    for event in &coverage.events {
        let mut quality = QualityEvent::new(
            &request.instrument_id,
            &request.interval,
            event.code.as_str(),
            event.severity,
            event.action,
        )
        .with_detail(json!({
            "scope": "requested-range",
            "requestedFromMs": request.from_ms,
            "requestedToMsExclusive": request.to_ms_exclusive,
            "bars": event.count,
        }))
        .with_range(event.range_start, event.range_end, event.count);
        quality.dataset_id = dataset_id;
        let id = provenance::record_quality_event(conn, &quality)?;
        recorded.push(QualityEventRow { id, created_at: String::new(), event: quality });
    }
    Ok(recorded)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::db;
    use crate::market::http::testing::FakeFetcher;
    use crate::market::registry::{default_crypto_instruments, ensure_builtin_calendars, register_instrument};
    use crate::market::snapshot::{dataset_market_status, DatasetMarketStatus, SnapshotStatus};

    const HOUR: i64 = 3_600_000;
    const DAY: i64 = 86_400_000;
    /// 2024-07-15T00:00:00Z, the day the committed fixtures cover.
    const FIXTURE_DAY: i64 = 1_721_001_600_000;
    /// 2026-09-18T00:00:00Z, the microsecond-era fixture day.
    const MICRO_DAY: i64 = 1_789_689_600_000;

    const BTC_MS_ZIP: &[u8] = include_bytes!("../../../fixtures/binance/BTCUSDT-1h-2024-07-15.zip");
    const BTC_MS_CHECKSUM: &[u8] =
        include_bytes!("../../../fixtures/binance/BTCUSDT-1h-2024-07-15.zip.CHECKSUM");
    const BTC_US_ZIP: &[u8] = include_bytes!("../../../fixtures/binance/BTCUSDT-1h-2026-09-18.zip");
    const BTC_US_CHECKSUM: &[u8] =
        include_bytes!("../../../fixtures/binance/BTCUSDT-1h-2026-09-18.zip.CHECKSUM");

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            if self.0.exists() {
                std::fs::remove_dir_all(&self.0)
                    .unwrap_or_else(|error| panic!("temp dir {} not removed: {error}", self.0.display()));
            }
        }
    }

    fn fresh_store() -> (ArtifactStore, TempDir) {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("aff-ingest-test-{}-{n}", std::process::id()));
        (ArtifactStore::in_data_dir(&dir), TempDir(dir))
    }

    fn workspace() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON").expect("foreign keys");
        db::apply_migrations(&conn).expect("migrations");
        ensure_builtin_calendars(&conn).expect("built-in calendars");
        for draft in default_crypto_instruments() {
            register_instrument(&conn, &draft).expect("instrument");
        }
        conn
    }

    fn daily(date: &str) -> String {
        format!("https://data.binance.vision/data/spot/daily/klines/BTCUSDT/1h/BTCUSDT-1h-{date}.zip")
    }

    fn monthly(month: &str) -> String {
        format!("https://data.binance.vision/data/spot/monthly/klines/BTCUSDT/1h/BTCUSDT-1h-{month}.zip")
    }

    /// The fixture day, served as the daily archive unit.
    fn fixture_day_fetcher() -> FakeFetcher {
        FakeFetcher::new()
            .with(&daily("2024-07-15"), BTC_MS_ZIP)
            .with(&format!("{}.CHECKSUM", daily("2024-07-15")), BTC_MS_CHECKSUM)
    }

    fn request(from_ms: i64, to_ms_exclusive: i64, as_of_ms: i64) -> IngestRequest {
        IngestRequest {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            from_ms,
            to_ms_exclusive,
            as_of_ms,
            cost_profile_version: Some("cost-profile-v1".into()),
            allow_rest: false,
        }
    }

    #[test]
    fn a_complete_day_is_retrieved_verified_recorded_and_becomes_a_snapshot() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let fetcher = fixture_day_fetcher();
        let report = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();

        assert_eq!(report.bars, 24);
        assert_eq!(report.unclosed_dropped, 0);
        // The month is asked for first, then the day it does not publish.
        assert_eq!(
            report.units.iter().map(|unit| unit.unit.as_str()).collect::<Vec<_>>(),
            vec!["monthly:2024-07", "daily:2024-07-15"]
        );
        assert!(matches!(report.units[0].status, UnitStatus::NotPublished));
        let UnitStatus::Fetched { bars, provenance_id, ref source_time_unit, converted_to_milliseconds } =
            report.units[1].status
        else {
            panic!("the day must be fetched: {:?}", report.units[1]);
        };
        assert_eq!((bars, source_time_unit.as_str(), converted_to_milliseconds), (24, "milliseconds", false));

        let IngestOutcome::Snapshot(snapshot) = &report.outcome else {
            panic!("a complete day must become a snapshot: {:?}", report.outcome)
        };
        assert_eq!(snapshot.status, SnapshotStatus::Ok);
        assert!(snapshot.qualification_eligible());
        assert_eq!(report.dataset_id, Some(1));
        assert_eq!(
            dataset_market_status(&conn, 1).unwrap(),
            DatasetMarketStatus::Registered(Box::new((**snapshot).clone()))
        );

        // The raw bytes are kept and still verify, and the retrieval is the
        // snapshot's recorded source.
        let sources = snapshot::snapshot_sources(&conn, snapshot.id).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, provenance_id);
        assert_eq!(provenance::read_raw(&store, &sources[0]).unwrap(), BTC_MS_ZIP);
        assert_eq!(sources[0].request_scope["publishedSha256"].as_str().unwrap().len(), 64);
        assert_eq!(sources[0].raw_media_type, "application/zip");
    }

    #[test]
    fn a_second_run_reuses_the_stored_bytes_instead_of_asking_again() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let fetcher = fixture_day_fetcher();
        let first = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();
        let requested_once = fetcher.requested().len();

        let again = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();
        assert!(matches!(again.units[1].status, UnitStatus::Cached { bars: 24, .. }));
        // Only the unpublished month was asked about again; the day's bytes
        // came from the store.
        assert_eq!(
            fetcher.requested().len() - requested_once,
            1,
            "a cached unit costs one request, not three: {:?}",
            fetcher.requested()
        );
        assert_eq!(again.dataset_id, first.dataset_id, "the same bars are the same dataset");
        assert_eq!(again.bars, 24);
        let (IngestOutcome::Snapshot(first), IngestOutcome::Snapshot(again)) = (&first.outcome, &again.outcome)
        else {
            panic!("both runs must produce a snapshot")
        };
        assert_eq!(first.snapshot_id, again.snapshot_id, "and the same snapshot");
    }

    #[test]
    fn a_microsecond_era_unit_is_converted_and_lands_on_the_same_grid() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let url = daily("2026-09-18");
        let fetcher = FakeFetcher::new()
            .with(&url, BTC_US_ZIP)
            .with(&format!("{url}.CHECKSUM"), BTC_US_CHECKSUM);
        let report = ingest(&mut conn, &store, &fetcher, &request(MICRO_DAY, MICRO_DAY + DAY, MICRO_DAY + DAY))
            .unwrap();

        let UnitStatus::Fetched { ref source_time_unit, converted_to_milliseconds, provenance_id, .. } =
            report.units[1].status
        else {
            panic!("the day must be fetched: {:?}", report.units[1])
        };
        assert_eq!((source_time_unit.as_str(), converted_to_milliseconds), ("microseconds", true));
        assert!(matches!(report.outcome, IngestOutcome::Snapshot(_)));
        // The conversion is recorded where the bytes are, not just applied.
        let row = provenance::get_provenance(&conn, provenance_id).unwrap().unwrap();
        assert_eq!(row.request_scope["sourceTimeUnit"], json!("microseconds"));
        assert_eq!(row.request_scope["convertedToMilliseconds"], json!(true));
        let stored = repositories::get_candles(&conn, 1, i64::MIN, i64::MAX).unwrap();
        assert_eq!(stored.len(), 24);
        assert_eq!(stored[0].timestamp, MICRO_DAY, "in milliseconds, on the hourly grid");
        assert_eq!(stored[23].timestamp, MICRO_DAY + 23 * HOUR);
    }

    /// The P07 acceptance review's first finding. An archive file downloaded
    /// today says nothing about when it first became obtainable, so it must
    /// never support a snapshot that claims to be point-in-time evidence
    /// from back then — which is what recording the archive PERIOD's end as
    /// the availability used to allow.
    #[test]
    fn a_history_download_can_never_become_forward_observed_evidence_for_its_own_period() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let fetcher = fixture_day_fetcher();
        let report = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();
        let UnitStatus::Fetched { provenance_id, .. } = report.units[1].status else {
            panic!("the day must be fetched: {:?}", report.units[1])
        };

        // Availability is the observation we really made — now — and the
        // period's end is recorded beside it as information, not as a claim.
        let row = provenance::get_provenance(&conn, provenance_id).unwrap().unwrap();
        assert_eq!(row.available_at.as_deref(), Some(row.retrieved_at.as_str()));
        assert_eq!(row.request_scope["availabilityBasis"], json!("observed-at-retrieval"));
        let period_end = chrono::DateTime::parse_from_rfc3339(
            row.request_scope["periodEndsAt"].as_str().expect("the period end is recorded"),
        )
        .unwrap()
        .timestamp_millis();
        let available = chrono::DateTime::parse_from_rfc3339(row.available_at.as_deref().unwrap())
            .unwrap()
            .timestamp_millis();
        assert_eq!(period_end, FIXTURE_DAY + DAY, "the day's last bar closed then");
        assert!(available > period_end, "but that is not when we could observe it");

        // The repro: ask for a forward-observed snapshot as of the end of
        // the period the file covers. It must be refused.
        let forward = snapshot::build_snapshot(
            &mut conn,
            &SnapshotRequest {
                instrument_id: "crypto:binance:BTCUSDT".into(),
                interval: "1h".into(),
                dataset_id: report.dataset_id.unwrap(),
                price_basis: PriceBasis::Raw,
                corporate_action_version: None,
                cost_profile_version: Some("cost-profile-v1".into()),
                kind: SnapshotKind::ForwardObserved,
                provenance_ids: vec![provenance_id],
                as_of_ms: FIXTURE_DAY + DAY,
            },
        )
        .unwrap();
        let SnapshotOutcome::Blocked { events } = forward else {
            panic!("a history download must not be forward-observed evidence: {forward:?}")
        };
        assert_eq!(
            events.iter().map(|row| row.event.code.as_str()).collect::<Vec<_>>(),
            vec!["availability_after_cut"]
        );
        assert_eq!(events[0].event.action, "record_availability");

        // Actual observation can support a later cut. The safety rule must
        // not disable forward evidence once the response has arrived.
        let admitted = snapshot::build_snapshot(&mut conn, &SnapshotRequest {
            instrument_id: "crypto:binance:BTCUSDT".into(), interval: "1h".into(),
            dataset_id: report.dataset_id.unwrap(), price_basis: PriceBasis::Raw,
            corporate_action_version: None, cost_profile_version: Some("cost-profile-v1".into()),
            kind: SnapshotKind::ForwardObserved, provenance_ids: vec![provenance_id],
            as_of_ms: available + 1,
        }).unwrap();
        let SnapshotOutcome::Created(snapshot) = admitted else { panic!("observed response must be admitted") };
        assert!(snapshot::get_snapshot(&conn, snapshot.id).unwrap().unwrap().qualification_eligible());
    }

    /// The review's second finding: keeping only the digest we parsed leaves
    /// "the checksum named this file, in this format" unreplayable.
    #[test]
    fn the_checksum_file_is_kept_as_its_own_retrieval_so_the_verification_can_be_replayed() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let fetcher = fixture_day_fetcher();
        let report = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();
        let UnitStatus::Fetched { provenance_id, .. } = report.units[1].status else {
            panic!("the day must be fetched: {:?}", report.units[1])
        };

        let archive = provenance::get_provenance(&conn, provenance_id).unwrap().unwrap();
        let checksum_id = archive.request_scope["checksumProvenanceId"].as_i64().expect("the checksum row");
        let checksum = provenance::get_provenance(&conn, checksum_id).unwrap().unwrap();
        assert_eq!(checksum.raw_media_type, "text/plain", "a checksum file is not a zip");
        assert_eq!(checksum.request_scope["kind"], json!("checksum"));
        assert_eq!(checksum.request_scope["vouchesFor"], json!("BTCUSDT-1h-2024-07-15.zip"));
        assert!(checksum.accepted);

        // Replay the whole verification from what was stored, with nothing
        // taken on trust: the checksum file's own bytes, then the archive's.
        let stored_checksum = provenance::read_raw(&store, &checksum).unwrap();
        assert_eq!(stored_checksum, BTC_MS_CHECKSUM);
        let digest = binance::parse_checksum(&stored_checksum, "BTCUSDT-1h-2024-07-15.zip").unwrap();
        assert_eq!(digest, archive.request_scope["publishedSha256"].as_str().unwrap());
        let stored_archive = provenance::read_raw(&store, &archive).unwrap();
        assert!(binance::verify_checksum(&stored_archive, &digest).is_ok());
        // And the stored checksum still refuses to vouch for another file.
        assert_eq!(
            binance::parse_checksum(&stored_checksum, "BTCUSDT-1h-2024-07-16.zip").unwrap_err().code,
            "checksum_file_name_mismatch"
        );
    }

    #[test]
    fn a_malformed_checksum_file_is_the_rejection_record_and_is_not_filed_as_an_archive() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let url = daily("2024-07-15");
        let fetcher = FakeFetcher::new()
            .with(&url, BTC_MS_ZIP)
            .with(&format!("{url}.CHECKSUM"), b"this is not a checksum line");
        let report = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();

        let UnitStatus::Rejected { ref code, provenance_id, .. } = report.units[1].status else {
            panic!("a malformed checksum must be refused: {:?}", report.units[1])
        };
        assert_eq!(code, "checksum_malformed");
        let row = provenance::get_provenance(&conn, provenance_id.unwrap()).unwrap().unwrap();
        assert!(!row.accepted);
        assert_eq!(row.raw_media_type, "text/plain");
        assert_eq!(provenance::read_raw(&store, &row).unwrap(), b"this is not a checksum line");
        // Exactly one record: the checksum file's. The archive was never
        // asked for, so there is nothing else to file.
        assert_eq!(provenance::list_provenance(&conn, "crypto:binance:BTCUSDT", "1h").unwrap().len(), 1);
        assert!(!fetcher.requested().contains(&url), "{:?}", fetcher.requested());
        assert_eq!(report.bars, 0);
    }

    #[test]
    fn legacy_archive_evidence_is_refetched_and_old_forward_snapshots_are_quarantined() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let req = request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY);
        let old = RawObservation {
            instrument_id: req.instrument_id.clone(), interval: req.interval.clone(),
            source: binance::SOURCE_ARCHIVE.into(), role: SeriesRole::Primary,
            request_scope: json!({
                "url": daily("2024-07-15"), "unit": "daily:2024-07-15",
                "fileName": "BTCUSDT-1h-2024-07-15.zip", "interval": "1h",
                "publishedSha256": super::super::sha256_hex(BTC_MS_ZIP),
            }),
            retrieved_at: Utc::now().to_rfc3339(),
            available_at: Some("2024-07-16T00:00:00Z".into()),
            accepted: true, rejection_reason: None, revision_of: None,
            media_type: "application/zip".into(),
        };
        let old_id = provenance::record_raw(&conn, &store, &old, BTC_MS_ZIP).unwrap().0;
        let before = provenance::get_provenance(&conn, old_id).unwrap().unwrap();
        let fetcher = fixture_day_fetcher();
        let report = ingest(&mut conn, &store, &fetcher, &req).unwrap();
        assert!(matches!(report.units[1].status, UnitStatus::Fetched { .. }), "old cache must not be reused");
        assert!(fetcher.requested().contains(&daily("2024-07-15")));
        assert_eq!(provenance::get_provenance(&conn, old_id).unwrap().unwrap(), before);

        let candidate = SnapshotRequest {
            instrument_id: req.instrument_id.clone(), interval: req.interval.clone(),
            dataset_id: report.dataset_id.unwrap(), price_basis: PriceBasis::Raw,
            corporate_action_version: None, cost_profile_version: req.cost_profile_version,
            kind: SnapshotKind::Historical, provenance_ids: vec![old_id], as_of_ms: req.as_of_ms,
        };
        // Old historical evidence stays readable for audit, without being
        // upgraded into known availability or silently rewriting its rows.
        let SnapshotOutcome::Created(historical) = snapshot::build_snapshot(&mut conn, &candidate).unwrap() else {
            panic!("historical evidence remains valid");
        };
        let forward = SnapshotRequest { kind: SnapshotKind::ForwardObserved, ..candidate };
        let SnapshotOutcome::Blocked { events } = snapshot::build_snapshot(&mut conn, &forward).unwrap() else {
            panic!("legacy availability must be quarantined");
        };
        assert_eq!(events[0].event.code, "availability_unknown");

        // Simulate the immutable forward row the unfixed build could mint.
        conn.execute(
            "INSERT INTO market_snapshots
             (snapshot_id, version, instrument_row_id, instrument_id, interval, dataset_id,
              dataset_hash, price_basis, calendar_id, corporate_action_version, cost_profile_version,
              kind, status, as_of, coverage_json, content_json)
             SELECT 'legacy-forward-fixture', version, instrument_row_id, instrument_id, interval, dataset_id,
                    dataset_hash, price_basis, calendar_id, corporate_action_version, cost_profile_version,
                    'forward-observed', status, as_of, coverage_json, content_json
             FROM market_snapshots WHERE id = ?1", [historical.id],
        ).unwrap();
        let stale_id = conn.last_insert_rowid();
        conn.execute("INSERT INTO market_snapshot_sources (snapshot_row_id, provenance_id) VALUES (?1, ?2)",
            rusqlite::params![stale_id, old_id]).unwrap();
        assert!(snapshot::get_snapshot(&conn, stale_id).unwrap_err().to_string().contains("unverified forward-observed"));
        assert!(snapshot::get_snapshot_by_id(&conn, "legacy-forward-fixture").is_err());
        assert!(snapshot::list_snapshots(&conn, 100).is_err());
        assert!(snapshot::dataset_market_status(&conn, forward.dataset_id).is_err());
        // Admission still refuses instead of returning an Existing row.
        assert!(matches!(snapshot::build_snapshot(&mut conn, &forward).unwrap(), SnapshotOutcome::Blocked { .. }));
        assert!(snapshot::get_snapshot(&conn, historical.id).unwrap().is_some());
        assert_eq!(conn.query_row("SELECT status FROM market_snapshots WHERE id=?1", [stale_id],
            |row| row.get::<_, String>(0)).unwrap(), "ok", "audit row was not rewritten");
    }

    #[test]
    fn retry_keeps_each_failed_zip_and_records_observation_after_the_response() {
        use std::sync::{Mutex, atomic::AtomicUsize};
        struct RetryFetcher {
            zip_requests: AtomicUsize,
            completed: Mutex<Option<chrono::DateTime<Utc>>>,
            bad: Vec<u8>,
        }
        impl HttpFetcher for RetryFetcher {
            fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
                let body = if url == daily("2024-07-15") {
                    if self.zip_requests.fetch_add(1, Ordering::SeqCst) == 0 {
                        self.bad.clone()
                    } else { BTC_MS_ZIP.to_vec() }
                } else if url == format!("{}.CHECKSUM", daily("2024-07-15")) {
                    BTC_MS_CHECKSUM.to_vec()
                } else { return Err(FetchError::NotFound(url.into())); };
                *self.completed.lock().unwrap() = Some(Utc::now());
                Ok(body)
            }
        }
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let mut bad = BTC_MS_ZIP.to_vec();
        bad[600] ^= 1;
        let fetcher = RetryFetcher { zip_requests: AtomicUsize::new(0), completed: Mutex::new(None), bad };
        let report = ingest(&mut conn, &store, &fetcher,
            &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY)).unwrap();
        assert!(matches!(report.outcome, IngestOutcome::Snapshot(_)));
        let rows = provenance::list_provenance(&conn, "crypto:binance:BTCUSDT", "1h").unwrap();
        assert_eq!(rows.len(), 4, "each checksum and each ZIP response is retained");
        let zips: Vec<_> = rows.iter().filter(|row| row.raw_media_type == "application/zip").collect();
        assert!(!zips[0].accepted);
        assert!(zips[0].rejection_reason.as_ref().unwrap().contains("checksum_mismatch"));
        assert_eq!(provenance::read_raw(&store, zips[0]).unwrap(), fetcher.bad);
        assert!(zips[1].accepted);
        for zip in &zips {
            let checksum_id = zip.request_scope["checksumProvenanceId"].as_i64().unwrap();
            let checksum = provenance::get_provenance(&conn, checksum_id).unwrap().unwrap();
            let digest = binance::parse_checksum(&provenance::read_raw(&store, &checksum).unwrap(),
                "BTCUSDT-1h-2024-07-15.zip").unwrap();
            assert_eq!(binance::verify_checksum(&provenance::read_raw(&store, zip).unwrap(), &digest).is_ok(), zip.accepted);
        }
        let completed = fetcher.completed.lock().unwrap().unwrap();
        let observed = chrono::DateTime::parse_from_rfc3339(&zips[1].retrieved_at).unwrap();
        assert!(observed >= completed, "request start is not an observation of the response");
        assert_eq!(zips[1].available_at.as_deref(), Some(zips[1].retrieved_at.as_str()));
    }

    #[test]
    fn a_zip_parse_rejection_retains_its_published_checksum_for_offline_replay() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let body = b"not a ZIP";
        let checksum_bytes = format!("{}  BTCUSDT-1h-2024-07-15.zip\n", super::super::sha256_hex(body));
        let url = daily("2024-07-15");
        let fetcher = FakeFetcher::new().with(&url, body)
            .with(&binance::checksum_url(&url), checksum_bytes.as_bytes());
        let report = ingest(&mut conn, &store, &fetcher,
            &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY)).unwrap();
        let UnitStatus::Rejected { code, provenance_id, .. } = &report.units[1].status else {
            panic!("bad ZIP must be rejected");
        };
        assert_eq!(code, "zip_not_an_archive");
        let zip = provenance::get_provenance(&conn, provenance_id.unwrap()).unwrap().unwrap();
        let checksum = provenance::get_provenance(&conn, zip.request_scope["checksumProvenanceId"].as_i64().unwrap())
            .unwrap().unwrap();
        assert_eq!(provenance::read_raw(&store, &checksum).unwrap(), checksum_bytes.as_bytes());
        let digest = binance::parse_checksum(&provenance::read_raw(&store, &checksum).unwrap(),
            "BTCUSDT-1h-2024-07-15.zip").unwrap();
        assert_eq!(zip.request_scope["publishedSha256"], digest);
        let original = provenance::read_raw(&store, &zip).unwrap();
        binance::verify_checksum(&original, &digest).unwrap();
        assert_eq!(parse_archive_bytes(&original).err().unwrap().code, code);
        assert_eq!(report.dataset_id, None);
    }

    #[test]
    fn cached_zip_requires_the_checksum_original_and_refetches_missing_evidence() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let fetcher = fixture_day_fetcher();
        let req = request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY);
        let first = ingest(&mut conn, &store, &fetcher, &req).unwrap();
        let UnitStatus::Fetched { provenance_id: first_id, .. } = first.units[1].status else { panic!("fetched") };
        let zip = provenance::get_provenance(&conn, first_id).unwrap().unwrap();
        let checksum = provenance::get_provenance(&conn, zip.request_scope["checksumProvenanceId"].as_i64().unwrap())
            .unwrap().unwrap();
        std::fs::remove_file(store.path_of(&checksum.raw_artifact_path).unwrap()).unwrap();
        let again = ingest(&mut conn, &store, &fetcher, &req).unwrap();
        let UnitStatus::Fetched { provenance_id: next_id, .. } = again.units[1].status else {
            panic!("ZIP alone is insufficient for a cache hit");
        };
        assert_ne!(first_id, next_id);
        assert_eq!(again.dataset_id, first.dataset_id);
        assert_eq!(provenance::read_raw(&store, &checksum).unwrap(), BTC_MS_CHECKSUM);
        let cached = ingest(&mut conn, &store, &fetcher, &req).unwrap();
        assert!(matches!(cached.units[1].status, UnitStatus::Cached { provenance_id, .. } if provenance_id == next_id));
    }

    #[test]
    fn a_checksum_that_does_not_match_is_kept_as_a_rejection_and_nothing_is_imported() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let url = daily("2024-07-15");
        let mut tampered = BTC_MS_ZIP.to_vec();
        tampered[600] ^= 0x01;
        let fetcher = FakeFetcher::new()
            .with(&url, &tampered)
            .with(&format!("{url}.CHECKSUM"), BTC_MS_CHECKSUM);
        let report = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();

        let UnitStatus::Rejected { ref code, provenance_id, .. } = report.units[1].status else {
            panic!("a tampered file must be rejected: {:?}", report.units[1])
        };
        assert_eq!(code, "checksum_mismatch");
        assert_eq!(report.bars, 0);
        assert_eq!(report.dataset_id, None);
        assert_eq!(report.outcome, IngestOutcome::NothingRetrieved);
        // It was re-fetched once before being refused, and the bytes that
        // failed are still there to inspect.
        assert_eq!(
            fetcher.requested().iter().filter(|asked| *asked == &url).count(),
            CHECKSUM_ATTEMPTS as usize
        );
        let row = provenance::get_provenance(&conn, provenance_id.unwrap()).unwrap().unwrap();
        assert!(!row.accepted);
        assert!(row.rejection_reason.as_deref().unwrap().starts_with("checksum_mismatch"));
        assert_eq!(provenance::read_raw(&store, &row).unwrap(), tampered);
        assert_eq!(dataset_market_status(&conn, 1).unwrap(), DatasetMarketStatus::Legacy);
    }

    #[test]
    fn a_gap_is_imported_honestly_and_blocks_with_the_range_and_the_action() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let fetcher = fixture_day_fetcher();
        // Ask for two days; only the first is published anywhere.
        let report = ingest(
            &mut conn,
            &store,
            &fetcher,
            &request(FIXTURE_DAY, FIXTURE_DAY + 2 * DAY, FIXTURE_DAY + 2 * DAY),
        )
        .unwrap();

        assert_eq!(report.bars, 24, "what exists is still imported");
        let IngestOutcome::RangeIncomplete { snapshot, events } = &report.outcome else {
            panic!("a missing day must refuse the range: {:?}", report.outcome)
        };
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.code, "missing_bar");
        assert_eq!(events[0].event.action, "refetch_range");
        assert_eq!(events[0].event.bar_count, Some(24));
        assert_eq!(events[0].event.range_start, Some(FIXTURE_DAY + DAY));
        assert_eq!(events[0].event.range_end, Some(FIXTURE_DAY + 2 * DAY - HOUR));
        assert_eq!(events[0].event.detail["scope"], json!("requested-range"));
        assert_eq!(report.range_coverage["expectedCount"], json!(48));
        assert_eq!(report.range_coverage["matchedCount"], json!(24));

        // The subtle part, stated rather than left implicit: the 24 bars
        // that DID arrive are a complete dataset in their own right, so they
        // are a snapshot — and the range is still refused, because the
        // question asked was about two days.
        let snapshot = snapshot.as_ref().expect("the retrieved day is internally complete");
        assert_eq!(snapshot.coverage["blocking"], json!(false));
        assert_eq!(
            dataset_market_status(&conn, 1).unwrap(),
            DatasetMarketStatus::Registered(snapshot.clone())
        );
        // The refusal is queryable, not just returned.
        let recorded = provenance::list_quality_events(&conn, "crypto:binance:BTCUSDT", "1h", 10).unwrap();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].event.code, "missing_bar");
    }

    #[test]
    fn the_rest_source_covers_only_closed_bars_the_archive_has_not_published() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        // Four closed hours of the fixture day, as the exchange answers.
        let body = rest_body(FIXTURE_DAY, 4);
        let url = binance::rest_klines_url("BTCUSDT", "1h", FIXTURE_DAY, FIXTURE_DAY + 4 * HOUR, 1000);
        let fetcher = FakeFetcher::new().with(&url, body.as_bytes());
        let report = ingest(
            &mut conn,
            &store,
            &fetcher,
            &IngestRequest {
                allow_rest: true,
                // The cut is mid-way through the fifth hour, so only four
                // bars are due at all.
                ..request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + 4 * HOUR + 1)
            },
        )
        .unwrap();

        assert_eq!(report.bars, 4);
        let rest = report.units.iter().find(|unit| unit.source == binance::SOURCE_REST).expect("a REST unit");
        assert!(matches!(rest.status, UnitStatus::Fetched { bars: 4, .. }));
        // Everything after the cut was never asked for.
        assert!(
            fetcher.requested().iter().all(|asked| !asked.contains(&format!("startTime={}", FIXTURE_DAY + 4 * HOUR))),
            "{:?}",
            fetcher.requested()
        );
        // Four of twenty-four bars: complete for what is due, and the rest
        // of the day is simply not due yet — so the range is NOT incomplete.
        let IngestOutcome::Snapshot(snapshot) = &report.outcome else {
            panic!("the due bars are complete: {:?}", report.outcome)
        };
        assert!(snapshot.qualification_eligible());
        assert_eq!(report.range_coverage["notDueCount"], json!(20));
        assert_eq!(report.range_coverage["matchedCount"], json!(4));
        assert_eq!(report.range_coverage["blocking"], json!(false));
    }

    #[test]
    fn two_sources_that_disagree_about_one_bar_stop_the_import() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        // The archive day, plus a REST answer that contradicts its first bar.
        let contradiction = rest_body_with_close(FIXTURE_DAY, 1, 1.0);
        let rest_url = binance::rest_klines_url("BTCUSDT", "1h", FIXTURE_DAY, FIXTURE_DAY + HOUR, 1000);
        let fetcher = FakeFetcher::new()
            .with(&daily("2024-07-15"), BTC_MS_ZIP)
            .with(&format!("{}.CHECKSUM", daily("2024-07-15")), BTC_MS_CHECKSUM)
            .with(&rest_url, contradiction.as_bytes());
        // Retrieve the day twice over: once as the archive unit and once as
        // a REST page for the same hour.
        let day = ingest(&mut conn, &store, &fetcher, &request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY))
            .unwrap();
        assert!(matches!(day.outcome, IngestOutcome::Snapshot(_)));

        let accepted = vec![
            Accepted { provenance_id: 1, rows: vec![kline(FIXTURE_DAY, 100.0)] },
            Accepted { provenance_id: 2, rows: vec![kline(FIXTURE_DAY, 101.0)] },
        ];
        let error = merge_rows(&accepted).unwrap_err();
        assert_eq!(error, (FIXTURE_DAY, vec![1, 2]));

        // Identical rows from two units are the same bar, not a conflict.
        let same = vec![
            Accepted { provenance_id: 1, rows: vec![kline(FIXTURE_DAY, 100.0)] },
            Accepted { provenance_id: 2, rows: vec![kline(FIXTURE_DAY, 100.0)] },
        ];
        assert_eq!(merge_rows(&same).unwrap().len(), 1);
    }

    #[test]
    fn a_range_this_adapter_cannot_serve_is_refused_before_anything_is_fetched() {
        let mut conn = workspace();
        let (store, _guard) = fresh_store();
        let fetcher = FakeFetcher::new();
        for (candidate, expected) in [
            (
                IngestRequest { interval: "2h".into(), ..request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY) },
                "is not one this contract knows",
            ),
            (
                request(FIXTURE_DAY + DAY, FIXTURE_DAY, FIXTURE_DAY + DAY),
                "ends before it starts",
            ),
            (
                IngestRequest {
                    instrument_id: "crypto:binance:XRPUSDT".into(),
                    ..request(FIXTURE_DAY, FIXTURE_DAY + DAY, FIXTURE_DAY + DAY)
                },
                "is not registered",
            ),
        ] {
            let error = ingest(&mut conn, &store, &fetcher, &candidate).unwrap_err().to_string();
            assert!(error.contains(expected), "expected {expected:?} in {error:?}");
        }
        assert!(fetcher.requested().is_empty(), "nothing was fetched");
    }

    #[test]
    fn the_unit_plan_follows_the_calendar_not_a_guess() {
        assert_eq!(
            months_in_range(FIXTURE_DAY, FIXTURE_DAY + 2 * DAY).unwrap(),
            vec![(2024, 7)]
        );
        // 2024-12-31T12:00Z to 2025-02-01T12:00Z touches three months —
        // which is exactly the range that spans the archive's unit change.
        let december = 1_735_646_400_000;
        assert_eq!(
            months_in_range(december, december + 32 * DAY).unwrap(),
            vec![(2024, 12), (2025, 1), (2025, 2)]
        );
        let days = days_of_month_in_range(2024, 7, FIXTURE_DAY, FIXTURE_DAY + 2 * DAY);
        assert_eq!(
            days.iter().map(|date| date.to_string()).collect::<Vec<_>>(),
            vec!["2024-07-15", "2024-07-16"]
        );
        // A range that ends exactly at midnight does not pull in that day.
        let days = days_of_month_in_range(2024, 7, FIXTURE_DAY, FIXTURE_DAY + DAY);
        assert_eq!(days.iter().map(|date| date.to_string()).collect::<Vec<_>>(), vec!["2024-07-15"]);
    }

    fn kline(open_time_ms: i64, close: f64) -> Kline {
        Kline {
            open_time_ms,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close,
            volume: 10.0,
            close_time_ms: open_time_ms + HOUR - 1,
        }
    }

    fn rest_body(start_ms: i64, bars: usize) -> String {
        rest_body_with_close(start_ms, bars, 100.5)
    }

    fn rest_body_with_close(start_ms: i64, bars: usize, close: f64) -> String {
        let rows: Vec<String> = (0..bars)
            .map(|index| {
                let open = start_ms + index as i64 * HOUR;
                format!(
                    "[{open},\"100.00000000\",\"101.00000000\",\"99.00000000\",\"{close:.8}\",\"10.00000000\",{},\"0\",1,\"0\",\"0\",\"0\"]",
                    open + HOUR - 1
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    }
}
