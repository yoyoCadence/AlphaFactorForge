//! P04a: a REAL discovery run driven over the loopback control API — the
//! acceptance of plan phase P04 at the runtime level: work continues after
//! every client is gone, a returning client adopts the same run from a
//! snapshot and the ledger cursor, and a service that is told to stop
//! drains the run to a paused checkpoint the next owner can resume.
use super::*;
use crate::db::ownership::{acquire, HolderKind};
use crate::db::runtime_ledger;
use crate::runtime::commands::{Dispatcher, ErrorCode, InFlightRequests, COMMAND_PROTOCOL_VERSION};
use crate::runtime::control_api::{ControlServer, ControlToken, EventNotifier, NotifyingSink, ServiceIdentity};
use crate::runtime::control_client::ControlClient;
use crate::runtime::service;

struct Served {
    server: Arc<ControlServer>,
    token: ControlToken,
    runner: DiscoveryRunner,
    workspace: String,
    gate: Arc<PermitGate>,
    started_rx: mpsc::Receiver<i64>,
}

impl Served {
    fn client(&self) -> ControlClient {
        ControlClient::new(self.server.port(), self.token.clone())
    }

    fn envelope(&self, request: &str, command: &str, payload: Value) -> Value {
        json!({
            "protocolVersion": COMMAND_PROTOCOL_VERSION,
            "workspaceId": self.workspace,
            "requestId": request,
            "command": command,
            "payload": payload,
        })
    }
}

/// The service's wiring (`runtime::service::run`) over an in-memory
/// workspace and a gated executor: the same dispatcher, sink, and server,
/// with the run holdable at each candidate.
fn served_gated_runner(db: &SharedDb) -> Served {
    let (epoch, instance_id, workspace) = {
        let mut conn = db.lock().unwrap();
        let acquired = acquire(&mut conn, HolderKind::Service, std::process::id()).unwrap();
        (acquired.epoch, acquired.instance_id, runtime_ledger::workspace_id(&conn).unwrap())
    };
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = DiscoveryRunner {
        executor: Arc::new(PermittedProductionExecutor { started: started_tx, gate: gate.clone() }),
        ..DiscoveryRunner::with_epoch(epoch)
    };
    let notifier = Arc::new(EventNotifier::default());
    let dispatcher = Arc::new(Dispatcher::new(
        db.clone(),
        runner.clone(),
        epoch,
        workspace.clone(),
        Arc::new(NotifyingSink(notifier.clone())),
        Arc::new(InFlightRequests::default()),
    ));
    let token = ControlToken::generate().unwrap();
    let identity = ServiceIdentity {
        workspace_id: workspace.clone(),
        epoch,
        holder_kind: HolderKind::Service.as_str(),
        instance_id,
        pid: std::process::id(),
    };
    let server = ControlServer::bind(dispatcher, token.clone(), identity, notifier).unwrap();
    Served { server: Arc::new(server), token, runner, workspace, gate, started_rx }
}

fn event_channels(page: &Value) -> Vec<(&str, i64)> {
    page["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| (event["channel"].as_str().unwrap(), event["eventId"].as_i64().unwrap()))
        .collect()
}

#[test]
fn review_drain_deadline_includes_the_pause_wait() {
    let db = migrated_db();
    let (dataset_id, hash) = import_dataset(&db, &alternating_candles(240, 1_577_836_800_000));
    let served = served_gated_runner(&db);
    let run = served.client().dispatch(&served.envelope("start", "discovery.start", runner_config(dataset_id, &hash, 2)))
        .unwrap().unwrap()["runId"].as_i64().unwrap();
    served.started_rx.recv_timeout(TEST_TIMEOUT).unwrap();
    served.server.request_shutdown();
    let (tx, rx) = mpsc::channel();
    let runner = served.runner.clone();
    let server = served.server.clone();
    let drainer = thread::spawn(move || {
        let drained = service::drain(&runner, &server, Duration::from_millis(100));
        tx.send(drained).unwrap();
    });
    wait_for_phase(&served.runner, run, ControlPhase::PauseRequested);
    let before_worker = rx.recv_timeout(Duration::from_secs(1));
    // Clean up even when testing the broken implementation.
    served.gate.release();
    drainer.join().unwrap();
    wait_for_coordinator_exit(&served.runner, run);
    assert_eq!(before_worker, Ok(false), "drain deadline includes the pause wait and reports incomplete work");
}

#[test]
fn review_shutdown_waits_for_an_admitted_start_before_draining_its_coordinator() {
    let db = migrated_db();
    let (dataset_id, hash) = import_dataset(&db, &alternating_candles(240, 1_577_836_800_000));
    let served = served_gated_runner(&db);
    let (admitted_tx, admitted_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel();
    served.server.after_admission(move || {
        admitted_tx.send(()).unwrap();
        go_rx.recv_timeout(TEST_TIMEOUT).unwrap();
    });
    let client = served.client();
    let envelope = served.envelope("start", "discovery.start", runner_config(dataset_id, &hash, 2));
    let starter = thread::spawn(move || client.dispatch(&envelope).unwrap().unwrap());
    admitted_rx.recv_timeout(TEST_TIMEOUT).unwrap();
    served.server.request_shutdown();
    let (tx, rx) = mpsc::channel();
    let runner = served.runner.clone();
    let server = served.server.clone();
    let drainer = thread::spawn(move || {
        let drained = service::drain(&runner, &server, TEST_TIMEOUT);
        tx.send(drained).unwrap();
    });
    let returned_before_start = rx.recv_timeout(Duration::from_millis(300)).is_ok();
    go_tx.send(()).unwrap();
    let run = starter.join().unwrap()["runId"].as_i64().unwrap();
    // Broken drain has returned already; request a pause solely for cleanup.
    let cleanup = if returned_before_start {
        let runner = served.runner.clone();
        let db = db.clone();
        Some(thread::spawn(move || runner.pause(&db, run)))
    } else { None };
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let phase = served.runner.control(run).unwrap().map(|c| c.state.lock().unwrap().phase);
        if matches!(phase, Some(ControlPhase::PauseRequested | ControlPhase::Paused))
            || served.runner.progress(&db, run).unwrap().status == RunStatus::Paused { break; }
        assert!(Instant::now() < deadline, "shutdown must discover and pause the admitted start");
        thread::sleep(Duration::from_millis(5));
    }
    served.gate.release();
    if let Some(cleanup) = cleanup { cleanup.join().unwrap().unwrap(); }
    drainer.join().unwrap();
    wait_for_coordinator_exit(&served.runner, run);
    assert!(!returned_before_start, "an admitted request must not create a coordinator after drain has returned");
    assert!(rx.recv_timeout(TEST_TIMEOUT).unwrap(), "both the admitted command and coordinator drained");
    assert_eq!(served.runner.progress(&db, run).unwrap().status, RunStatus::Paused);
}

/// P04 acceptance, "關 UI 工作持續；重連採用同一 run": the client that started
/// the run goes away while the run is in flight; the run keeps going; a new
/// client finds the same run through `discovery.active`, catches up on the
/// ledger from zero, and then follows it live through the long-poll, which
/// is woken by the runner's own events.
#[test]
fn a_run_started_over_the_api_continues_without_its_client_and_a_new_client_adopts_it() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let served = served_gated_runner(&db);

    let run_id = {
        // The first client: start, then disappear (dropped, no more requests).
        let first = served.client();
        let started = first
            .dispatch(&served.envelope("start", "discovery.start", runner_config(dataset_id, &dataset_hash, 2)))
            .unwrap()
            .expect("start accepted");
        started["runId"].as_i64().unwrap()
    };
    assert_eq!(served.started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0, "candidate 0 in flight");

    // Nobody is connected. The run proceeds to its second candidate anyway.
    served.gate.release();
    assert_eq!(served.started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 1, "candidate 1 in flight with no client attached");

    // A new client (the desktop reopened, or the MCP adapter) reconnects:
    // snapshot first, then the ledger from its cursor.
    let second = served.client();
    let active = second.dispatch(&served.envelope("active-1", "discovery.active", json!({}))).unwrap().unwrap();
    assert_eq!(active["run"]["runId"], run_id, "the same run, not a new one");
    assert_eq!(active["run"]["status"], "running");
    assert_eq!(active["run"]["counts"]["completedCandidates"], 1);
    let snapshot_version = active["stateVersion"].as_i64().unwrap();

    let history = second.events(0, None, Duration::ZERO).unwrap();
    let channels = event_channels(&history);
    assert!(channels.iter().any(|(channel, _)| *channel == DISCOVERY_RESULT_EVENT), "candidate 0's result is in the ledger: {channels:?}");
    assert!(!channels.iter().any(|(channel, _)| *channel == DISCOVERY_DONE_EVENT), "not done yet");
    assert!(history["stateVersion"].as_i64().unwrap() >= snapshot_version);
    let cursor = history["lastEventId"].as_i64().unwrap();
    assert_eq!(channels.last().unwrap().1, cursor);

    // Follow live: the long-poll is parked until the runner emits.
    let (page_tx, page_rx) = mpsc::channel();
    let follower = {
        let client = second.clone();
        thread::spawn(move || {
            let started = Instant::now();
            let page = client.events(cursor, None, Duration::from_secs(20)).unwrap();
            let _ = page_tx.send((page, started.elapsed()));
        })
    };
    assert!(page_rx.recv_timeout(Duration::from_millis(300)).is_err(), "nothing new: the poll waits");
    served.gate.release();
    let (page, waited) = page_rx.recv_timeout(TEST_TIMEOUT).expect("the poll returns once the runner emits");
    follower.join().unwrap();
    assert!(waited < Duration::from_secs(10), "woken by the event, not by the timeout: {waited:?}");
    let live = event_channels(&page);
    assert!(live.iter().all(|(_, id)| *id > cursor), "only events after the cursor: {live:?}");
    assert!(!live.is_empty());

    // The run completes with no client action; the ledger ends with Done.
    wait_for_status(&served.runner, &db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(&served.runner, run_id);
    let rest = second.events(page["lastEventId"].as_i64().unwrap(), None, Duration::from_secs(5)).unwrap();
    let everything = second.events(cursor, None, Duration::ZERO).unwrap();
    let all = event_channels(&everything);
    let done = all.iter().filter(|(channel, _)| *channel == DISCOVERY_DONE_EVENT).count();
    assert_eq!(done, 1, "{all:?}");
    let last = everything["events"].as_array().unwrap().last().unwrap();
    assert_eq!(last["channel"], DISCOVERY_DONE_EVENT);
    assert_eq!(last["payload"]["status"], "completed");
    assert_eq!(last["payload"]["runId"], run_id);
    let tail = second.events(everything["lastEventId"].as_i64().unwrap(), None, Duration::ZERO).unwrap();
    assert!(tail["events"].as_array().unwrap().is_empty(), "nothing after the end");
    assert_eq!(rest["lastEventId"], everything["lastEventId"]);
    let final_snapshot = second.dispatch(&served.envelope("active-2", "discovery.active", json!({}))).unwrap().unwrap();
    assert_eq!(final_snapshot["run"], Value::Null, "no active run remains");
}

/// A service told to stop while a run is in flight drains it: the candidate
/// finishes, the Paused transition commits under THIS epoch, the coordinator
/// exits, and only then does `drain` return. The next owner (epoch + 1)
/// finds a paused run — not an orphan for recovery — and resumes it.
#[test]
fn a_draining_service_pauses_the_run_at_a_checkpoint_the_next_owner_resumes() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let served = served_gated_runner(&db);
    let client = served.client();

    let started = client
        .dispatch(&served.envelope("start", "discovery.start", runner_config(dataset_id, &dataset_hash, 2)))
        .unwrap()
        .expect("start accepted");
    let run_id = started["runId"].as_i64().unwrap();
    assert_eq!(served.started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);

    // The shutdown request refuses new work at once, and the drain asks the
    // coordinator to pause; it cannot finish until the in-flight candidate
    // is allowed to complete.
    client.shutdown().unwrap();
    let refused = client
        .dispatch(&served.envelope("start-2", "discovery.start", runner_config(dataset_id, &dataset_hash, 1)))
        .unwrap()
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Busy);
    let (drained_tx, drained_rx) = mpsc::channel();
    let drainer = {
        let runner = served.runner.clone();
        let server = served.server.clone();
        thread::spawn(move || {
            assert!(service::drain(&runner, &server, TEST_TIMEOUT));
            let _ = drained_tx.send(());
        })
    };
    wait_for_phase(&served.runner, run_id, ControlPhase::PauseRequested);
    assert!(drained_rx.recv_timeout(Duration::from_millis(300)).is_err(), "the drain waits for the candidate");
    // Reads still answer during the drain (a client following the stop).
    let progress = client.dispatch(&served.envelope("p", "discovery.progress", json!({ "runId": run_id }))).unwrap().unwrap();
    assert_eq!(progress["run"]["status"], "running");

    served.gate.release();
    drained_rx.recv_timeout(TEST_TIMEOUT).expect("drained once the candidate completed");
    drainer.join().unwrap();
    assert!(served.runner.active_coordinator_run_ids().unwrap().is_empty());
    assert_eq!(served.runner.progress(&db, run_id).unwrap().status, RunStatus::Paused);
    assert_eq!(served.runner.progress(&db, run_id).unwrap().counts.completed_candidates, 1);
    let Served { server, .. } = served;
    Arc::try_unwrap(server).ok().expect("no other server handles").stop();

    // The next owner: a fresh epoch, and no orphan to repair.
    let epoch = acquire(&mut db.lock().unwrap(), HolderKind::DesktopEmbedded, 1).unwrap().epoch;
    assert_eq!(epoch, 2);
    let next = DiscoveryRunner::with_epoch(epoch);
    assert_eq!(next.recover_orphans(&db).unwrap(), RecoveryReport::default(), "paused by the drain, not orphaned");
    next.resume(db.clone(), Arc::new(RecordingSink::new(db.clone())), run_id).expect("resumed by the next owner");
    wait_for_status(&next, &db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(&next, run_id);
    assert_eq!(next.progress(&db, run_id).unwrap().counts.completed_candidates, 2);
}

// ---------- P04b: the desktop's forwarder over the same API ----------

/// The connect-mode forwarder hands a window every ledger row after its
/// cursor, in ledger order, exactly once, and keeps following across pages
/// while the run produces more.
#[test]
fn the_forwarder_replays_the_ledger_after_its_cursor_in_order_and_exactly_once() {
    use crate::runtime::connect::{EventForwarder, LedgerEventSink};

    #[derive(Default)]
    struct Recorder(Mutex<Vec<(String, i64, u64)>>, Mutex<Vec<String>>);
    impl LedgerEventSink for Recorder {
        fn emit(&self, channel: &str, payload: &Value) -> Result<(), String> {
            let run_id = payload["runId"].as_i64().unwrap_or(-1);
            let sequence = payload["sequence"].as_u64().unwrap_or(0);
            self.0.lock().unwrap().push((channel.to_string(), run_id, sequence));
            Ok(())
        }
        fn connection_lost(&self, reason: &str) {
            self.1.lock().unwrap().push(format!("lost: {reason}"));
        }
        fn connection_restored(&self) {
            self.1.lock().unwrap().push("restored".into());
        }
        fn resnapshot_needed(&self, reason: &str, _state_version: i64) {
            self.1.lock().unwrap().push(format!("resnapshot: {reason}"));
        }
    }

    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let served = served_gated_runner(&db);
    let client = served.client();

    // History the window is NOT told about: a first run completed before the
    // forwarder started (its snapshot covers it).
    let first = client
        .dispatch(&served.envelope("start-1", "discovery.start", runner_config(dataset_id, &dataset_hash, 1)))
        .unwrap()
        .unwrap()["runId"]
        .as_i64()
        .unwrap();
    assert_eq!(served.started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);
    served.gate.release();
    wait_for_status(&served.runner, &db, first, RunStatus::Completed);
    wait_for_coordinator_exit(&served.runner, first);
    let cursor = client.events(0, None, Duration::ZERO).unwrap()["lastEventId"].as_i64().unwrap();
    assert!(cursor > 0);

    let recorder = Arc::new(Recorder::default());
    let proxy = Arc::new(Mutex::new(crate::runtime::connect::ServiceProxy {
        manifest: crate::runtime::control_api::EndpointManifest {
            manifest_version: crate::runtime::control_api::MANIFEST_VERSION.into(),
            port: served.server.port(),
            workspace_id: served.workspace.clone(),
            epoch: 1,
            holder_kind: "service".into(),
            instance_id: "in-process".into(),
            pid: std::process::id(),
            service_version: crate::runtime::control_api::SERVICE_VERSION.into(),
            started_at: "2026-09-18T00:00:00Z".into(),
        },
        client: client.clone(),
    }));
    let forwarder = EventForwarder::spawn(proxy, std::env::temp_dir(), recorder.clone(), cursor).unwrap();
    let second = client
        .dispatch(&served.envelope("start-2", "discovery.start", runner_config(dataset_id, &dataset_hash, 2)))
        .unwrap()
        .unwrap()["runId"]
        .as_i64()
        .unwrap();
    assert_eq!(served.started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);
    served.gate.release();
    assert_eq!(served.started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 1);
    served.gate.release();
    wait_for_status(&served.runner, &db, second, RunStatus::Completed);
    wait_for_coordinator_exit(&served.runner, second);
    let deadline = Instant::now() + TEST_TIMEOUT;
    while !recorder.0.lock().unwrap().iter().any(|(channel, _, _)| channel == DISCOVERY_DONE_EVENT) {
        assert!(Instant::now() < deadline, "Done never forwarded: {:?}", recorder.0.lock().unwrap());
        thread::sleep(Duration::from_millis(20));
    }
    forwarder.stop();

    let forwarded = recorder.0.lock().unwrap().clone();
    assert!(forwarded.iter().all(|(_, run_id, _)| *run_id == second), "only the second run: {forwarded:?}");
    let sequences: Vec<u64> = forwarded.iter().map(|(_, _, sequence)| *sequence).collect();
    let mut sorted = sequences.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sequences, sorted, "ledger order, no duplicates: {forwarded:?}");
    assert_eq!(forwarded.iter().filter(|(channel, _, _)| channel == DISCOVERY_DONE_EVENT).count(), 1);
    assert_eq!(forwarded.iter().filter(|(channel, _, _)| channel == DISCOVERY_RESULT_EVENT).count(), 2);
    assert_eq!(forwarded.last().unwrap().0, DISCOVERY_DONE_EVENT);
    // Everything the ledger holds after the cursor was forwarded, no more.
    let ledger = client.events(cursor, None, Duration::ZERO).unwrap();
    assert_eq!(ledger["events"].as_array().unwrap().len(), forwarded.len());
    // Every row was delivered and no gap appeared, so the window was never
    // told to re-read; nor was the service ever lost.
    assert_eq!(*recorder.1.lock().unwrap(), Vec::<String>::new());
}
