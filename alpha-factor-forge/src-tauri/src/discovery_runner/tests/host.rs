//! P04b: the desktop's host modes over a REAL file-based workspace — own it
//! when it is free, connect when a service publishes it, hand an in-flight
//! run over to a service at a checkpoint and take it back, and refuse to
//! hand over while an embedded command is still running.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use super::*;
use crate::db::ownership::HolderKind;
use crate::runtime::connect::LedgerEventSink;
use crate::runtime::host::{self, Admission, HostError, HostMode, ServiceLauncher};
use crate::runtime::service::{self, RunOptions, ServiceError};
use crate::runtime::{lease, open_workspace};

fn fresh_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("aff-host-test-{}-{n}", std::process::id()))
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.0.exists() {
            return;
        }
        // A failed test may leave a service (and its database) alive in this
        // process; a second panic here would abort the whole test binary and
        // hide the first one, so the leftover is reported but not fatal.
        match std::fs::remove_dir_all(&self.0) {
            Ok(()) => {}
            Err(error) if std::thread::panicking() => eprintln!("temp dir {} not removed: {error}", self.0.display()),
            Err(error) => panic!("temp dir {} not removed: {error}", self.0.display()),
        }
    }
}

/// Records what a window would have been told.
#[derive(Default)]
struct RecordingLedgerSink {
    events: Mutex<Vec<(String, Value)>>,
    lost: AtomicUsize,
    restored: AtomicUsize,
    resnapshots: Mutex<Vec<(String, i64)>>,
}

impl RecordingLedgerSink {
    fn channels(&self) -> Vec<String> {
        self.events.lock().unwrap().iter().map(|(channel, _)| channel.clone()).collect()
    }

    fn wait_for_done(&self) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some((_, payload)) = self.events.lock().unwrap().iter().find(|(channel, _)| channel == DISCOVERY_DONE_EVENT) {
                return payload.clone();
            }
            assert!(Instant::now() < deadline, "no Done forwarded; got {:?}", self.channels());
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl LedgerEventSink for RecordingLedgerSink {
    fn emit(&self, channel: &str, payload: &Value) -> Result<(), String> {
        self.events.lock().unwrap().push((channel.to_string(), payload.clone()));
        Ok(())
    }

    fn connection_lost(&self, _reason: &str) {
        self.lost.fetch_add(1, Ordering::SeqCst);
    }

    fn connection_restored(&self) {
        self.restored.fetch_add(1, Ordering::SeqCst);
    }

    fn resnapshot_needed(&self, reason: &str, state_version: i64) {
        self.resnapshots.lock().unwrap().push((reason.to_string(), state_version));
    }
}

fn fast_service() -> RunOptions {
    RunOptions { heartbeat_period: Duration::from_millis(50), drain_timeout: Duration::from_secs(10) }
}

/// The service, in this process, on a thread — the launcher the hand-over
/// is given in tests instead of spawning the binary.
#[derive(Default)]
struct InProcessLauncher {
    threads: Mutex<Vec<JoinHandle<Result<(), ServiceError>>>>,
}

impl InProcessLauncher {
    fn join_all(&self) -> Vec<Result<(), ServiceError>> {
        self.threads.lock().unwrap().drain(..).map(|thread| thread.join().unwrap()).collect()
    }
}

impl ServiceLauncher for InProcessLauncher {
    fn launch(&self, data_dir: &std::path::Path) -> std::io::Result<()> {
        let dir = data_dir.to_path_buf();
        let thread = thread::spawn(move || service::run(&dir, fast_service()));
        self.threads.lock().unwrap().push(thread);
        Ok(())
    }
}

struct FailingLauncher;

impl ServiceLauncher for FailingLauncher {
    fn launch(&self, _data_dir: &std::path::Path) -> std::io::Result<()> {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no service binary here"))
    }
}

/// The service publishes its endpoint a moment after it starts; a desktop
/// that opens before that would simply own the free workspace, so wait.
fn wait_for_published(dir: &std::path::Path) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    while !matches!(crate::runtime::control_api::read_endpoint_files(dir), Ok(Some(_))) {
        assert!(Instant::now() < deadline, "the service never published in {}", dir.display());
        thread::sleep(Duration::from_millis(20));
    }
}

fn mode_kind(slot: &Mutex<HostMode>) -> &'static str {
    slot.lock().unwrap().kind()
}

fn embedded_epoch(slot: &Mutex<HostMode>) -> i64 {
    match &*slot.lock().unwrap() {
        HostMode::Embedded(workspace) => workspace.ownership.epoch,
        other => panic!("not embedded: {}", other.kind()),
    }
}

#[test]
fn the_desktop_owns_a_free_workspace_and_connects_to_a_published_service() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());

    // Free: own it (epoch 1), exactly as before P04b.
    let first = host::open_or_connect(&dir).unwrap();
    assert_eq!(first.kind(), host::DESKTOP_EMBEDDED);
    let HostMode::Embedded(workspace) = first else { unreachable!() };
    assert_eq!(workspace.ownership.epoch, 1);
    let workspace_id = workspace.workspace_id.clone();
    drop(workspace);

    // Owned by something that publishes nothing: refused with the reason.
    let foreign_lock = lease::try_lock_workspace(&dir).unwrap();
    let refused = host::open_or_connect(&dir).err().expect("owned without an endpoint");
    assert!(matches!(refused, HostError::CannotConnect(crate::runtime::connect::ConnectError::NotPublished)), "{refused}");
    drop(foreign_lock);

    // Owned by a service that answers: connect, proxy, follow.
    let service_dir = dir.clone();
    let service = thread::spawn(move || service::run(&service_dir, fast_service()));
    wait_for_published(&dir);
    let connected = match host::open_or_connect(&dir) {
        Ok(HostMode::Connected(connected)) => connected,
        Ok(other) => panic!("expected connect mode, got {}", other.kind()),
        Err(error) => panic!("never connected: {error}"),
    };
    let proxy = connected.proxy();
    assert_eq!(proxy.manifest.epoch, 2, "the service took the lease after the desktop");
    assert_eq!(proxy.workspace_id(), workspace_id);
    assert_eq!(proxy.manifest.holder_kind, "service");
    assert_eq!(proxy.active().unwrap(), Value::Null);
    // The non-owner's database connection reads the same rows and may not migrate.
    let datasets: i64 = connected.db.lock().unwrap().query_row("SELECT COUNT(*) FROM datasets", [], |r| r.get(0)).unwrap();
    assert_eq!(datasets, 0);
    let sink = Arc::new(RecordingLedgerSink::default());
    connected.follow_events(sink.clone(), connected.ledger_end().unwrap()).unwrap();

    // The service goes away underneath: the window is told once.
    assert_eq!(service::stop(&dir, TEST_TIMEOUT).unwrap(), service::StopOutcome::Stopped);
    service.join().unwrap().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while sink.lost.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline, "the forwarder never reported the lost service");
        thread::sleep(Duration::from_millis(20));
    }
    connected.stop_following();
    assert_eq!(sink.lost.load(Ordering::SeqCst), 1);
    drop(connected);

    // A stale endpoint left behind does not stop a free workspace being owned.
    assert_eq!(host::open_or_connect(&dir).unwrap().kind(), host::DESKTOP_EMBEDDED);
}

/// The P04 acceptance from the desktop's side: research started in the
/// desktop keeps going in a background service after the hand-over, at the
/// checkpoint the desktop committed (no orphan); the desktop follows it,
/// resumes it through the proxy, sees it finish, and takes the workspace
/// back afterwards.
#[test]
fn a_hand_over_moves_an_in_flight_run_to_the_service_at_a_checkpoint_and_the_take_back_returns_it() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let mut workspace = open_workspace(&dir.join(crate::db::DB_FILE_NAME), HolderKind::DesktopEmbedded).unwrap();
    let candles = alternating_candles(240, 1_577_836_800_000);
    let (dataset_id, dataset_hash) = import_dataset(&workspace.db, &candles);
    // The desktop's runner, but with a worker we can hold at each candidate.
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let mut gated = workspace.discovery.clone();
    gated.executor = Arc::new(PermittedProductionExecutor { started: started_tx, gate: gate.clone() });
    workspace.discovery = gated.clone();
    let db = workspace.db.clone();
    let run_id = gated
        .start(db.clone(), Arc::new(RecordingSink::new(db.clone())), runner_config(dataset_id, &dataset_hash, 2))
        .unwrap();
    let trial_ids = {
        let conn = db.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT trial_event_id FROM research_attempts WHERE discovery_run_id = ?1 ORDER BY candidate_index"
        ).unwrap();
        stmt.query_map([run_id], |row| row.get::<_, String>(0)).unwrap()
            .collect::<Result<Vec<_>, _>>().unwrap()
    };
    assert_eq!(trial_ids.len(), 2, "every candidate was registered before dispatch");
    let ledger = gated.trial_ledger.as_ref().unwrap();
    assert!(trial_ids.iter().all(|id| ledger.contains_event(id).unwrap()));
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0, "candidate 0 in flight in the desktop");

    let slot = Mutex::new(HostMode::Embedded(workspace));
    let admission = Admission::default();
    let launcher = InProcessLauncher::default();
    let sink = Arc::new(RecordingLedgerSink::default());
    let (done_tx, done_rx) = mpsc::channel();
    thread::scope(|scope| {
        scope.spawn(|| {
            let _ = done_tx.send(host::hand_over_to_service(&slot, &admission, &launcher, sink.clone(), &dir));
        });
        // The hand-over asks the coordinator to checkpoint and waits for it.
        wait_for_phase(&gated, run_id, ControlPhase::PauseRequested);
        assert!(done_rx.recv_timeout(Duration::from_millis(300)).is_err(), "waits for the checkpoint");
        assert_eq!(mode_kind(&slot), host::SWITCHING);
        assert!(admission.admit().is_err(), "no new embedded work during the hand-over");
        gate.release();
        done_rx.recv_timeout(TEST_TIMEOUT).expect("hand-over finished").unwrap_or_else(|error| panic!("{error}"));
    });
    assert_eq!(mode_kind(&slot), host::DESKTOP_CONNECT);
    assert!(admission.admit().is_ok(), "admission reopened");

    // The service owns the workspace (epoch 2) and found a PAUSED run — the
    // checkpoint the desktop committed — with one candidate done.
    let proxy = match &*slot.lock().unwrap() {
        HostMode::Connected(connected) => connected.proxy(),
        other => panic!("{}", other.kind()),
    };
    assert_eq!(proxy.manifest.epoch, 2);
    let active = proxy.active().unwrap();
    assert_eq!(active["runId"], run_id);
    assert_eq!(active["status"], "paused");
    assert_eq!(active["counts"]["completedCandidates"], 1);
    assert!(gated.active_coordinator_run_ids().unwrap().is_empty(), "the desktop's coordinator exited");

    // Resume through the proxy: the service's own workers finish it, and the
    // forwarded ledger tells the (would-be) window.
    proxy.resume(run_id).unwrap();
    let done = sink.wait_for_done();
    assert_eq!(done["status"], "completed");
    assert_eq!(done["runId"], run_id);
    let channels = sink.channels();
    assert!(channels.iter().any(|c| c == DISCOVERY_RESULT_EVENT), "{channels:?}");
    assert_eq!(proxy.progress(run_id).unwrap()["counts"]["completedCandidates"], 2);

    // Take it back: the service drains and stops, the desktop owns again.
    host::take_back_from_service(&slot, &admission, &dir).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED);
    assert_eq!(embedded_epoch(&slot), 3, "desktop 1 → service 2 → desktop 3");
    for outcome in launcher.join_all() {
        outcome.expect("the in-process service exited cleanly");
    }
    let HostMode::Embedded(workspace) = std::mem::replace(&mut *slot.lock().unwrap(), HostMode::Switching("test over".into())) else {
        unreachable!()
    };
    assert_eq!(workspace.recovery, RecoveryReport::default(), "nothing to repair after a clean take-back");
    assert_eq!(workspace.discovery.progress(&workspace.db, run_id).unwrap().status, RunStatus::Completed);
    assert!(crate::runtime::control_api::read_endpoint_files(&dir).unwrap().is_none(), "endpoint withdrawn");
    drop(workspace);
    drop(db);
}

#[test]
fn a_hand_over_waits_for_admitted_commands_and_a_failed_launch_returns_to_embedded() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let workspace = open_workspace(&dir.join(crate::db::DB_FILE_NAME), HolderKind::DesktopEmbedded).unwrap();
    let slot = Mutex::new(HostMode::Embedded(workspace));
    let admission = Admission::default();
    let sink = Arc::new(RecordingLedgerSink::default());

    // An embedded mutating command is still running: the hand-over waits.
    let in_flight = admission.admit().unwrap();
    let (done_tx, done_rx) = mpsc::channel();
    thread::scope(|scope| {
        scope.spawn(|| {
            let _ = done_tx.send(host::hand_over_to_service(&slot, &admission, &FailingLauncher, sink.clone(), &dir));
        });
        assert!(done_rx.recv_timeout(Duration::from_millis(300)).is_err(), "blocked on the admitted command");
        assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED, "the workspace was not taken yet");
        assert!(admission.admit().is_err(), "but no NEW command gets in");
        drop(in_flight);
        // Drained → released → launch fails → re-owned.
        let error = done_rx.recv_timeout(TEST_TIMEOUT).expect("finished").expect_err("the launch failed");
        assert!(matches!(error, HostError::SwitchFailed { now: host::DESKTOP_EMBEDDED, .. }), "{error}");
        assert!(error.to_string().contains("no service binary here"), "{error}");
    });
    assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED);
    assert_eq!(embedded_epoch(&slot), 2, "released and re-owned");
    assert!(admission.admit().is_ok(), "admission reopened");
    assert!(matches!(lease::try_lock_workspace(&dir), Err(crate::error::AppError::NotOwner(_))), "the desktop holds the lock again");

    // Take-back is only for connect mode.
    let error = host::take_back_from_service(&slot, &admission, &dir).unwrap_err();
    assert!(matches!(error, HostError::SwitchFailed { now: host::DESKTOP_EMBEDDED, .. }), "{error}");
    assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED);
    drop(std::mem::replace(&mut *slot.lock().unwrap(), HostMode::Switching("test over".into())));
}

#[test]
fn a_take_back_after_the_service_died_still_re_owns_the_workspace() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let service_dir = dir.clone();
    let service = thread::spawn(move || service::run(&service_dir, fast_service()));
    wait_for_published(&dir);
    let connected = match host::open_or_connect(&dir) {
        Ok(HostMode::Connected(connected)) => connected,
        Ok(other) => panic!("expected connect mode, got {}", other.kind()),
        Err(error) => panic!("never connected: {error}"),
    };
    let slot = Mutex::new(HostMode::Connected(connected));
    let admission = Admission::default();
    // The service is stopped by someone else (or crashed) before the take-back.
    assert_eq!(service::stop(&dir, TEST_TIMEOUT).unwrap(), service::StopOutcome::Stopped);
    service.join().unwrap().unwrap();

    host::take_back_from_service(&slot, &admission, &dir).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED);
    assert_eq!(embedded_epoch(&slot), 2);
    drop(std::mem::replace(&mut *slot.lock().unwrap(), HostMode::Switching("test over".into())));
}

/// The launcher the desktop really uses: the service BINARY, spawned
/// detached beside the test executable's own build output. When `cargo test`
/// built the binaries (it does whenever the package's integration tests are
/// in the plan) the whole hand-over runs against a real second process and
/// the take-back stops it; when the binary is absent the launcher must fail
/// with the reason and the desktop must fall back to embedded — either way
/// the assertion is exact.
#[test]
fn the_executable_launcher_hands_over_to_a_real_service_process_when_the_binary_is_built() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    // target/debug/deps/<test exe> -> target/debug/<service exe>
    let exe = std::env::current_exe()
        .ok()
        .and_then(|test_exe| test_exe.parent()?.parent().map(|debug| host::service_executable_beside(&debug.join("x"))))
        .expect("a build output directory");
    let launcher = host::ExecutableLauncher { exe: exe.clone() };
    let workspace = open_workspace(&dir.join(crate::db::DB_FILE_NAME), HolderKind::DesktopEmbedded).unwrap();
    let slot = Mutex::new(HostMode::Embedded(workspace));
    let admission = Admission::default();
    let sink = Arc::new(RecordingLedgerSink::default());

    let outcome = host::hand_over_to_service(&slot, &admission, &launcher, sink, &dir);
    if !exe.is_file() {
        let error = outcome.expect_err("no binary: the launch fails");
        assert!(error.to_string().contains("service binary not found"), "{error}");
        assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED, "fell back to embedded");
        eprintln!("service binary absent at {}; only the failure path was exercised", exe.display());
        drop(std::mem::replace(&mut *slot.lock().unwrap(), HostMode::Switching("test over".into())));
        return;
    }
    outcome.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(mode_kind(&slot), host::DESKTOP_CONNECT);
    let (pid, port) = match &*slot.lock().unwrap() {
        HostMode::Connected(connected) => (connected.proxy().manifest.pid, connected.proxy().manifest.port),
        other => panic!("{}", other.kind()),
    };
    assert_ne!(pid, std::process::id(), "a separate process owns the workspace");
    assert!(dir.join(host::SERVICE_LOG_FILE_NAME).is_file(), "the service's output goes to its log");

    host::take_back_from_service(&slot, &admission, &dir).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED);
    assert_eq!(embedded_epoch(&slot), 3);
    let refused = std::net::TcpStream::connect_timeout(&(std::net::Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(1));
    assert!(refused.is_err(), "the service process stopped listening");
    let log = std::fs::read_to_string(dir.join(host::SERVICE_LOG_FILE_NAME)).unwrap();
    assert!(log.contains("shutdown requested") && log.contains("stopped"), "{log}");
    drop(std::mem::replace(&mut *slot.lock().unwrap(), HostMode::Switching("test over".into())));
}

// ---------- 2026-09-18 acceptance regressions (H1, H2, M1) ----------

/// A workspace file with one dataset the service can run, created before
/// any owner (the desktop or the service) opens it.
fn seeded_workspace(dir: &std::path::Path) -> (i64, String) {
    let conn = crate::db::open_at(&dir.join(crate::db::DB_FILE_NAME)).unwrap();
    let db: SharedDb = Arc::new(Mutex::new(conn));
    let candles = alternating_candles(240, 1_577_836_800_000);
    import_dataset(&db, &candles)
}

/// A desktop connected to an in-process service on `dir`, following it.
fn connected_and_following(
    dir: &std::path::Path,
) -> (JoinHandle<Result<(), ServiceError>>, crate::runtime::connect::ConnectedHost, Arc<RecordingLedgerSink>) {
    let service_dir = dir.to_path_buf();
    let service = thread::spawn(move || service::run(&service_dir, fast_service()));
    wait_for_published(dir);
    let connected = match host::open_or_connect(dir) {
        Ok(HostMode::Connected(connected)) => connected,
        Ok(other) => panic!("expected connect mode, got {}", other.kind()),
        Err(error) => panic!("never connected: {error}"),
    };
    let sink = Arc::new(RecordingLedgerSink::default());
    connected.follow_events(sink.clone(), connected.ledger_end().unwrap()).unwrap();
    (service, connected, sink)
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(50));
    }
}

/// H1: a take-back whose `stop` is refused leaves the desktop connected —
/// and still FOLLOWING. The service keeps working; the window keeps seeing
/// what it does, including the run's Done.
#[test]
fn a_failed_take_back_keeps_forwarding_the_services_events() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let (dataset_id, dataset_hash) = seeded_workspace(&dir);
    let (service, connected, sink) = connected_and_following(&dir);
    let slot = Mutex::new(HostMode::Connected(connected));
    let admission = Admission::default();

    // The take-back's `stop` is refused (401): the token file names a token
    // the service does not know, while the desktop's own client still holds
    // the right one.
    let token_path = crate::runtime::control_api::token_path(&dir);
    let real_token = std::fs::read_to_string(&token_path).unwrap();
    std::fs::write(&token_path, "0".repeat(crate::runtime::control_api::TOKEN_HEX_LEN)).unwrap();
    let error = host::take_back_from_service(&slot, &admission, &dir).unwrap_err();
    assert!(matches!(error, HostError::SwitchFailed { now: host::DESKTOP_CONNECT, .. }), "{error}");
    std::fs::write(&token_path, real_token).unwrap();
    assert_eq!(mode_kind(&slot), host::DESKTOP_CONNECT);

    // Still connected in every sense: commands work AND events arrive.
    let proxy = match &*slot.lock().unwrap() {
        HostMode::Connected(connected) => {
            assert!(connected.is_following(), "the forwarder must survive a failed take-back");
            connected.proxy()
        }
        other => panic!("{}", other.kind()),
    };
    let run_id = proxy.start(runner_config(dataset_id, &dataset_hash, 1)).unwrap();
    let done = sink.wait_for_done();
    assert_eq!(done["runId"], run_id);
    assert_eq!(done["status"], "completed", "restored Connected mode must still forward the service's Done event");

    // A take-back that can reach the service succeeds afterwards.
    host::take_back_from_service(&slot, &admission, &dir).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(mode_kind(&slot), host::DESKTOP_EMBEDDED);
    service.join().unwrap().unwrap();
    drop(std::mem::replace(&mut *slot.lock().unwrap(), HostMode::Switching("test over".into())));
}

/// H2: the service completes a run but the ledger refuses the Done row
/// (contract §3: an append failure records a gap). The rows the window got
/// cannot show the completion, so the bridge must tell it to re-read —
/// once for that gap, not on every poll.
#[test]
fn a_missing_terminal_ledger_row_makes_the_window_re_read_once() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let (dataset_id, dataset_hash) = seeded_workspace(&dir);
    let (service, connected, sink) = connected_and_following(&dir);
    let proxy = connected.proxy();
    // A persistent trigger: it applies to the service's connection too.
    connected
        .db
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER deny_done BEFORE INSERT ON runtime_events
             WHEN NEW.channel = 'discovery://done' BEGIN SELECT RAISE(ABORT, 'deny_done'); END;",
        )
        .unwrap();

    let run_id = proxy.start(runner_config(dataset_id, &dataset_hash, 1)).unwrap();
    wait_until("the run to complete in the database", || {
        proxy.progress(run_id).map(|run| run["status"] == "completed").unwrap_or(false)
    });
    wait_until("the window to be told to re-read the gap", || {
        sink.resnapshots.lock().unwrap().iter().any(|(reason, _)| reason.contains("gap"))
    });
    let page = proxy.client.events(0, None, Duration::ZERO).unwrap();
    assert!(page["ledgerGap"].is_object(), "the refused append left a gap marker: {page}");
    assert!(!sink.channels().iter().any(|c| c == DISCOVERY_DONE_EVENT), "no Done row could be forwarded");
    let (reason, at) = sink.resnapshots.lock().unwrap().iter().find(|(reason, _)| reason.contains("gap")).unwrap().clone();
    assert!(reason.contains("gap"), "{reason}");
    assert_eq!(at, page["stateVersion"].as_i64().unwrap());
    assert_eq!(proxy.progress(run_id).unwrap()["status"], "completed", "what the re-read returns");

    // The same marker is not re-announced on later polls.
    thread::sleep(crate::runtime::connect::FORWARD_POLL + Duration::from_millis(500));
    let notifications = sink.resnapshots.lock().unwrap().clone();
    assert_eq!(notifications.iter().filter(|(reason, _)| reason.contains("gap")).count(), 1, "{notifications:?}");
    assert!(notifications.len() <= 2, "at most one initial sync plus one gap notification: {notifications:?}");
    assert_eq!(sink.lost.load(Ordering::SeqCst), 0);

    connected.db.lock().unwrap().execute_batch("DROP TRIGGER deny_done;").unwrap();
    connected.stop_following();
    assert_eq!(service::stop(&dir, TEST_TIMEOUT).unwrap(), service::StopOutcome::Stopped);
    service.join().unwrap().unwrap();
    drop(connected);
}

/// M1: the service goes away and comes back (a restart, new epoch). The
/// desktop's forwarder rediscovers THIS workspace's endpoint — refusing a
/// stale one and another workspace's copied here — replaces the proxy the
/// commands use, tells the window, and follows the new service's rows.
#[test]
fn a_restarted_service_is_rediscovered_and_another_workspaces_endpoint_is_refused() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let (dataset_id, dataset_hash) = seeded_workspace(&dir);
    let (first_service, connected, sink) = connected_and_following(&dir);
    let old_proxy = connected.proxy();
    assert_eq!(old_proxy.manifest.epoch, 1);

    // The service stops (its endpoint withdrawn): lost, once.
    assert_eq!(service::stop(&dir, TEST_TIMEOUT).unwrap(), service::StopOutcome::Stopped);
    first_service.join().unwrap().unwrap();
    wait_until("the lost notification", || sink.lost.load(Ordering::SeqCst) == 1);

    // Another workspace's live endpoint copied into this directory must
    // not be mistaken for ours.
    let other_dir = fresh_dir();
    let _other_guard = TempDir(other_dir.clone());
    let other_service = {
        let other = other_dir.clone();
        thread::spawn(move || service::run(&other, fast_service()))
    };
    wait_for_published(&other_dir);
    for name in [crate::runtime::control_api::MANIFEST_FILE_NAME, crate::runtime::control_api::TOKEN_FILE_NAME] {
        std::fs::copy(other_dir.join(name), dir.join(name)).unwrap();
    }
    thread::sleep(crate::runtime::connect::RECONNECT_PAUSE * 3);
    assert_eq!(sink.restored.load(Ordering::SeqCst), 0, "another workspace's endpoint was accepted");
    assert_eq!(service::stop(&other_dir, TEST_TIMEOUT).unwrap(), service::StopOutcome::Stopped);
    other_service.join().unwrap().unwrap();
    crate::runtime::control_api::remove_endpoint_files(&dir).unwrap();

    // The workspace's own service restarts (epoch 2): rediscovered.
    let second_service = {
        let dir = dir.clone();
        thread::spawn(move || service::run(&dir, fast_service()))
    };
    wait_until("the restored notification", || sink.restored.load(Ordering::SeqCst) == 1);
    let new_proxy = connected.proxy();
    assert_eq!(new_proxy.manifest.epoch, 2, "the host's proxy now names the restarted service");
    assert_ne!(new_proxy.manifest.instance_id, old_proxy.manifest.instance_id);
    assert!(new_proxy.info().is_ok(), "desktop proxy live");
    assert!(sink.resnapshots.lock().unwrap().iter().any(|(reason, _)| reason.contains("reconnected")), "the window re-reads after a reconnect");
    assert!(old_proxy.info().is_err(), "the old endpoint is gone for good");

    // Reads, commands, and events all flow through the new service.
    assert_eq!(new_proxy.active().unwrap(), Value::Null);
    let run_id = new_proxy.start(runner_config(dataset_id, &dataset_hash, 1)).unwrap();
    let done = sink.wait_for_done();
    assert_eq!(done["runId"], run_id);
    assert_eq!(done["status"], "completed");

    connected.stop_following();
    assert_eq!(service::stop(&dir, TEST_TIMEOUT).unwrap(), service::StopOutcome::Stopped);
    second_service.join().unwrap().unwrap();
    drop(connected);
}
