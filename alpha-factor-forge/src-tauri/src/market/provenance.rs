//! P06 — raw retrievals, their revisions, and the structured quality
//! evidence (`market_provenance`, `market_quality_events` in migration
//! 0008).
//!
//! One immutable row per retrieval, including the REJECTED ones: a rejection
//! that is not kept cannot be replayed or argued with. Raw bytes go to the
//! content-addressed artifact store and are referenced by checksum, so the
//! database stays small and the bytes stay provably unmodified.
//!
//! A revision points at what it revises. It never overwrites it, so the
//! chain IS the history: the head is the row nothing else revises, and every
//! earlier row remains exactly what the source said at the time.

use chrono::DateTime;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use alpha_factor_forge::discovery_core::market_foundation::{
    interval_ms, parse_instrument_id, SeriesRole, MARKET_PROVENANCE_VERSION,
};
use crate::error::{AppError, AppResult};
use crate::research::artifacts::{ArtifactRef, ArtifactStore};

use super::{canonical_json, sha256_hex, QualityEvent, MARKET_RAW_ARTIFACT_KIND};

/// Media types a raw response may be stored as, and the file extension each
/// one keeps in the artifact store. Deliberately short: P07/P09/P10 add what
/// their sources actually return, and an unlisted type is refused rather
/// than stored under a name that lies about its content.
pub const RAW_MEDIA_TYPES: [(&str, &str); 4] = [
    ("application/json", "json"),
    ("text/csv", "csv"),
    ("application/zip", "zip"),
    ("text/plain", "txt"),
];

fn extension_for(media_type: &str) -> AppResult<&'static str> {
    RAW_MEDIA_TYPES
        .iter()
        .find(|(name, _)| *name == media_type)
        .map(|(_, extension)| *extension)
        .ok_or_else(|| {
            AppError::Other(format!(
                "media type {media_type:?} is not one this contract stores raw responses as"
            ))
        })
}

/// What was asked for, what came back, and when it could first be seen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawObservation {
    pub instrument_id: String,
    pub interval: String,
    /// The retrieval source, e.g. `binance-archive`, `binance-rest`.
    pub source: String,
    /// `comparison` evidence can never become a snapshot component.
    pub role: SeriesRole,
    /// Endpoint, parameters, and the range that was requested.
    pub request_scope: Value,
    /// RFC 3339, UTC.
    pub retrieved_at: String,
    /// When this data could FIRST have been observed. `None` means unknown,
    /// and unknown availability can never support a forward-observed
    /// snapshot — re-downloading history does not make it point-in-time.
    pub available_at: Option<String>,
    pub accepted: bool,
    /// Required exactly when `accepted` is false.
    pub rejection_reason: Option<String>,
    /// The record this one revises; never an overwrite.
    pub revision_of: Option<i64>,
    pub media_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceRow {
    pub id: i64,
    pub record_hash: String,
    pub version: String,
    pub instrument_id: String,
    pub interval: String,
    pub source: String,
    pub role: SeriesRole,
    pub request_scope: Value,
    pub raw_sha256: String,
    pub raw_byte_len: i64,
    pub raw_artifact_path: String,
    pub raw_media_type: String,
    pub retrieved_at: String,
    pub available_at: Option<String>,
    pub accepted: bool,
    pub rejection_reason: Option<String>,
    pub revision_of: Option<i64>,
    pub created_at: String,
}

impl ProvenanceRow {
    /// The original P07 adapter guessed availability from the archive's
    /// period. Keep those immutable rows for audit, but do not trust them
    /// for cache reuse or forward-observed admission.
    pub fn has_observed_archive_availability(&self) -> bool {
        if self.request_scope["availabilityBasis"] != super::sources::binance::AVAILABILITY_BASIS {
            return false;
        }
        let available = self.available_at.as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok());
        let retrieved = DateTime::parse_from_rfc3339(&self.retrieved_at).ok();
        matches!((available, retrieved), (Some(a), Some(r)) if a == r)
    }

    /// The reference needed to read the raw bytes back and prove they are
    /// the ones that were recorded.
    pub fn artifact_ref(&self) -> ArtifactRef {
        ArtifactRef {
            kind: MARKET_RAW_ARTIFACT_KIND.to_string(),
            sha256: self.raw_sha256.clone(),
            byte_len: self.raw_byte_len,
            relative_path: self.raw_artifact_path.clone(),
        }
    }
}

fn utc_instant(name: &str, value: &str) -> AppResult<i64> {
    DateTime::parse_from_rfc3339(value)
        .map(|moment| moment.timestamp_millis())
        .map_err(|error| AppError::Other(format!("{name} is not an RFC 3339 instant: {error}")))
}

fn record_hash(observation: &RawObservation, raw_sha256: &str, raw_byte_len: i64) -> AppResult<String> {
    let content = json!({
        "version": MARKET_PROVENANCE_VERSION,
        "instrumentId": observation.instrument_id,
        "interval": observation.interval,
        "source": observation.source,
        "role": observation.role,
        "requestScope": observation.request_scope,
        "rawSha256": raw_sha256,
        "rawByteLen": raw_byte_len,
        "mediaType": observation.media_type,
        "retrievedAt": observation.retrieved_at,
        "availableAt": observation.available_at,
        "accepted": observation.accepted,
        "rejectionReason": observation.rejection_reason,
        "revisionOf": observation.revision_of,
    });
    Ok(sha256_hex(&canonical_json(&content)?))
}

/// Record one retrieval: store the bytes immutably, then reference them.
/// Returns the row id and whether this call created it; recording the same
/// observation twice is the same row.
pub fn record_raw(
    conn: &Connection,
    store: &ArtifactStore,
    observation: &RawObservation,
    bytes: &[u8],
) -> AppResult<(i64, bool)> {
    parse_instrument_id(&observation.instrument_id).map_err(|rule| {
        AppError::Other(format!(
            "instrument id {:?} is not valid: {}",
            observation.instrument_id,
            rule.as_str()
        ))
    })?;
    if interval_ms(&observation.interval).is_none() {
        return Err(AppError::Other(format!(
            "interval {:?} is not one this contract knows",
            observation.interval
        )));
    }
    if observation.source.trim().is_empty() {
        return Err(AppError::Other("a retrieval names its source".into()));
    }
    if observation.accepted != observation.rejection_reason.is_none() {
        return Err(AppError::Other(
            "a rejected retrieval states its reason, and an accepted one has none".into(),
        ));
    }
    let retrieved = utc_instant("retrievedAt", &observation.retrieved_at)?;
    if let Some(available_at) = &observation.available_at {
        let available = utc_instant("availableAt", available_at)?;
        if available > retrieved {
            return Err(AppError::Other(
                "data cannot be retrieved before it was available".into(),
            ));
        }
    }
    let extension = extension_for(&observation.media_type)?;
    let hash = record_hash(observation, &sha256_hex(bytes), bytes.len() as i64)?;
    // An exact retry is not a second child of the revision target. Resolve
    // it before checking for a fork, even if this revision was later revised.
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM market_provenance WHERE record_hash = ?1",
            [&hash],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        // Preserve artifact verification/recovery on retries.
        store.put_with_extension(MARKET_RAW_ARTIFACT_KIND, extension, bytes)?;
        return Ok((id, false));
    }

    if let Some(target) = observation.revision_of {
        let previous = get_provenance(conn, target)?.ok_or_else(|| {
            AppError::Other(format!("revision target {target} does not exist"))
        })?;
        if previous.instrument_id != observation.instrument_id
            || previous.interval != observation.interval
            || previous.source != observation.source
        {
            return Err(AppError::Other(format!(
                "revision target {target} is a different series; a revision never crosses sources"
            )));
        }
        let forked: Option<i64> = conn
            .query_row(
                "SELECT id FROM market_provenance WHERE revision_of = ?1 LIMIT 1",
                [target],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = forked {
            return Err(AppError::Other(format!(
                "revision target {target} is already revised by record {existing}; \
                 revise the head of the chain instead"
            )));
        }
    }

    let raw = store.put_with_extension(MARKET_RAW_ARTIFACT_KIND, extension, bytes)?;
    conn.execute(
        "INSERT INTO market_provenance
            (record_hash, version, instrument_id, interval, source, role, request_scope_json,
             raw_sha256, raw_byte_len, raw_artifact_path, raw_media_type, retrieved_at,
             available_at, accepted, rejection_reason, revision_of)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            hash,
            MARKET_PROVENANCE_VERSION,
            observation.instrument_id,
            observation.interval,
            observation.source,
            observation.role.as_str(),
            serde_json::to_string(&observation.request_scope)?,
            raw.sha256,
            raw.byte_len,
            raw.relative_path,
            observation.media_type,
            observation.retrieved_at,
            observation.available_at,
            i64::from(observation.accepted),
            observation.rejection_reason,
            observation.revision_of,
        ],
    )?;
    Ok((conn.last_insert_rowid(), true))
}

const PROVENANCE_COLUMNS: &str = "id, record_hash, version, instrument_id, interval, source, role, request_scope_json,
    raw_sha256, raw_byte_len, raw_artifact_path, raw_media_type, retrieved_at, available_at,
    accepted, rejection_reason, revision_of, created_at";

fn provenance_from_row(row: &Row<'_>) -> rusqlite::Result<ProvenanceRow> {
    let role: String = row.get(6)?;
    let scope: String = row.get(7)?;
    let accepted: i64 = row.get(14)?;
    Ok(ProvenanceRow {
        id: row.get(0)?,
        record_hash: row.get(1)?,
        version: row.get(2)?,
        instrument_id: row.get(3)?,
        interval: row.get(4)?,
        source: row.get(5)?,
        role: SeriesRole::parse(&role).unwrap_or(SeriesRole::Comparison),
        request_scope: serde_json::from_str(&scope).unwrap_or(Value::Null),
        raw_sha256: row.get(8)?,
        raw_byte_len: row.get(9)?,
        raw_artifact_path: row.get(10)?,
        raw_media_type: row.get(11)?,
        retrieved_at: row.get(12)?,
        available_at: row.get(13)?,
        accepted: accepted != 0,
        rejection_reason: row.get(15)?,
        revision_of: row.get(16)?,
        created_at: row.get(17)?,
    })
}

pub fn get_provenance(conn: &Connection, id: i64) -> AppResult<Option<ProvenanceRow>> {
    Ok(conn
        .query_row(
            &format!("SELECT {PROVENANCE_COLUMNS} FROM market_provenance WHERE id = ?1"),
            [id],
            provenance_from_row,
        )
        .optional()?)
}

/// Every record of one series, oldest first — rejections included.
pub fn list_provenance(
    conn: &Connection,
    instrument_id: &str,
    interval: &str,
) -> AppResult<Vec<ProvenanceRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PROVENANCE_COLUMNS} FROM market_provenance
         WHERE instrument_id = ?1 AND interval = ?2 ORDER BY id ASC"
    ))?;
    let rows = stmt
        .query_map(params![instrument_id, interval], provenance_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The record that revises `id`, if any — what makes `id` superseded.
pub fn revised_by(conn: &Connection, id: i64) -> AppResult<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM market_provenance WHERE revision_of = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?)
}

/// The revision chain `id` belongs to, oldest first. A superseded record is
/// never rewritten, so this is the full history of one retrieval.
pub fn revision_chain(conn: &Connection, id: i64) -> AppResult<Vec<ProvenanceRow>> {
    let mut root = get_provenance(conn, id)?
        .ok_or_else(|| AppError::Other(format!("provenance record {id} does not exist")))?;
    while let Some(previous) = root.revision_of {
        match get_provenance(conn, previous)? {
            Some(row) => root = row,
            None => break,
        }
    }
    let mut chain = vec![root];
    while let Some(next) = revised_by(conn, chain[chain.len() - 1].id)? {
        match get_provenance(conn, next)? {
            Some(row) => chain.push(row),
            None => break,
        }
    }
    Ok(chain)
}

/// Read the raw bytes back, proving they are the ones that were recorded.
pub fn read_raw(store: &ArtifactStore, row: &ProvenanceRow) -> AppResult<Vec<u8>> {
    store.read(&row.artifact_ref())
}

// ------------------------------------------------------------ quality events

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityEventRow {
    pub id: i64,
    pub created_at: String,
    #[serde(flatten)]
    pub event: QualityEvent,
}

/// Append one piece of evidence. Codes and actions outside the contract are
/// refused (`super::QualityEvent::validate`).
pub fn record_quality_event(conn: &Connection, event: &QualityEvent) -> AppResult<i64> {
    event.validate()?;
    conn.execute(
        "INSERT INTO market_quality_events
            (version, instrument_id, interval, code, severity, action, range_start, range_end,
             bar_count, dataset_id, provenance_id, snapshot_row_id, detail_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            alpha_factor_forge::discovery_core::market_foundation::MARKET_FOUNDATION_VERSION,
            event.instrument_id,
            event.interval,
            event.code,
            event.severity.as_str(),
            event.action,
            event.range_start,
            event.range_end,
            event.bar_count,
            event.dataset_id,
            event.provenance_id,
            event.snapshot_row_id,
            serde_json::to_string(&event.detail)?,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

const EVENT_COLUMNS: &str = "id, instrument_id, interval, code, severity, action, range_start, range_end, bar_count,
    dataset_id, provenance_id, snapshot_row_id, detail_json, created_at";

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<QualityEventRow> {
    let severity: String = row.get(4)?;
    let detail: String = row.get(12)?;
    Ok(QualityEventRow {
        id: row.get(0)?,
        created_at: row.get(13)?,
        event: QualityEvent {
            instrument_id: row.get(1)?,
            interval: row.get(2)?,
            code: row.get(3)?,
            severity: alpha_factor_forge::discovery_core::market_foundation::Severity::parse(&severity)
                .unwrap_or(alpha_factor_forge::discovery_core::market_foundation::Severity::Info),
            action: row.get(5)?,
            range_start: row.get(6)?,
            range_end: row.get(7)?,
            bar_count: row.get(8)?,
            dataset_id: row.get(9)?,
            provenance_id: row.get(10)?,
            snapshot_row_id: row.get(11)?,
            detail: serde_json::from_str(&detail).unwrap_or(Value::Null),
        },
    })
}

/// The evidence recorded for one series, newest first.
pub fn list_quality_events(
    conn: &Connection,
    instrument_id: &str,
    interval: &str,
    limit: usize,
) -> AppResult<Vec<QualityEventRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {EVENT_COLUMNS} FROM market_quality_events
         WHERE instrument_id = ?1 AND interval = ?2 ORDER BY id DESC LIMIT ?3"
    ))?;
    let rows = stmt
        .query_map(params![instrument_id, interval, limit as i64], event_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The evidence recorded against one dataset, oldest first — what blocked a
/// snapshot, and what to do about it.
pub fn dataset_quality_events(conn: &Connection, dataset_id: i64) -> AppResult<Vec<QualityEventRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {EVENT_COLUMNS} FROM market_quality_events WHERE dataset_id = ?1 ORDER BY id ASC"
    ))?;
    let rows = stmt
        .query_map([dataset_id], event_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::db;
    use alpha_factor_forge::discovery_core::market_foundation::Severity;

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
        let dir = std::env::temp_dir().join(format!("aff-market-test-{}-{n}", std::process::id()));
        (ArtifactStore::in_data_dir(&dir), TempDir(dir))
    }

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON").expect("foreign keys");
        db::apply_migrations(&conn).expect("migrations");
        conn
    }

    fn observation() -> RawObservation {
        RawObservation {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            source: "binance-archive".into(),
            role: SeriesRole::Primary,
            request_scope: json!({ "endpoint": "monthly", "month": "2024-07" }),
            retrieved_at: "2024-08-01T00:00:00Z".into(),
            available_at: Some("2024-07-31T23:00:00Z".into()),
            accepted: true,
            rejection_reason: None,
            revision_of: None,
            media_type: "text/csv".into(),
        }
    }

    #[test]
    fn a_retrieval_is_recorded_once_with_its_bytes_kept_and_verifiable() {
        let conn = memory_db();
        let (store, _guard) = fresh_store();
        let bytes = b"1721001600000,60000,61000,59000,60500,10\n";
        let (id, created) = record_raw(&conn, &store, &observation(), bytes).unwrap();
        assert!(created);
        assert_eq!(record_raw(&conn, &store, &observation(), bytes).unwrap(), (id, false));

        let row = get_provenance(&conn, id).unwrap().expect("row");
        assert_eq!(row.raw_byte_len, bytes.len() as i64);
        assert!(row.raw_artifact_path.ends_with(".csv"), "{}", row.raw_artifact_path);
        assert_eq!(read_raw(&store, &row).unwrap(), bytes);
        assert_eq!(row.available_at.as_deref(), Some("2024-07-31T23:00:00Z"));

        // Immutable, and kept.
        assert!(conn
            .execute("UPDATE market_provenance SET source = 'other' WHERE id = ?1", [id])
            .unwrap_err()
            .to_string()
            .contains("immutable"));
        assert!(conn
            .execute("DELETE FROM market_provenance WHERE id = ?1", [id])
            .unwrap_err()
            .to_string()
            .contains("never deleted"));
    }

    #[test]
    fn a_rejected_retrieval_is_kept_so_the_rejection_can_be_replayed() {
        let conn = memory_db();
        let (store, _guard) = fresh_store();
        let rejected = RawObservation {
            accepted: false,
            rejection_reason: Some("checksum mismatch against the published CHECKSUM file".into()),
            ..observation()
        };
        let (id, _) = record_raw(&conn, &store, &rejected, b"corrupt").unwrap();
        let row = get_provenance(&conn, id).unwrap().expect("row");
        assert!(!row.accepted);
        assert_eq!(read_raw(&store, &row).unwrap(), b"corrupt", "the bytes are still there");
        assert_eq!(list_provenance(&conn, &row.instrument_id, "1h").unwrap().len(), 1);

        let inconsistent = RawObservation {
            accepted: false,
            rejection_reason: None,
            ..observation()
        };
        assert!(record_raw(&conn, &store, &inconsistent, b"x")
            .unwrap_err()
            .to_string()
            .contains("states its reason"));
    }

    #[test]
    fn revision_retries_are_idempotent_but_different_children_remain_forks() {
        let conn = memory_db();
        let (store, _guard) = fresh_store();
        let (first, _) = record_raw(&conn, &store, &observation(), b"first").unwrap();
        let revision = RawObservation {
            retrieved_at: "2024-08-05T00:00:00Z".into(),
            revision_of: Some(first),
            ..observation()
        };
        let (second, created) = record_raw(&conn, &store, &revision, b"revised").unwrap();
        assert!(created);
        assert_eq!(record_raw(&conn, &store, &revision, b"revised").unwrap(), (second, false));
        assert_eq!(list_provenance(&conn, &revision.instrument_id, "1h").unwrap().len(), 2);
        for (candidate, bytes) in [
            (revision.clone(), b"different".as_slice()),
            (RawObservation { retrieved_at: "2024-08-06T00:00:00Z".into(), ..revision.clone() },
                b"revised".as_slice()),
        ] {
            let error = record_raw(&conn, &store, &candidate, bytes).unwrap_err().to_string();
            assert!(error.contains("already revised"), "{error}");
        }
        assert_eq!(list_provenance(&conn, &revision.instrument_id, "1h").unwrap().len(), 2);
        let third = RawObservation {
            retrieved_at: "2024-08-07T00:00:00Z".into(),
            revision_of: Some(second),
            ..observation()
        };
        let (third_id, _) = record_raw(&conn, &store, &third, b"third").unwrap();
        // A late retry of an old revision must not change the head or history.
        assert_eq!(record_raw(&conn, &store, &revision, b"revised").unwrap(), (second, false));
        assert_eq!(revision_chain(&conn, first).unwrap().iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![first, second, third_id]);
        assert_eq!(read_raw(&store, &get_provenance(&conn, second).unwrap().unwrap()).unwrap(), b"revised");
    }

    #[test]
    fn a_revision_adds_to_the_chain_and_never_overwrites_or_forks_it() {
        let conn = memory_db();
        let (store, _guard) = fresh_store();
        let (first, _) = record_raw(&conn, &store, &observation(), b"first").unwrap();
        let revision = RawObservation {
            retrieved_at: "2024-08-05T00:00:00Z".into(),
            revision_of: Some(first),
            ..observation()
        };
        let (second, _) = record_raw(&conn, &store, &revision, b"revised").unwrap();

        let chain = revision_chain(&conn, second).unwrap();
        assert_eq!(chain.iter().map(|row| row.id).collect::<Vec<_>>(), vec![first, second]);
        assert_eq!(revision_chain(&conn, first).unwrap().len(), 2, "either end finds the chain");
        assert_eq!(revised_by(&conn, first).unwrap(), Some(second));
        assert_eq!(revised_by(&conn, second).unwrap(), None, "the head is not superseded");
        // The superseded record still reads back byte for byte.
        let original = get_provenance(&conn, first).unwrap().expect("row");
        assert_eq!(read_raw(&store, &original).unwrap(), b"first");

        // A second revision of the same target would fork the history.
        let fork = RawObservation {
            retrieved_at: "2024-08-06T00:00:00Z".into(),
            revision_of: Some(first),
            ..observation()
        };
        assert!(record_raw(&conn, &store, &fork, b"fork")
            .unwrap_err()
            .to_string()
            .contains("already revised"));

        // A revision never crosses sources.
        let crossed = RawObservation {
            source: "coinbase-rest".into(),
            revision_of: Some(second),
            ..observation()
        };
        assert!(record_raw(&conn, &store, &crossed, b"other")
            .unwrap_err()
            .to_string()
            .contains("different series"));
        assert!(record_raw(
            &conn,
            &store,
            &RawObservation { revision_of: Some(9999), ..observation() },
            b"x"
        )
        .unwrap_err()
        .to_string()
        .contains("does not exist"));
    }

    #[test]
    fn an_impossible_retrieval_is_refused_by_rule() {
        let conn = memory_db();
        let (store, _guard) = fresh_store();
        let cases: Vec<(RawObservation, &str)> = vec![
            (
                RawObservation { instrument_id: "binance:BTCUSDT".into(), ..observation() },
                "instrument_id_shape",
            ),
            (RawObservation { interval: "2h".into(), ..observation() }, "not one this contract knows"),
            (RawObservation { source: " ".into(), ..observation() }, "names its source"),
            (
                RawObservation { media_type: "application/x-parquet".into(), ..observation() },
                "not one this contract stores",
            ),
            (
                RawObservation { retrieved_at: "yesterday".into(), ..observation() },
                "not an RFC 3339 instant",
            ),
            (
                RawObservation {
                    available_at: Some("2024-08-02T00:00:00Z".into()),
                    ..observation()
                },
                "cannot be retrieved before it was available",
            ),
        ];
        for (candidate, expected) in cases {
            let error = record_raw(&conn, &store, &candidate, b"x").unwrap_err().to_string();
            assert!(error.contains(expected), "expected {expected:?} in {error:?}");
        }
        assert!(list_provenance(&conn, "crypto:binance:BTCUSDT", "1h").unwrap().is_empty());
    }

    #[test]
    fn quality_events_are_append_only_evidence_with_an_action() {
        let conn = memory_db();
        let (store, _guard) = fresh_store();
        let (provenance_id, _) = record_raw(&conn, &store, &observation(), b"first").unwrap();
        let event = QualityEvent::new(
            "crypto:binance:BTCUSDT",
            "1h",
            "missing_bar",
            Severity::Blocking,
            "refetch_range",
        )
        .with_range(1_721_001_600_000, 1_721_005_200_000, 2)
        .with_detail(json!({ "note": "two bars absent from the monthly archive" }))
        .for_provenance(provenance_id);
        let id = record_quality_event(&conn, &event).unwrap();

        let listed = list_quality_events(&conn, "crypto:binance:BTCUSDT", "1h", 10).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);
        assert_eq!(listed[0].event, event);
        assert_eq!(listed[0].event.bar_count, Some(2));

        assert!(conn
            .execute("UPDATE market_quality_events SET code = 'other' WHERE id = ?1", [id])
            .unwrap_err()
            .to_string()
            .contains("immutable"));
        assert!(conn
            .execute("DELETE FROM market_quality_events WHERE id = ?1", [id])
            .unwrap_err()
            .to_string()
            .contains("never deleted"));
    }
}
