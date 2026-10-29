//! Original snapshot writers checked against independent frozen literals.
//! All IO is disposable; no source/candle authentication or admission follows.

use super::super::provenance::RawObservation;
use super::*;
use crate::db;
use crate::research::artifacts::ArtifactStore;
use rusqlite::types::ValueRef;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const FIXTURE: &str =
    include_str!("../../../fixtures/research/original-market-source-projections-v1.json");
const REGISTRY: &str =
    include_str!("../../../fixtures/research/original-market-registry-projections-v1.json");
const DECLARATION: &str =
    "#[cfg(test)]\n#[path = \"provenance_projection_controls.rs\"]\nmod projection_controls;\n\n";
const SNAPSHOT_DECLARATION: &str =
    "#[cfg(test)]\n#[path = \"snapshot_projection_controls.rs\"]\nmod projection_controls;\n\n";

fn fixture() -> Value {
    assert_eq!(
        sha256_hex(FIXTURE.replace("\r\n", "\n").as_bytes()),
        "7e762b0c2493fc4a78b6608cb04a78af6ef668c4b28b9e83341056e1927bc0ef"
    );
    let v: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(v["version"], "original-market-source-projections-v1");
    assert_eq!(v["snapshots"].as_array().unwrap().len(), 8);
    assert_eq!(v["datasets"].as_array().unwrap().len(), 2);
    assert_eq!(v["snapshotRefusals"].as_array().unwrap().len(), 11);
    v
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Cell {
    Null,
    Integer(i64),
    Real(u64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

type Rows = Vec<BTreeMap<String, Cell>>;
type Database = BTreeMap<String, Rows>;

fn cell(value: ValueRef<'_>) -> Cell {
    match value {
        ValueRef::Null => Cell::Null,
        ValueRef::Integer(n) => Cell::Integer(n),
        ValueRef::Real(n) => Cell::Real(n.to_bits()),
        ValueRef::Text(bytes) => Cell::Text(bytes.to_vec()),
        ValueRef::Blob(bytes) => Cell::Blob(bytes.to_vec()),
    }
}

fn rows(conn: &Connection, table: &str) -> Rows {
    assert!(TABLES.contains(&table));
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
        .unwrap();
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    stmt.query_map([], |row| {
        Ok(names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), cell(row.get_ref(i).unwrap())))
            .collect())
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

const TABLES: [&str; 9] = [
    "datasets",
    "candles",
    "market_calendars",
    "market_instruments",
    "market_provenance",
    "market_snapshots",
    "market_snapshot_sources",
    "market_quality_events",
    "sqlite_sequence",
];

fn database(conn: &Connection) -> Database {
    TABLES
        .iter()
        .map(|name| (name.to_string(), rows(conn, name)))
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct Inventory {
    directories: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, Vec<u8>>,
}

struct OwnedRoot {
    path: PathBuf,
    parent: PathBuf,
}

impl OwnedRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir().canonicalize().unwrap();
        for _ in 0..1024 {
            let path = parent.join(format!(
                "aff-original-snapshot-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path, parent },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("exclusive snapshot root: {error}"),
            }
        }
        panic!("bounded unused snapshot root allocation failed");
    }

    fn inventory(&self) -> Inventory {
        assert!(self.path.is_absolute());
        assert_eq!(self.path.parent(), Some(self.parent.as_path()));
        let prefix = format!("aff-original-snapshot-{}-", std::process::id());
        assert!(self
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix(&prefix)
            .is_some_and(|s| s.parse::<u64>().is_ok()));
        assert_eq!(self.path.canonicalize().unwrap(), self.path);
        let mut result = Inventory {
            directories: BTreeSet::new(),
            files: BTreeMap::new(),
        };
        let mut pending = vec![self.path.clone()];
        while let Some(path) = pending.pop() {
            assert!(path.starts_with(&self.path));
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                assert_eq!(
                    metadata.file_attributes() & 0x400,
                    0,
                    "reparse point refused"
                );
            }
            let relative = path.strip_prefix(&self.path).unwrap().to_path_buf();
            if metadata.is_dir() {
                result.directories.insert(relative);
                for entry in fs::read_dir(&path).unwrap() {
                    pending.push(entry.unwrap().path());
                }
            } else {
                assert!(metadata.is_file());
                result.files.insert(relative, fs::read(&path).unwrap());
            }
        }
        result
    }
}

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        // Validate the exact owned root and every entry before recursive removal.
        let _ = self.inventory();
        fs::remove_dir_all(&self.path).unwrap();
        assert!(!self.path.exists());
    }
}

struct Setup {
    conn: Option<Connection>,
    store: ArtifactStore,
    owned: OwnedRoot,
}

impl Setup {
    fn new() -> Self {
        let owned = OwnedRoot::new();
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        db::apply_migrations(&conn).unwrap();
        let store = ArtifactStore::in_data_dir(&owned.path);
        Self {
            conn: Some(conn),
            store,
            owned,
        }
    }

    fn conn(&self) -> &Connection {
        self.conn.as_ref().unwrap()
    }

    fn conn_mut(&mut self) -> &mut Connection {
        self.conn.as_mut().unwrap()
    }

    fn state(&self) -> (Database, Inventory) {
        (database(self.conn()), self.owned.inventory())
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        drop(self.conn.take());
    }
}

#[test]
fn frozen_snapshot_source_and_registry_pins() {
    let v = fixture();
    assert_eq!(v["baseline"], "fd1660fc49e00b6935bec9a9e5168df628980295");
    assert_eq!(
        sha256_hex(REGISTRY.replace("\r\n", "\n").as_bytes()),
        v["registryFixture"]["lfSha256"]
    );
    let original = include_str!("provenance.rs").replace("\r\n", "\n");
    assert_eq!(original.matches(DECLARATION).count(), 1);
    let snapshot = include_str!("snapshot.rs").replace("\r\n", "\n");
    // Only the separately planned, fixed cfg(test) annotation may be added later.
    assert_eq!(snapshot.matches(SNAPSHOT_DECLARATION).count(), 1);
    let sources = [
        (
            "src/market/provenance.rs",
            original.replacen(DECLARATION, "", 1),
        ),
        (
            "src/market/snapshot.rs",
            snapshot.replacen(SNAPSHOT_DECLARATION, "", 1),
        ),
        (
            "src/research/artifacts.rs",
            include_str!("../research/artifacts.rs").replace("\r\n", "\n"),
        ),
        (
            "src/identity.rs",
            include_str!("../identity.rs").replace("\r\n", "\n"),
        ),
        (
            "src/discovery_core/market_foundation.rs",
            include_str!("../discovery_core/market_foundation.rs").replace("\r\n", "\n"),
        ),
        (
            "src/research/mod.rs",
            include_str!("../research/mod.rs").replace("\r\n", "\n"),
        ),
        (
            "src/market/registry.rs",
            include_str!("registry.rs").replace("\r\n", "\n"),
        ),
        (
            "src/market/sources/binance.rs",
            include_str!("sources/binance.rs").replace("\r\n", "\n"),
        ),
    ];
    assert_eq!(v["originalSourcePins"].as_object().unwrap().len(), 8);
    for (path, text) in sources {
        let key = format!("alpha-factor-forge/src-tauri/{path}");
        assert_eq!(sha256_hex(text.as_bytes()), v["originalSourcePins"][key]);
    }
}

fn named<'a>(v: &'a Value, collection: &str, name: &str) -> &'a Value {
    v[collection]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == name)
        .unwrap()
}

fn scalar(v: &Value) -> Cell {
    match v {
        Value::Null => Cell::Null,
        Value::String(s) => Cell::Text(s.as_bytes().to_vec()),
        Value::Number(n) => Cell::Integer(n.as_i64().unwrap()),
        _ => panic!("unexpected frozen scalar"),
    }
}

fn register_instrument(setup: &Setup, name: &str, id: i64) {
    let r: Value = serde_json::from_str(REGISTRY).unwrap();
    let c = named(&r, "instruments", name);
    let spec = serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
    let (row, created) = registry::register_instrument(setup.conn(), &spec).unwrap();
    assert!(created);
    assert_eq!(row, id);
    assert_eq!(
        registry::get_instrument(setup.conn(), id)
            .unwrap()
            .unwrap()
            .content_hash,
        c["expected"]["contentHash"]
    );
}

fn prepare(v: &Value) -> Setup {
    let mut setup = Setup::new();
    let r: Value = serde_json::from_str(REGISTRY).unwrap();
    for name in ["calendar-0", "calendar-1"] {
        let c = named(&r, "calendars", name);
        let calendar = serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
        let (id, created) = registry::register_calendar(setup.conn(), &calendar).unwrap();
        assert!(created);
        let hash: String = setup
            .conn()
            .query_row(
                "SELECT content_hash FROM market_calendars WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hash, c["expected"]["contentHash"]);
    }
    register_instrument(&setup, "instrument-0", 1);
    for c in v["datasets"].as_array().unwrap() {
        let m = &c["metadata"];
        let dataset = repositories::Dataset {
            id: None,
            exchange: m["exchange"].as_str().unwrap().into(),
            symbol: m["symbol"].as_str().unwrap().into(),
            interval: m["interval"].as_str().unwrap().into(),
            start_time: m["startTime"].as_i64().unwrap(),
            end_time: m["endTime"].as_i64().unwrap(),
            candle_count: m["candleCount"].as_i64().unwrap(),
            source: m["source"].as_str().unwrap().into(),
            dataset_hash: m["datasetHash"].as_str().unwrap().into(),
        };
        let candles: Vec<repositories::Candle> =
            serde_json::from_value(c["candles"].clone()).unwrap();
        let input = serde_json::to_value((&dataset, &candles)).unwrap();
        let id = repositories::import_dataset_with_candles(setup.conn_mut(), &dataset, &candles)
            .unwrap();
        assert_eq!(id, c["rowId"]);
        let stored = repositories::get_dataset_by_id(setup.conn(), id).unwrap();
        assert_eq!(stored.dataset_hash, m["datasetHash"]);
        assert_eq!(stored.candle_count, m["candleCount"]);
        let stored_candles =
            repositories::get_candles(setup.conn(), id, i64::MIN, i64::MAX).unwrap();
        assert_eq!(stored_candles.len(), candles.len());
        for (actual, expected) in stored_candles.iter().zip(&candles) {
            assert_eq!(actual.timestamp, expected.timestamp);
            for (a, e) in [
                (actual.open, expected.open),
                (actual.high, expected.high),
                (actual.low, expected.low),
                (actual.close, expected.close),
                (actual.volume, expected.volume),
            ] {
                assert_eq!(a.to_bits(), e.to_bits());
            }
        }
        let before = setup.state();
        assert_eq!(
            repositories::import_dataset_with_candles(setup.conn_mut(), &dataset, &candles)
                .unwrap(),
            id
        );
        assert_eq!(setup.state(), before);
        assert_eq!(serde_json::to_value((&dataset, &candles)).unwrap(), input);
    }
    for step in v["provenanceSequence"].as_array().unwrap() {
        let c = named(v, "provenance", step["input"].as_str().unwrap());
        let observation: RawObservation =
            serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
        let raw = hex::decode(c["rawHex"].as_str().unwrap()).unwrap();
        let saved = (observation.clone(), raw.clone());
        assert_eq!(
            provenance::record_raw(setup.conn(), &setup.store, &observation, &raw).unwrap(),
            (
                step["expectedRow"].as_i64().unwrap(),
                step["created"].as_bool().unwrap()
            )
        );
        assert_eq!((observation, raw), saved);
    }
    assert_eq!(rows(setup.conn(), "market_provenance").len(), 9);
    assert_eq!(setup.owned.inventory().files.len(), 9);
    setup
}

// A complete state comparison, including sqlite_sequence, not just row counts.
fn transition(
    before: &(Database, Inventory),
    after: &(Database, Inventory),
    snapshots: usize,
    sources: usize,
    events: usize,
) {
    assert_eq!(
        after.1, before.1,
        "snapshot operations never alter raw artifacts"
    );
    for table in TABLES {
        let old = &before.0[table];
        let new = &after.0[table];
        let delta = match table {
            "market_snapshots" => snapshots,
            "market_snapshot_sources" => sources,
            "market_quality_events" => events,
            "sqlite_sequence" => continue,
            _ => 0,
        };
        assert_eq!(new.len(), old.len() + delta, "complete {table} membership");
        assert_eq!(&new[..old.len()], old, "complete existing {table} rows");
    }
    let mut expected = before.0["sqlite_sequence"].clone();
    for (table, delta) in [
        ("market_snapshots", snapshots),
        ("market_quality_events", events),
    ] {
        if delta == 0 {
            continue;
        }
        let name = Cell::Text(table.as_bytes().to_vec());
        let seq = Cell::Integer(after.0[table].len() as i64);
        if let Some(row) = expected.iter_mut().find(|row| row["name"] == name) {
            row.insert("seq".into(), seq);
        } else {
            expected.push(BTreeMap::from([("name".into(), name), ("seq".into(), seq)]));
        }
    }
    assert_eq!(
        after.0["sqlite_sequence"], expected,
        "all original sequence state"
    );
}

fn projection(setup: &Setup, c: &Value, actual: &SnapshotRow) {
    let e = &c["expected"];
    let id = e["rowId"].as_i64().unwrap();
    let all = rows(setup.conn(), "market_snapshots");
    let row = &all[(id - 1) as usize];
    assert_eq!(row.len(), e["scalars"].as_object().unwrap().len() + 3);
    assert_eq!(row["id"], Cell::Integer(id));
    for (key, value) in e["scalars"].as_object().unwrap() {
        assert_eq!(row[key], scalar(value), "{} original SQL {key}", c["id"]);
    }
    assert_eq!(row["content_json"], scalar(&e["contentJson"]));
    assert!(matches!(&row["created_at"], Cell::Text(s) if !s.is_empty()));
    assert_eq!(
        sha256_hex(e["contentJson"].as_str().unwrap().as_bytes()),
        e["snapshotId"]
    );
    assert_eq!(actual.id, id);
    assert_eq!(actual.snapshot_id, e["snapshotId"]);
    assert_eq!(actual.instrument_row_id, e["scalars"]["instrument_row_id"]);
    assert_eq!(actual.as_of, e["scalars"]["as_of"]);
    assert_eq!(actual.coverage, e["coverage"]);
    assert_eq!(
        actual.qualification_eligible(),
        e["qualificationEligible"].as_bool().unwrap()
    );
    assert_eq!(
        Cell::Text(actual.created_at.as_bytes().to_vec()),
        row["created_at"]
    );
    assert_eq!(
        get_snapshot(setup.conn(), id).unwrap().as_ref(),
        Some(actual)
    );
    assert_eq!(
        get_snapshot_by_id(setup.conn(), &actual.snapshot_id)
            .unwrap()
            .as_ref(),
        Some(actual)
    );
    let expected: Vec<i64> = serde_json::from_value(e["sourceRows"].clone()).unwrap();
    let source_rows: Vec<_> = rows(setup.conn(), "market_snapshot_sources")
        .into_iter()
        .filter(|row| row["snapshot_row_id"] == Cell::Integer(id))
        .collect();
    assert_eq!(source_rows.len(), expected.len());
    let wanted: BTreeSet<_> = expected.iter().copied().collect();
    let observed: BTreeSet<_> = source_rows
        .iter()
        .map(|row| {
            assert_eq!(row.len(), 2);
            match row["provenance_id"] {
                Cell::Integer(n) => n,
                _ => panic!("source id type"),
            }
        })
        .collect();
    assert_eq!(observed, wanted);
    let components = snapshot_sources(setup.conn(), id).unwrap();
    assert_eq!(
        components.iter().map(|row| row.id).collect::<Vec<_>>(),
        expected
    );
    let mut hashes: Vec<_> = components
        .iter()
        .map(|row| row.record_hash.clone())
        .collect();
    hashes.sort();
    let content: Value = serde_json::from_str(e["contentJson"].as_str().unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(hashes).unwrap(),
        content["sourceRecordHashes"]
    );
    let captured = registry::get_instrument(setup.conn(), actual.instrument_row_id)
        .unwrap()
        .unwrap();
    assert_eq!(captured.content_hash, content["instrumentContentHash"]);
}

// Independent, source-grounded event literals. Do not call the producer's event helpers.
fn expected_event(name: &str, request: &SnapshotRequest, snapshot: Option<i64>) -> Value {
    let (code, severity, action, provenance, detail) = match name {
        "cost-unconfirmed" => (
            "cost_profile_unconfirmed",
            "degraded",
            "confirm_costs",
            None,
            json!({"reason":"the user has not confirmed commission, slippage, and transaction tax"}),
        ),
        "etf-actions-unverified" => (
            "corporate_actions_unverified",
            "degraded",
            "verify_corporate_actions",
            None,
            json!({"reason":"dividend and split completeness is not verified for this range"}),
        ),
        "no-components" => (
            "missing_source",
            "blocking",
            "refetch_range",
            None,
            json!({"reason":"a snapshot requires at least one primary provenance record"}),
        ),
        "comparison-component" => (
            "source_conflict",
            "blocking",
            "separate_sources",
            Some(3),
            json!({"provenanceId":3,"conflicts":["comparison_role_not_combinable"]}),
        ),
        "rejected-component" | "late-rejected-component" => (
            "source_conflict",
            "blocking",
            "separate_sources",
            Some(4),
            json!({"provenanceId":4,"reason":"a rejected retrieval can never be a component","rejectionReason":"fixture rejection"}),
        ),
        "superseded-component" => (
            "superseded_source",
            "blocking",
            "rebuild_from_revision",
            Some(1),
            json!({"provenanceId":1,"revisedBy":5}),
        ),
        "other-publisher" => (
            "source_conflict",
            "blocking",
            "separate_sources",
            None,
            json!({"provenanceIds":[7,9],"conflicts":["source_mismatch"]}),
        ),
        "unknown-forward-availability" => (
            "availability_unknown",
            "blocking",
            "record_availability",
            Some(6),
            json!({"provenanceId":6,"availableAt":"2024-07-15T03:00:00Z","asOfMs":1721023200000_i64,
                "reason":"forward-observed evidence must have known availability at or before the cut"}),
        ),
        "late-forward-availability" => (
            "availability_after_cut",
            "blocking",
            "record_availability",
            Some(7),
            json!({"provenanceId":7,"availableAt":"2024-07-15T06:00:00Z","asOfMs":1721019600000_i64,
                "reason":"forward-observed evidence must have known availability at or before the cut"}),
        ),
        "unclosed-candle" => (
            "unclosed_bar",
            "blocking",
            "wait_for_close",
            None,
            json!({"bars":1}),
        ),
        _ => panic!("missing independent event literal for {name}"),
    };
    let range = (name == "unclosed-candle").then_some(1721012400000_i64);
    json!({"instrumentId":request.instrument_id,"interval":request.interval,"code":code,"severity":severity,
        "action":action,"rangeStart":range,"rangeEnd":range,"barCount":range.map(|_|1),
        "datasetId":request.dataset_id,"provenanceId":provenance,"snapshotRowId":snapshot,"detail":detail})
}

fn event_projection(setup: &Setup, id: i64, expected: &Value, actual: Option<&QualityEventRow>) {
    let all = rows(setup.conn(), "market_quality_events");
    let row = &all[(id - 1) as usize];
    assert_eq!(row.len(), 15);
    assert_eq!(row["id"], Cell::Integer(id));
    assert_eq!(row["version"], scalar(&json!("market-foundation-v1")));
    for (column, key) in [
        ("instrument_id", "instrumentId"),
        ("interval", "interval"),
        ("code", "code"),
        ("severity", "severity"),
        ("action", "action"),
        ("range_start", "rangeStart"),
        ("range_end", "rangeEnd"),
        ("bar_count", "barCount"),
        ("dataset_id", "datasetId"),
        ("provenance_id", "provenanceId"),
        ("snapshot_row_id", "snapshotRowId"),
    ] {
        assert_eq!(
            row[column],
            scalar(&expected[key]),
            "original event {column}"
        );
    }
    assert_eq!(
        row["detail_json"],
        scalar(&json!(serde_json::to_string(&expected["detail"]).unwrap()))
    );
    assert!(matches!(&row["created_at"], Cell::Text(s) if !s.is_empty()));
    let dataset = expected["datasetId"].as_i64().unwrap();
    let stored = provenance::dataset_quality_events(setup.conn(), dataset)
        .unwrap()
        .into_iter()
        .find(|r| r.id == id)
        .unwrap();
    assert_eq!(serde_json::to_value(&stored.event).unwrap(), *expected);
    assert_eq!(
        Cell::Text(stored.created_at.as_bytes().to_vec()),
        row["created_at"]
    );
    if let Some(actual) = actual {
        assert_eq!(actual, &stored);
    }
}

fn positive_calls(setup: &mut Setup, v: &Value) {
    for (i, c) in v["snapshots"].as_array().unwrap().iter().enumerate() {
        if i == 2 {
            register_instrument(setup, "instrument-1", 2);
        }
        if i == 6 {
            register_instrument(setup, "instrument-2", 3);
        }
        let request: SnapshotRequest =
            serde_json::from_str(c["requestJson"].as_str().unwrap()).unwrap();
        let saved = request.clone();
        let original = c["requestJson"].as_str().unwrap().as_bytes().to_vec();
        let before = setup.state();
        let outcome = build_snapshot(setup.conn_mut(), &request).unwrap();
        let created = c["expected"]["outcome"] == "created";
        let actual = match outcome {
            SnapshotOutcome::Created(row) if created => row,
            SnapshotOutcome::Existing(row) if !created => row,
            other => panic!("{} expected outcome, got {other:?}", c["id"]),
        };
        projection(setup, c, &actual);
        let events = c["expected"]["eventCodes"].as_array().unwrap();
        transition(
            &before,
            &setup.state(),
            usize::from(created),
            if created {
                c["expected"]["sourceRows"].as_array().unwrap().len()
            } else {
                0
            },
            events.len(),
        );
        assert!(events.len() <= 1);
        if !events.is_empty() {
            let expected = expected_event(c["id"].as_str().unwrap(), &request, Some(actual.id));
            assert_eq!(expected["code"], events[0]);
            event_projection(
                setup,
                (before.0["market_quality_events"].len() + 1) as i64,
                &expected,
                None,
            );
        }
        assert_eq!(request, saved);
        assert_eq!(c["requestJson"].as_str().unwrap().as_bytes(), original);
        if !created {
            assert_eq!(
                setup.state(),
                before,
                "Existing preserves admitting cut/coverage and all state"
            );
        }
    }
    assert_eq!(rows(setup.conn(), "market_snapshots").len(), 7);
    assert_eq!(rows(setup.conn(), "market_snapshot_sources").len(), 12);
    assert_eq!(rows(setup.conn(), "market_quality_events").len(), 2);
    let before = setup.state();
    for c in v["snapshots"].as_array().unwrap() {
        let row = get_snapshot(setup.conn(), c["expected"]["rowId"].as_i64().unwrap())
            .unwrap()
            .unwrap();
        projection(setup, c, &row);
    }
    assert_eq!(
        setup.state(),
        before,
        "captured original revisions/read-only lookup"
    );
}

#[test]
fn original_snapshot_calls_preserve_all_frozen_projections_and_revisions() {
    let v = fixture();
    let mut setup = prepare(&v);
    positive_calls(&mut setup, &v);
}

fn counts(setup: &Setup, expected: &Value) {
    for (table, key) in [
        ("market_snapshots", "snapshots"),
        ("market_snapshot_sources", "sources"),
        ("market_quality_events", "qualityEvents"),
    ] {
        assert_eq!(
            rows(setup.conn(), table).len() as u64,
            expected[key].as_u64().unwrap(),
            "frozen {key}"
        );
    }
}

#[test]
fn original_snapshot_refusals_preserve_complete_evidence_and_inputs() {
    let v = fixture();
    let mut setup = prepare(&v);
    positive_calls(&mut setup, &v);
    for c in v["snapshotRefusals"].as_array().unwrap() {
        let request: SnapshotRequest =
            serde_json::from_str(c["requestJson"].as_str().unwrap()).unwrap();
        let saved = request.clone();
        let original = c["requestJson"].as_str().unwrap().as_bytes().to_vec();
        counts(&setup, &c["beforeCounts"]);
        let before = setup.state();
        let outcome = build_snapshot(setup.conn_mut(), &request);
        if c["phase"] == "blocked" {
            let events = match outcome {
                Ok(SnapshotOutcome::Blocked { events }) => events,
                other => panic!("{} expected blocked, got {other:?}", c["id"]),
            };
            assert_eq!(
                serde_json::to_value(events.iter().map(|r| &r.event.code).collect::<Vec<_>>())
                    .unwrap(),
                c["eventCodes"]
            );
            assert_eq!(events.len(), 1);
            let expected = expected_event(c["id"].as_str().unwrap(), &request, None);
            event_projection(
                &setup,
                (before.0["market_quality_events"].len() + 1) as i64,
                &expected,
                Some(&events[0]),
            );
            transition(&before, &setup.state(), 0, 0, 1);
        } else {
            let error = outcome.expect_err("frozen error phase").to_string();
            assert!(
                error.contains(c["errorContains"].as_str().unwrap()),
                "{}: {error}",
                c["id"]
            );
            assert_eq!(
                setup.state(),
                before,
                "error is atomic, including sqlite_sequence"
            );
        }
        counts(&setup, &c["afterCounts"]);
        assert_eq!(request, saved);
        assert_eq!(c["requestJson"].as_str().unwrap().as_bytes(), original);
    }
}

#[test]
fn original_snapshot_and_relation_immutability_preserves_all_state() {
    let v = fixture();
    let mut setup = prepare(&v);
    positive_calls(&mut setup, &v);
    for sql in ["UPDATE market_snapshots SET as_of = as_of + 1 WHERE id = 1",
        "DELETE FROM market_snapshots WHERE id = 1",
        "UPDATE market_snapshot_sources SET provenance_id = 9 WHERE snapshot_row_id = 1 AND provenance_id = 2",
        "DELETE FROM market_snapshot_sources WHERE snapshot_row_id = 1 AND provenance_id = 2"] {
        let before = setup.state();
        let error = setup.conn().execute(sql, []).unwrap_err().to_string();
        assert!(error.contains("immutable") || error.contains("never deleted"), "{error}");
        assert_eq!(setup.state(), before, "refused {sql}");
    }
}

#[test]
fn original_snapshot_late_rejected_family_cannot_be_filtered_into_success() {
    let v = fixture();
    let mut setup = prepare(&v);
    positive_calls(&mut setup, &v);
    let c = named(&v, "snapshotRefusals", "late-rejected-component");
    let request: SnapshotRequest =
        serde_json::from_str(c["requestJson"].as_str().unwrap()).unwrap();
    assert_eq!(request.provenance_ids, vec![7, 4]);
    let saved = request.clone();
    let before = setup.state();
    let events = match build_snapshot(setup.conn_mut(), &request).unwrap() {
        SnapshotOutcome::Blocked { events } => events,
        other => panic!("late invalid member must block the complete family: {other:?}"),
    };
    assert_eq!(events.len(), 1);
    event_projection(
        &setup,
        3,
        &expected_event("late-rejected-component", &request, None),
        Some(&events[0]),
    );
    transition(&before, &setup.state(), 0, 0, 1);
    assert_eq!(request, saved);
}

#[test]
fn original_snapshot_verifies_current_candles_before_existing_return() {
    let v = fixture();
    let mut setup = prepare(&v);
    positive_calls(&mut setup, &v);
    // Existing forward snapshot bound to the latest BTC revision would be returned
    // if the stored payload were trusted merely because its metadata still matched.
    let request: SnapshotRequest =
        serde_json::from_str(v["snapshots"][3]["requestJson"].as_str().unwrap()).unwrap();
    let saved = request.clone();
    let original = setup.state();
    assert!(matches!(
        build_snapshot(setup.conn_mut(), &request).unwrap(),
        SnapshotOutcome::Existing(_)
    ));
    assert_eq!(setup.state(), original);
    assert_eq!(setup.conn().execute("UPDATE candles SET close = 100.25 WHERE dataset_id = 1 AND timestamp = 1721001600000", []).unwrap(), 1);
    let tampered = setup.state();
    for table in TABLES {
        if table != "candles" {
            assert_eq!(tampered.0[table], original.0[table]);
        }
    }
    assert_eq!(tampered.1, original.1);
    assert_ne!(tampered.0["candles"], original.0["candles"]);
    let error = build_snapshot(setup.conn_mut(), &request)
        .unwrap_err()
        .to_string();
    assert!(error.contains("dataset identity mismatch"), "{error}");
    assert_eq!(
        setup.state(),
        tampered,
        "tamper refusal before any writer/Existing"
    );
    assert_eq!(request, saved);
    assert_eq!(setup.conn().execute("UPDATE candles SET close = 100.5 WHERE dataset_id = 1 AND timestamp = 1721001600000", []).unwrap(), 1);
    assert_eq!(setup.state(), original);
    assert!(matches!(
        build_snapshot(setup.conn_mut(), &request).unwrap(),
        SnapshotOutcome::Existing(_)
    ));
    assert_eq!(setup.state(), original);
}
