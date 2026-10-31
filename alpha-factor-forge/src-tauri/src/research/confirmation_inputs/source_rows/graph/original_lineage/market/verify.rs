//! Original document/row consistency only; no current-head or payload authority.
use super::{constructed, copy, enter_payload, invalid, require, AppResult, Documents, Scope};
use crate::{
    market::{provenance::RAW_MEDIA_TYPES, registry::InstrumentDraft},
    research::{canonical_json, sha256_hex},
};
use alpha_factor_forge::discovery_core::market_foundation::{
    interval_ms, parse_instrument_id, sources_share_origin, validate_calendar, AssetType,
    CalendarKind, Market, SessionCalendar,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn keys(v: &Value, expected: &[&str]) -> AppResult<()> {
    require(
        v.as_object().is_some_and(|o| {
            o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k))
        }),
    )
}
fn string<'a>(v: &'a Value, key: &str) -> AppResult<&'a str> {
    v.get(key).and_then(Value::as_str).ok_or_else(invalid)
}
fn field(v: &Value, key: &str, expected: &str) -> AppResult<()> {
    require(string(v, key)? == expected)
}
fn optional(v: &Value, key: &str, expected: &Option<String>) -> AppResult<()> {
    require(match (v.get(key), expected) {
        (Some(Value::Null), None) => true,
        (Some(Value::String(a)), Some(b)) => a == b,
        _ => false,
    })
}
fn hash(v: &Value, expected: &str) -> AppResult<()> {
    require(sha256_hex(&canonical_json(v)?) == expected)
}
fn is_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn calendar(d: &Documents<'_>) -> AppResult<SessionCalendar> {
    let r = &d.rows.calendar;
    keys(
        &d.calendar,
        &[
            "calendarId",
            "kind",
            "timezone",
            "tradingWeekdays",
            "holidays",
            "earlyCloses",
            "version",
        ],
    )?;
    field(&d.calendar, "version", "session-calendar-v1")?;
    require(r.id > 0 && r.version == "session-calendar-v1")?;
    let c: SessionCalendar = serde_json::from_value(copy(&d.calendar)?).map_err(|_| invalid())?;
    require(
        validate_calendar(&c).is_none()
            && c.calendar_id == r.calendar_id
            && c.kind.as_str() == r.kind
            && c.timezone == r.timezone,
    )?;
    let mut original = serde_json::to_value(&c)?;
    original["version"] = json!("session-calendar-v1");
    constructed(&original)?;
    require(original == d.calendar)?;
    hash(&d.calendar, &r.content_hash)?;
    Ok(c)
}
fn specification(v: Option<f64>, bits: Option<u64>) -> bool {
    v.map(f64::to_bits) == bits
        && match v {
            None => true,
            Some(n) => n.is_finite() && n > 0.0,
        }
}
fn instrument(d: &Documents<'_>, calendar: &SessionCalendar) -> AppResult<AssetType> {
    let r = &d.rows.instrument;
    keys(
        &d.instrument,
        &[
            "instrumentId",
            "base",
            "quote",
            "assetType",
            "sessionCalendarId",
            "timezone",
            "lotSize",
            "priceStep",
            "minNotional",
            "listedFrom",
            "delistedAt",
            "suspensions",
            "sourceCapabilities",
            "version",
        ],
    )?;
    field(&d.instrument, "version", "market-instrument-v1")?;
    require(r.id > 0 && r.revision > 0 && r.version == "market-instrument-v1")?;
    let draft: InstrumentDraft =
        serde_json::from_value(copy(&d.instrument)?).map_err(|_| invalid())?;
    let parsed = parse_instrument_id(&draft.instrument_id).map_err(|_| invalid())?;
    let asset = match parsed.market {
        Market::Crypto => AssetType::SpotCrypto,
        Market::UsEtf | Market::TwEtf => AssetType::Etf,
    };
    require(
        draft.asset_type == asset
            && draft.asset_type.as_str() == r.asset_type
            && draft.instrument_id == r.instrument_id
            && parsed.market.as_str() == r.market
            && parsed.venue == r.venue
            && parsed.symbol == r.symbol
            && draft.base == r.base
            && draft.quote == r.quote
            && draft.timezone == r.timezone
            && draft.session_calendar_id == r.session_calendar_id
            && draft.session_calendar_id == calendar.calendar_id
            && draft.listed_from == r.listed_from
            && draft.delisted_at == r.delisted_at,
    )?;
    for s in [&draft.base, &draft.quote, &draft.timezone] {
        require(!s.trim().is_empty() && s.trim() == s)?;
    }
    require(
        specification(draft.lot_size, r.lot_size_bits)
            && specification(draft.price_step, r.price_step_bits)
            && specification(draft.min_notional, r.min_notional_bits)
            && draft.source_capabilities.is_object()
            && draft.source_capabilities == d.capabilities,
    )?;
    if let (Some(listed), Some(delisted)) = (draft.listed_from, draft.delisted_at) {
        require(delisted > listed)?;
    }
    let mut previous = None;
    for s in &draft.suspensions {
        require(s.to_ms_exclusive > s.from_ms && previous.is_none_or(|end| s.from_ms >= end))?;
        previous = Some(s.to_ms_exclusive);
    }
    require(
        calendar.kind
            == match asset {
                AssetType::SpotCrypto => CalendarKind::Continuous,
                AssetType::Etf => CalendarKind::TradingDays,
            },
    )?;
    let suspensions = serde_json::to_value(&draft.suspensions)?;
    constructed(&suspensions)?;
    require(suspensions == d.suspensions)?;
    let mut original = serde_json::to_value(&draft)?;
    original["version"] = json!("market-instrument-v1");
    constructed(&original)?;
    // Also rejects unknown nested Suspension fields discarded by serde.
    require(original == d.instrument)?;
    require(canonical_json(&original)? == canonical_json(&d.instrument)?)?;
    hash(&d.instrument, &r.content_hash)?;
    Ok(asset)
}
fn provenance(
    d: &Scope<'_>,
    rows: &super::SourceRowsObservation,
    cut: DateTime<Utc>,
) -> AppResult<()> {
    let p = d.row;
    require(
        p.id > 0
            && p.version == "market-provenance-v1"
            && p.instrument_id == rows.snapshot.instrument_id
            && p.interval == rows.snapshot.interval
            && interval_ms(&p.interval).is_some()
            && !p.source.trim().is_empty()
            && p.role == "primary"
            && p.accepted == 1
            && p.rejection_reason.is_none()
            && p.raw_byte_len >= 0
            && p.revision_of.is_none_or(|id| id > 0)
            && is_hash(&p.raw_sha256),
    )?;
    let retrieved = DateTime::parse_from_rfc3339(&p.retrieved_at).map_err(|_| invalid())?;
    let available = p
        .available_at
        .as_deref()
        .map(DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(|_| invalid())?;
    if let Some(available) = available {
        // record_raw's original validation deliberately compares milliseconds.
        require(available.timestamp_millis() <= retrieved.timestamp_millis())?;
    }
    if rows.snapshot.kind == "forward-observed" {
        let known = available.ok_or_else(invalid)?;
        require(known <= cut)?;
        if p.source == "binance-archive" {
            require(
                d.value.get("availabilityBasis").and_then(Value::as_str)
                    == Some("observed-at-retrieval")
                    && known == retrieved,
            )?;
        }
    }
    let extension = RAW_MEDIA_TYPES
        .iter()
        .find(|(media, _)| *media == p.raw_media_type)
        .map(|(_, ext)| *ext)
        .ok_or_else(invalid)?;
    require(
        p.raw_artifact_path == format!("{}/{}.{}", &p.raw_sha256[..2], p.raw_sha256, extension),
    )?;
    let identity = json!({"version":p.version,"instrumentId":p.instrument_id,"interval":p.interval,
        "source":p.source,"role":p.role,"requestScope":&d.value,"rawSha256":p.raw_sha256,
        "rawByteLen":p.raw_byte_len,"mediaType":p.raw_media_type,"retrievedAt":p.retrieved_at,
        "availableAt":p.available_at,"accepted":p.accepted==1,"rejectionReason":p.rejection_reason,
        "revisionOf":p.revision_of});
    constructed(&identity)?;
    hash(&identity, &p.record_hash)
}
fn coverage(d: &Documents<'_>) -> AppResult<()> {
    keys(
        &d.coverage,
        &[
            "version",
            "events",
            "blocking",
            "expectedCount",
            "notDueCount",
            "observedCount",
            "matchedCount",
            "issue",
        ],
    )?;
    field(&d.coverage, "version", "market-foundation-v1")?;
    let count = d.rows.dataset.candle_count;
    require(
        (1..=1_000_000).contains(&count)
            && d.coverage["events"].as_array().is_some_and(Vec::is_empty)
            && d.coverage["blocking"] == Value::Bool(false)
            && d.coverage["issue"].is_null()
            && d.coverage["notDueCount"].as_i64() == Some(0),
    )?;
    for key in ["expectedCount", "observedCount", "matchedCount"] {
        require(d.coverage[key].as_i64() == Some(count))?;
    }
    Ok(())
}
fn snapshot(d: &Documents<'_>, asset: AssetType) -> AppResult<()> {
    let r = d.rows;
    let s = &r.snapshot;
    let i = &r.instrument;
    let ds = &r.dataset;
    keys(
        &d.snapshot,
        &[
            "version",
            "instrumentId",
            "instrumentContentHash",
            "interval",
            "datasetHash",
            "priceBasis",
            "calendarId",
            "corporateActionVersion",
            "costProfileVersion",
            "kind",
            "sourceRecordHashes",
        ],
    )?;
    require(
        s.id > 0
            && s.version == "market-snapshot-v1"
            && s.status == "ok"
            && s.price_basis == "raw"
            && matches!(s.kind.as_str(), "historical" | "forward-observed")
            && s.instrument_row_id == i.id
            && s.instrument_id == i.instrument_id
            && s.instrument_id == r.roots.admission.instrument_id
            && s.calendar_id == r.calendar.calendar_id
            && s.dataset_id == ds.id
            && s.dataset_hash == ds.dataset_hash
            && s.interval == ds.interval
            && interval_ms(&ds.interval).is_some()
            && ds.symbol == i.symbol
            && ds.exchange.eq_ignore_ascii_case(&i.venue)
            && ds.start_time <= ds.end_time
            && ds.start_time == r.campaign_from_ms
            && ds.end_time == r.campaign_to_ms
            && s.cost_profile_version.is_some()
            && (asset != AssetType::Etf || s.corporate_action_version.is_some()),
    )?;
    for (key, expected) in [
        ("version", s.version.as_str()),
        ("instrumentId", s.instrument_id.as_str()),
        ("instrumentContentHash", i.content_hash.as_str()),
        ("interval", s.interval.as_str()),
        ("datasetHash", s.dataset_hash.as_str()),
        ("priceBasis", s.price_basis.as_str()),
        ("calendarId", s.calendar_id.as_str()),
        ("kind", s.kind.as_str()),
    ] {
        field(&d.snapshot, key, expected)?;
    }
    optional(
        &d.snapshot,
        "corporateActionVersion",
        &s.corporate_action_version,
    )?;
    optional(&d.snapshot, "costProfileVersion", &s.cost_profile_version)?;
    let source_map: BTreeMap<_, _> = d.scopes.iter().map(|p| (p.row.id, p)).collect();
    require(source_map.len() == d.scopes.len() && r.sources.len() == source_map.len())?;
    let mut ids = BTreeSet::new();
    for source in &r.sources {
        require(
            source.snapshot_row_id == s.id
                && ids.insert(source.provenance_id)
                && source_map.contains_key(&source.provenance_id),
        )?;
    }
    let first = d.scopes.first().ok_or_else(invalid)?;
    let cut = DateTime::<Utc>::from_timestamp_millis(s.as_of).ok_or_else(invalid)?;
    for source in &d.scopes {
        require(
            first.row.source == source.row.source
                || sources_share_origin(&first.row.source, &source.row.source),
        )?;
        provenance(source, r, cut)?;
    }
    let mut hashes: Vec<_> = d
        .scopes
        .iter()
        .map(|p| p.row.record_hash.as_str())
        .collect();
    hashes.sort_unstable();
    let stored = d.snapshot["sourceRecordHashes"]
        .as_array()
        .ok_or_else(invalid)?;
    require(
        stored.len() == hashes.len()
            && stored
                .iter()
                .zip(hashes)
                .all(|(a, b)| a.as_str() == Some(b)),
    )?;
    coverage(d)?;
    hash(&d.snapshot, &s.snapshot_id)
}
pub(super) fn all(d: &Documents<'_>) -> AppResult<()> {
    enter_payload();
    let calendar = calendar(d)?;
    let asset = instrument(d, &calendar)?;
    snapshot(d, asset)
}
