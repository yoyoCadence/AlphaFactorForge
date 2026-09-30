use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use rusqlite::Connection;
use serde_json::{json, Value};

use super::*;
use crate::{
    db,
    market::{
        provenance::RawObservation,
        registry,
        snapshot::{SnapshotKind, SnapshotOutcome, SnapshotRequest},
    },
    research::artifacts::ArtifactStore,
};
use alpha_factor_forge::discovery_core::{
    campaign::freeze_campaign,
    market_foundation::{PriceBasis, SeriesRole},
};

const HOUR: i64 = 3_600_000;
const T0: i64 = 1_721_001_600_000;

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn store() -> (ArtifactStore, TempDir) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "aff-campaign-snapshot-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    (ArtifactStore::in_data_dir(&path), TempDir(path))
}

fn source(revision_of: Option<i64>) -> RawObservation {
    RawObservation {
        instrument_id: "crypto:binance:BTCUSDT".into(),
        interval: "1h".into(),
        source: "binance-archive".into(),
        role: SeriesRole::Primary,
        request_scope: json!({"month":"2024-07"}),
        retrieved_at: "2024-08-01T00:00:00Z".into(),
        available_at: None,
        accepted: true,
        rejection_reason: None,
        revision_of,
        media_type: "text/csv".into(),
    }
}

fn setup() -> (Connection, ArtifactStore, TempDir, Value, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    db::apply_migrations(&conn).unwrap();
    registry::ensure_builtin_calendars(&conn).unwrap();
    let mut instrument = registry::default_crypto_instruments().remove(0);
    instrument.listed_from = Some(T0);
    registry::register_instrument(&conn, &instrument).unwrap();

    let candles: Vec<_> = (0..4)
        .map(|index| repositories::Candle {
            timestamp: T0 + index * HOUR,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 10.0,
        })
        .collect();
    let mut dataset = repositories::Dataset {
        id: None,
        exchange: "binance".into(),
        symbol: "BTCUSDT".into(),
        interval: "1h".into(),
        start_time: T0,
        end_time: T0 + 3 * HOUR,
        candle_count: 4,
        source: "import".into(),
        dataset_hash: String::new(),
    };
    dataset.dataset_hash = identity::dataset_content_hash(&dataset, &candles).unwrap();
    let dataset_id =
        repositories::import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap();
    let (artifact_store, temp) = store();
    let source_id = provenance::record_raw(&conn, &artifact_store, &source(None), b"raw")
        .unwrap()
        .0;
    let request = SnapshotRequest {
        instrument_id: instrument.instrument_id,
        interval: "1h".into(),
        dataset_id,
        price_basis: PriceBasis::Raw,
        corporate_action_version: None,
        cost_profile_version: Some("cost-profile-v1".into()),
        kind: SnapshotKind::Historical,
        provenance_ids: vec![source_id],
        as_of_ms: T0 + 4 * HOUR,
    };
    let SnapshotOutcome::Created(snapshot) = snapshot::build_snapshot(&mut conn, &request).unwrap()
    else {
        panic!("snapshot should be created")
    };
    let mut declaration: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-campaign-declaration-v1.json"
    ))
    .unwrap();
    let binding = &mut declaration["instruments"][0];
    binding["listedAtMs"] = json!(T0);
    binding["fromMs"] = json!(T0);
    binding["toMs"] = json!(T0 + 3 * HOUR);
    binding["datasetHash"] = json!(dataset.dataset_hash);
    binding["snapshotId"] = json!(snapshot.snapshot_id);
    binding["samplePolicy"]["minimumTotalBars"] = json!(4);
    (conn, artifact_store, temp, declaration, source_id)
}

fn reason(conn: &Connection, declaration: &Value) -> String {
    let frozen = freeze_campaign(declaration).unwrap();
    resolve_campaign_snapshots(conn, &frozen)
        .unwrap_err()
        .to_string()
}

#[test]
fn exact_authoritative_snapshot_resolves_without_writing_or_admission_status() {
    let (conn, _store, _temp, declaration, _) = setup();
    let before: i64 = conn
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .unwrap();
    let frozen = freeze_campaign(&declaration).unwrap();
    let rows = resolve_campaign_snapshots(&conn, &frozen).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].snapshot_id, declaration["instruments"][0]["snapshotId"]);
    assert_eq!(rows[0].bar_count, 4);
    assert_eq!(rows[0].cost_profile_version, "cost-profile-v1");
    let after: i64 = conn
        .query_row("SELECT total_changes()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(after, before);
}

#[test]
fn missing_or_mismatched_binding_never_falls_back_to_latest_snapshot() {
    let (conn, _store, _temp, mut declaration, _) = setup();
    for (pointer, value, expected) in [
        (
            "/instruments/0/snapshotId",
            json!("a".repeat(64)),
            "missing",
        ),
        (
            "/instruments/0/datasetHash",
            json!(format!("dataset-content-v2:{}", "b".repeat(64))),
            "hash",
        ),
        ("/instruments/0/interval", json!("4h"), "interval"),
        ("/instruments/0/listedAtMs", json!(T0 - 1), "listing"),
        ("/instruments/0/fromMs", json!(T0 + HOUR), "range"),
        ("/instruments/0/toMs", json!(T0 + 2 * HOUR), "range"),
        (
            "/instruments/0/samplePolicy/minimumTotalBars",
            json!(5),
            "too few",
        ),
    ] {
        let original = declaration.pointer(pointer).unwrap().clone();
        *declaration.pointer_mut(pointer).unwrap() = value;
        assert!(reason(&conn, &declaration).contains(expected), "{pointer}");
        *declaration.pointer_mut(pointer).unwrap() = original;
    }
}

#[test]
fn changed_candles_and_superseded_source_fail_closed() {
    let (conn, artifact_store, _temp, declaration, source_id) = setup();
    conn.execute("UPDATE candles SET close = 110 WHERE timestamp = ?1", [T0])
        .unwrap();
    assert!(reason(&conn, &declaration).contains("identity mismatch"));
    conn.execute(
        "UPDATE candles SET close = 100.5 WHERE timestamp = ?1",
        [T0],
    )
    .unwrap();
    assert!(resolve_campaign_snapshots(&conn, &freeze_campaign(&declaration).unwrap()).is_ok());
    provenance::record_raw(&conn, &artifact_store, &source(Some(source_id)), b"revised").unwrap();
    assert!(reason(&conn, &declaration).contains("superseded"));
}

#[test]
fn demo_and_degraded_snapshots_cannot_resolve() {
    let (mut conn, _store, _temp, mut declaration, source_id) = setup();
    let dataset_id = repositories::get_dataset_by_id(&conn, 1)
        .unwrap()
        .id
        .unwrap();
    for (kind, cost) in [
        (SnapshotKind::Demo, Some("cost-profile-v1")),
        (SnapshotKind::Historical, None),
    ] {
        let request = SnapshotRequest {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            dataset_id,
            price_basis: PriceBasis::Raw,
            corporate_action_version: None,
            cost_profile_version: cost.map(str::to_string),
            kind,
            provenance_ids: vec![source_id],
            as_of_ms: T0 + 4 * HOUR,
        };
        let SnapshotOutcome::Created(snapshot) =
            snapshot::build_snapshot(&mut conn, &request).unwrap()
        else {
            panic!("snapshot should be created")
        };
        declaration["instruments"][0]["snapshotId"] = json!(snapshot.snapshot_id);
        assert!(reason(&conn, &declaration).contains("demo or degraded"));
    }
}
