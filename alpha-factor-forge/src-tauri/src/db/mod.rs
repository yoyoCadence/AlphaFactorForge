// SKELETON — DB connection + migration runner.
// FULL parts: connection open, migration application, schema_version tracking.
// Verify: cargo check; runtime verified locally via `cargo tauri dev`.

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
        conn.execute_batch(sql)?;
        conn.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            [version],
        )?;
    }
    Ok(())
}
