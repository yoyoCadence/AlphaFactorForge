//! Real original producers precede the sealed capture; expected literals are independent.
use super::super::market;
use super::*;
use crate::{
    db::repositories,
    market::{
        provenance, registry,
        snapshot::{self as snapshots, SnapshotOutcome, SnapshotRequest},
    },
    research::{artifacts::ArtifactStore, history, sha256_hex},
};
use alpha_factor_forge::discovery_core::{
    campaign::freeze_campaign, config::parse_discovery_config,
};
use std::collections::{BTreeMap, BTreeSet};

#[path = "market_boundaries.rs"]
mod boundaries;
#[path = "market_negatives.rs"]
mod negatives;
#[path = "market_preparation.rs"]
mod preparation;

const ORACLE: &str =
    include_str!("../../../../../../../fixtures/research/original-market-consumer-v2.json");
const SOURCE: &str = include_str!(
    "../../../../../../../fixtures/research/original-market-source-projections-v1.json"
);
const REGISTRY: &str = include_str!(
    "../../../../../../../fixtures/research/original-market-registry-projections-v1.json"
);
const WARM: &str =
    include_str!("../../../../../../../fixtures/research/original-warmup-consumer-v1.json");

fn data(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
fn named<'a>(v: &'a Value, collection: &str, id: &str) -> &'a Value {
    v[collection]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()
}
fn oracle() -> Value {
    data(ORACLE)
}
fn case(id: &str) -> Value {
    named(&oracle(), "cases", id).clone()
}

#[derive(Debug, PartialEq, Eq)]
struct Inventory {
    dirs: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, Vec<u8>>,
}
struct OwnedRoot {
    parent: PathBuf,
    path: PathBuf,
}
impl OwnedRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir().canonicalize().unwrap();
        for _ in 0..1024 {
            let path = parent.join(format!(
                "aff-market-consumer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { parent, path },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("exclusive market root: {e}"),
            }
        }
        panic!("bounded unused market root allocation failed");
    }
    fn entries(&self, base: &std::path::Path) -> Inventory {
        assert_eq!(self.path.parent(), Some(self.parent.as_path()));
        assert_eq!(self.path.canonicalize().unwrap(), self.path);
        let prefix = format!("aff-market-consumer-{}-", std::process::id());
        assert!(self
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_prefix(&prefix)
            .is_some_and(|n| n.parse::<u64>().is_ok()));
        assert!(base.starts_with(&self.path));
        let mut result = Inventory {
            dirs: BTreeSet::new(),
            files: BTreeMap::new(),
        };
        let mut pending = vec![base.to_path_buf()];
        while let Some(p) = pending.pop() {
            assert!(p.starts_with(&self.path));
            let m = fs::symlink_metadata(&p).unwrap();
            assert!(!m.file_type().is_symlink());
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                assert_eq!(m.file_attributes() & 0x400, 0, "reparse refused");
            }
            let relative = p.strip_prefix(base).unwrap().to_path_buf();
            if m.is_dir() {
                result.dirs.insert(relative);
                for child in fs::read_dir(&p).unwrap() {
                    pending.push(child.unwrap().path());
                }
            } else {
                assert!(m.is_file());
                result.files.insert(relative, fs::read(&p).unwrap());
            }
        }
        result
    }
}
impl Drop for OwnedRoot {
    fn drop(&mut self) {
        let _ = self.entries(&self.path);
        fs::remove_dir_all(&self.path).unwrap();
        assert!(!self.path.exists());
    }
}
struct Actual {
    conn: Option<Connection>,
    lease: Option<OsLock>,
    expected: ExpectedWorkspace,
    store: ArtifactStore,
    campaign_id: String,
    owned: OwnedRoot,
}
impl Actual {
    fn market() -> Self {
        let owned = OwnedRoot::new();
        let lease = lease::try_lock_workspace(&owned.path).unwrap();
        let mut conn = db::open_at(&owned.path.join(db::DB_FILE_NAME)).unwrap();
        let acquired = ownership::acquire(
            &mut conn,
            ownership::HolderKind::DesktopEmbedded,
            std::process::id(),
        )
        .unwrap();
        let expected = ExpectedWorkspace::new(
            &runtime_ledger::workspace_id(&conn).unwrap(),
            acquired.epoch,
            ownership::HolderKind::DesktopEmbedded,
            &acquired.instance_id,
        )
        .unwrap();
        let store = ArtifactStore::in_data_dir(&owned.path);
        let mut f = Self {
            conn: Some(conn),
            lease: Some(lease),
            expected,
            store,
            campaign_id: String::new(),
            owned,
        };
        let registry_data = data(REGISTRY);
        let source = data(SOURCE);
        for id in ["calendar-0", "calendar-1"] {
            let c = named(&registry_data, "calendars", id);
            let spec = serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
            let (row, created) = registry::register_calendar(f.conn(), &spec).unwrap();
            assert!(created);
            assert_eq!(row, if id == "calendar-0" { 1 } else { 2 });
        }
        f.instrument("instrument-0", 1);
        for c in source["datasets"].as_array().unwrap() {
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
            let candles =
                serde_json::from_value::<Vec<repositories::Candle>>(c["candles"].clone()).unwrap();
            let id = repositories::import_dataset_with_candles(f.conn_mut(), &dataset, &candles)
                .unwrap();
            assert_eq!(id, c["rowId"]);
            assert_eq!(
                repositories::get_dataset_by_id(f.conn(), id)
                    .unwrap()
                    .dataset_hash,
                m["datasetHash"]
            );
        }
        for step in source["provenanceSequence"].as_array().unwrap() {
            let c = named(&source, "provenance", step["input"].as_str().unwrap());
            let input = serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
            let raw = hex::decode(c["rawHex"].as_str().unwrap()).unwrap();
            assert_eq!(
                provenance::record_raw(f.conn(), &f.store, &input, &raw).unwrap(),
                (
                    step["expectedRow"].as_i64().unwrap(),
                    step["created"].as_bool().unwrap()
                )
            );
            let hash: String = f
                .conn()
                .query_row(
                    "SELECT record_hash FROM market_provenance WHERE id=?",
                    [step["expectedRow"].as_i64().unwrap()],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(hash, c["expected"]["recordHash"]);
        }
        for (i, c) in source["snapshots"].as_array().unwrap().iter().enumerate() {
            if i == 2 {
                f.instrument("instrument-1", 2);
            }
            if i == 6 {
                f.instrument("instrument-2", 3);
            }
            let request: SnapshotRequest =
                serde_json::from_str(c["requestJson"].as_str().unwrap()).unwrap();
            let result = snapshots::build_snapshot(f.conn_mut(), &request).unwrap();
            let actual = match result {
                SnapshotOutcome::Created(s) if c["expected"]["outcome"] == "created" => s,
                SnapshotOutcome::Existing(s) if c["expected"]["outcome"] == "existing" => s,
                other => panic!("{}: {other:?}", c["id"]),
            };
            assert_eq!(actual.snapshot_id, c["expected"]["snapshotId"]);
            assert_eq!(
                actual.instrument_row_id,
                c["expected"]["scalars"]["instrument_row_id"]
            );
            assert_eq!(actual.coverage, c["expected"]["coverage"]);
            assert_eq!(actual.as_of, c["expected"]["scalars"]["as_of"]);
        }
        f
    }
    fn conn(&self) -> &Connection {
        self.conn.as_ref().unwrap()
    }
    fn conn_mut(&mut self) -> &mut Connection {
        self.conn.as_mut().unwrap()
    }
    fn instrument(&self, id: &str, row: i64) {
        let r = data(REGISTRY);
        let c = named(&r, "instruments", id);
        let spec = serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
        assert_eq!(
            registry::register_instrument(self.conn(), &spec).unwrap(),
            (row, true)
        );
        assert_eq!(
            registry::get_instrument(self.conn(), row)
                .unwrap()
                .unwrap()
                .content_hash,
            c["expected"]["contentHash"]
        );
    }
    fn from_case(c: &Value) -> Self {
        let mut f = Self::market();
        let family = &c["family"];
        let source = data(SOURCE);
        let instrument = named(&source, "snapshots", c["snapshotCase"].as_str().unwrap())
            ["expected"]["scalars"]["instrument_id"]
            .as_str()
            .unwrap()
            .to_owned();
        f.install(
            family["configJson"].as_str().unwrap(),
            family["campaignDocumentJson"].as_str().unwrap(),
            family["campaignId"].as_str().unwrap(),
            &instrument,
            family["interval"].as_str().unwrap(),
        );
        f.assert_original_family(c);
        f
    }
    fn install(
        &mut self,
        raw_json: &str,
        campaign_json: &str,
        campaign_id: &str,
        instrument: &str,
        interval: &str,
    ) {
        let raw: Value = serde_json::from_str(raw_json).unwrap();
        let campaign: Value = serde_json::from_str(campaign_json).unwrap();
        assert_eq!(
            freeze_campaign(&campaign).unwrap().campaign_id(),
            campaign_id
        );
        self.campaign_id = campaign_id.into();
        let config = parse_discovery_config(&raw, 64.0).unwrap();
        self.conn().execute("INSERT INTO discovery_runs(id,name,status,config_json) VALUES(1,'original market family','completed',?)",[raw_json]).unwrap();
        self.conn().execute("INSERT INTO research_campaigns(campaign_id,version,document_json) VALUES(?,'research-campaign-declaration-v1',?)",rusqlite::params![campaign_id,campaign_json]).unwrap();
        self.conn().execute("INSERT INTO campaign_run_admissions(id,discovery_run_id,campaign_id,instrument_id,batch_id,status,report_version,report_json,fee_pct,slip_pct) VALUES(1,1,?, ?,?,'NOT_ELIGIBLE','uninterpreted-report','null',?,?)",
            rusqlite::params![campaign_id,instrument,format!("trial-batch-v1:{}","b".repeat(64)),config.benchmark_costs.fee_pct,config.benchmark_costs.slip_pct]).unwrap();
        let strategies = crate::discovery_runner::source_row_strategies_for_test(&config).unwrap();
        let epoch = self.expected.epoch;
        let ids: Vec<_> = strategies
            .iter()
            .map(|s| {
                repositories::get_or_insert_verified_runner_strategy(
                    self.conn_mut(),
                    Some(epoch),
                    s,
                )
                .unwrap()
            })
            .collect();
        let lineage =
            crate::discovery_runner::source_row_lineage_for_test(&config, &raw, 1, &ids, interval)
                .unwrap();
        let events: Vec<_> = (0..ids.len())
            .map(|i| format!("trial-event-v1:{i:064x}"))
            .collect();
        let tx = self.conn_mut().transaction().unwrap();
        let written =
            history::register_run_lineage_with_events(&tx, &lineage, Some(&events)).unwrap();
        assert_eq!(written.len(), ids.len());
        for (i, id) in ids.iter().enumerate() {
            for segment in ["train", "validation"] {
                tx.execute("INSERT INTO discovery_jobs(discovery_run_id,candidate_index,strategy_id,dataset_id,segment,status) VALUES(1,?,?,?,?,'queued')",
                    rusqlite::params![i as i64,id,config.dataset.id,segment]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    fn assert_original_family(&self, c: &Value) {
        let f = &c["family"];
        let warm = data(WARM);
        let base = named(&warm, "cases", f["sourceCase"].as_str().unwrap());
        let raw: String = self
            .conn()
            .query_row(
                "SELECT config_json FROM discovery_runs WHERE id=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(raw, f["configJson"]);
        for (i, input) in f["inputs"].as_array().unwrap().iter().enumerate() {
            let (saved,engine):(String,String)=self.conn().query_row("SELECT input_fingerprint_json,engine_fingerprint_json FROM research_attempts WHERE candidate_index=?",[i as i64],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
            assert_eq!(saved, input["inputFingerprintJson"]);
            assert_eq!(sha256_hex(saved.as_bytes()), input["sha256"]);
            assert_eq!(engine, base["engineFingerprintJson"]);
            let (definition, dsl): (String, Option<String>) = self
                .conn()
                .query_row(
                    "SELECT original_definition_json,dsl_json FROM strategy_def WHERE id=?",
                    [i as i64 + 1],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                definition,
                base["candidates"][i]["freshStrategy"]["original_definition_json"]
            );
            assert_eq!(
                dsl,
                base["candidates"][i]["freshStrategy"]["dsl_json"]
                    .as_str()
                    .map(str::to_owned)
            );
        }
        for (i, h) in f["hypotheses"].as_array().unwrap().iter().enumerate() {
            let (hash,doc,app):(String,String,String)=self.conn().query_row("SELECT hypothesis_hash,content_json,applicability_json FROM hypotheses WHERE id=?",[i as i64+1],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
            assert_eq!(hash, h["contentHash"]);
            assert_eq!(doc, h["contentJson"]);
            assert_eq!(app, h["applicabilityJson"]);
        }
    }
    fn locator(&self) -> SourceLocator {
        SourceLocator::new(&self.campaign_id, 1).unwrap()
    }
    fn capture(&mut self) -> SourceRowsObservation {
        let expected = self.expected.clone();
        let locator = self.locator();
        let tx = self.conn_mut().transaction().unwrap();
        let rows = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator)).unwrap();
        tx.rollback().unwrap();
        rows
    }
    fn artifacts(&self) -> Inventory {
        self.owned.entries(self.store.root())
    }
}
impl Drop for Actual {
    fn drop(&mut self) {
        drop(self.conn.take());
        drop(self.lease.take());
    }
}

fn observed(r: &OriginalMarketProjectionObservation) -> Value {
    json!({"snapshotId":r.snapshot_id(),"instrumentRevisionHash":r.instrument_revision_hash(),
        "calendarHash":r.calendar_hash(),"sources":r.sources().iter().map(|s|json!({
            "provenanceId":s.provenance_id(),"recordHash":s.record_hash()})).collect::<Vec<_>>()})
}
fn refused<T>(r: AppResult<T>) {
    match r {
        Err(AppError::Other(code)) => assert_eq!(code, "original_market_invalid"),
        Err(e) => panic!("unexpected market error: {e}"),
        Ok(_) => panic!("expected complete market refusal"),
    }
}
fn preserves(
    f: &mut Actual,
    rows: &SourceRowsObservation,
    success: bool,
) -> Option<OriginalMarketProjectionObservation> {
    let saved = rows.clone();
    let sql = snapshot(f.conn());
    let artifacts = f.artifacts();
    let independent = Connection::open(f.owned.path.join(db::DB_FILE_NAME)).unwrap();
    independent.busy_timeout(Duration::ZERO).unwrap();
    assert!(lease::try_lock_workspace(&f.owned.path).is_err());
    let tx = f
        .conn_mut()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute_batch(
        "SAVEPOINT market_caller; UPDATE strategy_def SET name='pending caller' WHERE id=1;",
    )
    .unwrap();
    writer_busy(&independent);
    let result = unchanged(&tx, || observe_original_market(rows));
    writer_busy(&independent);
    let observed = if success {
        Some(result.unwrap())
    } else {
        refused(result);
        None
    };
    assert!(rows == &saved);
    tx.execute_batch("ROLLBACK TO market_caller; RELEASE market_caller;")
        .unwrap();
    assert!(!tx.is_autocommit());
    tx.rollback().unwrap();
    assert_eq!(snapshot(f.conn()), sql);
    assert_eq!(f.artifacts(), artifacts);
    observed
}

#[test]
fn market_frozen_contract_and_original_package_pins() {
    assert_eq!(
        sha256_hex(ORACLE.replace("\r\n", "\n").as_bytes()),
        "a12f373f2a13580263ccb752bf6be773b812e457baa259b2646810a501aabf01"
    );
    let v = oracle();
    assert_eq!(v["version"], "original-market-consumer-v2");
    assert_eq!(v["cases"].as_array().unwrap().len(), 4);
    assert_eq!(v["coherentRefusals"].as_array().unwrap().len(), 5);
    assert_eq!(v["negativeControls"].as_array().unwrap().len(), 14);
    for (name, text) in [
        ("original-market-source-projections-v1.json", SOURCE),
        ("original-market-registry-projections-v1.json", REGISTRY),
        ("original-warmup-consumer-v1.json", WARM),
        ("source-row-graph-v1.json", GRAPH),
        (
            "original-lineage-semantics-v1.json",
            include_str!(
                "../../../../../../../fixtures/research/original-lineage-semantics-v1.json"
            ),
        ),
        ("original-producer-projections-v1.json", PRODUCERS),
    ] {
        let pin = v["sourceFixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["path"] == name)
            .unwrap();
        assert_eq!(
            sha256_hex(text.replace("\r\n", "\n").as_bytes()),
            pin["lfSha256"]
        );
    }
}
#[test]
fn market_all_four_original_complete_families_and_joint_work_match_frozen_literals() {
    for c in oracle()["cases"].as_array().unwrap() {
        let mut f = Actual::from_case(c);
        let rows = f.capture();
        let planned = market::planned(&rows).unwrap();
        assert_eq!(planned["marketWork"], c["marketWork"], "{}", c["id"]);
        assert_eq!(
            planned["jointPreparation"], c["jointPreparation"],
            "{}",
            c["id"]
        );
        assert_eq!(planned["jointWork"], c["jointWork"], "{}", c["id"]);
        assert_eq!(planned["oldPreparation"], c["family"]["priorPreparation"]);
        let before = entries();
        let warm = warmup_controls::warm_entries();
        let result = preserves(&mut f, &rows, true).unwrap();
        assert_eq!(observed(&result), c["expectedMarket"]);
        assert_eq!(
            semantic_controls::observation(result.warmup().lineage()),
            c["family"]["expectedLineage"]
        );
        assert_eq!(
            serde_json::to_value(result.warmup().derivations()).unwrap(),
            c["family"]["expectedWarmup"]
        );
        assert!(result.clone() == result);
        assert_eq!(entries().1, before.1 + 1);
        let n = c["family"]["expectedLineage"]["candidates"]
            .as_array()
            .unwrap()
            .len();
        assert_eq!(warmup_controls::warm_entries().0, warm.0 + n);
    }
}
