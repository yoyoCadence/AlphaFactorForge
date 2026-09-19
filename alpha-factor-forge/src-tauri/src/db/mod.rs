// SKELETON — DB connection + migration runner.
// FULL parts: connection open, migration application, schema_version tracking.
// Verify: cargo check; runtime verified locally via `cargo tauri dev`.
//
// P02 (docs/research-runtime-contract.md §0/§5): this module knows nothing
// about the host. The desktop resolves its app-data directory in `main.rs` and
// passes a path here; a future service binary passes its own. The file name
// and the pragma order are part of what the CI native smoke lane asserts (the
// database at `<app_data_dir>/alphafactorforge.sqlite3` plus a WAL sidecar
// carrying the migrations), so neither moves.

pub mod discovery;
#[cfg(test)]
mod discovery_tests;
pub mod ownership;
pub mod repositories;
pub mod runtime_ledger;
pub(crate) mod validation_record;

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::AppResult;

/// The workspace database file inside the host's data directory. Fixed: the
/// desktop's existing databases live under this name and are never moved.
pub const DB_FILE_NAME: &str = "alphafactorforge.sqlite3";

/// How long a connection waits on a locked database before failing
/// (`ownership-lease-v1` §1.2 step 2). Today one process holds one
/// connection, so this only matters once a second host (P03/P04) exists —
/// it is set now so that host does not have to remember it.
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Ordered migrations. Each is applied once; applied versions are tracked
/// in the `schema_migrations` table. ADD new migrations to the END only.
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("../../migrations/0001_init.sql")),
    (
        "0002_validation_records",
        include_str!("../../migrations/0002_validation_records.sql"),
    ),
    (
        "0003_discovery_runner",
        include_str!("../../migrations/0003_discovery_runner.sql"),
    ),
    (
        "0004_workspace_ownership",
        include_str!("../../migrations/0004_workspace_ownership.sql"),
    ),
    (
        "0005_runtime_ledger",
        include_str!("../../migrations/0005_runtime_ledger.sql"),
    ),
    (
        "0006_request_effects",
        include_str!("../../migrations/0006_request_effects.sql"),
    ),
    (
        "0007_research_history",
        include_str!("../../migrations/0007_research_history.sql"),
    ),
];

/// Open (creating if needed) the workspace database at `db_path` and run
/// pending migrations. The parent directory is created; `journal_mode=WAL`
/// is set BEFORE the migrations (the CI smoke lane relies on the schema
/// therefore landing in the WAL sidecar), then `foreign_keys=ON` and the
/// busy timeout.
pub fn open_at(db_path: &Path) -> AppResult<Connection> {
    if let Some(dir) = db_path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(BUSY_TIMEOUT)?;

    apply_migrations(&conn)?;
    Ok(conn)
}

/// P04b: open the workspace database WITHOUT migrating — for a host that
/// does not own the workspace (the desktop in connect mode, contract §1.1)
/// and therefore may not run migrations or recovery. The same pragmas as
/// `open_at`; the schema must be exactly what this build knows: a version
/// this build does not know is `SchemaTooNew`, a known migration not yet
/// applied is `SchemaPending` (the owner runs an older build and must be
/// stopped so a current build can migrate). The file must already exist:
/// creating an empty workspace is the owner's job.
pub fn open_migrated(db_path: &Path) -> AppResult<Connection> {
    if !db_path.is_file() {
        return Err(crate::error::AppError::Other(format!(
            "no workspace database at {}; the owner has not created it",
            db_path.display()
        )));
    }
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    let has_table: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    let applied: Vec<String> = if has_table {
        let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        rows
    } else {
        Vec::new()
    };
    let unknown: Vec<&str> = applied
        .iter()
        .map(String::as_str)
        .filter(|version| !MIGRATIONS.iter().any(|(known, _)| known == version))
        .collect();
    if !unknown.is_empty() {
        return Err(crate::error::AppError::SchemaTooNew(format!(
            "this build knows migrations up to {}, but the database already has {}",
            MIGRATIONS.last().map(|(v, _)| *v).unwrap_or("none"),
            unknown.join(", ")
        )));
    }
    let pending: Vec<&str> = MIGRATIONS
        .iter()
        .map(|(version, _)| *version)
        .filter(|version| !applied.iter().any(|a| a == version))
        .collect();
    if !pending.is_empty() {
        return Err(crate::error::AppError::SchemaPending(format!(
            "the database still needs {}; only the workspace owner may migrate it",
            pending.join(", ")
        )));
    }
    Ok(conn)
}

/// Apply ONE migration and record its version in the SAME transaction.
///
/// SQLite DDL is transactional, but the version record used to be a separate
/// statement. Without this a migration that failed partway (say the second of
/// several `ALTER`s) would leave half a schema behind AND no version row — and
/// every retry would then die on "duplicate column name", leaving the database
/// permanently unupgradeable.
///
/// Extracted so the regression test can drive the REAL code path with a
/// deliberately broken migration, instead of re-implementing the transaction
/// and testing its own copy.
pub(crate) fn apply_one_migration(conn: &Connection, version: &str, sql: &str) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(sql)?;
    tx.execute(
        "INSERT INTO schema_migrations (version) VALUES (?1)",
        [version],
    )?;
    tx.commit()?;
    Ok(())
}

/// Create the bookkeeping table, refuse a database written by a NEWER build,
/// then apply any migration not yet recorded.
///
/// The refusal (P03a, contract §5.2): a version in `schema_migrations` that
/// this binary does not know means a later build has already migrated the
/// file. Opening it here would let old code write against a schema it has
/// never seen, so the whole open fails instead — reads included, which is
/// stricter than the contract's "must refuse to write" and deliberately so.
pub fn apply_migrations(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

    let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let applied = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    let unknown: Vec<String> = applied
        .into_iter()
        .filter(|version| !MIGRATIONS.iter().any(|(known, _)| known == version))
        .collect();
    if !unknown.is_empty() {
        return Err(crate::error::AppError::SchemaTooNew(format!(
            "this build knows migrations up to {}, but the database already has {}",
            MIGRATIONS.last().map(|(v, _)| *v).unwrap_or("none"),
            unknown.join(", ")
        )));
    }

    for (version, sql) in MIGRATIONS {
        let already: bool = conn
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1",
                [version],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if already {
            continue;
        }
        apply_one_migration(conn, version, sql)?;
    }
    Ok(())
}
