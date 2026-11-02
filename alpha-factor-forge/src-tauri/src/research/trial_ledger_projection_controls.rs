//! Frozen literal controls for original producers. No runtime factory or source proof.
use super::*;
use crate::db::{self, ownership};
use crate::runtime::lease::{self, OsLock};
use rusqlite::types::ValueRef;
use std::fs;

const ORACLE: &str = include_str!("../../../fixtures/research/original-trial-projections-v1.json");

pub(crate) fn oracle() -> Value {
    assert_eq!(
        sha256_hex(ORACLE.replace("\r\n", "\n").as_bytes()),
        "a06fa7b074c11d6b13cae343f92e658c60a1c19ce5985ddf2276bb6e45c4d2ac"
    );
    let v: Value = serde_json::from_str(ORACLE).unwrap();
    assert_eq!(v["cases"].as_array().unwrap().len(), 11);
    v
}

pub(crate) fn named(id: &str) -> Value {
    oracle()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()
        .clone()
}

pub(crate) fn input(v: &Value) -> TrialBatchInput {
    let text = |v: &Value| v.as_str().map(str::to_owned);
    TrialBatchInput {
        workspace_id: v["workspaceId"].as_str().unwrap().into(),
        instrument_id: text(&v["instrumentId"]),
        tests_per_trial: v["testsPerTrial"].as_u64().unwrap(),
        events: v["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| TrialEventInput {
                kind: match e["kind"].as_str().unwrap() {
                    "variant" => TrialKind::Variant,
                    "diagnostic" => TrialKind::Diagnostic,
                    "legacy" => TrialKind::Legacy,
                    other => panic!("unsupported finite fixture kind {other}"),
                },
                origin: match e["origin"]["type"].as_str().unwrap() {
                    "request" => TrialOrigin::Request {
                        request_id: e["origin"]["requestId"].as_str().unwrap().into(),
                        candidate_index: e["origin"]["candidateIndex"].as_u64().unwrap(),
                    },
                    "legacy" => TrialOrigin::Legacy {
                        attempt_key: e["origin"]["attemptKey"].as_str().unwrap().into(),
                    },
                    other => panic!("unsupported fixture origin {other}"),
                },
                hypothesis_hash: text(&e["hypothesisHash"]),
                strategy_hash: text(&e["strategyHash"]),
                dataset_hash: text(&e["datasetHash"]),
                snapshot_id: text(&e["snapshotId"]),
                split_hash: text(&e["splitHash"]),
                seeds_hash: text(&e["seedsHash"]),
                engine_fingerprint_hash: text(&e["engineFingerprintHash"]),
                benchmark_id: text(&e["benchmarkId"]),
                benchmark_params_hash: text(&e["benchmarkParamsHash"]),
                reproduction_of: text(&e["reproductionOf"]),
                benchmark_evidence: None,
            })
            .collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cell {
    Null,
    Integer(i64),
    Real(u64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

fn rows(conn: &Connection, sql: &str) -> Vec<Vec<Cell>> {
    let mut stmt = conn.prepare(sql).unwrap();
    let columns = stmt.column_count();
    let mut values = stmt
        .query_map([], |row| {
            (0..columns)
                .map(|i| {
                    Ok(match row.get_ref(i)? {
                        ValueRef::Null => Cell::Null,
                        ValueRef::Integer(v) => Cell::Integer(v),
                        ValueRef::Real(v) => Cell::Real(v.to_bits()),
                        ValueRef::Text(v) => Cell::Text(v.to_vec()),
                        ValueRef::Blob(v) => Cell::Blob(v.to_vec()),
                    })
                })
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    values.sort();
    values
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DbImage {
    schema: Vec<Vec<Cell>>,
    tables: BTreeMap<String, Vec<Vec<Cell>>>,
    changes: u64,
    user_version: i64,
    foreign_keys: i64,
    autocommit: bool,
}

pub(crate) fn image(conn: &Connection) -> DbImage {
    let tables = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    DbImage {
        schema: rows(
            conn,
            "SELECT type,name,tbl_name,rootpage,sql FROM sqlite_master ORDER BY type,name",
        ),
        tables: tables
            .into_iter()
            .map(|name| {
                let sql = format!("SELECT * FROM \"{}\"", name.replace('"', "\"\""));
                (name, rows(conn, &sql))
            })
            .collect(),
        changes: conn
            .query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap(),
        user_version: conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap(),
        foreign_keys: conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap(),
        autocommit: conn.is_autocommit(),
    }
}

pub(crate) fn ledger_image(ledger: &TrialLedger) -> DbImage {
    // Snapshot the actual writer counter; release this guard before any producer.
    image(&ledger.lock().unwrap())
}

pub(crate) fn assert_rollback(before: &DbImage, after: &DbImage) {
    assert_eq!(before.schema, after.schema);
    assert_eq!(before.tables, after.tables);
    assert_eq!(before.user_version, after.user_version);
    assert_eq!(before.foreign_keys, after.foreign_keys);
    assert_eq!(before.autocommit, after.autocommit);
    assert!(
        after.changes > before.changes,
        "SQLite retains its real write counter across rollback"
    );
}

/// Reframe only the fixture's declared workspace namespace to the real birth ID.
/// Expected bytes are derived without prepare_batch/event_id_for/batch_id_for.
pub(crate) fn real_workspace_case(case: &Value, workspace: &str) -> Value {
    let mut c = case.clone();
    c["batchInput"]["workspaceId"] = json!(workspace);
    let b = &mut c["expectedBatch"];
    let mut returned = Vec::new();
    let mut members = Vec::new();
    for event in b["events"].as_array_mut().unwrap() {
        let mut payload: Value =
            serde_json::from_str(event["payloadJson"].as_str().unwrap()).unwrap();
        let key = format!(
            "{workspace}:request:{}:candidate:{}",
            payload["requestId"].as_str().unwrap(),
            payload["candidateIndex"]
        );
        payload["workspaceId"] = json!(workspace);
        payload["idempotencyKey"] = json!(key);
        let raw = serde_json::to_string(&payload).unwrap();
        let id = format!("trial-event-v1:{}", sha256_hex(raw.as_bytes()));
        event["idempotencyKey"] = json!(key);
        event["payloadJson"] = json!(raw);
        event["eventId"] = json!(id);
        event["storedProjection"]["idempotency_key"] = json!(key);
        event["storedProjection"]["payload_json"] = json!(raw);
        event["storedProjection"]["event_id"] = json!(id);
        returned.push(id.clone());
        members.push((key, id));
    }
    members.sort();
    let raw = serde_json::to_string(
        &json!({"version":"trial-batch-v1", "familyId":b["familyId"], "members":members}),
    )
    .unwrap();
    let id = format!("trial-batch-v1:{}", sha256_hex(raw.as_bytes()));
    b["returnedEventIdsInputOrder"] = json!(returned);
    b["membersJson"] = json!(serde_json::to_string(&members).unwrap());
    b["batchIdentityJson"] = json!(raw);
    b["batchId"] = json!(id);
    for event in b["events"].as_array_mut().unwrap() {
        event["storedProjection"]["batch_id"] = json!(id);
    }
    c
}

fn assert_append(before: &DbImage, after: &DbImage) {
    assert_eq!(before.schema, after.schema);
    assert_eq!(before.user_version, after.user_version);
    assert_eq!(before.foreign_keys, after.foreign_keys);
    assert_eq!(before.autocommit, after.autocommit);
    assert!(after.changes > before.changes);
    for (name, values) in &before.tables {
        if name == "sqlite_sequence" {
            continue;
        }
        for value in values {
            assert!(after.tables[name].contains(value), "lost {name}: {value:?}");
        }
    }
}

pub(crate) fn fresh_delta(before: &DbImage, after: &DbImage, case: &Value) {
    assert_append(before, after);
    let n = case["batchInput"]["events"].as_array().unwrap().len();
    let family = usize::from(!case["expectedBatch"]["familyId"].is_null());
    let expected = BTreeMap::from([
        ("trial_events", n),
        ("trial_families", family),
        ("trial_batches", 1),
        ("batch_receipts", 1),
    ]);
    for (name, values) in &before.tables {
        if let Some(count) = expected.get(name.as_str()) {
            assert!(values.is_empty(), "fresh {name}");
            assert_eq!(after.tables[name].len(), *count, "{name}");
        } else {
            assert_eq!(
                values, &after.tables[name],
                "unexpected original-producer write to {name}"
            );
        }
    }
    assert_eq!(after.changes - before.changes, (n + 2 + 2 * family) as u64);
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Inventory {
    directories: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, Vec<u8>>,
}

fn database_file(relative: &Path) -> bool {
    [
        ("workspace", db::DB_FILE_NAME),
        ("evidence", REGISTRY_FILE_NAME),
    ]
    .iter()
    .any(|(dir, name)| {
        ["", "-wal", "-shm"]
            .iter()
            .any(|suffix| relative == Path::new(dir).join(format!("{name}{suffix}")))
    })
}

pub(crate) struct Harness {
    pub(crate) ledger: Option<TrialLedger>,
    conn: Option<Connection>,
    lease: Option<OsLock>,
    parent: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) registry: PathBuf,
    pub(crate) epoch: i64,
}

impl Harness {
    pub(crate) fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).unwrap();
        let h = hex::encode(bytes);
        let root = parent.join(format!(
            "aff-trial-projection-{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..]
        ));
        fs::create_dir(&root).unwrap();
        let workspace = root.join("workspace");
        let registry = root.join("evidence");
        let mut f = Self {
            ledger: None,
            conn: None,
            lease: None,
            parent,
            root,
            workspace,
            registry,
            epoch: 0,
        };
        fs::create_dir(&f.workspace).unwrap();
        f.lease = Some(lease::try_lock_workspace(&f.workspace).unwrap());
        f.conn = Some(db::open_at(&f.workspace.join(db::DB_FILE_NAME)).unwrap());
        f.epoch = ownership::acquire(
            f.conn_mut(),
            ownership::HolderKind::DesktopEmbedded,
            std::process::id(),
        )
        .unwrap()
        .epoch;
        fs::create_dir(f.workspace.join("preserved-artifacts")).unwrap();
        fs::write(
            f.workspace.join("preserved-artifacts/original.bin"),
            [0, 255, 1, 17],
        )
        .unwrap();
        f.ledger = Some(TrialLedger::open(&f.registry, &f.workspace).unwrap());
        f
    }
    pub(crate) fn conn(&self) -> &Connection {
        self.conn.as_ref().unwrap()
    }
    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        self.conn.as_mut().unwrap()
    }
    pub(crate) fn ledger(&self) -> &TrialLedger {
        self.ledger.as_ref().unwrap()
    }
    pub(crate) fn artifacts(&self) -> Inventory {
        let mut inventory = self.inventory();
        inventory.files.retain(|p, _| !database_file(p));
        inventory
    }
    fn inventory(&self) -> Inventory {
        assert_eq!(self.root.parent(), Some(self.parent.as_path()));
        assert_eq!(self.root.canonicalize().unwrap(), self.root);
        let name = self.root.file_name().unwrap().to_str().unwrap();
        let suffix = name.strip_prefix("aff-trial-projection-").unwrap();
        assert_eq!(suffix.len(), 36);
        assert_eq!(suffix.chars().filter(|c| *c == '-').count(), 4);
        assert!(suffix.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
        let mut pending = vec![self.root.clone()];
        let mut found = Inventory {
            directories: BTreeSet::new(),
            files: BTreeMap::new(),
        };
        while let Some(p) = pending.pop() {
            assert!(p.starts_with(&self.root));
            let m = fs::symlink_metadata(&p).unwrap();
            assert!(!m.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                assert_eq!(m.file_attributes() & 0x400, 0, "reparse refused");
            }
            if m.is_dir() {
                found
                    .directories
                    .insert(p.strip_prefix(&self.root).unwrap().to_path_buf());
                for entry in fs::read_dir(&p).unwrap() {
                    pending.push(entry.unwrap().path());
                }
            } else {
                assert!(m.is_file());
                let relative = p.strip_prefix(&self.root).unwrap().to_path_buf();
                // SQLite state is captured through its actual connection. A
                // Windows SHM file contains locked regions and is not an artifact.
                let bytes = if database_file(&relative) || m.len() == 0 {
                    Vec::new()
                } else {
                    fs::read(&p).unwrap_or_else(|e| panic!("artifact {p:?}: {e}"))
                };
                found.files.insert(relative, bytes);
            }
        }
        found
    }
    pub(crate) fn assert_workspace(&self, before: &DbImage, artifacts: &Inventory) {
        assert_eq!(&image(self.conn()), before);
        assert_eq!(&self.artifacts(), artifacts);
        assert!(
            lease::try_lock_workspace(&self.workspace).is_err(),
            "caller lease retained"
        );
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        drop(self.ledger.take());
        drop(self.conn.take());
        drop(self.lease.take());
        let _ = self.inventory();
        fs::remove_dir_all(&self.root).unwrap();
        assert!(!self.root.exists());
    }
}

fn prepared(c: &Value) {
    let input = input(&c["batchInput"]);
    let saved = input.clone();
    let b = prepare_batch(&input).unwrap();
    let e = &c["expectedBatch"];
    assert_eq!(b.family_id(), e["familyId"].as_str());
    assert_eq!(
        b.family.as_ref().map(|(_, key)| key.as_str()),
        e["familyKeyJson"].as_str()
    );
    assert_eq!(b.tests_per_trial, input.tests_per_trial);
    assert_eq!(b.protocol_json, e["protocolJson"]);
    assert_eq!(b.batch_id, e["batchId"]);
    assert_eq!(b.members_json, e["membersJson"]);
    assert_eq!(b.events.len(), input.events.len());
    let mut returned = vec![String::new(); b.events.len()];
    for (p, candidate) in b.events.iter().enumerate() {
        let literal = &e["events"][candidate.input_index];
        assert_eq!(candidate.input_index, literal["inputPosition"]);
        assert_eq!(
            candidate.kind.as_str(),
            input.events[candidate.input_index].kind.as_str()
        );
        assert!(candidate.effective);
        assert_eq!(candidate.idempotency_key, literal["idempotencyKey"]);
        assert_eq!(candidate.payload_json, literal["payloadJson"]);
        assert_eq!(
            candidate.payload,
            serde_json::from_str::<Value>(literal["payloadJson"].as_str().unwrap()).unwrap()
        );
        assert_eq!(candidate.event_id, literal["eventId"]);
        assert_eq!(
            candidate.payload["candidateIndex"],
            e["candidateOrderByMembers"][p]
        );
        returned[candidate.input_index] = candidate.event_id.clone();
    }
    assert_eq!(json!(returned), e["returnedEventIdsInputOrder"]);
    assert_eq!(input, saved);
}

pub(crate) fn stored(ledger: &TrialLedger, expected: &Value, registration: &BatchRegistration) {
    assert_eq!(registration.batch_id, expected["batchId"]);
    assert_eq!(
        json!(registration.event_ids),
        expected["returnedEventIdsInputOrder"]
    );
    let conn = ledger.lock().unwrap();
    let mut stmt = conn.prepare("SELECT event_id,seq,chain,family_id,idempotency_key,kind,effective,batch_id,payload_json,origin_registry_id,registered_at FROM trial_events ORDER BY seq").unwrap();
    let actual = stmt.query_map([], |r| Ok(json!({
        "event_id":r.get::<_,String>(0)?, "seq":r.get::<_,i64>(1)?, "chain":r.get::<_,String>(2)?,
        "family_id":r.get::<_,Option<String>>(3)?, "idempotency_key":r.get::<_,String>(4)?,
        "kind":r.get::<_,String>(5)?, "effective":r.get::<_,i64>(6)?, "batch_id":r.get::<_,String>(7)?,
        "payload_json":r.get::<_,String>(8)?, "origin_registry_id":r.get::<_,String>(9)?, "registered_at":r.get::<_,String>(10)?,
    }))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
    assert_eq!(actual.len(), expected["events"].as_array().unwrap().len());
    let mut chain =
        sha256_hex(format!("trial-ledger-v1:genesis:{}", ledger.registry_id()).as_bytes());
    for (position, row) in actual.iter().enumerate() {
        let literal = expected["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["eventId"] == row["event_id"])
            .unwrap();
        for (k, v) in literal["storedProjection"].as_object().unwrap() {
            assert_eq!(&row[k], v, "{k}");
        }
        assert_eq!(row["seq"], literal["freshSequence"]);
        assert_eq!(row["seq"], position + 1);
        chain = sha256_hex(format!("{chain}{}", row["event_id"].as_str().unwrap()).as_bytes());
        assert_eq!(row["chain"], chain);
        assert_eq!(row["origin_registry_id"], ledger.registry_id());
        chrono::DateTime::parse_from_rfc3339(row["registered_at"].as_str().unwrap()).unwrap();
    }
    let (family, members, origin, created): (Option<String>, String, String, String) = conn.query_row(
        "SELECT family_id,members_json,origin_registry_id,created_at FROM trial_batches WHERE batch_id=?", [&registration.batch_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert_eq!(family.as_deref(), expected["familyId"].as_str());
    assert_eq!(members, expected["membersJson"]);
    assert_eq!(origin, ledger.registry_id());
    chrono::DateTime::parse_from_rfc3339(&created).unwrap();
    if let Some(id) = family {
        let (key, protocol): (String, String) = conn
            .query_row(
                "SELECT family_key,protocol_json FROM trial_families WHERE family_id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(key, expected["familyKeyJson"]);
        assert_eq!(protocol, expected["protocolJson"]);
    }
    let receipt: (String, i64, i64) = conn.query_row("SELECT receipt_registry_id,family_effective_before,batch_effective_trials FROM batch_receipts WHERE batch_id=?", [&registration.batch_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        receipt,
        (ledger.registry_id().into(), 0, actual.len() as i64)
    );
    assert_eq!(
        registration.registration_receipt.receipt_registry_id,
        ledger.registry_id()
    );
    assert_eq!(registration.registration_receipt.family_effective_before, 0);
    assert_eq!(
        registration.registration_receipt.batch_effective_trials,
        actual.len() as u64
    );
}

#[test]
fn original_trial_all_literal_preparations_match_complete_original_format() {
    let v = oracle();
    for c in v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(v["batchControls"].as_array().unwrap())
    {
        prepared(c);
    }
}

#[test]
fn original_trial_all_fresh_registrations_and_replays_preserve_complete_state() {
    let v = oracle();
    for c in v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(v["batchControls"].as_array().unwrap())
    {
        let f = Harness::new();
        f.conn().execute_batch("SAVEPOINT caller_pending; INSERT INTO app_settings(key,value_json) VALUES('trial-projection-pending','{\"kept\":true}')").unwrap();
        let workspace = image(f.conn());
        let artifacts = f.artifacts();
        let batch = input(&c["batchInput"]);
        let saved = batch.clone();
        let empty = ledger_image(f.ledger());
        let first = f.ledger().register_batch(&batch).unwrap();
        assert!(!first.replayed);
        stored(f.ledger(), &c["expectedBatch"], &first);
        f.assert_workspace(&workspace, &artifacts);
        let state = ledger_image(f.ledger());
        fresh_delta(&empty, &state, c);
        let replay = f.ledger().register_batch(&batch).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.event_ids, first.event_ids);
        assert_eq!(replay.registration_receipt, first.registration_receipt);
        assert_eq!(replay.admission, first.admission);
        assert_eq!(ledger_image(f.ledger()), state);
        let mut reversed = batch.clone();
        reversed.events.reverse();
        let reverse_replay = f.ledger().register_batch(&reversed).unwrap();
        assert!(reverse_replay.replayed);
        assert_eq!(
            reverse_replay.event_ids,
            first.event_ids.iter().rev().cloned().collect::<Vec<_>>()
        );
        assert_eq!(ledger_image(f.ledger()), state);
        assert_eq!(batch, saved);
        f.assert_workspace(&workspace, &artifacts);
        f.conn()
            .execute_batch("ROLLBACK TO caller_pending; RELEASE caller_pending")
            .unwrap();
        assert!(f.conn().is_autocommit());
    }
}

#[test]
fn original_trial_all_validation_refusals_preserve_writer_counter_and_caller() {
    let v = oracle();
    let f = Harness::new();
    let baseline = input(&named("v1-market-small")["batchInput"]);
    f.ledger().register_batch(&baseline).unwrap();
    f.conn().execute_batch("SAVEPOINT caller_pending").unwrap();
    let workspace = image(f.conn());
    let artifacts = f.artifacts();
    let state = ledger_image(f.ledger());
    for c in v["producerValidationRefusals"].as_array().unwrap() {
        let batch = input(&c["batchInput"]);
        assert!(
            matches!(prepare_batch(&batch), Err(LedgerError::InvalidBatch(_))),
            "{}",
            c["id"]
        );
        assert!(
            matches!(
                f.ledger().register_batch(&batch),
                Err(LedgerError::InvalidBatch(_))
            ),
            "{}",
            c["id"]
        );
        assert_eq!(ledger_image(f.ledger()), state);
        f.assert_workspace(&workspace, &artifacts);
    }
    f.conn()
        .execute_batch("ROLLBACK TO caller_pending; RELEASE caller_pending")
        .unwrap();
}

#[test]
fn original_trial_subset_superset_and_changed_key_conflict_precedence() {
    let f = Harness::new();
    let c = named("v1-twelve-lexical");
    let batch = input(&c["batchInput"]);
    f.ledger().register_batch(&batch).unwrap();
    let before = ledger_image(f.ledger());
    let workspace = image(f.conn());
    let artifacts = f.artifacts();
    let mut subset = batch.clone();
    subset.events.truncate(2);
    assert!(matches!(
        f.ledger().register_batch(&subset),
        Err(LedgerError::BatchConflict(_))
    ));
    let mut superset = batch.clone();
    let mut extra = batch.events[0].clone();
    if let TrialOrigin::Request {
        candidate_index, ..
    } = &mut extra.origin
    {
        *candidate_index = 12;
    }
    superset.events.push(extra);
    assert!(matches!(
        f.ledger().register_batch(&superset),
        Err(LedgerError::BatchConflict(_))
    ));
    // Lexically earlier unchanged index0 would conflict on subset membership;
    // changed index1 must win in the original complete per-key comparison pass.
    subset.events[1].strategy_hash = Some("changed-original-claim".into());
    assert!(matches!(
        f.ledger().register_batch(&subset),
        Err(LedgerError::IdempotencyConflict(_))
    ));
    assert_eq!(ledger_image(f.ledger()), before);
    f.assert_workspace(&workspace, &artifacts);
    let mut addition = batch.clone();
    for event in &mut addition.events {
        if let TrialOrigin::Request { request_id, .. } = &mut event.origin {
            *request_id = "separate-complete-request".into();
        }
    }
    let added = f.ledger().register_batch(&addition).unwrap();
    assert!(!added.replayed);
    assert_append(&before, &ledger_image(f.ledger()));
    f.assert_workspace(&workspace, &artifacts);
}

#[test]
fn original_trial_known_family_protocol_upgrade_is_append_only_and_not_identity() {
    let f = Harness::new();
    let c = named("v1-market-small");
    let batch = input(&c["batchInput"]);
    let first = f.ledger().register_batch(&batch).unwrap();
    let before = ledger_image(f.ledger());
    let workspace = image(f.conn());
    let artifacts = f.artifacts();
    let mut upgrade = batch.clone();
    upgrade.tests_per_trial = 3;
    let replay = f.ledger().register_batch(&upgrade).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.batch_id, first.batch_id);
    assert_eq!(replay.event_ids, first.event_ids);
    assert_eq!(replay.registration_receipt, first.registration_receipt);
    let after = ledger_image(f.ledger());
    assert_append(&before, &after);
    for (name, rows) in &before.tables {
        if name != "family_protocol_upgrades" {
            assert_eq!(rows, &after.tables[name], "upgrade changed {name}");
        }
    }
    assert_eq!(after.changes, before.changes + 1);
    assert_eq!(after.tables["family_protocol_upgrades"].len(), 1);
    {
        let conn = f.ledger().lock().unwrap();
        let (pin, count, protocol): (String, i64, String) = conn.query_row(
            "SELECT f.protocol_json,u.tests_per_trial,u.protocol_json FROM trial_families f JOIN family_protocol_upgrades u USING(family_id)", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(pin, c["expectedBatch"]["protocolJson"]);
        assert_eq!(count, 3);
        assert_eq!(protocol, "{\"correction\":\"holm\",\"testsPerTrial\":3}");
    }
    match replay.admission {
        Admission::Count(count) => assert_eq!(count.tests_per_trial(), 3),
        other => panic!("known family admission: {other:?}"),
    }
    assert!(matches!(
        f.ledger().register_batch(&batch),
        Err(LedgerError::FamilyProtocolMismatch(_))
    ));
    assert_eq!(ledger_image(f.ledger()), after);
    let same = f.ledger().register_batch(&upgrade).unwrap();
    assert!(same.replayed);
    assert_eq!(ledger_image(f.ledger()), after);
    f.assert_workspace(&workspace, &artifacts);
}
