// FULL — shared error type for command results.
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    // ---- ownership (P03a, research-runtime-contract §1 / §2 error codes) ----
    /// This process does not hold the workspace lease (another host does).
    #[error("not owner: {0}")]
    NotOwner(String),
    /// The lease moved to a newer epoch after this writer started.
    #[error("stale owner: {0}")]
    StaleOwner(String),
    /// The database was written by a newer build than this one.
    #[error("schema newer than this build: {0}")]
    SchemaTooNew(String),
    /// P04b: a non-owner opened a database that still needs migrations
    /// this build knows; only the lease holder may apply them.
    #[error("schema behind this build: {0}")]
    SchemaPending(String),
    #[error("{0}")]
    Other(String),
}

// Tauri requires command errors to be Serialize. Emit a flat string.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
