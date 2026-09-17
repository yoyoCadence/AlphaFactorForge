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
    server: ControlServer,
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
    Served { server, token, runner, workspace, gate, started_rx }
}

fn event_channels(page: &Value) -> Vec<(&str, i64)> {
    page["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| (event["channel"].as_str().unwrap(), event["eventId"].as_i64().unwrap()))
        .collect()
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
        let db = db.clone();
        thread::spawn(move || {
            service::drain(&runner, &db, TEST_TIMEOUT);
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
    server.stop();

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
