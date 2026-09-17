//! Host-agnostic application runtime (docs/research-runtime-contract.md).
//!
//! P02: everything a host must do to bring a workspace up — open the database
//! at a path, apply migrations, build the discovery runner, and repair
//! orphaned work — lives here once, so the desktop (`main.rs` setup) and a
//! future headless service binary (P04) reach the same state through the
//! same code.
//!
//! P03a: bringing a workspace up now also means OWNING it, in the contract's
//! fixed order (§1.2): OS lock → open SQLite → migrations → ownership epoch →
//! orphan recovery → heartbeat. A host that cannot take the lock gets
//! `NotOwner` and nothing else happens (no open, no migration, no recovery).
//! The `OwnershipHandle` returned inside the `Workspace` keeps the lock and
//! the heartbeat alive; the host must hold it for as long as it wants to
//! own the workspace (the desktop stores it in `AppState`).
//!
//! What this module deliberately does NOT know: where the data directory is
//! (the host resolves it), how events reach a UI (`DiscoveryEventSink` is a
//! trait the host implements), or anything about scheduling. Nothing here
//! imports `tauri`, and `boundary_tests` keeps it — and the modules it
//! composes — that way.

#[cfg(test)]
mod boundary_tests;
pub mod lease;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::db::ownership::{self, HolderKind, HEARTBEAT_PERIOD};
use crate::db::{self, discovery::RecoveryReport};
use crate::discovery_runner::DiscoveryRunner;
use crate::error::{AppError, AppResult};

/// Shared database handle: one connection, one coordinator/writer at a time.
pub type SharedDb = Arc<Mutex<rusqlite::Connection>>;

/// An opened, migrated, owned, and repaired workspace.
pub struct Workspace {
    pub db: SharedDb,
    pub discovery: DiscoveryRunner,
    /// What startup repair did. Persistence-only: orphaned `running` work is
    /// paused/requeued and no CPU work resumes without a user command.
    pub recovery: RecoveryReport,
    /// The lease. Drop it and the workspace is no longer owned.
    pub ownership: OwnershipHandle,
}

/// Proof of ownership for one acquisition: the OS lock, the epoch every write
/// carries, and the heartbeat thread that keeps `heartbeat_seq` moving.
/// Dropping it stops the heartbeat, then releases the OS lock.
pub struct OwnershipHandle {
    pub epoch: i64,
    pub instance_id: String,
    pub kind: HolderKind,
    heartbeat: Option<Heartbeat>,
    /// Declared last so it is dropped last: the heartbeat must stop before
    /// the lock is released (field drop order is declaration order).
    _lock: lease::OsLock,
}

impl OwnershipHandle {
    /// True once a heartbeat was refused with `StaleOwner` — the epoch moved
    /// under this holder, which cannot happen while it holds the OS lock and
    /// therefore indicates the lock file was tampered with or replaced.
    pub fn lost(&self) -> bool {
        self.heartbeat.as_ref().is_some_and(|hb| hb.lost.load(Ordering::SeqCst))
    }
}

impl Drop for OwnershipHandle {
    fn drop(&mut self) {
        if let Some(hb) = self.heartbeat.take() {
            hb.stop();
        }
    }
}

struct Heartbeat {
    stop: Arc<(Mutex<bool>, Condvar)>,
    lost: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Heartbeat {
    fn spawn(db: SharedDb, epoch: i64, period: Duration) -> std::io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let lost = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            let lost = lost.clone();
            thread::Builder::new()
                .name("workspace-heartbeat".into())
                .spawn(move || heartbeat_loop(db, epoch, period, stop, lost))?
        };
        Ok(Self { stop, lost, thread: Some(thread) })
    }

    fn stop(mut self) {
        let (flag, changed) = &*self.stop;
        if let Ok(mut stopped) = flag.lock() {
            *stopped = true;
        }
        changed.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Bump `heartbeat_seq` every `period` until told to stop. A `StaleOwner`
/// refusal ends the loop and sets `lost`; any other error is reported and the
/// next beat is attempted, because a transient busy database must not make
/// a live owner look dead.
fn heartbeat_loop(
    db: SharedDb,
    epoch: i64,
    period: Duration,
    stop: Arc<(Mutex<bool>, Condvar)>,
    lost: Arc<AtomicBool>,
) {
    let (flag, changed) = &*stop;
    loop {
        {
            let Ok(mut stopped) = flag.lock() else { return };
            while !*stopped {
                let (guard, timeout) = match changed.wait_timeout(stopped, period) {
                    Ok(pair) => pair,
                    Err(_) => return,
                };
                stopped = guard;
                if timeout.timed_out() {
                    break;
                }
            }
            if *stopped {
                return;
            }
        }
        let beat = db
            .lock()
            .map_err(|_| AppError::Other("db lock poisoned".into()))
            .and_then(|conn| ownership::heartbeat(&conn, epoch));
        match beat {
            Ok(_) => {}
            Err(AppError::StaleOwner(message)) => {
                eprintln!("workspace heartbeat stopped: {message}");
                lost.store(true, Ordering::SeqCst);
                return;
            }
            Err(error) => eprintln!("workspace heartbeat skipped: {error}"),
        }
    }
}

/// Own and open the workspace whose database lives at `db_path`, in the
/// contract's order. `kind` names the host for the ownership row.
pub fn open_workspace(db_path: &Path, kind: HolderKind) -> AppResult<Workspace> {
    open_workspace_with(db_path, kind, HEARTBEAT_PERIOD)
}

/// `open_workspace` with an explicit heartbeat period (tests use a short one).
pub fn open_workspace_with(
    db_path: &Path,
    kind: HolderKind,
    heartbeat_period: Duration,
) -> AppResult<Workspace> {
    let data_dir = db_path
        .parent()
        .ok_or_else(|| AppError::Other(format!("database path has no parent: {}", db_path.display())))?;
    // 1. The OS lock, before anything touches the database (§1.2 step 1).
    let lock = lease::try_lock_workspace(data_dir)?;
    // 2–3. Open (busy timeout) and migrate — only the lock holder gets here.
    let mut conn = db::open_at(db_path)?;
    // 4. Record ourselves and bump the epoch.
    let acquired = ownership::acquire(&mut conn, kind, std::process::id())?;
    let db: SharedDb = Arc::new(Mutex::new(conn));
    // 5. Orphan recovery, as the holder of the new epoch.
    let discovery = DiscoveryRunner::with_epoch(acquired.epoch);
    let recovery = discovery.recover_orphans(&db)?;
    // 6. Heartbeat.
    let heartbeat = Heartbeat::spawn(db.clone(), acquired.epoch, heartbeat_period)?;
    Ok(Workspace {
        db,
        discovery,
        recovery,
        ownership: OwnershipHandle {
            epoch: acquired.epoch,
            instance_id: acquired.instance_id,
            kind,
            heartbeat: Some(heartbeat),
            _lock: lock,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    use super::*;
    use crate::db::ownership::lease_is_stale;

    /// A unique, never-shared path under the OS temp dir. The directory does
    /// not exist yet, which is the desktop's first-launch situation.
    fn fresh_db_path() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir()
            .join(format!("aff-runtime-test-{}-{n}", std::process::id()))
            .join("nested")
            .join(db::DB_FILE_NAME)
    }

    /// Removes the test's temp root on drop. Every test drops its workspace
    /// (and with it the SQLite connection and the lock) BEFORE this guard
    /// runs, otherwise Windows refuses to delete the open files; a failed
    /// removal is reported instead of silently leaving directories behind
    /// (P02 review).
    struct TempRoot(PathBuf);

    impl Drop for TempRoot {
        fn drop(&mut self) {
            if self.0.exists() {
                std::fs::remove_dir_all(&self.0)
                    .unwrap_or_else(|error| panic!("temp root {} not removed: {error}", self.0.display()));
            }
        }
    }

    fn temp_root_of(db_path: &Path) -> TempRoot {
        TempRoot(db_path.parent().unwrap().parent().unwrap().to_path_buf())
    }

    fn pragma<T: rusqlite::types::FromSql>(conn: &rusqlite::Connection, name: &str) -> T {
        conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
            .expect("pragma readable")
    }

    fn open(path: &Path) -> Workspace {
        open_workspace(path, HolderKind::DesktopEmbedded).expect("open workspace")
    }

    #[test]
    fn open_workspace_creates_the_directory_migrates_and_sets_the_connection_pragmas() {
        let path = fresh_db_path();
        let _root = temp_root_of(&path);
        assert!(!path.parent().unwrap().exists(), "the directory must not pre-exist");

        let workspace = open(&path);
        assert!(path.is_file(), "database file created at the given path");
        assert!(path.with_file_name(lease::LOCK_FILE_NAME).is_file(), "lock file beside it");
        assert_eq!(workspace.recovery, RecoveryReport { runs_paused: 0, jobs_requeued: 0 });

        let conn = workspace.db.lock().unwrap();
        assert_eq!(pragma::<String>(&conn, "journal_mode"), "wal");
        assert_eq!(pragma::<i64>(&conn, "foreign_keys"), 1);
        assert_eq!(
            pragma::<i64>(&conn, "busy_timeout"),
            db::BUSY_TIMEOUT.as_millis() as i64,
            "ownership-lease-v1 §1.2: a 5 s busy timeout"
        );
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, 4, "0001–0004 applied on first open");
        drop(conn);
        drop(workspace);
    }

    #[test]
    fn reopening_the_same_path_is_idempotent_and_keeps_stored_rows() {
        let path = fresh_db_path();
        let _root = temp_root_of(&path);
        let first = open(&path);
        first
            .db
            .lock()
            .unwrap()
            .execute("INSERT INTO app_settings (key, value_json) VALUES ('p02', '\"kept\"')", [])
            .expect("write a row");
        drop(first);

        let second = open(&path);
        let conn = second.db.lock().unwrap();
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, 4, "no migration is re-applied");
        let value: String = conn
            .query_row("SELECT value_json FROM app_settings WHERE key = 'p02'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "\"kept\"");
        drop(conn);
        drop(second);
    }

    #[test]
    fn open_workspace_repairs_an_orphaned_running_run_before_returning() {
        let path = fresh_db_path();
        let _root = temp_root_of(&path);
        {
            // Leave the database exactly as a crash would: a `running` run row
            // with a `running` job, and no process to own it.
            let conn = db::open_at(&path).expect("open");
            conn.execute_batch(
                "INSERT INTO datasets (exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash)
                 VALUES ('t', 'T', '1h', 1, 2, 0, 'import', 'dataset-content-v2:aa');
                 INSERT INTO strategy_def (name, type, original_definition_json, source, strategy_hash)
                 VALUES ('s', 'params', '{}', 'manual', 'strategy-v2:bb');
                 INSERT INTO discovery_runs (id, name, status, config_json, started_at)
                 VALUES (7, 'orphan', 'running', '{}', datetime('now'));
                 INSERT INTO discovery_jobs (discovery_run_id, strategy_id, dataset_id, segment, status, candidate_index)
                 VALUES (7, 1, 1, 'train', 'running', 0);",
            )
            .expect("seed an orphan");
        }

        let workspace = open(&path);
        assert_eq!(workspace.recovery, RecoveryReport { runs_paused: 1, jobs_requeued: 1 });
        let conn = workspace.db.lock().unwrap();
        let status: String = conn
            .query_row("SELECT status FROM discovery_runs WHERE id = 7", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "paused", "D5: an orphaned run is paused, never resumed");
        let job: String = conn
            .query_row("SELECT status FROM discovery_jobs WHERE discovery_run_id = 7", [], |r| r.get(0))
            .unwrap();
        assert_eq!(job, "queued");
        drop(conn);
        drop(workspace);
    }

    // ---------- P03a: ownership ----------

    #[test]
    fn a_second_host_cannot_open_an_owned_workspace_and_takes_over_only_after_release() {
        let path = fresh_db_path();
        let _root = temp_root_of(&path);

        let first = open(&path);
        assert_eq!(first.ownership.epoch, 1, "first acquisition of a fresh workspace");
        assert_eq!(first.discovery.epoch(), Some(1), "the runner writes under the lease epoch");
        assert_eq!(first.ownership.kind, HolderKind::DesktopEmbedded);

        // 雙啟: a second host (here: a service) is refused before it opens,
        // migrates, or recovers anything; the first owner is untouched.
        let refused = open_workspace(&path, HolderKind::Service);
        assert!(matches!(refused, Err(AppError::NotOwner(_))), "got {:?}", refused.err());
        {
            let conn = first.db.lock().unwrap();
            let row = ownership::read(&conn).unwrap().unwrap();
            assert_eq!(row.epoch, 1);
            assert_eq!(row.holder_kind, "desktop-embedded");
            assert_eq!(row.holder_instance_id, first.ownership.instance_id);
            assert_eq!(row.pid, i64::from(std::process::id()));
        }

        // Owner exit/crash: the OS releases the lock; the next host bumps the
        // epoch, and a writer still holding epoch 1 is now stale.
        drop(first);
        let second = open_workspace(&path, HolderKind::Service).expect("take over after release");
        assert_eq!(second.ownership.epoch, 2);
        assert_eq!(second.ownership.kind, HolderKind::Service);
        {
            let conn = second.db.lock().unwrap();
            assert_eq!(ownership::read(&conn).unwrap().unwrap().holder_kind, "service");
            let stale = crate::db::discovery::assert_owner(&conn, Some(1));
            assert!(matches!(stale, Err(AppError::StaleOwner(_))));
            assert!(crate::db::discovery::assert_owner(&conn, Some(2)).is_ok());
        }
        drop(second);
    }

    #[test]
    fn a_runner_left_over_from_a_previous_epoch_cannot_start_write_or_recover() {
        let path = fresh_db_path();
        let _root = temp_root_of(&path);

        let first = open(&path);
        // Keep the old runner around, as a process would that lost the lease
        // (a P04 hand-over, or a tampered lock file), and let another host in.
        let old_runner = first.discovery.clone();
        let db = first.db.clone();
        drop(first);
        let second = open_workspace(&path, HolderKind::Service).expect("second owner");

        // The old runner's connection sees the new epoch and every write path
        // refuses before touching a row (contract §1.3). `start` is covered
        // at the store level (`discovery_tests`): it parses its config before
        // it reaches the database, so an unparsable config would fail first.
        let recovered = old_runner.recover_orphans(&db);
        assert!(matches!(recovered, Err(AppError::StaleOwner(_))), "recover: {recovered:?}");
        let cancelled = old_runner.cancel(&db, Arc::new(NullSink), 1);
        assert!(matches!(cancelled, Err(AppError::StaleOwner(_))), "cancel: {cancelled:?}");
        {
            let conn = db.lock().unwrap();
            let runs: i64 = conn.query_row("SELECT COUNT(*) FROM discovery_runs", [], |r| r.get(0)).unwrap();
            assert_eq!(runs, 0, "nothing was written by the stale runner");
        }

        // The rightful owner is unaffected.
        assert_eq!(second.discovery.epoch(), Some(2));
        drop(db);
        drop(second);
    }

    #[test]
    fn the_heartbeat_moves_the_counter_and_stops_when_the_handle_is_dropped() {
        let path = fresh_db_path();
        let _root = temp_root_of(&path);
        let period = Duration::from_millis(20);
        let workspace = open_workspace_with(&path, HolderKind::DesktopEmbedded, period)
            .expect("open with a fast heartbeat");

        let seq_at = |db: &SharedDb| ownership::read(&db.lock().unwrap()).unwrap().unwrap().heartbeat_seq;
        let started = Instant::now();
        let before = seq_at(&workspace.db);
        while seq_at(&workspace.db) < before + 3 {
            assert!(started.elapsed() < Duration::from_secs(5), "heartbeat never advanced");
            thread::sleep(period);
        }
        assert!(!workspace.ownership.lost());
        // Liveness as a reader judges it: the counter moved within its own
        // interval, so the owner is alive regardless of wall clocks.
        assert!(!lease_is_stale(before, seq_at(&workspace.db), Duration::from_secs(60)));

        // Dropping the handle stops the beat; the counter freezes.
        let db = workspace.db.clone();
        drop(workspace);
        let frozen = seq_at(&db);
        thread::sleep(period * 5);
        assert_eq!(seq_at(&db), frozen, "no beat after the handle is gone");
        drop(db);
    }

    #[test]
    fn a_database_written_by_a_newer_build_is_refused_before_anything_runs() {
        let path = fresh_db_path();
        let _root = temp_root_of(&path);
        {
            let conn = db::open_at(&path).expect("open");
            conn.execute(
                "INSERT INTO schema_migrations (version) VALUES ('0099_from_the_future')",
                [],
            )
            .unwrap();
        }
        let refused = open_workspace(&path, HolderKind::DesktopEmbedded);
        let error = refused.err().expect("a newer schema must refuse the open");
        assert!(matches!(error, AppError::SchemaTooNew(_)), "got {error:?}");
        assert!(error.to_string().contains("0099_from_the_future"));
        assert!(error.to_string().contains("0004_workspace_ownership"), "names what this build knows");

        // Refused BEFORE ownership: the row is still unowned, and the lock was
        // released with the failed attempt so a matching build could open it.
        let conn = db::open_at(&path);
        assert!(matches!(conn, Err(AppError::SchemaTooNew(_))), "plain open is refused too");
        let raw = rusqlite::Connection::open(&path).unwrap();
        let epoch: i64 = raw
            .query_row("SELECT epoch FROM workspace_ownership WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(epoch, 0, "no acquisition happened");
        drop(raw);
        let lock = lease::try_lock_workspace(path.parent().unwrap()).expect("lock released after the refusal");
        drop(lock);
    }

    /// A sink for the stale-runner test: nothing may be emitted because
    /// nothing may be written.
    struct NullSink;

    impl crate::discovery_runner::DiscoveryEventSink for NullSink {
        fn emit(&self, _event: &crate::discovery_runner::DiscoveryEvent) -> Result<(), String> {
            panic!("a stale runner must not emit");
        }
    }
}
