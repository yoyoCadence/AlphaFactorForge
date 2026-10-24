//! Pre-code literal controls against the actual fresh-row and lineage writers.
//! Declared metadata in an isolated DB proves projection, never source authority.

use super::*;
use crate::db::ownership::{self, HolderKind};
use crate::research::history as research_history;
use crate::runtime::lease::{self, OsLock};
use alpha_factor_forge::discovery_core::enumerate::CandidateSeeds;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const CONTROLS: &str =
    include_str!("../../../../fixtures/research/original-producer-projections-v1.json");

fn control(id: &str) -> Value {
    serde_json::from_str::<Value>(CONTROLS).unwrap()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()
        .clone()
}

fn fresh_root() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..1024 {
        let root = std::env::temp_dir().join(format!(
            "aff-original-producer-control-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        match fs::create_dir(&root) {
            Ok(()) => return root,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("new producer-control directory: {e}"),
        }
    }
    panic!("no unused producer-control directory within bounded retries");
}

struct Fixture {
    root: PathBuf,
    conn: Option<Connection>,
    lease: Option<OsLock>,
    epoch: i64,
}

impl Fixture {
    fn new() -> Self {
        // Assemble the guard first, so setup failures also close and clean only
        // the root just created. Existing names are skipped, never reused.
        let mut f = Self {
            root: fresh_root(),
            conn: None,
            lease: None,
            epoch: 0,
        };
        f.lease = Some(lease::try_lock_workspace(&f.root).unwrap());
        f.conn = Some(crate::db::open_at(&f.root.join(crate::db::DB_FILE_NAME)).unwrap());
        f.epoch = ownership::acquire(
            f.conn.as_mut().unwrap(),
            HolderKind::DesktopEmbedded,
            std::process::id(),
        )
        .unwrap()
        .epoch;
        f
    }

    fn conn(&self) -> &Connection {
        self.conn.as_ref().unwrap()
    }

    fn seed_references(&self, case: &Value, config: &ResolvedDiscoveryConfig) {
        self.conn().execute(
            "INSERT INTO datasets(id,exchange,symbol,interval,start_time,end_time,candle_count,source,dataset_hash) VALUES(?1,'projection-control','declared',?2,0,0,0,'test-reference-only',?3)",
            params![config.dataset.id, case["interval"].as_str().unwrap(), config.dataset.content_hash],
        ).unwrap();
        self.conn().execute(
            "INSERT INTO discovery_runs(id,name,status,config_json) VALUES(?1,'projection control','idle',?2)",
            params![case["runId"].as_i64().unwrap(), case["configJson"].as_str().unwrap()],
        ).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.conn.take());
        drop(self.lease.take());
        if let Err(error) = fs::remove_dir_all(&self.root) {
            if std::thread::panicking() {
                eprintln!("owned producer-control cleanup failed: {error}");
            } else {
                panic!("owned producer-control cleanup failed: {error}");
            }
        }
    }
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

fn stored_hypothesis(conn: &Connection, id: i64) -> Value {
    conn.query_row("SELECT hypothesis_hash,version,source,mechanism,applicability_json,failure_modes,strategy_hash,strategy_id,parent_strategy_id,variation_kind,content_json FROM hypotheses WHERE id=?1", [id], |r| Ok(json!({
        "hash":r.get::<_,String>(0)?, "version":r.get::<_,String>(1)?,
        "source":r.get::<_,String>(2)?, "mechanism":r.get::<_,String>(3)?,
        "applicabilityJson":r.get::<_,String>(4)?, "failureModes":r.get::<_,String>(5)?,
        "strategyHash":r.get::<_,String>(6)?, "strategyId":r.get::<_,Option<i64>>(7)?,
        "parentStrategyId":r.get::<_,Option<i64>>(8)?, "variationKind":r.get::<_,Option<String>>(9)?,
        "documentJson":r.get::<_,String>(10)?,
    }))).unwrap()
}

fn stored_attempt(conn: &Connection, id: i64) -> Value {
    conn.query_row("SELECT attempt_key,hypothesis_id,strategy_id,dataset_id,discovery_run_id,candidate_index,status,input_fingerprint_json,engine_fingerprint_json,outcome_json,result_artifact_id,trial_event_id,epoch FROM research_attempts WHERE id=?1", [id], |r| Ok(json!({
        "attemptKey":r.get::<_,String>(0)?, "hypothesisId":r.get::<_,i64>(1)?,
        "strategyId":r.get::<_,i64>(2)?, "datasetId":r.get::<_,i64>(3)?,
        "runId":r.get::<_,Option<i64>>(4)?, "index":r.get::<_,Option<i64>>(5)?,
        "status":r.get::<_,String>(6)?, "inputJson":r.get::<_,String>(7)?,
        "engineJson":r.get::<_,String>(8)?, "outcomeJson":r.get::<_,Option<String>>(9)?,
        "artifactId":r.get::<_,Option<i64>>(10)?, "eventId":r.get::<_,Option<String>>(11)?,
        "epoch":r.get::<_,Option<i64>>(12)?,
    }))).unwrap()
}

fn conforms(id: &str) {
    let case = control(id);
    let raw: Value = serde_json::from_str(case["configJson"].as_str().unwrap()).unwrap();
    let config = parse_discovery_config(&raw, case["parserCores"].as_f64().unwrap()).unwrap();
    let plan = enumerate_candidates(&config).unwrap();
    let expected = case["candidates"].as_array().unwrap();
    assert_eq!(plan.candidates.len(), expected.len(), "{id}");
    assert_eq!(
        sha256_hex(&canonical_json(&raw).unwrap()),
        case["configHash"],
        "{id}"
    );
    let engine = execution::engine_fingerprint(&config);
    assert_eq!(
        serde_json::to_string(&engine).unwrap(),
        case["engineFingerprintJson"],
        "{id}"
    );
    assert_eq!(
        sha256_hex(case["engineFingerprintJson"].as_str().unwrap().as_bytes()),
        case["engineJsonSha256"],
        "{id}"
    );
    let mut f = Fixture::new();
    f.seed_references(&case, &config);
    let mut scheduled = Vec::new();
    for (candidate, expected) in plan.candidates.into_iter().zip(expected) {
        assert_eq!(candidate.index, expected["index"].as_i64().unwrap(), "{id}");
        assert_eq!(candidate.strategy_hash, expected["strategyHash"], "{id}");
        assert_eq!(candidate.base_id, expected["baseId"], "{id}");
        assert_eq!(
            serde_json::to_value(&candidate.applied_axes).unwrap(),
            expected["appliedAxes"],
            "{id}"
        );
        assert_eq!(
            serde_json::to_value(candidate.seeds).unwrap(),
            expected["seeds"],
            "{id}"
        );
        let dto = candidate_strategy_def(&candidate).unwrap();
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            expected["freshStrategy"],
            "{id}"
        );
        let strategy_id =
            repositories::get_or_insert_verified_runner_strategy(f.conn(), Some(f.epoch), &dto)
                .unwrap();
        assert_eq!(
            strategy_id,
            expected["strategyId"].as_i64().unwrap(),
            "{id}"
        );
        let stored = repositories::get_strategy_by_id(f.conn(), strategy_id).unwrap();
        let mut stored_expected = expected["freshStrategy"].clone();
        stored_expected["id"] = json!(strategy_id);
        assert_eq!(
            serde_json::to_value(stored).unwrap(),
            stored_expected,
            "{id}"
        );
        scheduled.push(ScheduledCandidate {
            candidate,
            strategy_id,
        });
    }
    let dataset = VerifiedDataset {
        id: config.dataset.id,
        content_hash: config.dataset.content_hash.clone(),
        interval: case["interval"].as_str().unwrap().into(),
        candle_count: 0,
    };
    let lineage = run_lineage(
        &config,
        &raw,
        case["runId"].as_i64().unwrap(),
        &dataset,
        &scheduled,
        Some(f.epoch),
    )
    .unwrap();
    assert_eq!(lineage.len(), expected.len(), "{id}");
    let hypotheses = case["hypotheses"].as_array().unwrap();
    for ((h, a), expected) in lineage.iter().zip(expected) {
        assert_eq!(h.hash().unwrap(), expected["hypothesisHash"], "{id}");
        let h_expected = hypotheses
            .iter()
            .find(|h| h["hash"] == expected["hypothesisHash"])
            .unwrap();
        assert_eq!(
            serde_json::to_string(&h.content().unwrap()).unwrap(),
            h_expected["documentJson"],
            "{id}"
        );
        assert_eq!(
            serde_json::to_string(&a.input_fingerprint).unwrap(),
            expected["inputFingerprintJson"],
            "{id}"
        );
        assert_eq!(
            serde_json::to_string(&a.engine_fingerprint).unwrap(),
            case["engineFingerprintJson"],
            "{id}"
        );
        assert_eq!(a.attempt_key, expected["attemptKey"], "{id}");
    }
    let epoch = f.epoch;
    let tx = f.conn.as_mut().unwrap().transaction().unwrap();
    let registered =
        research_history::register_run_lineage_with_events(&tx, &lineage, None).unwrap();
    // Complete hypothesis content deduplicates; attempts keep their unique keys.
    for ((hypothesis, _), (registered_hypothesis, _)) in lineage.iter().zip(&registered) {
        let (again, created) = research_history::register_hypothesis(&tx, hypothesis).unwrap();
        assert_eq!(
            again, *registered_hypothesis,
            "{id}: complete-content repeat"
        );
        assert!(!created, "{id}: existing complete hypothesis");
    }
    assert_eq!(count(&tx, "hypotheses"), hypotheses.len() as i64, "{id}");
    assert_eq!(
        count(&tx, "research_attempts"),
        expected.len() as i64,
        "{id}"
    );
    for ((hypothesis_id, attempt_id), expected) in registered.iter().zip(expected) {
        let h = hypotheses
            .iter()
            .find(|h| h["hash"] == expected["hypothesisHash"])
            .unwrap();
        let doc: Value = serde_json::from_str(h["documentJson"].as_str().unwrap()).unwrap();
        assert_eq!(
            stored_hypothesis(&tx, *hypothesis_id),
            json!({
                "hash":h["hash"], "version":doc["version"], "source":doc["source"], "mechanism":doc["mechanism"],
                "applicabilityJson":h["applicabilityJson"], "failureModes":doc["failureModes"],
                "strategyHash":doc["strategyHash"], "strategyId":doc["strategyId"], "parentStrategyId":doc["parentStrategyId"],
                "variationKind":doc["variationKind"], "documentJson":h["documentJson"],
            }),
            "{id}"
        );
        assert_eq!(
            stored_attempt(&tx, *attempt_id),
            json!({
                "attemptKey":expected["attemptKey"], "hypothesisId":hypothesis_id,
                "strategyId":expected["strategyId"], "datasetId":config.dataset.id,
                "runId":case["runId"], "index":expected["index"], "status":"submitted",
                "inputJson":expected["inputFingerprintJson"], "engineJson":case["engineFingerprintJson"],
                "outcomeJson":null, "artifactId":null, "eventId":null, "epoch":epoch,
            }),
            "{id}"
        );
    }
    tx.commit().unwrap();
    for table in [
        "candles",
        "discovery_jobs",
        "research_artifacts",
        "backtest_summary",
        "trades",
    ] {
        assert_eq!(
            count(f.conn(), table),
            0,
            "{id}: no execution/output in {table}"
        );
    }
    assert_eq!(count(f.conn(), "strategy_def"), expected.len() as i64);
}

#[test]
fn ordered_axes_original_producer_conforms() {
    conforms("v1-ordered-axes");
}

#[test]
fn deduplicated_winning_bases_original_producer_conforms() {
    conforms("v1-shared-dedup");
}

#[test]
fn fixed_dsl_original_producer_conforms() {
    conforms("v2-fixed-dsl");
}

#[test]
fn walk_forward_original_producer_conforms() {
    conforms("v3-walk-forward");
}

#[test]
fn library_conflict_preserves_bytes_metadata_and_contradictory_dsl_claim() {
    let case = control("v2-fixed-dsl");
    let raw = serde_json::from_str(case["configJson"].as_str().unwrap()).unwrap();
    let config = parse_discovery_config(&raw, 64.0).unwrap();
    let candidate = enumerate_candidates(&config).unwrap().candidates.remove(0);
    let f = Fixture::new();
    let mut library = candidate_strategy_def(&candidate).unwrap();
    library.name = "saved library name".into();
    library.source = "manual".into();
    library.lifecycle = "validated".into();
    let original: Value = serde_json::from_str(&library.original_definition_json).unwrap();
    // A distinct order/whitespace representation preserves the same identity.
    let fields: Vec<_> = original
        .as_object()
        .unwrap()
        .iter()
        .rev()
        .map(|(key, value)| {
            format!(
                "{}: {}",
                serde_json::to_string(key).unwrap(),
                serde_json::to_string_pretty(value).unwrap()
            )
        })
        .collect();
    library.original_definition_json = format!(" \n{{\n{}\n}}", fields.join(",\n"));
    library.dsl_json = Some("{}".into()); // retained contradiction is not consumer acceptance
    library.param_schema_json = Some("null".into());
    let id = repositories::insert_verified_strategy(f.conn(), &library).unwrap();
    let before =
        serde_json::to_value(repositories::get_strategy_by_id(f.conn(), id).unwrap()).unwrap();
    let fresh = candidate_strategy_def(&candidate).unwrap();
    assert_ne!(
        library.original_definition_json,
        fresh.original_definition_json
    );
    assert_eq!(
        serde_json::from_str::<Value>(&library.original_definition_json).unwrap(),
        original
    );
    assert_eq!(
        repositories::get_or_insert_verified_runner_strategy(f.conn(), Some(f.epoch), &fresh)
            .unwrap(),
        id
    );
    let after =
        serde_json::to_value(repositories::get_strategy_by_id(f.conn(), id).unwrap()).unwrap();
    assert_eq!(after, before);
    assert_eq!(count(f.conn(), "strategy_def"), 1);
    assert_ne!(
        after["dsl_json"],
        case["candidates"][0]["freshStrategy"]["dsl_json"]
    );
    // This claim disagrees with the preserved original executable document.
    assert_ne!(
        serde_json::from_str::<Value>(after["dsl_json"].as_str().unwrap()).unwrap(),
        original["dsl"]
    );
}

#[test]
fn intended_numeric_hash_is_refused_before_any_repository_write() {
    let controls: Value = serde_json::from_str(CONTROLS).unwrap();
    let refusal = &controls["numericRefusal"];
    let audit: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/research/numeric-json-audit-v1.json"
    ))
    .unwrap();
    let source = audit["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == refusal["case"])
        .unwrap();
    let mut strategy: Value =
        serde_json::from_str(source["definitionJson"].as_str().unwrap()).unwrap();
    let intended_bits =
        u64::from_str_radix(refusal["intendedFeeBits"].as_str().unwrap(), 16).unwrap();
    strategy["feePct"] = json!(f64::from_bits(intended_bits));
    let candidate = EnumeratedCandidate {
        index: 0,
        base_id: "numeric-audit".into(),
        strategy_hash: refusal["intendedStrategyHash"].as_str().unwrap().into(),
        applied_axes: BTreeMap::new(),
        strategy,
        seeds: CandidateSeeds {
            random_entry: 943067094,
        },
    };
    let f = Fixture::new();
    let before: (i64, i64) = f
        .conn()
        .query_row(
            "SELECT total_changes(),mutation_seq FROM runtime_state WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let dto = candidate_strategy_def(&candidate).unwrap();
    let reread: Value = serde_json::from_str(&dto.original_definition_json).unwrap();
    assert_eq!(
        format!("{:016x}", reread["feePct"].as_f64().unwrap().to_bits()),
        refusal["legacyReparsedFeeBits"]
    );
    assert_eq!(dto.strategy_hash, refusal["intendedStrategyHash"]);
    let error = repositories::get_or_insert_verified_runner_strategy(f.conn(), Some(f.epoch), &dto)
        .unwrap_err();
    assert!(
        matches!(error, crate::error::AppError::Other(ref message) if message.starts_with("strategy identity mismatch:"))
    );
    assert_eq!(count(f.conn(), "strategy_def"), 0);
    let after: (i64, i64) = f
        .conn()
        .query_row(
            "SELECT total_changes(),mutation_seq FROM runtime_state WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(after, before);
    assert!(f.conn().is_autocommit());
}
