//! P12d-2a: read-only resolution of a frozen campaign against exact P06 snapshots.
//!
//! The returned rows describe what was verified at this read. They are not an
//! admission decision or a freshness fence; P12d-2b must bind them to a run
//! and repeat the checks in its admission transaction.
#![allow(dead_code)] // P12d-2b adds the first runtime caller.

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;

use alpha_factor_forge::discovery_core::{
    campaign::FrozenCampaignDeclaration,
    market_data::ensure_admissible,
    market_foundation::{
        parse_instrument_id, AssetType, SeriesRole, MARKET_INSTRUMENT_VERSION,
        MARKET_PROVENANCE_VERSION, MARKET_SNAPSHOT_VERSION,
    },
};

use crate::{
    db::repositories,
    error::{AppError, AppResult},
    identity,
    market::{provenance, registry, snapshot},
};

use super::{canonical_json, sha256_hex};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Declaration {
    instruments: Vec<InstrumentBinding>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstrumentBinding {
    instrument_id: String,
    listed_at_ms: u64,
    delisted_at_ms: Option<u64>,
    snapshot_id: String,
    dataset_hash: String,
    interval: String,
    from_ms: u64,
    to_ms: u64,
    sample_policy: SamplePolicy,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SamplePolicy {
    minimum_total_bars: u64,
}

/// Exact row references for later transactional admission. No status or
/// qualification claim is carried beyond the current database read.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedInstrument {
    pub instrument_id: String,
    pub snapshot_id: String,
    pub snapshot_row_id: i64,
    pub instrument_row_id: i64,
    pub dataset_id: i64,
    pub dataset_hash: String,
    pub bar_count: u64,
    pub cost_profile_version: String,
}

fn reject(instrument_id: &str, reason: &str) -> AppError {
    AppError::Other(format!("campaign snapshot {instrument_id}: {reason}"))
}

fn require(instrument_id: &str, condition: bool, reason: &str) -> AppResult<()> {
    if condition {
        Ok(())
    } else {
        Err(reject(instrument_id, reason))
    }
}

/// Resolve only the IDs named by the immutable declaration. No latest-snapshot
/// fallback, write, ledger read, or result-derived threshold is allowed here.
pub fn resolve_campaign_snapshots(
    conn: &Connection,
    campaign: &FrozenCampaignDeclaration,
) -> AppResult<Vec<ResolvedInstrument>> {
    let declaration: Declaration = serde_json::from_value(campaign.document().clone())?;
    declaration
        .instruments
        .iter()
        .map(|binding| resolve_one(conn, binding))
        .collect()
}

/// The same exact resolution for one declared instrument (P12d-2c): a
/// campaign-bound run needs only its own binding, not every instrument's.
pub fn resolve_campaign_instrument(
    conn: &Connection,
    campaign: &FrozenCampaignDeclaration,
    instrument_id: &str,
) -> AppResult<ResolvedInstrument> {
    let declaration: Declaration = serde_json::from_value(campaign.document().clone())?;
    let binding = declaration
        .instruments
        .iter()
        .find(|binding| binding.instrument_id == instrument_id)
        .ok_or_else(|| reject(instrument_id, "instrument is not declared by this campaign"))?;
    resolve_one(conn, binding)
}

fn resolve_one(conn: &Connection, binding: &InstrumentBinding) -> AppResult<ResolvedInstrument> {
    let id = &binding.instrument_id;
    let snapshot = snapshot::get_snapshot_by_id(conn, &binding.snapshot_id)?
        .ok_or_else(|| reject(id, "declared snapshot is missing"))?;
    require(
        id,
        snapshot.version == MARKET_SNAPSHOT_VERSION,
        "unsupported snapshot version",
    )?;
    require(
        id,
        snapshot.qualification_eligible(),
        "snapshot is demo or degraded",
    )?;
    require(
        id,
        snapshot.instrument_id == *id && snapshot.interval == binding.interval,
        "snapshot instrument or interval differs from declaration",
    )?;
    require(
        id,
        snapshot.dataset_hash == binding.dataset_hash,
        "snapshot dataset hash differs from declaration",
    )?;
    require(
        id,
        snapshot.coverage["blocking"] == false,
        "snapshot coverage is not clean",
    )?;

    let instrument = registry::get_instrument(conn, snapshot.instrument_row_id)?
        .ok_or_else(|| reject(id, "snapshot instrument revision is missing"))?;
    require(
        id,
        instrument.version == MARKET_INSTRUMENT_VERSION
            && instrument.instrument_id == *id
            && instrument.session_calendar_id == snapshot.calendar_id,
        "snapshot instrument revision or calendar differs",
    )?;
    require(
        id,
        registry::get_calendar(conn, &snapshot.calendar_id)?.is_some(),
        "snapshot calendar is missing",
    )?;
    require(
        id,
        instrument.listed_from == i64::try_from(binding.listed_at_ms).ok()
            && instrument.delisted_at == binding.delisted_at_ms.and_then(|v| i64::try_from(v).ok()),
        "declared listing period differs from frozen instrument revision",
    )?;
    let from = i64::try_from(binding.from_ms).map_err(|_| reject(id, "invalid range start"))?;
    let to = i64::try_from(binding.to_ms).map_err(|_| reject(id, "invalid range end"))?;
    require(
        id,
        instrument.listed_from.is_some_and(|start| start <= from)
            && instrument.delisted_at.is_none_or(|end| to < end),
        "dataset range is outside listing period",
    )?;

    let dataset = repositories::get_dataset_by_id(conn, snapshot.dataset_id)?;
    let parsed = parse_instrument_id(id).map_err(|_| reject(id, "invalid instrument identity"))?;
    require(
        id,
        dataset.exchange == parsed.venue
            && dataset.symbol == parsed.symbol
            && dataset.interval == binding.interval
            && dataset.start_time == from
            && dataset.end_time == to
            && dataset.dataset_hash == binding.dataset_hash,
        "dataset identity, interval, or exact range differs from declaration",
    )?;
    let candles = repositories::get_candles(conn, snapshot.dataset_id, i64::MIN, i64::MAX)?;
    let normalized = identity::verify_dataset_identity(&dataset, &candles)?;
    ensure_admissible(normalized.iter().map(repositories::db_candle_fields))
        .map_err(|error| reject(id, &error.0))?;
    require(
        id,
        u64::try_from(normalized.len()).is_ok_and(|count| {
            count >= binding.sample_policy.minimum_total_bars
                && snapshot.coverage["matchedCount"] == json!(count)
        }),
        "dataset has too few bars or snapshot coverage differs",
    )?;

    let cost = snapshot
        .cost_profile_version
        .as_deref()
        .filter(|version| !version.trim().is_empty())
        .ok_or_else(|| reject(id, "snapshot has no confirmed cost profile"))?;
    require(
        id,
        instrument.asset_type != AssetType::Etf
            || snapshot
                .corporate_action_version
                .as_deref()
                .is_some_and(|v| !v.trim().is_empty()),
        "ETF snapshot has no corporate-action version",
    )?;

    let sources = snapshot::snapshot_sources(conn, snapshot.id)?;
    require(id, !sources.is_empty(), "snapshot has no source evidence")?;
    let source_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM market_snapshot_sources WHERE snapshot_row_id = ?1",
        [snapshot.id],
        |row| row.get(0),
    )?;
    require(
        id,
        usize::try_from(source_count).ok() == Some(sources.len()),
        "snapshot source evidence is incomplete",
    )?;
    for source in &sources {
        require(
            id,
            source.version == MARKET_PROVENANCE_VERSION
                && source.accepted
                && source.role == SeriesRole::Primary
                && source.instrument_id == *id
                && source.interval == binding.interval
                && source.rejection_reason.is_none()
                && provenance::revised_by(conn, source.id)?.is_none(),
            "snapshot source is rejected, superseded, or belongs to another series",
        )?;
    }

    // P06 hashes the snapshot's content document. Reconstruct it from the
    // authoritative row and its exact source set to detect a broken binding.
    let mut source_hashes: Vec<_> = sources
        .iter()
        .map(|source| source.record_hash.clone())
        .collect();
    source_hashes.sort();
    let content = json!({
        "version": snapshot.version,
        "instrumentId": snapshot.instrument_id,
        "instrumentContentHash": instrument.content_hash,
        "interval": snapshot.interval,
        "datasetHash": snapshot.dataset_hash,
        "priceBasis": snapshot.price_basis.as_str(),
        "calendarId": snapshot.calendar_id,
        "corporateActionVersion": snapshot.corporate_action_version,
        "costProfileVersion": snapshot.cost_profile_version,
        "kind": snapshot.kind.as_str(),
        "sourceRecordHashes": source_hashes,
    });
    require(
        id,
        sha256_hex(&canonical_json(&content)?) == binding.snapshot_id,
        "snapshot content identity differs from declared ID",
    )?;

    Ok(ResolvedInstrument {
        instrument_id: id.clone(),
        snapshot_id: snapshot.snapshot_id,
        snapshot_row_id: snapshot.id,
        instrument_row_id: instrument.id,
        dataset_id: snapshot.dataset_id,
        dataset_hash: binding.dataset_hash.clone(),
        bar_count: normalized.len() as u64,
        cost_profile_version: cost.to_string(),
    })
}

#[cfg(test)]
mod tests;
