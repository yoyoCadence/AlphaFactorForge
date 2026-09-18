//! P03b: the versioned command envelope driving a REAL discovery run — one
//! `discovery.start` per request id however often it is sent, and a ledger a
//! reconnecting reader can page from a cursor after taking a snapshot.
use super::*;
use crate::db::ownership::{acquire, HolderKind};
use crate::db::runtime_ledger;
use crate::discovery_runner::ControlPhase;
use crate::runtime::commands::{Dispatcher, ErrorCode, InFlightRequests, COMMAND_PROTOCOL_VERSION, EVENT_PROTOCOL_VERSION};

fn owned_dispatcher(db: &SharedDb) -> (Dispatcher, String, Arc<RecordingSink>) {
    let (epoch, workspace) = {
        let mut conn = db.lock().unwrap();
        let epoch = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap().epoch;
        (epoch, runtime_ledger::workspace_id(&conn).unwrap())
    };
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let dispatcher = Dispatcher::new(
        db.clone(),
        DiscoveryRunner::with_epoch(epoch),
        epoch,
        workspace.clone(),
        sink.clone(),
        Arc::new(InFlightRequests::default()),
    );
    (dispatcher, workspace, sink)
}

fn envelope(workspace: &str, request: &str, command: &str, payload: Value) -> Value {
    json!({
        "protocolVersion": COMMAND_PROTOCOL_VERSION,
        "workspaceId": workspace,
        "requestId": request,
        "command": command,
        "payload": payload,
    })
}

#[test]
fn one_request_id_starts_exactly_one_run_and_the_ledger_replays_it_from_a_cursor() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 1);
    let (dispatcher, workspace, sink) = owned_dispatcher(&db);

    // 重複命令: the same envelope three times starts ONE run and answers the
    // same run id each time; a reused id with another payload is refused.
    let first = dispatcher
        .dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()))
        .expect("first start");
    let run_id = first["runId"].as_i64().expect("run id");
    let second = dispatcher
        .dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()))
        .expect("replayed start");
    assert_eq!(second, first, "the replay is the stored first outcome");
    let conflict = dispatcher.dispatch(envelope(
        &workspace,
        "start-1",
        "discovery.start",
        runner_config(dataset_id, &dataset_hash, 2),
    ));
    assert_eq!(conflict.unwrap_err().code, ErrorCode::DuplicateRequest);
    {
        let conn = db.lock().unwrap();
        let runs: i64 = conn.query_row("SELECT COUNT(*) FROM discovery_runs", [], |r| r.get(0)).unwrap();
        assert_eq!(runs, 1, "one run for one request id");
    }

    // Let the run finish (through the envelope's own progress read), then
    // check the ledger holds exactly what the host saw.
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let snapshot = dispatcher
            .dispatch(envelope(&workspace, "p", "discovery.progress", json!({ "runId": run_id })))
            .expect("progress read");
        if snapshot["run"]["status"] == "completed" {
            break;
        }
        assert!(Instant::now() < deadline, "run did not complete: {snapshot}");
        thread::sleep(Duration::from_millis(5));
    }
    sink.wait_for(|event| matches!(event, DiscoveryEvent::Done(_)));
    sink.assert_all_observed_after_commit();

    // 重連: snapshot first, then page the ledger from a cursor. Every event the
    // host received is in the ledger, in order, under this epoch, and the
    // per-run sequence inside the payload is strictly increasing.
    let active = dispatcher
        .dispatch(envelope(&workspace, "a", "discovery.active", json!({})))
        .expect("active read");
    assert_eq!(active["run"], Value::Null, "the run is terminal, so no active run");
    let snapshot_version = active["stateVersion"].as_i64().unwrap();
    assert!(snapshot_version > 0);

    let mut cursor = 0;
    let mut collected = Vec::new();
    loop {
        let page = dispatcher
            .dispatch(envelope(&workspace, "e", "events.read", json!({ "afterEventId": cursor, "limit": 2 })))
            .expect("events page");
        let events = page["events"].as_array().unwrap().clone();
        if events.is_empty() {
            assert_eq!(page["lastEventId"].as_i64().unwrap(), cursor, "cursor reached the end");
            break;
        }
        cursor = events.last().unwrap()["eventId"].as_i64().unwrap();
        collected.extend(events);
    }
    let host_seen = sink.snapshot().len();
    assert_eq!(collected.len(), host_seen, "every forwarded event was ledgered");
    assert!(collected.windows(2).all(|pair| pair[0]["eventId"].as_i64() < pair[1]["eventId"].as_i64()));
    assert!(collected
        .windows(2)
        .all(|pair| pair[0]["payload"]["sequence"].as_u64() < pair[1]["payload"]["sequence"].as_u64()));
    for event in &collected {
        assert_eq!(event["protocolVersion"], EVENT_PROTOCOL_VERSION);
        assert_eq!(event["eventVersion"], DISCOVERY_EVENT_VERSION);
        assert_eq!(event["entity"]["kind"], "discovery_run");
        assert_eq!(event["entity"]["id"], run_id.to_string());
        assert_eq!(event["epoch"], 1);
    }
    let last = collected.last().unwrap();
    assert_eq!(last["channel"], DISCOVERY_DONE_EVENT);
    assert_eq!(last["payload"]["status"], "completed");
    assert!(collected.iter().any(|event| event["channel"] == DISCOVERY_RESULT_EVENT));
}

// ---------- 2026-09-17 acceptance regressions (R1, R2, R3) ----------

/// A dispatcher whose runner's executor waits for permits, so a run can be
/// held in flight and paused deterministically.
fn gated_dispatcher(
    db: &SharedDb,
) -> (Dispatcher, DiscoveryRunner, String, Arc<RecordingSink>, Arc<PermitGate>, mpsc::Receiver<i64>) {
    let (epoch, workspace) = {
        let mut conn = db.lock().unwrap();
        let epoch = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap().epoch;
        (epoch, runtime_ledger::workspace_id(&conn).unwrap())
    };
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = DiscoveryRunner {
        executor: Arc::new(PermittedProductionExecutor { started: started_tx, gate: gate.clone() }),
        ..DiscoveryRunner::with_epoch(epoch)
    };
    let dispatcher = Dispatcher::new(
        db.clone(),
        runner.clone(),
        epoch,
        workspace.clone(),
        sink.clone(),
        Arc::new(InFlightRequests::default()),
    );
    // The clone shares the coordinator controls, so tests can observe phases.
    (dispatcher, runner, workspace, sink, gate, started_rx)
}

/// Start a two-candidate run through the envelope and pause it after the
/// first candidate drains, leaving a paused run with no coordinator — the
/// state every lifecycle command below acts on.
fn paused_run(
    dispatcher: &Dispatcher,
    runner: &DiscoveryRunner,
    db: &SharedDb,
    workspace: &str,
    config: Value,
    gate: &PermitGate,
    started_rx: &mpsc::Receiver<i64>,
) -> i64 {
    let started = dispatcher
        .dispatch(envelope(workspace, "start", "discovery.start", config))
        .expect("start");
    let run_id = started["runId"].as_i64().unwrap();
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).expect("first candidate in flight"), 0);

    let (pause_tx, pause_rx) = mpsc::channel();
    thread::scope(|scope| {
        scope.spawn(|| {
            let _ = pause_tx.send(dispatcher.dispatch(envelope(workspace, "pause", "discovery.pause", json!({ "runId": run_id }))));
        });
        wait_for_phase(runner, run_id, ControlPhase::PauseRequested);
        gate.release();
        pause_rx.recv_timeout(TEST_TIMEOUT).expect("pause answered").expect("pause succeeds after drain");
    });
    wait_for_coordinator_exit(runner, run_id);
    assert_eq!(runner.progress(db, run_id).unwrap().status, RunStatus::Paused);
    run_id
}

fn deny(db: &SharedDb, name: &str, on: &str) {
    db.lock()
        .unwrap()
        .execute_batch(&format!("CREATE TEMP TRIGGER {name} BEFORE {on} BEGIN SELECT RAISE(ABORT, '{name}'); END;"))
        .unwrap();
}

fn allow(db: &SharedDb, name: &str) {
    db.lock().unwrap().execute_batch(&format!("DROP TRIGGER {name};")).unwrap();
}

fn done_events(sink: &RecordingSink) -> usize {
    sink.snapshot().iter().filter(|e| matches!(e.event, DiscoveryEvent::Done(_))).count()
}

/// R1: the receipt fails after the cancel committed. The first answer is the
/// success (the effect row is durable, so it is recoverable); the retry
/// recovers exactly that answer from the effect, executes nothing, and
/// repairs the receipt.
#[test]
fn a_success_whose_receipt_failed_is_recovered_on_retry_without_executing_again() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, sink, gate, started_rx) = gated_dispatcher(&db);
    let run_id = paused_run(&dispatcher, &runner, &db, &workspace, runner_config(dataset_id, &dataset_hash, 2), &gate, &started_rx);
    let done_before = done_events(&sink);

    deny(&db, "deny_receipt", "UPDATE ON command_requests");
    let first = dispatcher
        .dispatch(envelope(&workspace, "cancel-1", "discovery.cancel", json!({ "runId": run_id })))
        .expect("the cancel itself committed");
    assert_eq!(first, json!({ "runId": run_id }));
    {
        let conn = db.lock().unwrap();
        assert_eq!(runtime_ledger::read_request(&conn, "cancel-1").unwrap().unwrap().status, "pending", "receipt was refused");
        let effect = discovery::read_request_outcome(&conn, "cancel-1").unwrap().expect("outcome row committed with the cancel");
        assert_eq!(effect.stage, crate::db::discovery::OutcomeStage::Accepted);
        assert_eq!(effect.outcome_json.as_deref(), Some(r#"{"runId":1}"#));
        assert_eq!((effect.run_id, effect.command.as_str()), (run_id, "discovery.cancel"));
        assert_eq!(discovery::get_discovery_run(&conn, run_id).unwrap().status, RunStatus::Cancelled);
    }
    assert_eq!(done_events(&sink), done_before + 1, "the host saw exactly one Done");
    allow(&db, "deny_receipt");

    // Fault cleared: the same request id replays the first success — from
    // the effect, not by cancelling again — and the receipt is repaired.
    let again = dispatcher
        .dispatch(envelope(&workspace, "cancel-1", "discovery.cancel", json!({ "runId": run_id })))
        .expect("recovered");
    assert_eq!(again, first);
    assert_eq!(done_events(&sink), done_before + 1, "nothing was executed again");
    {
        let conn = db.lock().unwrap();
        let stored = runtime_ledger::read_request(&conn, "cancel-1").unwrap().unwrap();
        assert_eq!(stored.status, "succeeded");
        assert_eq!(stored.result_json.as_deref(), Some(r#"{"runId":1}"#));
    }
    // And a third time, now a plain replay of the repaired receipt.
    let third = dispatcher
        .dispatch(envelope(&workspace, "cancel-1", "discovery.cancel", json!({ "runId": run_id })))
        .expect("replayed");
    assert_eq!(third, first);
    assert_eq!(done_events(&sink), done_before + 1);

    // A NEW request to cancel the now-cancelled run is a genuine, final failure.
    let new_request = dispatcher.dispatch(envelope(&workspace, "cancel-2", "discovery.cancel", json!({ "runId": run_id })));
    let error = new_request.unwrap_err();
    assert!(!error.retryable, "{error:?}");
}

/// R2: a ledger append fails after the cancel committed. A reader that took
/// its snapshot before the cancel and then follows the cursor gets no event —
/// but `stateVersion` moved and the gap marker is set, so it re-snapshots and
/// sees the terminal state.
#[test]
fn a_reader_following_the_cursor_detects_a_missed_event_and_resnapshots() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, sink, gate, started_rx) = gated_dispatcher(&db);
    let run_id = paused_run(&dispatcher, &runner, &db, &workspace, runner_config(dataset_id, &dataset_hash, 2), &gate, &started_rx);

    // The reader's starting point: snapshot (paused) + cursor at the ledger end.
    let snapshot = dispatcher.dispatch(envelope(&workspace, "r", "discovery.active", json!({}))).unwrap();
    assert_eq!(snapshot["run"]["status"], "paused");
    let snapshot_version = snapshot["stateVersion"].as_i64().unwrap();
    let page = dispatcher.dispatch(envelope(&workspace, "r", "events.read", json!({}))).unwrap();
    let cursor = page["lastEventId"].as_i64().unwrap();
    assert!(page["events"].as_array().unwrap().iter().all(|e| e["eventId"].as_i64().unwrap() <= cursor));
    assert_eq!(page["ledgerGap"], Value::Null);
    assert_eq!(page["ledgerDegraded"], false);

    deny(&db, "deny_events", "INSERT ON runtime_events");
    let done_before = done_events(&sink);
    dispatcher
        .dispatch(envelope(&workspace, "cancel-1", "discovery.cancel", json!({ "runId": run_id })))
        .expect("cancel commits regardless of the ledger");
    assert_eq!(done_events(&sink), done_before + 1, "the host still received the Done");
    allow(&db, "deny_events");

    // Cursor read: no event arrived, but the state moved and the gap is marked.
    let page = dispatcher.dispatch(envelope(&workspace, "r", "events.read", json!({ "afterEventId": cursor }))).unwrap();
    assert_eq!(page["events"].as_array().unwrap().len(), 0, "the Done never reached the ledger");
    assert_eq!(page["lastEventId"], cursor);
    let version_now = page["stateVersion"].as_i64().unwrap();
    assert!(version_now > snapshot_version, "state moved past the snapshot: {version_now} > {snapshot_version}");
    assert_eq!(page["ledgerDegraded"], true);
    let gap = &page["ledgerGap"];
    assert_eq!(gap["epoch"], 1);
    assert!(gap["stateVersion"].as_i64().unwrap() > snapshot_version);

    // The reader's rule — state moved with no events → re-snapshot — catches up.
    let resnapshot = dispatcher.dispatch(envelope(&workspace, "r", "discovery.progress", json!({ "runId": run_id }))).unwrap();
    assert_eq!(resnapshot["run"]["status"], "cancelled");
    assert!(resnapshot["stateVersion"].as_i64().unwrap() >= version_now);
}

/// R3: a failure that gets recorded is final for its request id, and says
/// so; the same id replays it verbatim even after the cause is gone, and only
/// a new request id makes progress.
#[test]
fn a_recorded_busy_is_final_and_not_labelled_retryable() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, _sink, gate, started_rx) = gated_dispatcher(&db);
    let run_id = paused_run(&dispatcher, &runner, &db, &workspace, runner_config(dataset_id, &dataset_hash, 2), &gate, &started_rx);

    // The paused run holds the single active slot: a second start is refused.
    let config = runner_config(dataset_id, &dataset_hash, 1);
    let refused = dispatcher.dispatch(envelope(&workspace, "start-2", "discovery.start", config.clone()));
    let error = refused.unwrap_err();
    assert_eq!(error.code, ErrorCode::Busy, "{error:?}");
    assert!(!error.retryable, "recorded → final, never retryable with the same id");
    assert!(error.message.contains("new requestId"));

    // The cause goes away; the same id still replays the recorded refusal.
    dispatcher
        .dispatch(envelope(&workspace, "cancel-1", "discovery.cancel", json!({ "runId": run_id })))
        .expect("cancel frees the slot");
    let replayed = dispatcher.dispatch(envelope(&workspace, "start-2", "discovery.start", config.clone()));
    assert_eq!(replayed.unwrap_err(), error);
    {
        let conn = db.lock().unwrap();
        let runs: i64 = conn.query_row("SELECT COUNT(*) FROM discovery_runs", [], |r| r.get(0)).unwrap();
        assert_eq!(runs, 1, "the replay started nothing");
    }

    // A new request id makes progress.
    let fresh = dispatcher
        .dispatch(envelope(&workspace, "start-3", "discovery.start", config))
        .expect("a new request id starts a run");
    let run_2 = fresh["runId"].as_i64().unwrap();
    assert_eq!(run_2, 2);
    // Let it finish so no coordinator outlives the test.
    for _ in 0..4 {
        gate.release();
    }
    let deadline = Instant::now() + TEST_TIMEOUT;
    while runner.progress(&db, run_2).unwrap().status != RunStatus::Completed {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    wait_for_coordinator_exit(&runner, run_2);
}

// ---------- 2026-09-17 re-review regressions (R1: the FIRST outcome, immutably) ----------

/// Like `PermittedProductionExecutor`, but the candidate FAILS once released:
/// the run is accepted, then fails later for reasons that have nothing to do
/// with the command that started it.
struct FailAfterPermit {
    started: mpsc::Sender<i64>,
    gate: Arc<PermitGate>,
}

impl CandidateExecutor for FailAfterPermit {
    fn execute(&self, work: &CandidateWork) -> Result<CandidateExecutionOutput, String> {
        self.started
            .send(work.candidate.index)
            .map_err(|_| "test executor start receiver dropped".to_string())?;
        self.gate.acquire()?;
        Err("review: candidate execution failed".into())
    }
}

fn dispatcher_with_executor(
    db: &SharedDb,
    executor: Arc<dyn CandidateExecutor>,
) -> (Dispatcher, DiscoveryRunner, String, Arc<RecordingSink>) {
    let (epoch, workspace) = {
        let mut conn = db.lock().unwrap();
        let epoch = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap().epoch;
        (epoch, runtime_ledger::workspace_id(&conn).unwrap())
    };
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let runner = DiscoveryRunner { executor, ..DiscoveryRunner::with_epoch(epoch) };
    let dispatcher = Dispatcher::new(
        db.clone(),
        runner.clone(),
        epoch,
        workspace.clone(),
        sink.clone(),
        Arc::new(InFlightRequests::default()),
    );
    (dispatcher, runner, workspace, sink)
}

fn wait_for_run_status(runner: &DiscoveryRunner, db: &SharedDb, run_id: i64, expected: RunStatus) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    while runner.progress(db, run_id).unwrap().status != expected {
        assert!(Instant::now() < deadline, "run {run_id} never reached {expected:?}");
        thread::sleep(Duration::from_millis(5));
    }
}

fn runs_count(db: &SharedDb) -> i64 {
    db.lock().unwrap().query_row("SELECT COUNT(*) FROM discovery_runs", [], |r| r.get(0)).unwrap()
}

fn receipt_status(db: &SharedDb, request_id: &str) -> Option<String> {
    runtime_ledger::read_request(&db.lock().unwrap(), request_id).unwrap().map(|r| r.status)
}

/// Re-review case 1: a `start` that was accepted stays accepted even though a
/// worker failed the run afterwards; the retry replays `Ok`, never the later
/// failure, and starts nothing.
#[test]
fn an_accepted_start_replays_ok_even_after_the_run_fails_later() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let (dispatcher, runner, workspace, _sink) =
        dispatcher_with_executor(&db, Arc::new(FailAfterPermit { started: started_tx, gate: gate.clone() }));
    let config = runner_config(dataset_id, &dataset_hash, 1);

    deny(&db, "deny_receipt", "UPDATE ON command_requests");
    let first = dispatcher.dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone())).expect("accepted");
    assert_eq!(first, json!({ "runId": 1 }));
    assert_eq!(receipt_status(&db, "start-1").as_deref(), Some("pending"), "receipt refused");
    {
        let conn = db.lock().unwrap();
        let row = discovery::read_request_outcome(&conn, "start-1").unwrap().expect("outcome row");
        assert_eq!(row.stage, crate::db::discovery::OutcomeStage::Accepted);
    }

    // Now the worker fails the run.
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);
    gate.release();
    wait_for_run_status(&runner, &db, 1, RunStatus::Failed);
    wait_for_coordinator_exit(&runner, 1);
    allow(&db, "deny_receipt");

    // The first answer, not the run's later fate.
    let retry = dispatcher.dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone())).expect("replayed");
    assert_eq!(retry, first);
    assert_eq!(runs_count(&db), 1, "nothing started again");
    assert_eq!(receipt_status(&db, "start-1").as_deref(), Some("succeeded"), "receipt repaired");
    assert_eq!(runner.progress(&db, 1).unwrap().status, RunStatus::Failed, "the run's own fate is untouched");
}

/// Re-review case 2: a `start` that created its run row but failed to queue
/// the jobs answered with that failure; the retry replays the failure —
/// never a success for an idle, job-less run — and starts nothing.
#[test]
fn a_start_that_failed_after_creating_its_run_replays_the_failure_not_a_success() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, _sink, _gate, _started_rx) = gated_dispatcher(&db);
    let config = runner_config(dataset_id, &dataset_hash, 1);

    deny(&db, "deny_jobs", "INSERT ON discovery_jobs");
    deny(&db, "deny_receipt", "UPDATE ON command_requests");
    let first = dispatcher.dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()));
    let first_error = first.unwrap_err();
    assert_eq!(first_error.code, ErrorCode::Validation, "{first_error:?}");
    assert!(first_error.message.contains("deny_jobs"), "{first_error:?}");
    assert!(!first_error.retryable, "recorded with the run row: final");
    {
        let conn = db.lock().unwrap();
        let row = discovery::read_request_outcome(&conn, "start-1").unwrap().expect("rejection recorded");
        assert_eq!(row.stage, crate::db::discovery::OutcomeStage::Rejected);
        assert_eq!(discovery::get_discovery_run(&conn, 1).unwrap().status, RunStatus::Idle);
        assert_eq!(discovery::list_discovery_jobs(&conn, 1).unwrap().len(), 0);
    }
    allow(&db, "deny_jobs");
    allow(&db, "deny_receipt");

    let retry = dispatcher.dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()));
    assert_eq!(retry.unwrap_err(), first_error, "the first failure, verbatim");
    assert_eq!(runs_count(&db), 1);
    assert_eq!(runner.progress(&db, 1).unwrap().status, RunStatus::Idle, "still idle, still no coordinator");
    assert_eq!(receipt_status(&db, "start-1").as_deref(), Some("failed"));
}

/// The crash-shaped variant of case 2: the run row exists but neither an
/// acceptance nor a rejection could be recorded. The retry reports that
/// fact — it does not turn it into a success and does not start again.
#[test]
fn a_start_that_only_began_replays_an_honest_failure() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, _sink, _gate, _started_rx) = gated_dispatcher(&db);
    let config = runner_config(dataset_id, &dataset_hash, 1);

    deny(&db, "deny_jobs", "INSERT ON discovery_jobs");
    deny(&db, "deny_outcome_progress", "UPDATE ON request_outcomes");
    deny(&db, "deny_receipt", "UPDATE ON command_requests");
    let first = dispatcher.dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()));
    let first_error = first.unwrap_err();
    assert_eq!(first_error.code, ErrorCode::Validation);
    // Nothing durable but the `begun` row could be written, so this is NOT
    // final: the caller is told the outcome was not recorded.
    assert!(first_error.retryable, "{first_error:?}");
    assert!(first_error.message.contains("could not be recorded"));
    {
        let conn = db.lock().unwrap();
        let row = discovery::read_request_outcome(&conn, "start-1").unwrap().expect("begun row");
        assert_eq!(row.stage, crate::db::discovery::OutcomeStage::Begun);
    }
    for name in ["deny_jobs", "deny_outcome_progress", "deny_receipt"] {
        allow(&db, name);
    }

    let retry = dispatcher.dispatch(envelope(&workspace, "start-1", "discovery.start", config.clone()));
    let error = retry.unwrap_err();
    assert_eq!(error.code, ErrorCode::Validation);
    assert!(error.message.contains("never completed its admission"), "{error:?}");
    assert!(!error.retryable);
    assert_eq!(runs_count(&db), 1, "no second run");
    assert_eq!(runner.progress(&db, 1).unwrap().status, RunStatus::Idle);
    assert_eq!(receipt_status(&db, "start-1").as_deref(), Some("failed"), "the fact is now the receipt");
}

/// Re-review case 3: completion wins the race with a pause. `pause` answers
/// success; that acceptance is recorded with the completion, so a retry
/// replays the success instead of pausing a run that no longer has a
/// coordinator.
#[test]
fn a_pause_that_completion_won_replays_its_success() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, _sink, gate, started_rx) = gated_dispatcher(&db);
    let config = runner_config(dataset_id, &dataset_hash, 1);

    let started = dispatcher.dispatch(envelope(&workspace, "start", "discovery.start", config)).expect("start");
    let run_id = started["runId"].as_i64().unwrap();
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);

    deny(&db, "deny_receipt", "UPDATE ON command_requests");
    let (pause_tx, pause_rx) = mpsc::channel();
    let first = thread::scope(|scope| {
        scope.spawn(|| {
            let _ = pause_tx.send(dispatcher.dispatch(envelope(&workspace, "pause-1", "discovery.pause", json!({ "runId": run_id }))));
        });
        wait_for_phase(&runner, run_id, ControlPhase::PauseRequested);
        // The only candidate completes: completion wins, pause is "successful".
        gate.release();
        pause_rx.recv_timeout(TEST_TIMEOUT).expect("pause answered")
    });
    assert_eq!(first.as_ref().unwrap(), &json!({ "runId": run_id }));
    wait_for_run_status(&runner, &db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(&runner, run_id);
    assert_eq!(receipt_status(&db, "pause-1").as_deref(), Some("pending"));
    {
        let conn = db.lock().unwrap();
        let row = discovery::read_request_outcome(&conn, "pause-1").unwrap().expect("accepted with the completion");
        assert_eq!(row.stage, crate::db::discovery::OutcomeStage::Accepted);
        assert_eq!(row.command, "discovery.pause");
    }
    allow(&db, "deny_receipt");

    let retry = dispatcher.dispatch(envelope(&workspace, "pause-1", "discovery.pause", json!({ "runId": run_id })));
    assert_eq!(retry.unwrap(), first.unwrap(), "the recorded success, not a fresh pause");
    assert_eq!(receipt_status(&db, "pause-1").as_deref(), Some("succeeded"));
    assert_eq!(runner.progress(&db, run_id).unwrap().status, RunStatus::Completed);
}

/// `resume` whose checkpoint failed after the run had already switched back
/// to running: the rejection is recorded with the failure, so the retry
/// replays it and does not resume again.
#[test]
fn a_resume_that_failed_after_beginning_replays_the_failure() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, _sink, gate, started_rx) = gated_dispatcher(&db);
    let run_id = paused_run(&dispatcher, &runner, &db, &workspace, runner_config(dataset_id, &dataset_hash, 2), &gate, &started_rx);

    // The Paused -> Running transition does not touch progress_json; the
    // resumed checkpoint does, and is denied.
    db.lock()
        .unwrap()
        .execute_batch(
            "CREATE TEMP TRIGGER deny_checkpoint BEFORE UPDATE OF progress_json ON discovery_runs
             BEGIN SELECT RAISE(ABORT, 'deny_checkpoint'); END;",
        )
        .unwrap();
    deny(&db, "deny_receipt", "UPDATE ON command_requests");
    let first = dispatcher.dispatch(envelope(&workspace, "resume-1", "discovery.resume", json!({ "runId": run_id })));
    let first_error = first.unwrap_err();
    assert!(first_error.message.contains("deny_checkpoint"), "{first_error:?}");
    assert!(!first_error.retryable, "recorded with the run failure: final");
    {
        let conn = db.lock().unwrap();
        let row = discovery::read_request_outcome(&conn, "resume-1").unwrap().expect("rejected with the failure");
        assert_eq!(row.stage, crate::db::discovery::OutcomeStage::Rejected);
        assert_eq!(discovery::get_discovery_run(&conn, run_id).unwrap().status, RunStatus::Failed);
    }
    allow(&db, "deny_checkpoint");
    allow(&db, "deny_receipt");

    let retry = dispatcher.dispatch(envelope(&workspace, "resume-1", "discovery.resume", json!({ "runId": run_id })));
    assert_eq!(retry.unwrap_err(), first_error);
    assert_eq!(runner.progress(&db, run_id).unwrap().status, RunStatus::Failed, "not resumed again");
    assert_eq!(receipt_status(&db, "resume-1").as_deref(), Some("failed"));
}

// ---------- 2026-09-17 third-review regressions (H1 overlap, H2 cancel rollback) ----------

/// H1: two envelopes with the same request id arriving together execute the
/// command exactly once and both receive that one outcome — whichever wins
/// the claim executes, the other waits and replays the receipt it left.
#[test]
fn overlapping_duplicates_execute_once_and_receive_the_same_outcome() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, sink, gate, started_rx) = gated_dispatcher(&db);
    let run_id = paused_run(&dispatcher, &runner, &db, &workspace, runner_config(dataset_id, &dataset_hash, 2), &gate, &started_rx);
    let done_before = done_events(&sink);

    let barrier = std::sync::Barrier::new(2);
    let (a, b) = thread::scope(|scope| {
        let send = |label: &'static str| {
            let dispatcher = &dispatcher;
            let workspace = &workspace;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                (label, dispatcher.dispatch(envelope(workspace, "cancel-dup", "discovery.cancel", json!({ "runId": run_id }))))
            })
        };
        let a = send("a");
        let b = send("b");
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(a.1.as_ref().unwrap(), &json!({ "runId": run_id }), "{a:?}");
    assert_eq!(b.1.as_ref().unwrap(), &json!({ "runId": run_id }), "{b:?}");
    assert_eq!(done_events(&sink), done_before + 1, "the cancel ran exactly once");
    assert_eq!(receipt_status(&db, "cancel-dup").as_deref(), Some("succeeded"));
    assert_eq!(runner.progress(&db, run_id).unwrap().status, RunStatus::Cancelled);

    // And a third, later send is a plain replay of the same answer.
    let again = dispatcher.dispatch(envelope(&workspace, "cancel-dup", "discovery.cancel", json!({ "runId": run_id })));
    assert_eq!(again.unwrap(), json!({ "runId": run_id }));
    assert_eq!(done_events(&sink), done_before + 1);
}

/// H1, deterministically: A is held between its reservation and its
/// execution; B, sent meanwhile with the same id, must not get in — it waits
/// for A's claim and then replays A's outcome. (With the claim removed, B
/// would run inside the window and A would execute a second time.)
#[test]
fn a_duplicate_sent_while_the_first_is_between_reservation_and_execution_waits_and_replays() {
    use crate::runtime::commands::test_hooks::AFTER_RESERVATION;

    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, sink, gate, started_rx) = gated_dispatcher(&db);
    let run_id = paused_run(&dispatcher, &runner, &db, &workspace, runner_config(dataset_id, &dataset_hash, 2), &gate, &started_rx);
    let done_before = done_events(&sink);

    let (a_reserved_tx, a_reserved_rx) = mpsc::channel::<()>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let (b_done_tx, b_done_rx) = mpsc::channel();
    let (a, b) = thread::scope(|scope| {
        let a = scope.spawn(|| {
            AFTER_RESERVATION.with(|slot| {
                *slot.borrow_mut() = Some(Box::new(move || {
                    let _ = a_reserved_tx.send(());
                    let _ = go_rx.recv_timeout(TEST_TIMEOUT);
                }))
            });
            dispatcher.dispatch(envelope(&workspace, "cancel-dup", "discovery.cancel", json!({ "runId": run_id })))
        });
        a_reserved_rx.recv_timeout(TEST_TIMEOUT).expect("A is past its reservation");

        let b = scope.spawn(|| {
            let result = dispatcher.dispatch(envelope(&workspace, "cancel-dup", "discovery.cancel", json!({ "runId": run_id })));
            let _ = b_done_tx.send(());
            result
        });
        assert!(b_done_rx.recv_timeout(Duration::from_millis(300)).is_err(), "B must wait for A's claim");
        assert_eq!(done_events(&sink), done_before, "nothing executed while A is held");

        go_tx.send(()).unwrap();
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(a.as_ref().unwrap(), &json!({ "runId": run_id }), "A executed: {a:?}");
    assert_eq!(b.as_ref().unwrap(), &json!({ "runId": run_id }), "B replayed A: {b:?}");
    assert_eq!(done_events(&sink), done_before + 1, "the cancel ran exactly once");
    assert_eq!(receipt_status(&db, "cancel-dup").as_deref(), Some("succeeded"));
}

/// H2: a cancel whose transaction rolls back must not consume the pending
/// pause's request id. The pause is still waiting, completion later commits
/// and records its acceptance, and a retry of the pause replays that success.
#[test]
fn a_rolled_back_cancel_leaves_the_pause_request_to_the_transaction_that_answers_it() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (dispatcher, runner, workspace, _sink, gate, started_rx) = gated_dispatcher(&db);
    let config = runner_config(dataset_id, &dataset_hash, 1);

    let started = dispatcher.dispatch(envelope(&workspace, "start", "discovery.start", config)).expect("start");
    let run_id = started["runId"].as_i64().unwrap();
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);

    deny(&db, "deny_receipt", "UPDATE ON command_requests");
    let (pause_tx, pause_rx) = mpsc::channel();
    let first = thread::scope(|scope| {
        scope.spawn(|| {
            let _ = pause_tx.send(dispatcher.dispatch(envelope(&workspace, "pause-1", "discovery.pause", json!({ "runId": run_id }))));
        });
        wait_for_phase(&runner, run_id, ControlPhase::PauseRequested);

        // A cancel that cannot commit: the run stays running, the pause
        // stays waiting, and its request id must stay with it.
        db.lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TRIGGER deny_cancel BEFORE UPDATE OF status ON discovery_runs
                 WHEN NEW.status = 'cancelled' BEGIN SELECT RAISE(ABORT, 'deny_cancel'); END;",
            )
            .unwrap();
        let cancelled = dispatcher.dispatch(envelope(&workspace, "cancel-1", "discovery.cancel", json!({ "runId": run_id })));
        let cancel_error = cancelled.unwrap_err();
        assert!(cancel_error.message.contains("deny_cancel"), "{cancel_error:?}");
        assert!(pause_rx.try_recv().is_err(), "the pause is still waiting");
        assert_eq!(runner.progress(&db, run_id).unwrap().status, RunStatus::Running);
        {
            let conn = db.lock().unwrap();
            assert!(discovery::read_request_outcome(&conn, "pause-1").unwrap().is_none(), "nothing recorded for the pause yet");
            assert!(discovery::read_request_outcome(&conn, "cancel-1").unwrap().is_none(), "the rolled-back cancel left no outcome");
        }
        allow(&db, "deny_cancel");

        // Completion wins over the still-pending pause and records its success.
        gate.release();
        pause_rx.recv_timeout(TEST_TIMEOUT).expect("pause answered")
    });
    assert_eq!(first.as_ref().unwrap(), &json!({ "runId": run_id }));
    wait_for_run_status(&runner, &db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(&runner, run_id);
    {
        let conn = db.lock().unwrap();
        let row = discovery::read_request_outcome(&conn, "pause-1").unwrap().expect("pause accepted with the completion");
        assert_eq!(row.stage, crate::db::discovery::OutcomeStage::Accepted);
    }
    assert_eq!(receipt_status(&db, "pause-1").as_deref(), Some("pending"), "receipt was refused");
    allow(&db, "deny_receipt");

    let retry = dispatcher.dispatch(envelope(&workspace, "pause-1", "discovery.pause", json!({ "runId": run_id })));
    assert_eq!(retry.unwrap(), first.unwrap(), "the recorded success, not 'no active coordinator'");
    assert_eq!(receipt_status(&db, "pause-1").as_deref(), Some("succeeded"));
}
