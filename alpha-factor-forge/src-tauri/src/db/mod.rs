// SKELETON — DB connection + migration runner.
// FULL parts: connection open, migration application, schema_version tracking.
// Verify: cargo check; runtime verified locally via `cargo tauri dev`.

pub mod discovery;
#[cfg(test)]
mod discovery_tests;
pub mod repositories;

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::error::AppResult;

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
];

/// Open the database in the OS app-data dir and run pending migrations.
pub fn initialize(app: &AppHandle) -> AppResult<Connection> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| crate::error::AppError::Other(format!("no app data dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;
    let db_path = dir.join("alphafactorforge.sqlite3");

    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;

    apply_migrations(&conn)?;
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

/// Create the bookkeeping table, then apply any migration not yet recorded.
pub fn apply_migrations(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;

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
