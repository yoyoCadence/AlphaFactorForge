//! Original retrieval writers checked against independent frozen literals.
//! All IO is disposable; no source/candle authentication or admission follows.

use super::*;
use crate::db;
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
    assert_eq!(v["provenance"].as_array().unwrap().len(), 9);
    assert_eq!(v["provenanceSequence"].as_array().unwrap().len(), 12);
    assert_eq!(v["provenanceRefusals"].as_array().unwrap().len(), 13);
    v
}

#[derive(Debug, Eq, PartialEq)]
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
                "aff-original-provenance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path, parent },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("exclusive provenance root: {error}"),
            }
        }
        panic!("bounded unused provenance root allocation failed");
    }

    fn inventory(&self) -> Inventory {
        assert!(self.path.is_absolute());
        assert_eq!(self.path.parent(), Some(self.parent.as_path()));
        let prefix = format!("aff-original-provenance-{}-", std::process::id());
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

    fn state(&self) -> (Database, Inventory) {
        (database(self.conn()), self.owned.inventory())
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        drop(self.conn.take());
    }
}

fn named<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["provenance"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == name)
        .unwrap()
}

fn input(c: &Value) -> (RawObservation, Vec<u8>) {
    (
        serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap(),
        hex::decode(c["rawHex"].as_str().unwrap()).unwrap(),
    )
}

fn scalar(v: &Value) -> Cell {
    match v {
        Value::Null => Cell::Null,
        Value::String(s) => Cell::Text(s.as_bytes().to_vec()),
        Value::Number(n) => Cell::Integer(n.as_i64().unwrap()),
        _ => panic!("unexpected frozen scalar"),
    }
}

fn projection(setup: &Setup, c: &Value) {
    let e = &c["expected"];
    let id = c["expectedRow"].as_i64().unwrap();
    let all = rows(setup.conn(), "market_provenance");
    let row = &all[(id - 1) as usize];
    assert_eq!(row.len(), e["scalars"].as_object().unwrap().len() + 3);
    assert_eq!(row["id"], Cell::Integer(id));
    assert_eq!(row["record_hash"], scalar(&e["recordHash"]));
    for (key, value) in e["scalars"].as_object().unwrap() {
        assert_eq!(row[key], scalar(value), "{} raw SQL {key}", c["id"]);
    }
    assert!(matches!(&row["created_at"], Cell::Text(s) if !s.is_empty()));
    let raw = hex::decode(c["rawHex"].as_str().unwrap()).unwrap();
    let path = setup
        .store
        .path_of(e["scalars"]["raw_artifact_path"].as_str().unwrap())
        .unwrap();
    assert!(path.starts_with(&setup.owned.path));
    assert_eq!(fs::read(path).unwrap(), raw);
    assert_eq!(sha256_hex(&raw), e["scalars"]["raw_sha256"]);
    assert_eq!(
        raw.len() as i64,
        e["scalars"]["raw_byte_len"].as_i64().unwrap()
    );
    let stored = get_provenance(setup.conn(), id).unwrap().unwrap();
    let reference = stored.artifact_ref();
    assert_eq!(reference.kind, e["artifactKind"]);
    assert_eq!(reference.relative_path, e["scalars"]["raw_artifact_path"]);
    assert_eq!(reference.sha256, e["scalars"]["raw_sha256"]);
    assert_eq!(
        reference.byte_len,
        e["scalars"]["raw_byte_len"].as_i64().unwrap()
    );
    assert_eq!(read_raw(&setup.store, &stored).unwrap(), raw);
}

fn register_all(setup: &Setup, v: &Value) {
    assert!(rows(setup.conn(), "market_provenance").is_empty());
    for step in v["provenanceSequence"].as_array().unwrap() {
        let c = named(v, step["input"].as_str().unwrap());
        let (observation, raw) = input(c);
        let saved_input = observation.clone();
        let saved_raw = raw.clone();
        let original_text = c["inputJson"].as_str().unwrap().as_bytes().to_vec();
        let before = setup.state();
        let id = step["expectedRow"].as_i64().unwrap();
        let created = step["created"].as_bool().unwrap();
        assert_eq!(
            record_raw(setup.conn(), &setup.store, &observation, &raw).unwrap(),
            (id, created)
        );
        let after = setup.state();
        for table in TABLES {
            if table != "market_provenance" && table != "sqlite_sequence" {
                assert_eq!(
                    after.0[table], before.0[table],
                    "unrelated market table {table}"
                );
            }
        }
        let old_rows = &before.0["market_provenance"];
        let new_rows = &after.0["market_provenance"];
        assert_eq!(new_rows.len(), old_rows.len() + usize::from(created));
        assert_eq!(&new_rows[..old_rows.len()], old_rows);
        assert!(before.1.directories.is_subset(&after.1.directories));
        for (path, bytes) in &before.1.files {
            assert_eq!(after.1.files.get(path), Some(bytes));
        }
        assert_eq!(
            after.1.files.len(),
            before.1.files.len() + usize::from(created)
        );
        if !created {
            assert_eq!(after, before, "retry preserves complete DB/artifact state");
        } else {
            let seq: i64 = setup
                .conn()
                .query_row(
                    "SELECT seq FROM sqlite_sequence WHERE name='market_provenance'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(seq, id);
        }
        projection(setup, c);
        assert_eq!(observation, saved_input);
        assert_eq!(raw, saved_raw);
        assert_eq!(c["inputJson"].as_str().unwrap().as_bytes(), original_text);
    }
    let before = setup.state();
    assert_eq!(before.0["market_provenance"].len(), 9);
    let mut files = BTreeMap::new();
    let mut directories = BTreeSet::from([
        PathBuf::new(),
        PathBuf::from("artifacts"),
        PathBuf::from("artifacts/staging"),
    ]);
    for c in v["provenance"].as_array().unwrap() {
        let path = PathBuf::from("artifacts").join(
            c["expected"]["scalars"]["raw_artifact_path"]
                .as_str()
                .unwrap(),
        );
        directories.insert(path.parent().unwrap().to_path_buf());
        files.insert(path, hex::decode(c["rawHex"].as_str().unwrap()).unwrap());
    }
    assert_eq!(before.1, Inventory { directories, files });
    for id in [1, 5, 7] {
        let chain = revision_chain(setup.conn(), id).unwrap();
        assert_eq!(
            serde_json::to_value(chain.iter().map(|p| p.id).collect::<Vec<_>>()).unwrap(),
            v["revisionChain"]
        );
    }
    for (id, previous, next) in [
        (1, None, Some(5)),
        (5, Some(1), Some(7)),
        (7, Some(5), None),
    ] {
        assert_eq!(
            get_provenance(setup.conn(), id)
                .unwrap()
                .unwrap()
                .revision_of,
            previous
        );
        assert_eq!(revised_by(setup.conn(), id).unwrap(), next);
    }
    assert!(get_provenance(setup.conn(), 2)
        .unwrap()
        .unwrap()
        .has_observed_archive_availability());
    assert!(!get_provenance(setup.conn(), 6)
        .unwrap()
        .unwrap()
        .has_observed_archive_availability());
    assert_eq!(
        setup.state(),
        before,
        "read-only original revision/availability queries"
    );
}

#[test]
fn frozen_provenance_source_and_registry_pins() {
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
    assert!(snapshot.matches(SNAPSHOT_DECLARATION).count() <= 1);
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

#[test]
fn original_record_hashes_preserve_all_frozen_fields_and_inputs() {
    let v = fixture();
    for c in v["provenance"].as_array().unwrap() {
        let (observation, raw) = input(c);
        let saved = observation.clone();
        let bytes = raw.clone();
        let expected = &c["expected"];
        assert_eq!(
            sha256_hex(expected["identityJson"].as_str().unwrap().as_bytes()),
            expected["recordHash"]
        );
        for _ in 0..2 {
            assert_eq!(
                record_hash(&observation, &sha256_hex(&raw), raw.len() as i64).unwrap(),
                expected["recordHash"],
                "{} original identity",
                c["id"]
            );
            assert_eq!(observation, saved);
            assert_eq!(raw, bytes);
        }
    }
}

#[test]
fn original_registrations_retries_files_and_revision_chain_conform() {
    register_all(&Setup::new(), &fixture());
}

#[test]
fn original_staged_refusals_preserve_database_artifact_inventory_and_inputs() {
    let v = fixture();
    let setup = Setup::new();
    register_all(&setup, &v);
    for c in v["provenanceRefusals"].as_array().unwrap() {
        let before = setup.state();
        let text = c["inputJson"].as_str().unwrap();
        let saved_text = text.as_bytes().to_vec();
        let raw = hex::decode(c["rawHex"].as_str().unwrap()).unwrap();
        let saved_raw = raw.clone();
        let parsed = serde_json::from_str::<RawObservation>(text);
        let error = if c["phase"] == "decode" {
            parsed.unwrap_err().to_string()
        } else {
            assert_eq!(c["phase"], "record");
            let observation = parsed.expect("typed original before record refusal");
            let saved = observation.clone();
            let err = record_raw(setup.conn(), &setup.store, &observation, &raw)
                .unwrap_err()
                .to_string();
            assert_eq!(observation, saved);
            err
        };
        assert!(
            error.contains(c["errorContains"].as_str().unwrap()),
            "{}: {error}",
            c["id"]
        );
        assert_eq!(setup.state(), before, "{} complete state", c["id"]);
        assert_eq!(text.as_bytes(), saved_text);
        assert_eq!(raw, saved_raw);
    }
}

#[test]
fn original_exact_retry_recovers_missing_file_and_refuses_corruption() {
    let v = fixture();
    let setup = Setup::new();
    register_all(&setup, &v);
    let c = named(&v, "revision");
    let (observation, raw) = input(c);
    let saved = observation.clone();
    let saved_raw = raw.clone();
    let original = setup.state();
    let path = setup
        .store
        .path_of(
            c["expected"]["scalars"]["raw_artifact_path"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    assert!(path.starts_with(&setup.owned.path));
    fs::remove_file(&path).unwrap();
    assert_eq!(database(setup.conn()), original.0);
    assert_eq!(
        setup.owned.inventory().files.len() + 1,
        original.1.files.len()
    );
    assert_eq!(
        record_raw(setup.conn(), &setup.store, &observation, &raw).unwrap(),
        (5, false)
    );
    assert_eq!(
        setup.state(),
        original,
        "same row and exact recovered bytes"
    );
    fs::write(&path, b"controlled owned-file corruption\n").unwrap();
    let corrupted = setup.state();
    assert_eq!(corrupted.0, original.0);
    for _ in 0..2 {
        let error = record_raw(setup.conn(), &setup.store, &observation, &raw)
            .unwrap_err()
            .to_string();
        assert!(error.contains("does not match its name"), "{error}");
        let stored = get_provenance(setup.conn(), 5).unwrap().unwrap();
        assert!(read_raw(&setup.store, &stored)
            .unwrap_err()
            .to_string()
            .contains("does not match its recorded checksum"));
        assert_eq!(
            setup.state(),
            corrupted,
            "refusal rewrites neither DB nor file"
        );
        assert_eq!(observation, saved);
        assert_eq!(raw, saved_raw);
    }
    fs::write(path, &raw).unwrap();
    assert_eq!(setup.state(), original);
    assert_eq!(
        record_raw(setup.conn(), &setup.store, &observation, &raw).unwrap(),
        (5, false)
    );
    assert_eq!(setup.state(), original);
}

#[test]
fn original_append_only_provenance_triggers_preserve_all_evidence() {
    let setup = Setup::new();
    register_all(&setup, &fixture());
    let before = setup.state();
    for (sql, fragment) in [
        (
            "UPDATE market_provenance SET source='changed' WHERE id=1",
            "immutable",
        ),
        ("DELETE FROM market_provenance WHERE id=1", "never deleted"),
    ] {
        assert!(setup
            .conn()
            .execute(sql, [])
            .unwrap_err()
            .to_string()
            .contains(fragment));
        assert_eq!(setup.state(), before);
    }
}
