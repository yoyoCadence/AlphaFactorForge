//! Disposable proposed-state staging from the caller's actual fixed snapshot.
//! No receiver mutation/commit, export-size ceiling or whole-database allocation.
use super::LedgerError;
use rusqlite::{types::Value, Connection, Transaction};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn bad(detail: &str) -> LedgerError {
    LedgerError::StateInvalid(format!("registry preflight: {detail}"))
}
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

struct Temporary {
    path: Option<PathBuf>,
    base: PathBuf,
}
impl Temporary {
    fn new() -> Result<Self, LedgerError> {
        let base = fs::canonicalize(std::env::temp_dir())?;
        for _ in 0..8 {
            let mut random = [0u8; 16];
            getrandom::fill(&mut random)
                .map_err(|_| bad("temporary-name randomness unavailable"))?;
            let path = base.join(format!(
                "aff-registry-preflight-{}-{}",
                std::process::id(),
                hex::encode(random)
            ));
            let mut builder = fs::DirBuilder::new();
            builder.recursive(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => {
                    let owned = Self {
                        path: Some(path),
                        base,
                    };
                    let mut options = fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.mode(0o600);
                    }
                    drop(options.open(owned.database())?);
                    return Ok(owned);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(bad("cannot allocate unique private temporary directory"))
    }
    fn database(&self) -> PathBuf {
        self.path
            .as_ref()
            .expect("owned directory")
            .join("proposal.sqlite3")
    }
    fn remove(mut self) -> Result<(), LedgerError> {
        self.remove_path()?;
        self.path = None;
        Ok(())
    }
    fn remove_path(&self) -> std::io::Result<()> {
        let original = self.path.as_ref().expect("owned directory");
        let resolved = fs::canonicalize(original)?;
        if resolved != *original || resolved.parent() != Some(self.base.as_path()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "owned preflight cleanup path changed",
            ));
        }
        fs::remove_dir_all(resolved)
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        // Only the random directory atomically created above is ever removed.
        if self.path.is_some() {
            let _ = self.remove_path();
        }
    }
}

struct Definition {
    kind: String,
    name: String,
    sql: String,
}

/// Clone actual LOCAL schema/typed rows, never SQL supplied by an archive.
/// Source transaction remains open. Optimizer statistics are not ledger facts.
fn copy(source: &Transaction<'_>, destination: &mut Connection) -> Result<(), LedgerError> {
    if source.is_autocommit() {
        return Err(bad("source transaction is not active"));
    }
    let definitions:Vec<Definition>=source.prepare("SELECT type,name,sql FROM sqlite_schema WHERE sql IS NOT NULL AND name NOT GLOB 'sqlite_stat[0-9]*' ORDER BY type,name")?
        .query_map([],|r|Ok(Definition{kind:r.get(0)?,name:r.get(1)?,sql:r.get(2)?}))?.collect::<Result<_,_>>()?;
    // Foreign keys can cycle in complete confirmation history. Enable them
    // after the private clone transaction, then check before proposal writes.
    destination.execute_batch("PRAGMA foreign_keys=OFF")?;
    let tx = destination.transaction()?;
    for definition in definitions
        .iter()
        .filter(|d| d.kind == "table" && d.name != "sqlite_sequence")
    {
        tx.execute(&definition.sql, [])?;
    }
    for definition in definitions
        .iter()
        .filter(|d| d.kind == "table" && d.name != "sqlite_sequence")
    {
        copy_table(source, &tx, &definition.name)?;
    }
    if definitions.iter().any(|d| d.name == "sqlite_sequence") {
        // Explicit original high-water marks can exceed MAX(current row ID).
        tx.execute("DELETE FROM sqlite_sequence", [])?;
        copy_table(source, &tx, "sqlite_sequence")?;
    }
    for kind in ["index", "view", "trigger"] {
        for definition in definitions.iter().filter(|d| d.kind == kind) {
            tx.execute(&definition.sql, [])?;
        }
    }
    tx.commit()?;
    let recursive: i64 = source.query_row("PRAGMA recursive_triggers", [], |r| r.get(0))?;
    destination.pragma_update(None, "recursive_triggers", recursive)?;
    destination.execute_batch("PRAGMA foreign_keys=ON")?;
    if destination
        .prepare("PRAGMA foreign_key_check")?
        .exists([])?
    {
        return Err(bad("copied source foreign-key dependency missing"));
    }
    if source.is_autocommit() {
        return Err(bad("source snapshot ended during copy"));
    }
    Ok(())
}

fn copy_table(
    source: &Connection,
    destination: &Transaction<'_>,
    name: &str,
) -> Result<(), LedgerError> {
    let name_sql = quoted(name);
    let columns: Vec<(String, i64, i64)> = source
        .prepare(&format!("PRAGMA table_xinfo({name_sql})"))?
        .query_map([], |r| Ok((r.get(1)?, r.get(5)?, r.get(6)?)))?
        .collect::<Result<_, _>>()?;
    let writable: Vec<String> = columns
        .iter()
        .filter(|(_, _, hidden)| *hidden == 0)
        .map(|(name, _, _)| quoted(name))
        .collect();
    let without_rowid: bool = source.query_row(
        "SELECT wr FROM pragma_table_list WHERE schema='main' AND name=?1",
        [name],
        |r| r.get(0),
    )?;
    let (projection, order) = if without_rowid {
        let mut pk: Vec<_> = columns.iter().filter(|(_, pk, _)| *pk > 0).collect();
        pk.sort_by_key(|(_, pk, _)| *pk);
        (
            writable.join(","),
            pk.iter()
                .map(|(name, _, _)| quoted(name))
                .collect::<Vec<_>>()
                .join(","),
        )
    } else {
        let alias = ["rowid", "_rowid_", "oid"]
            .into_iter()
            .find(|alias| {
                !columns
                    .iter()
                    .any(|(name, _, _)| name.eq_ignore_ascii_case(alias))
            })
            .ok_or_else(|| bad("all intrinsic row-ID aliases are shadowed"))?;
        (
            std::iter::once(quoted(alias))
                .chain(writable)
                .collect::<Vec<_>>()
                .join(","),
            quoted(alias),
        )
    };
    let mut read = source.prepare(&format!(
        "SELECT {projection} FROM {name_sql} ORDER BY {order}"
    ))?;
    let width = read.column_count();
    let marks = vec!["?"; width].join(",");
    let mut write = destination.prepare(&format!(
        "INSERT INTO {name_sql} ({projection}) VALUES ({marks})"
    ))?;
    let mut rows = read.query([])?;
    while let Some(row) = rows.next()? {
        // At most one row is owned; large pre-existing primary history is
        // streamed to disk without re-exporting or collecting the whole DB.
        let values: Vec<Value> = (0..width).map(|i| row.get(i)).collect::<Result<_, _>>()?;
        write.execute(rusqlite::params_from_iter(values.iter()))?;
    }
    Ok(())
}

fn with_database<T>(
    source: &Transaction<'_>,
    path: &Path,
    inspect: impl FnOnce(&Transaction<'_>) -> Result<T, LedgerError>,
) -> Result<T, LedgerError> {
    let mut conn = Connection::open(path)?;
    let result = (|| {
        copy(source, &mut conn)?;
        let proposal = conn.transaction()?;
        let result = inspect(&proposal).and_then(|value| {
            if proposal.prepare("PRAGMA foreign_key_check")?.exists([])? {
                return Err(bad("proposed foreign-key dependency missing"));
            }
            Ok(value)
        });
        proposal.rollback()?;
        if source.is_autocommit() {
            return Err(bad("caller transaction ended during proposal"));
        }
        result
    })();
    conn.close().map_err(|(conn, error)| {
        drop(conn);
        LedgerError::Db(error)
    })?;
    result
}
fn with_owned<T>(
    source: &Transaction<'_>,
    owned: Temporary,
    inspect: impl FnOnce(&Transaction<'_>) -> Result<T, LedgerError>,
) -> Result<T, LedgerError> {
    let result = with_database(source, &owned.database(), inspect);
    // Even callback/copy errors explicitly clean up before returning. Drop is
    // the panic/early-failure backstop; cleanup errors never allow real writes.
    owned.remove()?;
    result
}

/// Caller validates source/current evidence and owns the real transaction
/// (IMMEDIATE for final import). Run the SAME union/final checks privately,
/// then return owned results only after rollback, close and owned cleanup.
/// Does not commit, mutate or release the caller's transaction. No live caller
/// until the next atomic-union section consumes this staging primitive.
pub(super) fn with_snapshot<T>(
    source: &Transaction<'_>,
    inspect: impl FnOnce(&Transaction<'_>) -> Result<T, LedgerError>,
) -> Result<T, LedgerError> {
    with_owned(source, Temporary::new()?, inspect)
}

#[cfg(test)]
#[path = "trial_ledger_preflight_tests.rs"]
mod tests;
