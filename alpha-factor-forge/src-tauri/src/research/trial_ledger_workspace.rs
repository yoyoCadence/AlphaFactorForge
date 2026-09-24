//! P12b-2 workspace adoption. Registry commits precede workspace writes;
//! repeated opens replay identical legacy batches after a crash between them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use super::history::RunLineage;
use super::trial_ledger::{
    read_workspace_binding, write_workspace_binding, BindingCheck, LedgerBinding, TrialBatchInput,
    TrialEventInput, TrialKind, TrialLedger, TrialOrigin,
};
use super::{canonical_json, sha256_hex};
use crate::error::{AppError, AppResult};

pub struct BoundLedger {
    pub ledger: Arc<TrialLedger>,
    pub report: LedgerWorkspaceReport,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerWorkspaceReport {
    pub backfilled_attempts: usize,
    pub orphan_event_ids: Vec<String>,
    /// A validation record with no P05 attempt is uncounted history. These
    /// families cannot be qualified until a later contract resolves it.
    pub legacy_trials_unknown: Vec<String>,
}

pub fn registry_dir(_workspace_dir: &Path) -> AppResult<PathBuf> {
    #[cfg(all(debug_assertions, not(test)))]
    if let Some(path) = std::env::var_os("AFF_TEST_TRIAL_REGISTRY_DIR") {
        // Only isolated temp workspaces from the test harness may override
        // a debug service binary. Release builds ignore the seam entirely.
        let workspace = std::fs::canonicalize(_workspace_dir)?;
        let temp = std::fs::canonicalize(std::env::temp_dir())?;
        if workspace.starts_with(temp)
            && workspace
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("aff-"))
        {
            return Ok(PathBuf::from(path));
        }
    }
    #[cfg(test)]
    {
        // Unit tests own isolated workspaces, never the user's actual registry.
        let canonical = std::fs::canonicalize(_workspace_dir)?;
        let key = sha256_hex(canonical.to_string_lossy().as_bytes());
        Ok(std::env::temp_dir().join(format!(
            "aff-trial-ledger-test-{}-{key}",
            std::process::id()
        )))
    }
    #[cfg(not(test))]
    {
        // The service and desktop share one per-user local registry even if
        // their workspace lives under a synced or removable directory.
        let base = dirs::data_local_dir().ok_or_else(|| {
            AppError::Other("registry_missing: cannot locate the local app-data directory".into())
        })?;
        Ok(base.join("com.alphafactorforge.trial-ledger"))
    }
}

fn ledger_error(error: impl std::fmt::Display) -> AppError {
    AppError::Other(error.to_string())
}

pub fn require_current(ledger: &TrialLedger, conn: &Connection) -> AppResult<LedgerBinding> {
    let saved = read_workspace_binding(conn)
        .map_err(ledger_error)?
        .ok_or_else(|| ledger_error("registry_missing: workspace binding is absent"))?;
    match ledger.check_binding(Some(&saved)).map_err(ledger_error)? {
        BindingCheck::Current(head) => Ok(head),
        blocked => Err(ledger_error(blocked.code())),
    }
}

pub fn adopt(
    conn: &mut Connection,
    workspace_dir: &Path,
    workspace_id: &str,
) -> AppResult<BoundLedger> {
    let saved = read_workspace_binding(conn).map_err(ledger_error)?;
    let dir = registry_dir(workspace_dir)?;
    let ledger = Arc::new(
        match saved {
            Some(_) => TrialLedger::open_existing(&dir, workspace_dir),
            None => TrialLedger::open(&dir, workspace_dir),
        }
        .map_err(ledger_error)?,
    );
    let head = match ledger.check_binding(saved.as_ref()).map_err(ledger_error)? {
        BindingCheck::Current(head) => head,
        blocked => return Err(ledger_error(blocked.code())),
    };
    let assignments = if saved.is_none() {
        backfill(conn, &ledger, workspace_id)?
    } else {
        Vec::new()
    };
    if saved.is_none() {
        let current = match ledger.check_binding(Some(&head)).map_err(ledger_error)? {
            BindingCheck::Current(head) => head,
            blocked => return Err(ledger_error(blocked.code())),
        };
        let tx = conn.transaction()?;
        for (id, event) in &assignments {
            tx.execute(
                "UPDATE research_attempts SET trial_event_id = ?1 WHERE id = ?2 AND trial_event_id IS NULL",
                params![event, id],
            )?;
        }
        write_workspace_binding(&tx, &current).map_err(ledger_error)?;
        tx.commit()?;
    }
    let report = LedgerWorkspaceReport {
        backfilled_attempts: assignments.len(),
        orphan_event_ids: orphan_events(conn, &ledger, workspace_id)?,
        legacy_trials_unknown: unknown_legacy_families(conn)?,
    };
    Ok(BoundLedger { ledger, report })
}

/// A pre-P05 paused run may have queued jobs but no attempt rows. Record
/// those historical candidates before the resume transaction creates rows.
pub fn register_missing_lineage(
    conn: &Connection,
    ledger: &TrialLedger,
    workspace_id: &str,
    lineage: &RunLineage,
) -> AppResult<(BTreeMap<String, String>, LedgerBinding)> {
    require_current(ledger, conn)?;
    let mut groups: BTreeMap<Option<String>, Vec<(String, TrialEventInput)>> = BTreeMap::new();
    for (hypothesis, attempt) in lineage {
        let linked: Option<Option<String>> = conn
            .query_row(
                "SELECT trial_event_id FROM research_attempts WHERE attempt_key = ?1",
                [&attempt.attempt_key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(linked) = linked {
            let id = linked.ok_or_else(|| ledger_error("trial_not_registered"))?;
            if !ledger.contains_event(&id).map_err(ledger_error)? {
                return Err(ledger_error(
                    "trial_not_registered: event missing from registry",
                ));
            }
            continue;
        }
        let dataset_hash = attempt
            .input_fingerprint
            .get("datasetHash")
            .and_then(Value::as_str)
            .ok_or_else(|| ledger_error("dataset hash missing from resumed lineage"))?;
        let strategy_hash = attempt
            .input_fingerprint
            .get("strategyHash")
            .and_then(Value::as_str)
            .ok_or_else(|| ledger_error("strategy hash missing from resumed lineage"))?;
        let snapshot = unique_snapshot(conn, attempt.dataset_id, dataset_hash)?;
        let event = TrialEventInput {
            kind: TrialKind::Legacy,
            origin: TrialOrigin::Legacy {
                attempt_key: attempt.attempt_key.clone(),
            },
            hypothesis_hash: Some(hypothesis.hash()?),
            strategy_hash: Some(strategy_hash.into()),
            dataset_hash: Some(dataset_hash.into()),
            snapshot_id: snapshot.as_ref().map(|(_, id)| id.clone()),
            split_hash: None,
            seeds_hash: attempt
                .input_fingerprint
                .get("seeds")
                .map(hash_value)
                .transpose()?,
            engine_fingerprint_hash: Some(hash_value(&attempt.engine_fingerprint)?),
            benchmark_id: None,
            benchmark_params_hash: None,
            reproduction_of: None,
            benchmark_evidence: None,
        };
        groups
            .entry(snapshot.map(|(instrument, _)| instrument))
            .or_default()
            .push((attempt.attempt_key.clone(), event));
    }
    let mut registered = BTreeMap::new();
    for (instrument_id, members) in groups {
        let result = ledger
            .register_batch(&TrialBatchInput {
                workspace_id: workspace_id.into(),
                instrument_id,
                tests_per_trial: 1,
                events: members.iter().map(|(_, event)| event.clone()).collect(),
            })
            .map_err(ledger_error)?;
        for ((key, _), id) in members.into_iter().zip(result.event_ids) {
            registered.insert(key, id);
        }
    }
    let head = require_current(ledger, conn)?;
    Ok((registered, head))
}

#[derive(Clone)]
struct LegacyAttempt {
    id: i64,
    key: String,
    hypothesis_hash: String,
    strategy_hash: String,
    dataset_hash: String,
    snapshot: Option<(String, String)>,
    input: Value,
    engine: Value,
    run_config: Option<Value>,
}

fn recover_legacy_split(attempt: &LegacyAttempt) -> AppResult<Option<String>> {
    let Some(raw) = &attempt.run_config else {
        return Ok(None);
    };
    if attempt.input.get("configHash").and_then(Value::as_str) != Some(hash_value(raw)?.as_str()) {
        return Ok(None);
    }
    let cores = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1) as f64;
    let Ok(config) = alpha_factor_forge::discovery_core::config::parse_discovery_config(raw, cores)
    else {
        return Ok(None);
    };
    Ok(Some(hash_value(&serde_json::json!({
        "split": config.contracts.split,
        "embargo": config.contracts.embargo,
    }))?))
}

/// A dataset can have several snapshots over time. Without a unique frozen
/// link on the attempt, guessing one would assign the wrong trial family.
pub fn unique_snapshot(
    conn: &Connection,
    dataset_id: i64,
    dataset_hash: &str,
) -> AppResult<Option<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT instrument_id, snapshot_id FROM market_snapshots
         WHERE dataset_id = ?1 AND dataset_hash = ?2 ORDER BY snapshot_id",
    )?;
    let rows = stmt
        .query_map(params![dataset_id, dataset_hash], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(if rows.len() == 1 {
        Some(rows[0].clone())
    } else {
        None
    })
}

fn hash_value(value: &Value) -> AppResult<String> {
    Ok(sha256_hex(&canonical_json(value)?))
}

fn backfill(
    conn: &Connection,
    ledger: &TrialLedger,
    workspace_id: &str,
) -> AppResult<Vec<(i64, String)>> {
    let attempts = {
        let mut stmt = conn.prepare(
            "SELECT a.id, a.attempt_key, h.hypothesis_hash, s.strategy_hash,
                    d.dataset_hash, a.dataset_id, a.input_fingerprint_json,
                    a.engine_fingerprint_json, r.config_json
             FROM research_attempts a JOIN hypotheses h ON h.id = a.hypothesis_id
             JOIN strategy_def s ON s.id = a.strategy_id
             JOIN datasets d ON d.id = a.dataset_id
             LEFT JOIN discovery_runs r ON r.id = a.discovery_run_id
             WHERE a.trial_event_id IS NULL ORDER BY a.id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, Option<String>>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut out = Vec::new();
        for (
            id,
            key,
            hypothesis_hash,
            strategy_hash,
            dataset_hash,
            dataset_id,
            input,
            engine,
            run_config,
        ) in rows
        {
            out.push(LegacyAttempt {
                id,
                key,
                hypothesis_hash,
                strategy_hash,
                snapshot: unique_snapshot(conn, dataset_id, &dataset_hash)?,
                dataset_hash,
                input: serde_json::from_str(&input)?,
                engine: serde_json::from_str(&engine)?,
                run_config: run_config.and_then(|text| serde_json::from_str(&text).ok()),
            });
        }
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM research_attempts WHERE trial_event_id IS NULL",
            [],
            |r| r.get(0),
        )?;
        if out.len() as i64 != total {
            return Err(ledger_error(
                "legacy_history_broken: an attempt has missing identity rows",
            ));
        }
        out
    };
    let mut families: BTreeMap<Option<String>, Vec<LegacyAttempt>> = BTreeMap::new();
    for attempt in attempts {
        families
            .entry(
                attempt
                    .snapshot
                    .as_ref()
                    .map(|(instrument, _)| instrument.clone()),
            )
            .or_default()
            .push(attempt);
    }
    let mut assigned = Vec::new();
    for (instrument_id, family) in families {
        let events = family
            .iter()
            .map(|attempt| {
                Ok(TrialEventInput {
                    kind: TrialKind::Legacy,
                    origin: TrialOrigin::Legacy {
                        attempt_key: attempt.key.clone(),
                    },
                    hypothesis_hash: Some(attempt.hypothesis_hash.clone()),
                    strategy_hash: Some(attempt.strategy_hash.clone()),
                    dataset_hash: Some(attempt.dataset_hash.clone()),
                    snapshot_id: attempt.snapshot.as_ref().map(|(_, id)| id.clone()),
                    split_hash: recover_legacy_split(attempt)?,
                    seeds_hash: attempt.input.get("seeds").map(hash_value).transpose()?,
                    engine_fingerprint_hash: Some(hash_value(&attempt.engine)?),
                    benchmark_id: None,
                    benchmark_params_hash: None,
                    reproduction_of: None,
                    benchmark_evidence: None,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let result = ledger
            .register_batch(&TrialBatchInput {
                workspace_id: workspace_id.into(),
                instrument_id,
                tests_per_trial: 1,
                events,
            })
            .map_err(ledger_error)?;
        assigned.extend(
            family
                .iter()
                .zip(result.event_ids)
                .map(|(attempt, event)| (attempt.id, event)),
        );
    }
    Ok(assigned)
}

fn orphan_events(
    conn: &Connection,
    ledger: &TrialLedger,
    workspace_id: &str,
) -> AppResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT trial_event_id FROM research_attempts WHERE trial_event_id IS NOT NULL")?;
    let bound: BTreeSet<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(ledger
        .workspace_event_ids(workspace_id)
        .map_err(ledger_error)?
        .into_iter()
        .filter(|id| !bound.contains(id))
        .collect())
}

fn unknown_legacy_families(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT v.dataset_id, d.dataset_hash FROM validation_records v
         JOIN datasets d ON d.id = v.dataset_id
         WHERE NOT EXISTS (
           SELECT 1 FROM research_attempts a
           WHERE a.strategy_id = v.strategy_id AND a.dataset_id = v.dataset_id
             AND a.discovery_run_id IS v.discovery_run_id
         ) ORDER BY v.dataset_id",
    )?;
    let datasets = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut families = BTreeSet::new();
    for (id, hash) in datasets {
        if let Some((instrument, _)) = unique_snapshot(conn, id, &hash)? {
            families.insert(super::trial_ledger::family_id_for(&instrument).map_err(ledger_error)?);
        }
    }
    Ok(families.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn legacy_split_is_recovered_only_from_the_frozen_run_config() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/rs-core/runner-config-v1.json"
        ))
        .unwrap();
        let raw = fixture["enumerationCases"][0]["input"].clone();
        let config_hash = hash_value(&raw).unwrap();
        let attempt = LegacyAttempt {
            id: 1,
            key: "old".into(),
            hypothesis_hash: "h".into(),
            strategy_hash: "s".into(),
            dataset_hash: "d".into(),
            snapshot: None,
            input: serde_json::json!({"configHash": config_hash}),
            engine: serde_json::json!({}),
            run_config: Some(raw),
        };
        assert!(recover_legacy_split(&attempt).unwrap().is_some());
        let mut changed = attempt;
        changed.input["configHash"] = serde_json::json!("different");
        assert_eq!(recover_legacy_split(&changed).unwrap(), None);
    }

    #[test]
    fn terminal_legacy_attempt_is_backfilled_once_and_its_event_link_is_frozen() {
        let root = std::env::temp_dir().join(format!(
            "aff-ledger-workspace-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let registry = registry_dir(&root).unwrap();
        let mut conn = crate::db::open_at(&root.join(crate::db::DB_FILE_NAME)).unwrap();
        conn.execute_batch(
            "INSERT INTO datasets (exchange, symbol, interval, start_time, end_time, source, dataset_hash)
             VALUES ('t','T','1h',1,2,'import','dataset-content-v2:legacy');
             INSERT INTO strategy_def (name, type, original_definition_json, source, strategy_hash)
             VALUES ('s','params','{}','manual','strategy-v2:legacy');
             INSERT INTO hypotheses (hypothesis_hash, version, source, mechanism, applicability_json,
                                     failure_modes, strategy_hash, content_json)
             VALUES ('h','hypothesis-v1','manual','m','{}','f','strategy-v2:legacy','{}');
             INSERT INTO research_attempts (attempt_key, hypothesis_id, strategy_id, dataset_id,
                                            status, input_fingerprint_json, engine_fingerprint_json)
             VALUES ('old-attempt',1,1,1,'completed','{}','{}');
             INSERT INTO market_calendars (calendar_id, version, kind, timezone, content_hash, definition_json)
             VALUES ('cal','v1','continuous','UTC','calendar-hash','{}');
             INSERT INTO market_instruments (instrument_id, revision, content_hash, version, market,
                 venue, symbol, base, quote, asset_type, session_calendar_id, timezone,
                 suspensions_json, source_capabilities_json, content_json)
             VALUES ('crypto:binance:BTCUSDT',1,'instrument-hash','v1','crypto','binance',
                 'BTCUSDT','BTC','USDT','spot-crypto','cal','UTC','[]','{}','{}');
             INSERT INTO market_snapshots (snapshot_id, version, instrument_row_id, instrument_id,
                 interval, dataset_id, dataset_hash, price_basis, calendar_id, kind, status,
                 as_of, coverage_json, content_json)
             VALUES ('snapshot-one','v1',1,'crypto:binance:BTCUSDT','1h',1,
                 'dataset-content-v2:legacy','raw','cal','historical','ok',2,'{}','{}');
             INSERT INTO strategy_def (name, type, original_definition_json, source, strategy_hash)
             VALUES ('pre-P05','params','{}','manual','strategy-v2:pre-p05');
             INSERT INTO validation_records (strategy_id, dataset_id, record_version,
                 gate_passed, record_json)
             VALUES (2,1,'validation-record-v1',0,'{}');"
        ).unwrap();
        let id = crate::db::runtime_ledger::workspace_id(&conn).unwrap();
        let first = adopt(&mut conn, &root, &id).unwrap();
        assert_eq!(first.report.backfilled_attempts, 1);
        assert!(first.report.orphan_event_ids.is_empty());
        assert_eq!(
            first.report.legacy_trials_unknown,
            vec![crate::research::trial_ledger::family_id_for("crypto:binance:BTCUSDT").unwrap()]
        );
        let event: String = conn
            .query_row(
                "SELECT trial_event_id FROM research_attempts WHERE attempt_key = 'old-attempt'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(first.ledger.contains_event(&event).unwrap());
        assert!(conn.execute(
            "UPDATE research_attempts SET trial_event_id = 'changed' WHERE attempt_key = 'old-attempt'", []
        ).is_err());
        assert!(conn.execute(
            "UPDATE research_attempts SET status = 'submitted' WHERE attempt_key = 'old-attempt'", []
        ).is_err());
        conn.execute_batch(
            "INSERT INTO discovery_runs (id, name, status, config_json, started_at)
             VALUES (2, 'unregistered', 'running', '{}', datetime('now'));
             INSERT INTO discovery_jobs (discovery_run_id, candidate_index, strategy_id, dataset_id, segment, status)
             VALUES (2, 0, 1, 1, 'train', 'queued'), (2, 0, 1, 1, 'validation', 'queued');
             INSERT INTO research_attempts (attempt_key, hypothesis_id, strategy_id, dataset_id,
                                            discovery_run_id, candidate_index, status,
                                            input_fingerprint_json, engine_fingerprint_json)
             VALUES ('unregistered',1,1,1,2,0,'submitted','{}','{}');"
        ).unwrap();
        let refused = crate::db::discovery::claim_candidate_jobs_with_attempt(&conn, None, 2, 0)
            .expect_err("a bound workspace cannot claim an unregistered attempt");
        assert!(
            refused.to_string().contains("trial_not_registered"),
            "{refused}"
        );
        let queued: i64 = conn.query_row(
            "SELECT COUNT(*) FROM discovery_jobs WHERE discovery_run_id = 2 AND status = 'queued'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(queued, 2, "claim rollback keeps both jobs queued");
        conn.execute_batch(
            "INSERT INTO discovery_runs (id, name, status, config_json)
             VALUES (3, 'cannot enqueue without ledger', 'idle', '{}');",
        )
        .unwrap();
        let enqueue = crate::db::discovery::start_discovery_run_bound(
            &mut conn,
            None,
            3,
            &[crate::db::discovery::CandidateJobSpec {
                candidate_index: 0,
                strategy_id: 1,
                dataset_id: 1,
            }],
            None,
            None,
        )
        .expect_err("a bound workspace cannot enqueue without event IDs");
        assert!(
            enqueue.to_string().contains("trial_not_registered"),
            "{enqueue}"
        );
        let jobs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM discovery_jobs WHERE discovery_run_id = 3",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(jobs, 0);
        let orphan_batch = TrialBatchInput {
            workspace_id: id.clone(),
            instrument_id: None,
            tests_per_trial: 1,
            events: vec![TrialEventInput {
                kind: TrialKind::Legacy,
                origin: TrialOrigin::Legacy {
                    attempt_key: "orphan-attempt".into(),
                },
                hypothesis_hash: Some("h".into()),
                strategy_hash: Some("strategy-v2:legacy".into()),
                dataset_hash: Some("dataset-content-v2:legacy".into()),
                snapshot_id: None,
                split_hash: None,
                seeds_hash: None,
                engine_fingerprint_hash: Some(hash_value(&serde_json::json!({})).unwrap()),
                benchmark_id: None,
                benchmark_params_hash: None,
                reproduction_of: None,
                benchmark_evidence: None,
            }],
        };
        let orphan = first.ledger.register_batch(&orphan_batch).unwrap();
        let replay = first.ledger.register_batch(&orphan_batch).unwrap();
        assert!(replay.replayed);
        assert_eq!(orphan.event_ids, replay.event_ids);
        drop(first);
        let second = adopt(&mut conn, &root, &id).unwrap();
        assert_eq!(second.report.backfilled_attempts, 0);
        assert_eq!(second.report.orphan_event_ids, orphan.event_ids);
        drop(second);
        drop(conn);
        let registry_file = registry.join(super::super::trial_ledger::REGISTRY_FILE_NAME);
        std::fs::remove_file(&registry_file).unwrap();
        let mut conn = crate::db::open_at(&root.join(crate::db::DB_FILE_NAME)).unwrap();
        let error = adopt(&mut conn, &root, &id)
            .err()
            .expect("bound registry is missing");
        assert!(error.to_string().contains("registry_missing"), "{error}");
        assert!(
            !registry_file.exists(),
            "a missing bound registry is never recreated"
        );
        drop(conn);
        std::fs::remove_dir_all(registry).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
