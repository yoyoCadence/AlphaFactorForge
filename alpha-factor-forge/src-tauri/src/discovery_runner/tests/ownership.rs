//! P03a acceptance regressions: real OS lease transfer and separate connections.
use super::*;
use crate::db::ownership::HolderKind;
use crate::runtime::{open_workspace_with, OwnershipHandle, Workspace};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TempRoot(PathBuf);
impl TempRoot {
    fn new(label: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("aff-epoch-test-{}-{label}", std::process::id()));
        assert!(!root.exists());
        Self(root)
    }
    fn path(&self) -> PathBuf {
        self.0.join("workspace.sqlite3")
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(self
            .0
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("aff-epoch-test-"));
        std::fs::remove_dir_all(&self.0).expect("remove closed ownership test workspace");
    }
}
fn open(path: &Path, kind: HolderKind) -> Workspace {
    open_workspace_with(path, kind, Duration::from_secs(60)).unwrap()
}

struct TakeoverSink {
    path: PathBuf,
    first: Mutex<Option<OwnershipHandle>>,
    second: Mutex<Option<Workspace>>,
}
impl DiscoveryEventSink for TakeoverSink {
    fn emit(&self, event: &DiscoveryEvent) -> Result<(), String> {
        if let DiscoveryEvent::Progress(progress) = event {
            if progress.sequence == 1 {
                drop(self.first.lock().unwrap().take());
                let second = open(&self.path, HolderKind::Service);
                assert_eq!(second.ownership.epoch, 2);
                assert_eq!(second.recovery.runs_paused, 1);
                // The successor explicitly resumes, but dispatches no worker:
                // any subsequent job change must come from the old coordinator.
                discovery::transition_run(
                    &second.db.lock().unwrap(),
                    Some(second.ownership.epoch),
                    progress.run_id,
                    RunStatus::Running,
                )
                .unwrap();
                *self.second.lock().unwrap() = Some(second);
            }
        }
        Ok(())
    }
}
struct CountingExecutor(Arc<AtomicUsize>);
impl CandidateExecutor for CountingExecutor {
    fn execute(&self, work: &CandidateWork) -> Result<CandidateExecutionOutput, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        ProductionExecutor.execute(work)
    }
}

#[test]
fn stale_coordinator_cannot_claim_or_dispatch_after_takeover() {
    let root = TempRoot::new("claim");
    let first = open(&root.path(), HolderKind::DesktopEmbedded);
    let db = first.db.clone();
    let (dataset_id, hash) = import_dataset(&db, &alternating_candles(240, 1_577_836_800_000));
    let executed = Arc::new(AtomicUsize::new(0));
    let runner = DiscoveryRunner {
        executor: Arc::new(CountingExecutor(executed.clone())),
        ..first.discovery
    };
    let sink = Arc::new(TakeoverSink {
        path: root.path(),
        first: Mutex::new(Some(first.ownership)),
        second: Mutex::new(None),
    });
    let run_id = runner
        .start(
            db.clone(),
            sink.clone(),
            runner_config(dataset_id, &hash, 1),
        )
        .unwrap();
    wait_for_coordinator_exit(&runner, run_id);
    let conn = db.lock().unwrap();
    let queued: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM discovery_jobs WHERE discovery_run_id = ?1 AND status = 'queued'",
            [run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        queued, 2,
        "stale claim must leave both successor jobs queued"
    );
    assert_eq!(
        executed.load(Ordering::SeqCst),
        0,
        "stale coordinator must never dispatch"
    );
    assert_eq!(
        discovery::get_discovery_run(&conn, run_id).unwrap().status,
        RunStatus::Running
    );
}

#[test]
fn cancel_is_fenced_when_takeover_occurs_after_preflight() {
    let root = TempRoot::new("cancel");
    let first = open(&root.path(), HolderKind::DesktopEmbedded);
    let db = first.db.clone();
    let runner = first.discovery;
    let run_id =
        discovery::create_discovery_run(&db.lock().unwrap(), None, "paused run", "{}").unwrap();
    db.lock()
        .unwrap()
        .execute(
            "UPDATE discovery_runs SET status = 'paused' WHERE id = ?1",
            [run_id],
        )
        .unwrap();
    let second = Arc::new(Mutex::new(None));
    let second_slot = second.clone();
    let path = root.path();
    let lease = first.ownership;
    AFTER_EPOCH_PREFLIGHT.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            // The old connection's mutex is STILL held, but does not lock the
            // successor's separate SQLite connection.
            drop(lease);
            let successor = open(&path, HolderKind::Service);
            assert_eq!(successor.ownership.epoch, 2);
            *second_slot.lock().unwrap() = Some(successor);
        }))
    });
    let before = run_snapshot(&db, run_id);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let result = runner.cancel(&db, sink.clone(), run_id);
    assert!(
        matches!(result, Err(AppError::StaleOwner(_))),
        "cancel: {result:?}"
    );
    assert_eq!(
        run_snapshot(&db, run_id),
        before,
        "stale cancel must change no rows"
    );
    assert!(
        sink.snapshot().is_empty(),
        "refused write must publish no success"
    );
}

#[test]
fn takeover_cannot_advance_epoch_inside_an_admitted_write() {
    use crate::db::ownership::{acquire, read, write_transaction};
    use crate::runtime::lease::try_lock_workspace;
    let root = TempRoot::new("serialization");
    let first = open(&root.path(), HolderKind::DesktopEmbedded);
    let mut successor_conn = Connection::open(root.path()).unwrap();
    successor_conn.busy_timeout(Duration::ZERO).unwrap();
    let conn = first.db.lock().unwrap();
    let tx = write_transaction(&conn, Some(first.ownership.epoch)).unwrap();
    // Release the real OS lease after the transaction has admitted this write.
    // The successor can take the OS lock, but SQLite must serialize its epoch
    // update AFTER the admitted transaction commits or rolls back.
    drop(first.ownership);
    let _next_lock = try_lock_workspace(&root.0).unwrap();
    let blocked = acquire(&mut successor_conn, HolderKind::Service, 2);
    assert!(
        matches!(blocked, Err(AppError::Db(rusqlite::Error::SqliteFailure(ref error, _)))
        if error.code == rusqlite::ErrorCode::DatabaseBusy),
        "{blocked:?}"
    );
    assert_eq!(read(&successor_conn).unwrap().unwrap().epoch, 1);
    tx.execute(
        "INSERT INTO discovery_runs (name, status, config_json) VALUES ('admitted', 'idle', '{}')",
        [],
    )
    .unwrap();
    tx.commit().unwrap();
    assert_eq!(
        acquire(&mut successor_conn, HolderKind::Service, 2)
            .unwrap()
            .epoch,
        2
    );
    assert!(matches!(
        write_transaction(&conn, Some(1)),
        Err(AppError::StaleOwner(_))
    ));
}
