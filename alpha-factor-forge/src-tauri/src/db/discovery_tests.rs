//! RUNNER-STORE-001 store behaviour: state machine, the atomic candidate
//! commit, crash recovery, and idempotency.
//!
//! `status = 'done'` is the runner's only checkpoint, so a partially applied
//! assessment would make a resumed run skip work that never actually landed.
//!
//! Reading order matters here. EVERY guard in the store rejects BEFORE writing
//! anything, so a test that triggers a guard and then asserts "nothing was
//! written" passes whether or not the rollback works. The single rollback
//! proof is `a_failure_at_the_last_write_rolls_back_jobs_lifecycle_and_progress`,
//! which injects its failure at the LAST write in the transaction; it is
//! mutation-verified against a commit-on-drop transaction. Guard tests are
//! named so they cannot be mistaken for atomicity evidence.

use rusqlite::{params, Connection};

use super::discovery::*;
use crate::db::repositories::{
    insert_validation_record_for_run, BacktestSummary, TradeRow, ValidationRecordRow,
};

const BENCH: &str = r#"{"version":"bench-record-v1","benchmarks":[],"randomEntry":{}}"#;

fn mem_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable foreign keys");
    crate::db::apply_migrations(&conn).expect("apply migrations");
    conn
}

/// One dataset plus `count` distinct candidate strategies.
fn parents(conn: &Connection, count: usize) -> (i64, Vec<i64>) {
    conn.execute(
        "INSERT INTO datasets
            (exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash)
         VALUES ('test','BTCUSDT','1h',1,10,10,'test','dataset-content-v2:store-test')",
        [],
    )
    .unwrap();
    let dataset_id = conn.last_insert_rowid();
    let mut strategies = Vec::new();
    for index in 0..count {
        conn.execute(
            "INSERT INTO strategy_def
                (name, type, original_definition_json, source, strategy_hash, lifecycle)
             VALUES (?1, 'params', '{}', 'manual', ?2, 'candidate')",
            params![format!("s{index}"), format!("strategy-v2:{index:064}")],
        )
        .unwrap();
        strategies.push(conn.last_insert_rowid());
    }
    (dataset_id, strategies)
}

fn summary(strategy_id: i64, dataset_id: i64, segment: &str) -> BacktestSummary {
    BacktestSummary {
        id: None,
        strategy_id,
        dataset_id,
        segment: segment.into(),
        start_time: 1,
        end_time: 10,
        net_return: Some(0.1),
        cagr: None,
        max_drawdown: None,
        sharpe: None,
        sortino: None,
        calmar: None,
        win_rate: None,
        trade_count: Some(1),
        profit_factor: None,
        avg_trade_return: None,
        median_trade_return: None,
        exposure: None,
        turnover: None,
        largest_win: None,
        largest_loss: None,
        consecutive_losses: None,
        gate_passed: None,
        score: None,
        score_breakdown_json: None,
        benchmark_result_json: None,
        created_at: None,
    }
}

/// A bundle the PERSIST-001 validator accepts, carrying the given verdict.
fn bundle(
    strategy_id: i64,
    dataset_id: i64,
    passed: bool,
    score: f64,
) -> (BacktestSummary, BacktestSummary, ValidationRecordRow) {
    let train = summary(strategy_id, dataset_id, "train");
    let mut validation = summary(strategy_id, dataset_id, "validation");
    validation.gate_passed = Some(passed);
    validation.benchmark_result_json = Some(BENCH.into());
    let breakdown = format!("{{\"score\":{score}}}");
    let record_json = if passed {
        validation.score = Some(score);
        validation.score_breakdown_json = Some(breakdown.clone());
        format!(
            "{{\"version\":\"validation-record-v1\",\"strategyId\":{strategy_id},\
             \"datasetId\":{dataset_id},\"gatePassed\":true,\"score\":{breakdown},\
             \"benchmark\":{BENCH}}}"
        )
    } else {
        format!(
            "{{\"version\":\"validation-record-v1\",\"strategyId\":{strategy_id},\
             \"datasetId\":{dataset_id},\"gatePassed\":false,\"score\":null,\
             \"benchmark\":{BENCH}}}"
        )
    };
    let record = ValidationRecordRow {
        id: None,
        strategy_id,
        dataset_id,
        record_version: "validation-record-v1".into(),
        gate_passed: passed,
        score: if passed { Some(score) } else { None },
        record_json,
        created_at: None,
    };
    (train, validation, record)
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

fn lifecycle(conn: &Connection, strategy_id: i64) -> String {
    conn.query_row(
        "SELECT lifecycle FROM strategy_def WHERE id = ?1",
        [strategy_id],
        |r| r.get(0),
    )
    .unwrap()
}

fn started_run(conn: &mut Connection, dataset_id: i64, strategies: &[i64]) -> i64 {
    let run_id = create_discovery_run(conn, "run", "{}").unwrap();
    let specs: Vec<CandidateJobSpec> = strategies
        .iter()
        .enumerate()
        .map(|(index, strategy_id)| CandidateJobSpec {
            candidate_index: index as i64,
            strategy_id: *strategy_id,
            dataset_id,
        })
        .collect();
    start_discovery_run(conn, run_id, &specs).unwrap();
    run_id
}

const NO_TRADES: &[TradeRow] = &[];

fn commit(
    conn: &mut Connection,
    run_id: i64,
    candidate_index: i64,
    bundle: &(BacktestSummary, BacktestSummary, ValidationRecordRow),
    progress_json: Option<&str>,
) -> crate::error::AppResult<i64> {
    commit_candidate_assessment(
        conn,
        &CandidateAssessment {
            run_id,
            candidate_index,
            train_summary: &bundle.0,
            train_trades: NO_TRADES,
            validation_summary: &bundle.1,
            validation_trades: NO_TRADES,
            record: &bundle.2,
            progress_json,
        },
    )
}

// ---------- run lifecycle ----------

#[test]
fn starting_a_run_queues_both_segment_rows_per_candidate() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    let jobs = list_discovery_jobs(&conn, run_id).unwrap();
    assert_eq!(jobs.len(), 4, "two candidates x train+validation");
    assert!(jobs.iter().all(|j| j.status == JobStatus::Queued));
    // A Test row cannot exist: Segment has no Test variant to construct.
    assert_eq!(
        jobs.iter().filter(|j| j.segment == Segment::Train).count(),
        2
    );
    assert_eq!(
        jobs.iter()
            .filter(|j| j.segment == Segment::Validation)
            .count(),
        2
    );
    let run = get_discovery_run(&conn, run_id).unwrap();
    assert_eq!(run.status, RunStatus::Running);
    assert!(run.started_at.is_some());
}

#[test]
fn only_one_non_terminal_run_may_exist_globally() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let first = started_run(&mut conn, dataset_id, &strategies);

    // A second idle draft is fine — idle holds no slot.
    let second = create_discovery_run(&conn, "second", "{}").unwrap();
    let specs = [CandidateJobSpec {
        candidate_index: 0,
        strategy_id: strategies[0],
        dataset_id,
    }];
    assert!(
        start_discovery_run(&mut conn, second, &specs).is_err(),
        "a second running run must be refused"
    );

    // Pausing keeps the slot occupied.
    transition_run(&conn, first, RunStatus::Paused).unwrap();
    assert!(start_discovery_run(&mut conn, second, &specs).is_err());

    // Only a terminal first run frees the slot.
    transition_run(&conn, first, RunStatus::Cancelled).unwrap();
    start_discovery_run(&mut conn, second, &specs).unwrap();
    assert_eq!(
        active_discovery_run(&conn).unwrap().map(|r| r.id),
        Some(second)
    );
}

#[test]
fn a_refused_start_leaves_no_partial_job_set() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    started_run(&mut conn, dataset_id, &strategies);
    let blocked = create_discovery_run(&conn, "blocked", "{}").unwrap();

    let specs = [CandidateJobSpec {
        candidate_index: 0,
        strategy_id: strategies[0],
        dataset_id,
    }];
    assert!(start_discovery_run(&mut conn, blocked, &specs).is_err());
    assert_eq!(
        list_discovery_jobs(&conn, blocked).unwrap().len(),
        0,
        "the whole start rolled back"
    );
    assert_eq!(
        get_discovery_run(&conn, blocked).unwrap().status,
        RunStatus::Idle
    );
}

#[test]
fn completion_is_reachable_only_through_the_deriving_path() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    // `running -> completed` is absent from the transition table, so a caller
    // cannot mark a run complete while skipping the best-strategy derivation.
    assert!(
        transition_run(&conn, run_id, RunStatus::Completed).is_err(),
        "completion must go through complete_discovery_run"
    );
    assert_eq!(
        get_discovery_run(&conn, run_id).unwrap().status,
        RunStatus::Running
    );
}

#[test]
fn a_run_with_unfinished_jobs_cannot_be_completed() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    // Only candidate 0 finished; candidate 1 is still queued.
    commit(
        &mut conn,
        run_id,
        0,
        &bundle(strategies[0], dataset_id, true, 1.5),
        None,
    )
    .unwrap();
    let blocked = complete_discovery_run(&mut conn, run_id);
    assert!(blocked.is_err(), "a draining queue blocks completion");
    assert_eq!(
        get_discovery_run(&conn, run_id).unwrap().status,
        RunStatus::Running,
        "the refused completion left the run running"
    );

    // Skipping the remainder (cancellation bookkeeping) drains the queue.
    skip_remaining_jobs(&conn, run_id).unwrap();
    assert_eq!(
        complete_discovery_run(&mut conn, run_id).unwrap(),
        Some(strategies[0])
    );
}

#[test]
fn illegal_state_transitions_are_refused() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    assert!(transition_run(&conn, run_id, RunStatus::Idle).is_err());
    transition_run(&conn, run_id, RunStatus::Paused).unwrap();
    // paused reaches a terminal state only via running or cancel.
    assert!(transition_run(&conn, run_id, RunStatus::Completed).is_err());
    assert!(transition_run(&conn, run_id, RunStatus::Failed).is_err());
    transition_run(&conn, run_id, RunStatus::Running).unwrap();
    transition_run(&conn, run_id, RunStatus::Cancelled).unwrap();
    for target in [RunStatus::Running, RunStatus::Paused, RunStatus::Completed] {
        assert!(
            transition_run(&conn, run_id, target).is_err(),
            "a terminal run must not move to {}",
            target.as_str()
        );
    }
}

#[test]
fn duplicate_candidate_segment_jobs_are_impossible() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    let duplicate = conn.execute(
        "INSERT INTO discovery_jobs
            (discovery_run_id, candidate_index, strategy_id, dataset_id, segment, status)
         VALUES (?1, 0, ?2, ?3, 'train', 'queued')",
        params![run_id, strategies[0], dataset_id],
    );
    assert!(duplicate.is_err(), "(run, candidate, segment) is unique");

    // The same candidate index in a DIFFERENT run stays legal.
    transition_run(&conn, run_id, RunStatus::Cancelled).unwrap();
    let other = create_discovery_run(&conn, "other", "{}").unwrap();
    start_discovery_run(
        &mut conn,
        other,
        &[CandidateJobSpec {
            candidate_index: 0,
            strategy_id: strategies[0],
            dataset_id,
        }],
    )
    .unwrap();
}

#[test]
fn start_rejects_empty_duplicate_or_negative_candidate_indexes() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = create_discovery_run(&conn, "run", "{}").unwrap();
    let spec = |index: i64| CandidateJobSpec {
        candidate_index: index,
        strategy_id: strategies[0],
        dataset_id,
    };
    assert!(start_discovery_run(&mut conn, run_id, &[]).is_err());
    assert!(start_discovery_run(&mut conn, run_id, &[spec(-1)]).is_err());
    assert!(start_discovery_run(&mut conn, run_id, &[spec(0), spec(0)]).is_err());
    assert_eq!(list_discovery_jobs(&conn, run_id).unwrap().len(), 0);
}

// ---------- the atomic candidate commit ----------

#[test]
fn committing_a_candidate_writes_the_whole_assessment() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);
    let b = bundle(strategies[0], dataset_id, true, 1.5);

    let record_id = commit(&mut conn, run_id, 0, &b, Some("{\"done\":1}")).unwrap();

    assert!(record_id > 0);
    assert_eq!(count(&conn, "backtest_summary"), 2);
    assert_eq!(count(&conn, "validation_records"), 1);

    let jobs = list_discovery_jobs(&conn, run_id).unwrap();
    assert!(jobs.iter().all(|j| j.status == JobStatus::Done));
    assert!(
        jobs.iter().all(|j| j.result_id.is_some()),
        "each job points at its own summary"
    );
    assert_ne!(jobs[0].result_id, jobs[1].result_id);

    let linked: i64 = conn
        .query_row(
            "SELECT discovery_run_id FROM validation_records WHERE id = ?1",
            [record_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(linked, run_id);
    assert_eq!(lifecycle(&conn, strategies[0]), "validated");
    assert_eq!(
        get_discovery_run(&conn, run_id).unwrap().progress_json,
        Some("{\"done\":1}".into())
    );
}

#[test]
fn a_broken_job_pair_is_rejected_before_anything_is_written() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    // Remove one paired row. This fails the identity pre-check, which runs
    // BEFORE any write — so this proves the guard, NOT the rollback. The
    // rollback is proven by `a_failure_after_the_writes_rolls_everything_back`.
    conn.execute(
        "DELETE FROM discovery_jobs
         WHERE discovery_run_id = ?1 AND candidate_index = 0 AND segment = 'validation'",
        [run_id],
    )
    .unwrap();

    let b = bundle(strategies[0], dataset_id, true, 1.5);
    assert!(commit(&mut conn, run_id, 0, &b, Some("{\"done\":1}")).is_err());

    assert_eq!(count(&conn, "backtest_summary"), 0);
    assert_eq!(count(&conn, "validation_records"), 0);
    assert_eq!(lifecycle(&conn, strategies[0]), "candidate", "no promotion");
    let remaining = list_discovery_jobs(&conn, run_id).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].status, JobStatus::Queued, "no half checkpoint");
}

/// A re-commit of an already-`done` candidate is rejected by the job-status
/// pre-check, which runs BEFORE any write.
///
/// This WAS the rollback proof, back when the earliest rejection for this
/// input was the per-run uniqueness rule on the record INSERT. Adding that
/// pre-check (PR #74 review) moved the failure earlier and silently downgraded
/// this into a guard test — caught only by re-running the commit-on-drop
/// mutation, under which it now passes. The rollback proof is
/// `a_failure_at_the_last_write_rolls_back_jobs_lifecycle_and_progress`.
#[test]
fn re_committing_a_done_candidate_is_rejected_before_any_write() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    let first = bundle(strategies[0], dataset_id, true, 1.5);
    let first_trades = [TradeRow {
        entry_time: 1,
        exit_time: 2,
        side: "LONG".into(),
        entry_price: 10.0,
        exit_price: 11.0,
        pnl: 1.0,
        pnl_pct: 0.1,
        reason: Some("first".into()),
    }];
    commit_candidate_assessment(
        &mut conn,
        &CandidateAssessment {
            run_id,
            candidate_index: 0,
            train_summary: &first.0,
            train_trades: &first_trades,
            validation_summary: &first.1,
            validation_trades: &first_trades,
            record: &first.2,
            progress_json: Some("{\"done\":1}"),
        },
    )
    .unwrap();

    // A second assessment of the same candidate with DIFFERENT payloads. The
    // summaries upsert and the trades are replaced before the record insert
    // violates the uniqueness rule.
    let mut second = bundle(strategies[0], dataset_id, true, 9.9);
    second.0.net_return = Some(-42.0);
    second.1.net_return = Some(-42.0);
    let second_trades = [TradeRow {
        entry_time: 5,
        exit_time: 6,
        side: "SHORT".into(),
        entry_price: 20.0,
        exit_price: 19.0,
        pnl: -1.0,
        pnl_pct: -0.05,
        reason: Some("second".into()),
    }];
    let outcome = commit_candidate_assessment(
        &mut conn,
        &CandidateAssessment {
            run_id,
            candidate_index: 0,
            train_summary: &second.0,
            train_trades: &second_trades,
            validation_summary: &second.1,
            validation_trades: &second_trades,
            record: &second.2,
            progress_json: Some("{\"done\":2}"),
        },
    );
    assert!(outcome.is_err(), "the second assessment must be refused");

    // Everything the failed transaction touched must show the FIRST values.
    let net: f64 = conn
        .query_row(
            "SELECT net_return FROM backtest_summary WHERE segment = 'train'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(net, 0.1, "the stored summary is untouched");

    let reason: String = conn
        .query_row("SELECT reason FROM trades LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(reason, "first", "the stored trades are untouched");
    assert_eq!(
        count(&conn, "trades"),
        2,
        "one trade per summary, unchanged"
    );
    assert_eq!(count(&conn, "validation_records"), 1);
    assert_eq!(
        get_discovery_run(&conn, run_id).unwrap().progress_json,
        Some("{\"done\":1}".into())
    );
}

/// THE rollback proof. Every guard in this module rejects BEFORE writing, so
/// the only way to exercise the transaction's rollback is to fail at the LAST
/// write — here a trigger on the progress update. Everything written earlier
/// (summaries, trades, record, BOTH job rows, and the lifecycle promotion)
/// must be undone. Mutation-verified: setting the transaction's drop behaviour
/// to `Commit` makes this test fail.
#[test]
fn a_failure_at_the_last_write_rolls_back_jobs_lifecycle_and_progress() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    conn.execute_batch(
        "CREATE TRIGGER boom BEFORE UPDATE OF progress_json ON discovery_runs
         WHEN NEW.progress_json = '{\"boom\":1}'
         BEGIN SELECT RAISE(ABORT, 'test-injected failure'); END;",
    )
    .unwrap();

    let b = bundle(strategies[0], dataset_id, true, 1.5);
    assert!(commit(&mut conn, run_id, 0, &b, Some("{\"boom\":1}")).is_err());

    assert_eq!(count(&conn, "backtest_summary"), 0, "summaries rolled back");
    assert_eq!(count(&conn, "validation_records"), 0, "record rolled back");
    assert_eq!(
        lifecycle(&conn, strategies[0]),
        "candidate",
        "the lifecycle promotion rolled back"
    );
    let run = get_discovery_run(&conn, run_id).unwrap();
    assert_eq!(run.progress_json, None, "progress rolled back");
    for job in list_discovery_jobs(&conn, run_id).unwrap() {
        assert_eq!(job.status, JobStatus::Queued, "job rows rolled back");
        assert_eq!(job.result_id, None);
    }
}

#[test]
fn a_candidate_cannot_be_committed_twice() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);
    let b = bundle(strategies[0], dataset_id, true, 1.5);

    commit(&mut conn, run_id, 0, &b, None).unwrap();
    assert!(
        commit(&mut conn, run_id, 0, &b, None).is_err(),
        "the per-run uniqueness rule blocks a second assessment"
    );
    assert_eq!(count(&conn, "validation_records"), 1);
    assert_eq!(count(&conn, "backtest_summary"), 2, "no extra summary rows");
}

#[test]
fn commits_are_refused_outside_a_running_run_or_for_a_foreign_candidate() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    // Candidate 0's rows name strategies[0]; another strategy is a different
    // candidate and must not be committed under this index.
    let foreign = bundle(strategies[1], dataset_id, true, 1.5);
    assert!(commit(&mut conn, run_id, 0, &foreign, None).is_err());

    // An unknown candidate index has no queued pair.
    let own = bundle(strategies[0], dataset_id, true, 1.5);
    assert!(commit(&mut conn, run_id, 99, &own, None).is_err());

    // A paused run must not absorb results.
    transition_run(&conn, run_id, RunStatus::Paused).unwrap();
    assert!(commit(&mut conn, run_id, 0, &own, None).is_err());
    assert_eq!(count(&conn, "validation_records"), 0);
}

// ---------- D6 lifecycle + promotion ----------

#[test]
fn lifecycle_follows_the_gate_and_never_demotes_a_validated_strategy() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    let failing = bundle(strategies[1], dataset_id, false, 0.0);
    commit(&mut conn, run_id, 1, &failing, None).unwrap();
    assert_eq!(lifecycle(&conn, strategies[1]), "rejected");

    let passing = bundle(strategies[0], dataset_id, true, 2.0);
    commit(&mut conn, run_id, 0, &passing, None).unwrap();
    assert_eq!(lifecycle(&conn, strategies[0]), "validated");

    // A later FAILING assessment in another run must not demote it (D6).
    transition_run(&conn, run_id, RunStatus::Cancelled).unwrap();
    let second = create_discovery_run(&conn, "second", "{}").unwrap();
    start_discovery_run(
        &mut conn,
        second,
        &[CandidateJobSpec {
            candidate_index: 0,
            strategy_id: strategies[0],
            dataset_id,
        }],
    )
    .unwrap();
    let later_failure = bundle(strategies[0], dataset_id, false, 0.0);
    commit(&mut conn, second, 0, &later_failure, None).unwrap();
    assert_eq!(
        lifecycle(&conn, strategies[0]),
        "validated",
        "a validated strategy is never demoted by a later failure"
    );
}

#[test]
fn a_rejected_strategy_is_promoted_when_it_later_passes() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let first = started_run(&mut conn, dataset_id, &strategies);
    commit(
        &mut conn,
        first,
        0,
        &bundle(strategies[0], dataset_id, false, 0.0),
        None,
    )
    .unwrap();
    assert_eq!(lifecycle(&conn, strategies[0]), "rejected");

    transition_run(&conn, first, RunStatus::Cancelled).unwrap();
    let second = create_discovery_run(&conn, "second", "{}").unwrap();
    start_discovery_run(
        &mut conn,
        second,
        &[CandidateJobSpec {
            candidate_index: 0,
            strategy_id: strategies[0],
            dataset_id,
        }],
    )
    .unwrap();
    commit(
        &mut conn,
        second,
        0,
        &bundle(strategies[0], dataset_id, true, 1.0),
        None,
    )
    .unwrap();
    assert_eq!(lifecycle(&conn, strategies[0]), "validated");
}

#[test]
fn completion_picks_the_highest_scoring_gate_passer() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 3);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    for (index, (passed, score)) in [(true, 1.0), (false, 0.0), (true, 3.0)]
        .into_iter()
        .enumerate()
    {
        let b = bundle(strategies[index], dataset_id, passed, score);
        commit(&mut conn, run_id, index as i64, &b, None).unwrap();
    }

    let best = complete_discovery_run(&mut conn, run_id).unwrap();
    assert_eq!(best, Some(strategies[2]), "highest finite score wins");
    let run = get_discovery_run(&conn, run_id).unwrap();
    assert_eq!(run.status, RunStatus::Completed);
    assert_eq!(run.best_strategy_id, Some(strategies[2]));
    assert!(run.completed_at.is_some());
}

#[test]
fn completion_breaks_score_ties_by_candidate_index() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);
    for (index, strategy_id) in strategies.iter().enumerate() {
        let b = bundle(*strategy_id, dataset_id, true, 2.0);
        commit(&mut conn, run_id, index as i64, &b, None).unwrap();
    }
    assert_eq!(
        complete_discovery_run(&mut conn, run_id).unwrap(),
        Some(strategies[0]),
        "an equal score resolves to the lower candidate index"
    );
}

#[test]
fn completion_records_no_winner_when_nothing_passes() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);
    commit(
        &mut conn,
        run_id,
        0,
        &bundle(strategies[0], dataset_id, false, 0.0),
        None,
    )
    .unwrap();
    assert_eq!(complete_discovery_run(&mut conn, run_id).unwrap(), None);
    assert_eq!(
        get_discovery_run(&conn, run_id).unwrap().best_strategy_id,
        None
    );
}

// ---------- crash recovery ----------

#[test]
fn recovery_pauses_orphaned_runs_and_requeues_only_in_flight_jobs() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    // Candidate 0 committed before the crash; candidate 1 was in flight.
    commit(
        &mut conn,
        run_id,
        0,
        &bundle(strategies[0], dataset_id, true, 1.5),
        None,
    )
    .unwrap();
    conn.execute(
        "UPDATE discovery_jobs SET status = 'running'
         WHERE discovery_run_id = ?1 AND candidate_index = 1",
        [run_id],
    )
    .unwrap();

    let report = recover_orphaned_runs(&mut conn).unwrap();
    assert_eq!(report.runs_paused, 1);
    assert_eq!(
        report.jobs_requeued, 2,
        "the in-flight pair returns to queued"
    );

    let run = get_discovery_run(&conn, run_id).unwrap();
    assert_eq!(
        run.status,
        RunStatus::Paused,
        "CPU work is never auto-resumed"
    );
    for job in list_discovery_jobs(&conn, run_id).unwrap() {
        let expected = if job.candidate_index == 0 {
            JobStatus::Done
        } else {
            JobStatus::Queued
        };
        assert_eq!(job.status, expected, "done checkpoints survive recovery");
    }

    // Idempotent: a second pass finds nothing to do.
    assert_eq!(
        recover_orphaned_runs(&mut conn).unwrap(),
        RecoveryReport::default()
    );
}

#[test]
fn recovery_leaves_paused_and_terminal_runs_alone() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let paused = started_run(&mut conn, dataset_id, &strategies);
    transition_run(&conn, paused, RunStatus::Paused).unwrap();

    assert_eq!(
        recover_orphaned_runs(&mut conn).unwrap(),
        RecoveryReport::default(),
        "an already-paused run is not an orphan"
    );
    assert_eq!(
        get_discovery_run(&conn, paused).unwrap().status,
        RunStatus::Paused
    );
}

// ---------- cancellation / failure bookkeeping ----------

#[test]
fn skipping_and_failing_never_rewrite_a_done_checkpoint() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);
    commit(
        &mut conn,
        run_id,
        0,
        &bundle(strategies[0], dataset_id, true, 1.5),
        None,
    )
    .unwrap();

    assert_eq!(skip_remaining_jobs(&conn, run_id).unwrap(), 2);
    let jobs = list_discovery_jobs(&conn, run_id).unwrap();
    assert!(jobs
        .iter()
        .filter(|j| j.candidate_index == 0)
        .all(|j| j.status == JobStatus::Done));
    assert!(jobs
        .iter()
        .filter(|j| j.candidate_index == 1)
        .all(|j| j.status == JobStatus::Skipped));

    assert!(
        fail_candidate_jobs(&conn, run_id, 1, "  ").is_err(),
        "a failure must carry evidence"
    );
    assert_eq!(
        fail_candidate_jobs(&conn, run_id, 0, "engine crash").unwrap(),
        0,
        "a done candidate is not retroactively failed"
    );
    assert!(list_discovery_jobs(&conn, run_id)
        .unwrap()
        .iter()
        .filter(|j| j.candidate_index == 0)
        .all(|j| j.status == JobStatus::Done && j.error_message.is_none()));
}

#[test]
fn a_terminal_job_is_never_resurrected_or_stripped_of_its_evidence() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 2);
    let run_id = started_run(&mut conn, dataset_id, &strategies);

    // Candidate 1 failed with evidence. A late-arriving result must NOT flip
    // it to done and erase why it failed.
    assert_eq!(
        fail_candidate_jobs(&conn, run_id, 1, "engine crash").unwrap(),
        2
    );
    let late = bundle(strategies[1], dataset_id, true, 1.5);
    assert!(
        commit(&mut conn, run_id, 1, &late, None).is_err(),
        "a failed candidate must not be completed by a late result"
    );
    for job in list_discovery_jobs(&conn, run_id)
        .unwrap()
        .iter()
        .filter(|j| j.candidate_index == 1)
    {
        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error_message.as_deref(), Some("engine crash"));
    }
    assert_eq!(count(&conn, "validation_records"), 0);

    // A second failure must not overwrite the first one's evidence either.
    assert_eq!(
        fail_candidate_jobs(&conn, run_id, 1, "a later, different reason").unwrap(),
        0
    );
    assert!(list_discovery_jobs(&conn, run_id)
        .unwrap()
        .iter()
        .filter(|j| j.candidate_index == 1)
        .all(|j| j.error_message.as_deref() == Some("engine crash")));

    // Skipped is terminal too.
    skip_remaining_jobs(&conn, run_id).unwrap();
    let skipped = bundle(strategies[0], dataset_id, true, 1.5);
    assert!(
        commit(&mut conn, run_id, 0, &skipped, None).is_err(),
        "a skipped candidate must not be completed"
    );
}

#[test]
fn two_candidates_may_not_share_one_strategy_and_dataset() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = create_discovery_run(&conn, "run", "{}").unwrap();

    // Enumeration deduplicates by strategy hash, so this can only come from a
    // caller building the queue wrong. It must fail at enqueue, not after the
    // second candidate's backtests have already run.
    let duplicated = [
        CandidateJobSpec {
            candidate_index: 0,
            strategy_id: strategies[0],
            dataset_id,
        },
        CandidateJobSpec {
            candidate_index: 1,
            strategy_id: strategies[0],
            dataset_id,
        },
    ];
    assert!(start_discovery_run(&mut conn, run_id, &duplicated).is_err());
    assert_eq!(list_discovery_jobs(&conn, run_id).unwrap().len(), 0);
}

// ---------- schema integrity ----------

/// The job-status pre-check now rejects a re-commit before the record INSERT
/// is ever reached, so no store-level test exercises 0003's per-run
/// uniqueness index any more. It is still the last line of defence if a job
/// row is manipulated directly, so it is asserted at the schema level here.
#[test]
fn per_run_assessment_uniqueness_is_enforced_by_the_schema() {
    let conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    conn.execute(
        "INSERT INTO discovery_runs (name, status, config_json) VALUES ('r','idle','{}')",
        [],
    )
    .unwrap();
    let run_id = conn.last_insert_rowid();
    let (_, _, record) = bundle(strategies[0], dataset_id, true, 1.5);

    insert_validation_record_for_run(&conn, &record, Some(run_id)).unwrap();
    assert!(
        insert_validation_record_for_run(&conn, &record, Some(run_id)).is_err(),
        "a run may hold at most one assessment per (strategy, dataset)"
    );
    assert_eq!(count(&conn, "validation_records"), 1);
}

#[test]
fn a_run_that_produced_records_cannot_be_deleted() {
    let mut conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let run_id = started_run(&mut conn, dataset_id, &strategies);
    commit(
        &mut conn,
        run_id,
        0,
        &bundle(strategies[0], dataset_id, true, 1.5),
        None,
    )
    .unwrap();

    // ON DELETE RESTRICT: nulling the linkage would silently erase which run
    // produced an immutable audit record.
    assert!(
        conn.execute("DELETE FROM discovery_runs WHERE id = ?1", [run_id])
            .is_err(),
        "a run with validation records must not be deletable"
    );
    let linked: Option<i64> = conn
        .query_row(
            "SELECT discovery_run_id FROM validation_records LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(linked, Some(run_id), "provenance survives");
}

#[test]
fn a_failed_migration_records_no_version_and_leaves_no_partial_schema() {
    // The DDL and its version row must commit together. Simulate a migration
    // whose second statement fails: without one transaction the first ALTER
    // would survive unrecorded, and every retry would then die on
    // "duplicate column name".
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE probe (id INTEGER PRIMARY KEY);",
    )
    .unwrap();

    let broken = "ALTER TABLE probe ADD COLUMN added INTEGER; \
                  ALTER TABLE nonexistent ADD COLUMN boom INTEGER;";
    let tx = conn.unchecked_transaction().unwrap();
    let outcome = tx.execute_batch(broken);
    assert!(outcome.is_err());
    drop(tx);

    assert!(
        conn.prepare("SELECT added FROM probe").is_err(),
        "the partial DDL rolled back, so a retry is not stuck on a duplicate column"
    );
    let recorded: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        recorded, 0,
        "no version was recorded for a failed migration"
    );
}

// ---------- PERSIST-001 compatibility ----------

#[test]
fn manual_records_stay_outside_the_per_run_uniqueness_rule() {
    let conn = mem_db();
    let (dataset_id, strategies) = parents(&conn, 1);
    let (_, _, record) = bundle(strategies[0], dataset_id, true, 1.5);
    // Two manual assessments of the same strategy/dataset remain legal: the
    // 0003 index is partial on a non-null run id.
    for _ in 0..2 {
        insert_validation_record_for_run(&conn, &record, None).unwrap();
    }
    assert_eq!(count(&conn, "validation_records"), 2);
}
