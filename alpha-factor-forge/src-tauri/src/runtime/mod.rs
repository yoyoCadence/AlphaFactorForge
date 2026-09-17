//! P02 — host-agnostic application runtime (docs/research-runtime-contract.md).
//!
//! Everything a host must do to bring a workspace up — open the database at a
//! path, apply migrations, build the discovery runner, and repair orphaned
//! work — lives here once, so the desktop (`main.rs` setup) and a future
//! headless service binary (P04) reach the same state through the same code.
//!
//! What this module deliberately does NOT know: where the data directory is
//! (the host resolves it), how events reach a UI (`DiscoveryEventSink` is a
//! trait the host implements), or anything about scheduling. Nothing here
//! imports `tauri`, and `boundary_tests` keeps it — and the modules it
//! composes — that way.

#[cfg(test)]
mod boundary_tests;

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::db::{self, discovery::RecoveryReport};
use crate::discovery_runner::DiscoveryRunner;
use crate::error::AppResult;

/// Shared database handle: one connection, one coordinator/writer at a time.
pub type SharedDb = Arc<Mutex<rusqlite::Connection>>;

/// An opened, migrated, and repaired workspace.
pub struct Workspace {
    pub db: SharedDb,
    pub discovery: DiscoveryRunner,
    /// What startup repair did. Persistence-only: orphaned `running` work is
    /// paused/requeued and no CPU work resumes without a user command.
    pub recovery: RecoveryReport,
}

/// Open the workspace database at `db_path` (see `db::open_at`), build the
/// runner, and run orphan recovery — in that order, exactly as the desktop
/// did before P02. The path is the host's only input.
pub fn open_workspace(db_path: &Path) -> AppResult<Workspace> {
    let conn = db::open_at(db_path)?;
    let db: SharedDb = Arc::new(Mutex::new(conn));
    let discovery = DiscoveryRunner::default();
    let recovery = discovery.recover_orphans(&db)?;
    Ok(Workspace { db, discovery, recovery })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

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

    fn pragma<T: rusqlite::types::FromSql>(conn: &rusqlite::Connection, name: &str) -> T {
        conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
            .expect("pragma readable")
    }

    #[test]
    fn open_workspace_creates_the_directory_migrates_and_sets_the_connection_pragmas() {
        let path = fresh_db_path();
        assert!(!path.parent().unwrap().exists(), "the directory must not pre-exist");

        let workspace = open_workspace(&path).expect("fresh open");
        assert!(path.is_file(), "database file created at the given path");
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
        assert_eq!(applied, 3, "0001–0003 applied on first open");
        drop(conn);

        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn reopening_the_same_path_is_idempotent_and_keeps_stored_rows() {
        let path = fresh_db_path();
        let first = open_workspace(&path).expect("first open");
        first
            .db
            .lock()
            .unwrap()
            .execute("INSERT INTO app_settings (key, value_json) VALUES ('p02', '\"kept\"')", [])
            .expect("write a row");
        drop(first);

        let second = open_workspace(&path).expect("second open of the same file");
        let conn = second.db.lock().unwrap();
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, 3, "no migration is re-applied");
        let value: String = conn
            .query_row("SELECT value_json FROM app_settings WHERE key = 'p02'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "\"kept\"");
        drop(conn);

        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn open_workspace_repairs_an_orphaned_running_run_before_returning() {
        let path = fresh_db_path();
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

        let workspace = open_workspace(&path).expect("open with an orphan present");
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

        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }
}
