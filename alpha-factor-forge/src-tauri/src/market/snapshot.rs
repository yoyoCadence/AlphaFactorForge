//! P06 — snapshots: what research and paper are allowed to read
//! (`market_snapshots`, `market_snapshot_sources` in migration 0008).
//!
//! A snapshot binds one dataset to one instrument revision, one calendar
//! version, one price basis, and the provenance it came from. It exists
//! only when the coverage audit found nothing blocking, so "a snapshot
//! exists" is itself the evidence that the series was complete under a
//! stated calendar at a stated time.
//!
//! Everything that blocks is written down as a quality event instead, with
//! the range it covers and the action that would resolve it — a refusal
//! that says "no" without saying "which bars" is not useful to anyone.
//!
//! A dataset with no snapshot is LEGACY: unchanged, still importable and
//! backtestable exactly as before, and simply unable to acquire new
//! qualification on its own (`docs/market-contract.md` §0).

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::repositories;
use alpha_factor_forge::discovery_core::market_foundation::{
    audit_coverage, detect_time_unit, expected_bar_starts, series_conflicts, CoverageRequest,
    ExpectedRangeRequest, PriceBasis, SeriesIdentity, SeriesRole, Severity, TimeUnit, AssetType,
    MARKET_SNAPSHOT_VERSION,
};
use crate::error::{AppError, AppResult};

use super::provenance::{self, ProvenanceRow, QualityEventRow};
use super::registry::{self, InstrumentRow};
use super::{canonical_json, is_blocked, sha256_hex, QualityEvent};

/// What the data is for. The three never mix: a demo series can never
/// become evidence, and a forward-observed one must know when each bar was
/// first observable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotKind {
    Demo,
    Historical,
    ForwardObserved,
}

impl SnapshotKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SnapshotKind::Demo => "demo",
            SnapshotKind::Historical => "historical",
            SnapshotKind::ForwardObserved => "forward-observed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "demo" => Some(SnapshotKind::Demo),
            "historical" => Some(SnapshotKind::Historical),
            "forward-observed" => Some(SnapshotKind::ForwardObserved),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotStatus {
    Ok,
    Degraded,
}

impl SnapshotStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SnapshotStatus::Ok => "ok",
            SnapshotStatus::Degraded => "degraded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "ok" => Some(SnapshotStatus::Ok),
            "degraded" => Some(SnapshotStatus::Degraded),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRequest {
    pub instrument_id: String,
    pub interval: String,
    pub dataset_id: i64,
    pub price_basis: PriceBasis,
    /// `None` = corporate-action completeness is not verified. For an ETF
    /// that makes the snapshot degraded; for spot crypto it does not apply.
    pub corporate_action_version: Option<String>,
    /// `None` = the user has not confirmed the cost model, so nothing built
    /// on this snapshot may qualify.
    pub cost_profile_version: Option<String>,
    pub kind: SnapshotKind,
    /// The primary-source records this series was built from.
    pub provenance_ids: Vec<i64>,
    /// The data cut the coverage audit is measured against.
    pub as_of_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    pub id: i64,
    pub snapshot_id: String,
    pub version: String,
    pub instrument_row_id: i64,
    pub instrument_id: String,
    pub interval: String,
    pub dataset_id: i64,
    pub dataset_hash: String,
    pub price_basis: PriceBasis,
    pub calendar_id: String,
    pub corporate_action_version: Option<String>,
    pub cost_profile_version: Option<String>,
    pub kind: SnapshotKind,
    pub status: SnapshotStatus,
    pub as_of: i64,
    pub coverage: Value,
    pub created_at: String,
}

impl SnapshotRow {
    /// Whether anything built on this snapshot may be promoted. Degraded
    /// data and demo data never qualify; that is the fail-closed default the
    /// contract asks for, decided in one place.
    pub fn qualification_eligible(&self) -> bool {
        self.status == SnapshotStatus::Ok && self.kind != SnapshotKind::Demo
    }
}

/// What a build produced: a snapshot, the one that already existed for the
/// same inputs, or the evidence that says why there is none.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum SnapshotOutcome {
    Created(SnapshotRow),
    Existing(SnapshotRow),
    Blocked { events: Vec<QualityEventRow> },
}

/// Whether a dataset carries market semantics at all.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum DatasetMarketStatus {
    /// Imported before this contract, or from an unidentified source. It
    /// still works exactly as it always did; it just cannot qualify.
    Legacy,
    /// Boxed because a legacy dataset carries nothing: the two variants
    /// would otherwise differ by the whole snapshot in size.
    Registered(Box<SnapshotRow>),
}

fn event(
    request: &SnapshotRequest,
    code: &str,
    severity: Severity,
    action: &str,
    detail: Value,
) -> QualityEvent {
    QualityEvent::new(&request.instrument_id, &request.interval, code, severity, action)
        .with_detail(detail)
        .for_dataset(request.dataset_id)
}

/// Build a snapshot, or record why it cannot exist.
///
/// Everything happens in one transaction: a blocked build writes its whole
/// evidence set or none of it, and a successful one writes the snapshot, its
/// sources, and its degraded evidence together.
pub fn build_snapshot(conn: &mut Connection, request: &SnapshotRequest) -> AppResult<SnapshotOutcome> {
    let instrument = registry::latest_instrument(conn, &request.instrument_id)?.ok_or_else(|| {
        AppError::Other(format!(
            "instrument {} is not registered; register it before building a snapshot",
            request.instrument_id
        ))
    })?;
    let calendar = registry::get_calendar(conn, &instrument.session_calendar_id)?.ok_or_else(|| {
        AppError::Other(format!(
            "calendar {} is missing for instrument {}",
            instrument.session_calendar_id, instrument.instrument_id
        ))
    })?;
    let dataset = repositories::get_dataset_by_id(conn, request.dataset_id)?;
    // Stored metadata is not proof of the current payload. Read all rows,
    // including any outside the declared bounds, and fail before any write
    // or Existing return if identity or candle plausibility has changed.
    let candles = repositories::get_candles(conn, request.dataset_id, i64::MIN, i64::MAX)?;
    let normalized = crate::identity::verify_dataset_identity(&dataset, &candles)?;
    alpha_factor_forge::discovery_core::market_data::ensure_admissible(
        normalized.iter().map(repositories::db_candle_fields),
    )
    .map_err(|error| AppError::Other(error.0))?;
    let components = load_components(conn, request)?;

    let mut events: Vec<QualityEvent> = Vec::new();
    events.extend(identity_events(request, &instrument, &dataset));
    events.extend(component_events(conn, request, &instrument, &components)?);

    let timestamps: Vec<i64> = normalized.iter().map(|candle| candle.timestamp).collect();
    events.extend(time_unit_events(request, &timestamps));
    events.extend(coverage_events(request, &instrument, &calendar, &dataset, &timestamps));
    events.extend(semantics_events(request, &instrument));

    let transaction = conn.transaction()?;
    if is_blocked(&events) {
        let mut recorded = Vec::with_capacity(events.len());
        for event in &events {
            let id = provenance::record_quality_event(&transaction, event)?;
            recorded.push(QualityEventRow {
                id,
                created_at: String::new(),
                event: event.clone(),
            });
        }
        transaction.commit()?;
        // Re-read so the caller sees the stored rows, timestamps included.
        return Ok(SnapshotOutcome::Blocked {
            events: provenance::dataset_quality_events(conn, request.dataset_id)?
                .into_iter()
                .filter(|row| recorded.iter().any(|stored| stored.id == row.id))
                .collect(),
        });
    }

    let status = if events.iter().any(|event| event.severity == Severity::Degraded) {
        SnapshotStatus::Degraded
    } else {
        SnapshotStatus::Ok
    };
    let coverage = coverage_report_value(request, &instrument, &calendar, &dataset, &timestamps);
    let mut source_hashes: Vec<String> = components
        .iter()
        .map(|component| component.record_hash.clone())
        .collect();
    source_hashes.sort();
    let content = json!({
        "version": MARKET_SNAPSHOT_VERSION,
        "instrumentId": request.instrument_id,
        "instrumentContentHash": instrument.content_hash,
        "interval": request.interval,
        "datasetHash": dataset.dataset_hash,
        "priceBasis": request.price_basis.as_str(),
        "calendarId": calendar.calendar_id,
        "corporateActionVersion": request.corporate_action_version,
        "costProfileVersion": request.cost_profile_version,
        "kind": request.kind.as_str(),
        "sourceRecordHashes": source_hashes,
    });
    // `asOf` is deliberately NOT part of the identity: the dataset, the
    // instrument revision, the calendar, and the sources are all immutable,
    // so the same inputs describe the same snapshot whenever it is built.
    // The as-of that admitted it is stored beside the coverage it produced.
    let snapshot_id = sha256_hex(&canonical_json(&content)?);

    if let Some(existing) = get_snapshot_by_id(&transaction, &snapshot_id)? {
        transaction.commit()?;
        return Ok(SnapshotOutcome::Existing(existing));
    }
    transaction.execute(
        "INSERT INTO market_snapshots
            (snapshot_id, version, instrument_row_id, instrument_id, interval, dataset_id,
             dataset_hash, price_basis, calendar_id, corporate_action_version,
             cost_profile_version, kind, status, as_of, coverage_json, content_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            snapshot_id,
            MARKET_SNAPSHOT_VERSION,
            instrument.id,
            request.instrument_id,
            request.interval,
            request.dataset_id,
            dataset.dataset_hash,
            request.price_basis.as_str(),
            calendar.calendar_id,
            request.corporate_action_version,
            request.cost_profile_version,
            request.kind.as_str(),
            status.as_str(),
            request.as_of_ms,
            serde_json::to_string(&coverage)?,
            serde_json::to_string(&content)?,
        ],
    )?;
    let row_id = transaction.last_insert_rowid();
    {
        let mut statement = transaction.prepare(
            "INSERT INTO market_snapshot_sources (snapshot_row_id, provenance_id) VALUES (?1, ?2)",
        )?;
        for component in &components {
            statement.execute(params![row_id, component.id])?;
        }
    }
    for event in &events {
        let mut event = event.clone();
        event.snapshot_row_id = Some(row_id);
        provenance::record_quality_event(&transaction, &event)?;
    }
    transaction.commit()?;
    let created = get_snapshot(conn, row_id)?.expect("the snapshot just inserted");
    Ok(SnapshotOutcome::Created(created))
}

fn load_components(conn: &Connection, request: &SnapshotRequest) -> AppResult<Vec<ProvenanceRow>> {
    let mut components = Vec::with_capacity(request.provenance_ids.len());
    for id in &request.provenance_ids {
        let row = provenance::get_provenance(conn, *id)?
            .ok_or_else(|| AppError::Other(format!("provenance record {id} does not exist")))?;
        components.push(row);
    }
    Ok(components)
}

/// The dataset must be the series the instrument names. The venue is
/// compared case-insensitively because `datasets.exchange` is free text
/// entered by a user, while an instrument's venue is normalised; the symbol
/// is compared exactly, because case can be meaningful in a symbol.
fn identity_events(
    request: &SnapshotRequest,
    instrument: &InstrumentRow,
    dataset: &repositories::Dataset,
) -> Vec<QualityEvent> {
    let mut mismatches = Vec::new();
    if dataset.interval != request.interval {
        mismatches.push(json!({ "field": "interval", "dataset": dataset.interval, "requested": request.interval }));
    }
    if dataset.symbol != instrument.symbol {
        mismatches.push(json!({ "field": "symbol", "dataset": dataset.symbol, "instrument": instrument.symbol }));
    }
    if !dataset.exchange.eq_ignore_ascii_case(&instrument.venue) {
        mismatches.push(json!({ "field": "venue", "dataset": dataset.exchange, "instrument": instrument.venue }));
    }
    if mismatches.is_empty() {
        return Vec::new();
    }
    vec![event(
        request,
        "dataset_instrument_mismatch",
        Severity::Blocking,
        "separate_sources",
        json!({ "mismatches": mismatches }),
    )]
}

/// A component must be an accepted, current, primary record of exactly this
/// series. Anything else is kept as evidence and refused as a component.
fn component_events(
    conn: &Connection,
    request: &SnapshotRequest,
    instrument: &InstrumentRow,
    components: &[ProvenanceRow],
) -> AppResult<Vec<QualityEvent>> {
    let mut events = Vec::new();
    if components.is_empty() {
        events.push(event(
            request,
            "missing_source",
            Severity::Blocking,
            "refetch_range",
            json!({ "reason": "a snapshot requires at least one primary provenance record" }),
        ));
    }
    let cut = if request.kind == SnapshotKind::ForwardObserved {
        Some(DateTime::<Utc>::from_timestamp_millis(request.as_of_ms).ok_or_else(|| {
            AppError::Other("snapshot asOf is not a representable instant".into())
        })?)
    } else {
        None
    };
    let snapshot_identity = |source: &str, role: SeriesRole| SeriesIdentity {
        instrument_id: request.instrument_id.clone(),
        quote: instrument.quote.clone(),
        interval: request.interval.clone(),
        source: source.to_string(),
        role,
        price_basis: request.price_basis,
    };
    for component in components {
        let conflicts = series_conflicts(
            &snapshot_identity(&component.source, SeriesRole::Primary),
            &SeriesIdentity {
                instrument_id: component.instrument_id.clone(),
                quote: instrument.quote.clone(),
                interval: component.interval.clone(),
                source: component.source.clone(),
                role: component.role,
                price_basis: request.price_basis,
            },
        );
        if !conflicts.is_empty() {
            events.push(
                event(
                    request,
                    "source_conflict",
                    Severity::Blocking,
                    "separate_sources",
                    json!({ "provenanceId": component.id, "conflicts": conflicts }),
                )
                .for_provenance(component.id),
            );
        }
        if !component.accepted {
            events.push(
                event(
                    request,
                    "source_conflict",
                    Severity::Blocking,
                    "separate_sources",
                    json!({
                        "provenanceId": component.id,
                        "reason": "a rejected retrieval can never be a component",
                        "rejectionReason": component.rejection_reason,
                    }),
                )
                .for_provenance(component.id),
            );
        }
        if let Some(newer) = provenance::revised_by(conn, component.id)? {
            events.push(
                event(
                    request,
                    "superseded_source",
                    Severity::Blocking,
                    "rebuild_from_revision",
                    json!({ "provenanceId": component.id, "revisedBy": newer }),
                )
                .for_provenance(component.id),
            );
        }
        if let Some(cut) = cut {
            let available = component.available_at.as_deref()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok());
            let code = match available {
                Some(moment) if moment <= cut => None,
                Some(_) => Some("availability_after_cut"),
                None => Some("availability_unknown"),
            };
            if let Some(code) = code {
                events.push(
                    event(
                        request,
                        code,
                        Severity::Blocking,
                        "record_availability",
                        json!({
                            "provenanceId": component.id,
                            "availableAt": component.available_at,
                            "asOfMs": request.as_of_ms,
                            "reason": "forward-observed evidence must have known availability at or before the cut",
                        }),
                    )
                    .for_provenance(component.id),
                );
            }
        }
    }
    // Two components must be one series with each other, not just with the
    // request: that is what stops a Binance month and a Coinbase month from
    // being spliced together.
    for (index, left) in components.iter().enumerate() {
        for right in components.iter().skip(index + 1) {
            let conflicts = series_conflicts(
                &snapshot_identity(&left.source, left.role),
                &snapshot_identity(&right.source, right.role),
            );
            if !conflicts.is_empty() {
                events.push(event(
                    request,
                    "source_conflict",
                    Severity::Blocking,
                    "separate_sources",
                    json!({
                        "provenanceIds": [left.id, right.id],
                        "conflicts": conflicts,
                    }),
                ));
            }
        }
    }
    Ok(events)
}

/// Defence in depth. Stored candles cannot be in another unit —
/// `market-data-quality-v1` rejects anything outside the millisecond
/// plausibility window at import — so this is unreachable for a dataset that
/// went through the normal path, and it stays because the cost is one pass
/// and the failure it catches is a silently wrong series.
fn time_unit_events(request: &SnapshotRequest, timestamps: &[i64]) -> Vec<QualityEvent> {
    if timestamps.is_empty() {
        return Vec::new();
    }
    let as_f64: Vec<f64> = timestamps.iter().map(|value| *value as f64).collect();
    let verdict = detect_time_unit(&as_f64);
    if verdict.unit == Some(TimeUnit::Milliseconds) {
        return Vec::new();
    }
    vec![event(
        request,
        "time_unit_mismatch",
        Severity::Blocking,
        "verify_time_unit",
        json!({
            "unit": verdict.unit.map(|unit| unit.as_str()),
            "issue": verdict.issue.map(|issue| issue.as_str()),
            "index": verdict.index,
        }),
    )]
}

fn expected_range<'a>(
    instrument: &'a InstrumentRow,
    calendar: &'a alpha_factor_forge::discovery_core::market_foundation::SessionCalendar,
    interval: &'a str,
    dataset: &repositories::Dataset,
) -> ExpectedRangeRequest<'a> {
    ExpectedRangeRequest {
        calendar,
        interval,
        from_ms: dataset.start_time,
        // The dataset's range is inclusive of its last bar's START, so the
        // expectation runs to one cadence past it.
        to_ms_exclusive: dataset.end_time.saturating_add(1),
        listed_from_ms: instrument.listed_from,
        delisted_at_ms: instrument.delisted_at,
        suspensions: &instrument.suspensions,
    }
}

fn coverage_events(
    request: &SnapshotRequest,
    instrument: &InstrumentRow,
    calendar: &alpha_factor_forge::discovery_core::market_foundation::SessionCalendar,
    dataset: &repositories::Dataset,
    timestamps: &[i64],
) -> Vec<QualityEvent> {
    let expectation = expected_bar_starts(&expected_range(instrument, calendar, &request.interval, dataset));
    if let Some(issue) = expectation.issue {
        return vec![event(
            request,
            "expected_range_unavailable",
            Severity::Blocking,
            "review_calendar",
            json!({ "issue": issue.as_str(), "calendarId": calendar.calendar_id }),
        )];
    }
    let report = audit_coverage(&CoverageRequest {
        interval: &request.interval,
        expected: &expectation.timestamps,
        observed: timestamps,
        as_of_ms: request.as_of_ms,
    });
    report
        .events
        .iter()
        .map(|coverage| {
            event(
                request,
                coverage.code.as_str(),
                coverage.severity,
                coverage.action,
                json!({ "bars": coverage.count }),
            )
            .with_range(coverage.range_start, coverage.range_end, coverage.count)
        })
        .collect()
}

/// The two degraded defaults: unverified corporate actions (ETFs only — a
/// spot crypto pair has none) and an unconfirmed cost model.
fn semantics_events(request: &SnapshotRequest, instrument: &InstrumentRow) -> Vec<QualityEvent> {
    let mut events = Vec::new();
    if instrument.asset_type == AssetType::Etf && request.corporate_action_version.is_none() {
        events.push(event(
            request,
            "corporate_actions_unverified",
            Severity::Degraded,
            "verify_corporate_actions",
            json!({ "reason": "dividend and split completeness is not verified for this range" }),
        ));
    }
    if request.cost_profile_version.is_none() {
        events.push(event(
            request,
            "cost_profile_unconfirmed",
            Severity::Degraded,
            "confirm_costs",
            json!({ "reason": "the user has not confirmed commission, slippage, and transaction tax" }),
        ));
    }
    events
}

fn coverage_report_value(
    request: &SnapshotRequest,
    instrument: &InstrumentRow,
    calendar: &alpha_factor_forge::discovery_core::market_foundation::SessionCalendar,
    dataset: &repositories::Dataset,
    timestamps: &[i64],
) -> Value {
    let expectation = expected_bar_starts(&expected_range(instrument, calendar, &request.interval, dataset));
    let report = audit_coverage(&CoverageRequest {
        interval: &request.interval,
        expected: &expectation.timestamps,
        observed: timestamps,
        as_of_ms: request.as_of_ms,
    });
    serde_json::to_value(&report).unwrap_or(Value::Null)
}

#[cfg(test)]
fn candle_timestamps(conn: &Connection, dataset_id: i64) -> AppResult<Vec<i64>> {
    let mut stmt =
        conn.prepare("SELECT timestamp FROM candles WHERE dataset_id = ?1 ORDER BY timestamp ASC")?;
    let rows = stmt
        .query_map([dataset_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

const SNAPSHOT_COLUMNS: &str = "id, snapshot_id, version, instrument_row_id, instrument_id, interval, dataset_id,
    dataset_hash, price_basis, calendar_id, corporate_action_version, cost_profile_version, kind,
    status, as_of, coverage_json, created_at";

fn snapshot_from_row(row: &Row<'_>) -> rusqlite::Result<SnapshotRow> {
    let price_basis: String = row.get(8)?;
    let kind: String = row.get(12)?;
    let status: String = row.get(13)?;
    let coverage: String = row.get(15)?;
    Ok(SnapshotRow {
        id: row.get(0)?,
        snapshot_id: row.get(1)?,
        version: row.get(2)?,
        instrument_row_id: row.get(3)?,
        instrument_id: row.get(4)?,
        interval: row.get(5)?,
        dataset_id: row.get(6)?,
        dataset_hash: row.get(7)?,
        price_basis: PriceBasis::parse(&price_basis).unwrap_or(PriceBasis::Raw),
        calendar_id: row.get(9)?,
        corporate_action_version: row.get(10)?,
        cost_profile_version: row.get(11)?,
        kind: SnapshotKind::parse(&kind).unwrap_or(SnapshotKind::Demo),
        status: SnapshotStatus::parse(&status).unwrap_or(SnapshotStatus::Degraded),
        as_of: row.get(14)?,
        coverage: serde_json::from_str(&coverage).unwrap_or(Value::Null),
        created_at: row.get(16)?,
    })
}

pub fn get_snapshot(conn: &Connection, row_id: i64) -> AppResult<Option<SnapshotRow>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SNAPSHOT_COLUMNS} FROM market_snapshots WHERE id = ?1"),
            [row_id],
            snapshot_from_row,
        )
        .optional()?)
}

pub fn get_snapshot_by_id(conn: &Connection, snapshot_id: &str) -> AppResult<Option<SnapshotRow>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SNAPSHOT_COLUMNS} FROM market_snapshots WHERE snapshot_id = ?1"),
            [snapshot_id],
            snapshot_from_row,
        )
        .optional()?)
}

pub fn list_snapshots(conn: &Connection, limit: usize) -> AppResult<Vec<SnapshotRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SNAPSHOT_COLUMNS} FROM market_snapshots ORDER BY id DESC LIMIT ?1"
    ))?;
    let rows = stmt
        .query_map([limit as i64], snapshot_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The provenance a snapshot was built from, oldest first.
pub fn snapshot_sources(conn: &Connection, snapshot_row_id: i64) -> AppResult<Vec<ProvenanceRow>> {
    let mut stmt = conn.prepare(
        "SELECT provenance_id FROM market_snapshot_sources WHERE snapshot_row_id = ?1 ORDER BY provenance_id ASC",
    )?;
    let ids = stmt
        .query_map([snapshot_row_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(row) = provenance::get_provenance(conn, id)? {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Whether a dataset has market semantics, and which. The newest snapshot
/// wins: snapshots are immutable, so a later one was built from a newer
/// instrument revision or better-known semantics.
pub fn dataset_market_status(conn: &Connection, dataset_id: i64) -> AppResult<DatasetMarketStatus> {
    let row = conn
        .query_row(
            &format!(
                "SELECT {SNAPSHOT_COLUMNS} FROM market_snapshots
                 WHERE dataset_id = ?1 ORDER BY id DESC LIMIT 1"
            ),
            [dataset_id],
            snapshot_from_row,
        )
        .optional()?;
    Ok(match row {
        Some(snapshot) => DatasetMarketStatus::Registered(Box::new(snapshot)),
        None => DatasetMarketStatus::Legacy,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;

    use super::*;
    use crate::db;
    use alpha_factor_forge::discovery_core::market_foundation::{CalendarKind, SessionCalendar, Suspension};
    use crate::market::provenance::RawObservation;
    use crate::market::registry::{
        default_crypto_instruments, ensure_builtin_calendars,
        register_calendar, register_instrument, InstrumentDraft, CRYPTO_24X7_CALENDAR_ID,
    };
    use crate::research::artifacts::ArtifactStore;

    const HOUR: i64 = 3_600_000;
    const DAY: i64 = 86_400_000;
    /// 2024-07-15T00:00:00Z, a Monday.
    const T0: i64 = 1_721_001_600_000;

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
        let dir = std::env::temp_dir().join(format!("aff-snapshot-test-{}-{n}", std::process::id()));
        (ArtifactStore::in_data_dir(&dir), TempDir(dir))
    }

    fn memory_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON").expect("foreign keys");
        db::apply_migrations(&conn).expect("migrations");
        ensure_builtin_calendars(&conn).expect("built-in calendars");
        let draft = default_crypto_instruments().into_iter().next().expect("BTCUSDT");
        register_instrument(&conn, &draft).expect("instrument");
        import_dataset(&mut conn, "binance", "BTCUSDT", "1h", &hourly_bars(4));
        conn
    }

    fn hourly_bars(count: i64) -> Vec<i64> {
        (0..count).map(|index| T0 + index * HOUR).collect()
    }

    /// Import through the real repository path, so every dataset a snapshot
    /// is built from went through identity and admission first.
    fn import_dataset(
        conn: &mut Connection,
        exchange: &str,
        symbol: &str,
        interval: &str,
        timestamps: &[i64],
    ) -> i64 {
        let candles: Vec<repositories::Candle> = timestamps
            .iter()
            .map(|timestamp| repositories::Candle {
                timestamp: *timestamp,
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: 10.0,
            })
            .collect();
        let mut dataset = repositories::Dataset {
            id: None,
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            interval: interval.to_string(),
            start_time: *timestamps.first().expect("at least one bar"),
            end_time: *timestamps.last().expect("at least one bar"),
            candle_count: candles.len() as i64,
            source: "import".to_string(),
            dataset_hash: String::new(),
        };
        dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, &candles)
            .expect("dataset hash");
        repositories::import_dataset_with_candles(conn, &dataset, &candles).expect("import")
    }

    fn record(conn: &Connection, store: &ArtifactStore, observation: &RawObservation, bytes: &[u8]) -> i64 {
        provenance::record_raw(conn, store, observation, bytes).expect("provenance").0
    }

    fn observation() -> RawObservation {
        RawObservation {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            source: "binance-archive".into(),
            role: SeriesRole::Primary,
            request_scope: json!({ "month": "2024-07" }),
            retrieved_at: "2024-08-01T00:00:00Z".into(),
            available_at: Some("2024-07-31T23:00:00Z".into()),
            accepted: true,
            rejection_reason: None,
            revision_of: None,
            media_type: "text/csv".into(),
        }
    }

    fn request(dataset_id: i64, provenance_ids: Vec<i64>) -> SnapshotRequest {
        SnapshotRequest {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            dataset_id,
            price_basis: PriceBasis::Raw,
            corporate_action_version: None,
            cost_profile_version: Some("cost-profile-v1".into()),
            kind: SnapshotKind::Historical,
            provenance_ids,
            as_of_ms: T0 + 4 * HOUR,
        }
    }

    fn codes(events: &[QualityEventRow]) -> Vec<&str> {
        events.iter().map(|row| row.event.code.as_str()).collect()
    }

    #[test]
    fn every_snapshot_kind_requires_source_evidence() {
        for kind in [SnapshotKind::Historical, SnapshotKind::ForwardObserved, SnapshotKind::Demo] {
            let mut conn = memory_db();
            let candidate = SnapshotRequest { kind, ..request(1, vec![]) };
            let outcome = build_snapshot(&mut conn, &candidate).unwrap();
            let SnapshotOutcome::Blocked { events } = outcome else {
                panic!("missing provenance must block: {outcome:?}");
            };
            assert_eq!(codes(&events), vec!["missing_source"]);
            assert_eq!(events[0].event.action, "refetch_range");
            assert_eq!(events, provenance::dataset_quality_events(&conn, 1).unwrap());
            assert_eq!(snapshot_storage_counts(&conn), (0, 0, 1));
            assert_eq!(dataset_market_status(&conn, 1).unwrap(), DatasetMarketStatus::Legacy);
        }
    }

    #[test]
    fn forward_observed_availability_must_not_exceed_the_cut() {
        for (available, expected_code) in [
            (None, Some("availability_unknown")),
            (Some("2024-07-15T03:59:59.999Z"), None),
            (Some("2024-07-15T04:00:00Z"), None),
            (Some("2024-07-15T12:00:00+08:00"), None),
            (Some("2024-07-15T04:00:00.000001Z"), Some("availability_after_cut")),
            (Some("2024-07-31T23:00:00Z"), Some("availability_after_cut")),
        ] {
            let mut conn = memory_db();
            let (store, _guard) = fresh_store();
            let raw = record(&conn, &store, &RawObservation {
                available_at: available.map(str::to_string),
                ..observation()
            }, b"bars");
            let candidate = SnapshotRequest {
                kind: SnapshotKind::ForwardObserved,
                ..request(1, vec![raw])
            };
            let outcome = build_snapshot(&mut conn, &candidate).unwrap();
            if let Some(expected_code) = expected_code {
                let SnapshotOutcome::Blocked { events } = outcome else {
                    panic!("{available:?} must block: {outcome:?}");
                };
                assert_eq!(codes(&events), vec![expected_code]);
                assert_eq!(events[0].event.provenance_id, Some(raw));
                assert_eq!(events[0].event.detail["asOfMs"], json!(candidate.as_of_ms));
                assert_eq!(events, provenance::dataset_quality_events(&conn, 1).unwrap());
                assert_eq!(snapshot_storage_counts(&conn), (0, 0, 1));
            } else {
                let SnapshotOutcome::Created(snapshot) = outcome else {
                    panic!("{available:?} should be admissible: {outcome:?}");
                };
                assert!(snapshot.qualification_eligible());
                assert_eq!(build_snapshot(&mut conn, &candidate).unwrap(),
                    SnapshotOutcome::Existing(snapshot));
                // Existing identity cannot bypass an earlier availability cut.
                let earlier = SnapshotRequest {
                    as_of_ms: T0 + 4 * HOUR - 2,
                    ..candidate
                };
                let outcome = build_snapshot(&mut conn, &earlier).unwrap();
                let SnapshotOutcome::Blocked { events } = outcome else {
                    panic!("earlier cut must revalidate: {outcome:?}");
                };
                assert!(codes(&events).contains(&"availability_after_cut"));
            }
        }
    }

    fn snapshot_storage_counts(conn: &Connection) -> (i64, i64, i64) {
        conn.query_row(
            "SELECT (SELECT COUNT(*) FROM market_snapshots),
                    (SELECT COUNT(*) FROM market_snapshot_sources),
                    (SELECT COUNT(*) FROM market_quality_events)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap()
    }

    #[test]
    fn changed_payload_or_metadata_is_rejected_before_create_or_existing() {
        for already_created in [false, true] {
            for (mutation, expected_error) in [
                ("UPDATE candles SET close = 100.25 WHERE dataset_id = 1", "identity mismatch"),
                ("UPDATE candles SET close = 0 WHERE dataset_id = 1", "identity mismatch"),
                ("UPDATE datasets SET candle_count = 99 WHERE id = 1", "count or time bounds"),
                ("UPDATE datasets SET start_time = start_time + 3600000 WHERE id = 1", "count or time bounds"),
                ("UPDATE datasets SET end_time = end_time - 3600000 WHERE id = 1", "count or time bounds"),
                ("INSERT INTO candles (dataset_id, timestamp, open, high, low, close, volume)
                  SELECT dataset_id, MAX(timestamp) + 3600000, 100, 101, 99, 100.5, 10
                  FROM candles WHERE dataset_id = 1", "count or time bounds"),
            ] {
                let mut conn = memory_db();
                let (store, _guard) = fresh_store();
                let raw = record(&conn, &store, &observation(), b"bars");
                let candidate = request(1, vec![raw]);
                if already_created {
                    assert!(matches!(build_snapshot(&mut conn, &candidate).unwrap(),
                        SnapshotOutcome::Created(_)));
                }
                let before = snapshot_storage_counts(&conn);
                conn.execute(mutation, []).unwrap();
                let error = build_snapshot(&mut conn, &candidate).unwrap_err().to_string();
                assert!(error.contains(expected_error), "{mutation}: {error}");
                assert_eq!(snapshot_storage_counts(&conn), before, "no admission writes");
            }
        }
    }

    #[test]
    fn matching_hash_does_not_admit_an_invalid_stored_candle() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        let raw = record(&conn, &store, &observation(), b"bars");
        // Simulate data stored before the plausibility gate: identity matches
        // these bytes, but a negative volume still cannot support a snapshot.
        conn.execute("UPDATE candles SET volume = -1 WHERE dataset_id = 1", []).unwrap();
        let dataset = repositories::get_dataset_by_id(&conn, 1).unwrap();
        let candles = repositories::get_candles(&conn, 1, i64::MIN, i64::MAX).unwrap();
        let hash = crate::identity::dataset_content_hash(&dataset, &candles).unwrap();
        conn.execute("UPDATE datasets SET dataset_hash = ?1 WHERE id = 1", [hash]).unwrap();
        let error = build_snapshot(&mut conn, &request(1, vec![raw])).unwrap_err().to_string();
        assert!(error.contains("volume_negative"), "{error}");
        assert_eq!(snapshot_storage_counts(&conn), (0, 0, 0));
        assert_eq!(repositories::get_candles(&conn, 1, i64::MIN, i64::MAX).unwrap()[0].volume, -1.0);
    }

    #[test]
    fn a_complete_series_becomes_a_snapshot_research_may_read() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        let dataset_id = 1;
        let raw = record(&conn, &store, &observation(), b"bars");
        let outcome = build_snapshot(&mut conn, &request(dataset_id, vec![raw])).unwrap();
        let SnapshotOutcome::Created(snapshot) = outcome else {
            panic!("expected a snapshot, got {outcome:?}");
        };
        assert_eq!(snapshot.status, SnapshotStatus::Ok);
        assert!(snapshot.qualification_eligible());
        assert_eq!(snapshot.calendar_id, CRYPTO_24X7_CALENDAR_ID);
        assert_eq!(snapshot.snapshot_id.len(), 64);
        assert_eq!(snapshot.coverage["blocking"], json!(false));
        assert_eq!(snapshot.coverage["matchedCount"], json!(4));
        assert_eq!(
            snapshot_sources(&conn, snapshot.id).unwrap().iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![raw]
        );
        assert_eq!(
            dataset_market_status(&conn, dataset_id).unwrap(),
            DatasetMarketStatus::Registered(Box::new(snapshot.clone()))
        );

        // Building it again is the same snapshot, not a second one.
        let again = build_snapshot(&mut conn, &request(dataset_id, vec![raw])).unwrap();
        assert_eq!(again, SnapshotOutcome::Existing(snapshot.clone()));
        assert_eq!(list_snapshots(&conn, 10).unwrap().len(), 1);

        // And it cannot be edited or removed afterwards.
        assert!(conn
            .execute("UPDATE market_snapshots SET status = 'ok' WHERE id = ?1", [snapshot.id])
            .unwrap_err()
            .to_string()
            .contains("immutable"));
        assert!(conn
            .execute("DELETE FROM market_snapshots WHERE id = ?1", [snapshot.id])
            .unwrap_err()
            .to_string()
            .contains("never deleted"));
    }

    #[test]
    fn a_dataset_nobody_registered_is_legacy_and_keeps_working() {
        let mut conn = memory_db();
        let legacy = import_dataset(&mut conn, "csv", "BTCUSDT", "1h", &hourly_bars(3));
        assert_eq!(dataset_market_status(&conn, legacy).unwrap(), DatasetMarketStatus::Legacy);
        // It is still a perfectly readable dataset; nothing about it changed.
        assert_eq!(repositories::get_dataset_by_id(&conn, legacy).unwrap().candle_count, 3);
        assert_eq!(candle_timestamps(&conn, legacy).unwrap().len(), 3);
    }

    #[test]
    fn a_gap_blocks_the_snapshot_and_says_which_bars_and_what_to_do() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        // Bars 2 and 3 of a six-bar range never arrived.
        let sparse = import_dataset(
            &mut conn,
            "binance",
            "BTCUSDT",
            "1h",
            &[T0, T0 + HOUR, T0 + 4 * HOUR, T0 + 5 * HOUR],
        );
        let raw = record(&conn, &store, &observation(), b"sparse");
        let outcome = build_snapshot(
            &mut conn,
            &SnapshotRequest { as_of_ms: T0 + 6 * HOUR, ..request(sparse, vec![raw]) },
        )
        .unwrap();
        let SnapshotOutcome::Blocked { events } = outcome else {
            panic!("a gap must block, got {outcome:?}");
        };
        assert_eq!(codes(&events), vec!["missing_bar"]);
        let gap = &events[0].event;
        assert_eq!(gap.severity, Severity::Blocking);
        assert_eq!(gap.action, "refetch_range");
        assert_eq!((gap.range_start, gap.range_end, gap.bar_count), (Some(T0 + 2 * HOUR), Some(T0 + 3 * HOUR), Some(2)));
        assert_eq!(gap.dataset_id, Some(sparse));
        assert!(list_snapshots(&conn, 10).unwrap().is_empty(), "nothing was admitted");
        assert_eq!(dataset_market_status(&conn, sparse).unwrap(), DatasetMarketStatus::Legacy);
        // The evidence is queryable by dataset, which is how a blocked range
        // is reported back to the user.
        assert_eq!(provenance::dataset_quality_events(&conn, sparse).unwrap().len(), 1);
    }

    #[test]
    fn a_second_venue_or_a_rejected_or_superseded_source_can_never_be_a_component() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        let dataset_id = 1;

        // A Coinbase record for a Binance snapshot: source confusion.
        let coinbase = record(
            &conn,
            &store,
            &RawObservation {
                instrument_id: "crypto:coinbase:BTC-USD".into(),
                source: "coinbase-rest".into(),
                ..observation()
            },
            b"coinbase",
        );
        let binance = record(&conn, &store, &observation(), b"binance");
        let outcome = build_snapshot(&mut conn, &request(dataset_id, vec![binance, coinbase])).unwrap();
        let SnapshotOutcome::Blocked { events } = outcome else {
            panic!("mixed sources must block, got {outcome:?}");
        };
        assert!(codes(&events).iter().all(|code| *code == "source_conflict"));
        let conflicts = events
            .iter()
            .filter_map(|row| row.event.detail["conflicts"].as_array())
            .flatten()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        assert!(conflicts.contains(&"venue_mismatch"), "{conflicts:?}");
        assert!(conflicts.contains(&"source_mismatch"), "{conflicts:?}");

        // A rejected retrieval.
        let rejected = record(
            &conn,
            &store,
            &RawObservation {
                accepted: false,
                rejection_reason: Some("CHECKSUM mismatch".into()),
                ..observation()
            },
            b"corrupt",
        );
        let outcome = build_snapshot(&mut conn, &request(dataset_id, vec![rejected])).unwrap();
        let SnapshotOutcome::Blocked { events } = outcome else {
            panic!("a rejected source must block, got {outcome:?}");
        };
        assert_eq!(codes(&events), vec!["source_conflict"]);
        assert!(events[0].event.detail["reason"].as_str().unwrap().contains("rejected"));

        // A superseded retrieval: the revision must be used instead.
        let revised = record(
            &conn,
            &store,
            &RawObservation {
                retrieved_at: "2024-08-09T00:00:00Z".into(),
                revision_of: Some(binance),
                ..observation()
            },
            b"revised",
        );
        let outcome = build_snapshot(&mut conn, &request(dataset_id, vec![binance])).unwrap();
        let SnapshotOutcome::Blocked { events } = outcome else {
            panic!("a superseded source must block, got {outcome:?}");
        };
        assert_eq!(codes(&events), vec!["superseded_source"]);
        assert_eq!(events[0].event.action, "rebuild_from_revision");
        assert_eq!(events[0].event.detail["revisedBy"], json!(revised));
        // Built from the revision, the same data is admissible.
        let outcome = build_snapshot(&mut conn, &request(dataset_id, vec![revised])).unwrap();
        assert!(matches!(outcome, SnapshotOutcome::Created(_)), "{outcome:?}");
    }

    #[test]
    fn a_dataset_that_is_not_the_instruments_series_is_refused() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        let other = import_dataset(&mut conn, "coinbase", "ETHUSDT", "1h", &hourly_bars(4));
        let raw = record(&conn, &store, &observation(), b"bars");
        let outcome = build_snapshot(&mut conn, &request(other, vec![raw])).unwrap();
        let SnapshotOutcome::Blocked { events } = outcome else {
            panic!("a mismatched dataset must block, got {outcome:?}");
        };
        assert_eq!(codes(&events), vec!["dataset_instrument_mismatch"]);
        let fields = events[0].event.detail["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["field"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(fields, vec!["symbol", "venue"]);
        assert_eq!(events[0].event.action, "separate_sources");

        // The venue comparison is case-insensitive, because `datasets`
        // carries free text while an instrument's venue is normalised.
        let capitalised = import_dataset(&mut conn, "Binance", "BTCUSDT", "1h", &hourly_bars(5));
        let outcome = build_snapshot(
            &mut conn,
            &SnapshotRequest { as_of_ms: T0 + 5 * HOUR, ..request(capitalised, vec![raw]) },
        )
        .unwrap();
        assert!(matches!(outcome, SnapshotOutcome::Created(_)), "{outcome:?}");
    }

    #[test]
    fn unverified_corporate_actions_and_unconfirmed_costs_make_a_snapshot_degraded() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        let dataset_id = 1;
        let raw = record(&conn, &store, &observation(), b"bars");

        // Spot crypto has no corporate actions, so only the cost model can
        // degrade it.
        let outcome = build_snapshot(
            &mut conn,
            &SnapshotRequest { cost_profile_version: None, ..request(dataset_id, vec![raw]) },
        )
        .unwrap();
        let SnapshotOutcome::Created(snapshot) = outcome else {
            panic!("expected a degraded snapshot, got {outcome:?}");
        };
        assert_eq!(snapshot.status, SnapshotStatus::Degraded);
        assert!(!snapshot.qualification_eligible(), "degraded data never qualifies");
        let events = provenance::dataset_quality_events(&conn, dataset_id).unwrap();
        assert_eq!(codes(&events), vec!["cost_profile_unconfirmed"]);
        assert_eq!(events[0].event.snapshot_row_id, Some(snapshot.id));

        // A demo series never qualifies either, however clean it is.
        let demo = build_snapshot(
            &mut conn,
            &SnapshotRequest { kind: SnapshotKind::Demo, ..request(dataset_id, vec![raw]) },
        )
        .unwrap();
        let SnapshotOutcome::Created(demo) = demo else { panic!("expected a demo snapshot") };
        assert_eq!(demo.status, SnapshotStatus::Ok);
        assert!(!demo.qualification_eligible());
    }

    #[test]
    fn an_etf_without_verified_corporate_actions_is_degraded_and_a_holiday_is_not_a_gap() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        // A weekday calendar with Thursday 2024-07-18 closed, and an ETF on it.
        register_calendar(
            &conn,
            &SessionCalendar {
                calendar_id: "fixture-nyse-v1".into(),
                kind: CalendarKind::TradingDays,
                timezone: "America/New_York".into(),
                trading_weekdays: vec![1, 2, 3, 4, 5],
                holidays: vec!["2024-07-18".into()],
                early_closes: Vec::new(),
            },
        )
        .unwrap();
        register_instrument(
            &conn,
            &InstrumentDraft {
                instrument_id: "us-etf:nyse-arca:SPY".into(),
                base: "SPY".into(),
                quote: "USD".into(),
                asset_type: AssetType::Etf,
                session_calendar_id: "fixture-nyse-v1".into(),
                timezone: "America/New_York".into(),
                lot_size: Some(1.0),
                price_step: Some(0.01),
                min_notional: None,
                listed_from: None,
                delisted_at: None,
                suspensions: vec![Suspension { from_ms: T0 + DAY, to_ms_exclusive: T0 + 2 * DAY }],
                source_capabilities: json!({ "tiingo": { "raw": true } }),
            },
        )
        .unwrap();
        // Monday, Wednesday, Friday: Tuesday halted, Thursday a holiday,
        // and the weekend is not a trading day.
        let dataset_id = import_dataset(
            &mut conn,
            "nyse-arca",
            "SPY",
            "1d",
            &[T0, T0 + 2 * DAY, T0 + 4 * DAY],
        );
        let raw = record(
            &conn,
            &store,
            &RawObservation {
                instrument_id: "us-etf:nyse-arca:SPY".into(),
                interval: "1d".into(),
                source: "tiingo".into(),
                media_type: "application/json".into(),
                ..observation()
            },
            b"{}",
        );
        let outcome = build_snapshot(
            &mut conn,
            &SnapshotRequest {
                instrument_id: "us-etf:nyse-arca:SPY".into(),
                interval: "1d".into(),
                dataset_id,
                price_basis: PriceBasis::Raw,
                corporate_action_version: None,
                cost_profile_version: Some("cost-profile-v1".into()),
                kind: SnapshotKind::Historical,
                provenance_ids: vec![raw],
                as_of_ms: T0 + 7 * DAY,
            },
        )
        .unwrap();
        let SnapshotOutcome::Created(snapshot) = outcome else {
            panic!("the weekend, the holiday, and the halt are not gaps: {outcome:?}");
        };
        assert_eq!(snapshot.status, SnapshotStatus::Degraded);
        assert!(!snapshot.qualification_eligible());
        assert_eq!(snapshot.coverage["events"], json!([]), "no missing bars");
        let events = provenance::dataset_quality_events(&conn, dataset_id).unwrap();
        assert_eq!(codes(&events), vec!["corporate_actions_unverified"]);
        assert_eq!(events[0].event.action, "verify_corporate_actions");
    }

    #[test]
    fn a_forward_observed_snapshot_needs_to_know_when_each_bar_was_observable() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        let dataset_id = 1;
        let unknown = record(
            &conn,
            &store,
            &RawObservation { available_at: None, ..observation() },
            b"bars",
        );
        let outcome = build_snapshot(
            &mut conn,
            &SnapshotRequest { kind: SnapshotKind::ForwardObserved, ..request(dataset_id, vec![unknown]) },
        )
        .unwrap();
        let SnapshotOutcome::Blocked { events } = outcome else {
            panic!("unknown availability must block, got {outcome:?}");
        };
        assert_eq!(codes(&events), vec!["availability_unknown"]);
        assert_eq!(events[0].event.action, "record_availability");
        // The same record is fine for a historical snapshot.
        let outcome = build_snapshot(&mut conn, &request(dataset_id, vec![unknown])).unwrap();
        assert!(matches!(outcome, SnapshotOutcome::Created(_)), "{outcome:?}");
    }

    #[test]
    fn an_unknown_instrument_or_provenance_id_is_an_error_not_silent_evidence() {
        let mut conn = memory_db();
        let missing_instrument = build_snapshot(
            &mut conn,
            &SnapshotRequest { instrument_id: "crypto:binance:XRPUSDT".into(), ..request(1, vec![]) },
        )
        .unwrap_err()
        .to_string();
        assert!(missing_instrument.contains("is not registered"), "{missing_instrument}");
        let missing_provenance = build_snapshot(&mut conn, &request(1, vec![4242]))
            .unwrap_err()
            .to_string();
        assert!(missing_provenance.contains("does not exist"), "{missing_provenance}");
        assert!(provenance::dataset_quality_events(&conn, 1).unwrap().is_empty());
    }
}
