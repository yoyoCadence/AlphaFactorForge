//! P03a — the OS-level half of workspace ownership
//! (docs/research-runtime-contract.md §1.2 step 1, `ownership-lease-v1`).
//!
//! An exclusive advisory lock on `<data dir>/ownership.lock`, taken BEFORE the
//! SQLite file is opened and held for as long as the owning `Workspace`
//! lives. The operating system releases it when the process exits or
//! crashes, which is the only way ownership ever changes hands in P03a: a
//! host that cannot take the lock is told `NotOwner` and never retries or
//! preempts (§1.4). Sleep, clock changes, and stale heartbeats do not release
//! it — a sleeping owner is still the owner.
//!
//! `std::fs::File::try_lock` (Rust 1.89+) is used so no locking crate is
//! added; on Windows it is `LockFileEx`, on Unix `flock`.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// The lock file's name inside the workspace data directory (contract §1.2).
pub const LOCK_FILE_NAME: &str = "ownership.lock";

/// Holds the exclusive lock; dropping it releases the lock.
#[derive(Debug)]
pub struct OsLock {
    _file: File,
    path: PathBuf,
}

impl OsLock {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Try once to take the workspace lock. `NotOwner` when another process holds
/// it; any other I/O problem is reported as such. Creates the directory and
/// the (empty) lock file if they do not exist yet.
pub fn try_lock_workspace(data_dir: &Path) -> AppResult<OsLock> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join(LOCK_FILE_NAME);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(OsLock { _file: file, path }),
        Err(TryLockError::WouldBlock) => Err(AppError::NotOwner(format!(
            "another host holds the workspace lock at {}",
            path.display()
        ))),
        Err(TryLockError::Error(error)) => Err(AppError::Io(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn fresh_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("aff-lease-test-{}-{n}", std::process::id()))
    }

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            if self.0.exists() {
                std::fs::remove_dir_all(&self.0)
                    .unwrap_or_else(|error| panic!("temp dir {} not removed: {error}", self.0.display()));
            }
        }
    }

    #[test]
    fn a_second_holder_is_refused_until_the_first_releases() {
        let dir = fresh_dir();
        let _guard = TempDir(dir.clone());

        let first = try_lock_workspace(&dir).expect("first holder");
        assert!(first.path().is_file());

        // 雙啟: the same process asking again models a second host exactly —
        // the lock is per open file description, not per process.
        let second = try_lock_workspace(&dir);
        assert!(matches!(second, Err(AppError::NotOwner(_))), "got {second:?}");

        // owner crash / exit: dropping the handle is what the OS does for a
        // dead process, and only then can another host take over.
        drop(first);
        let third = try_lock_workspace(&dir).expect("lock free after release");
        drop(third);
    }

    #[test]
    fn locking_creates_the_data_directory_and_lock_file() {
        let dir = fresh_dir().join("deeper");
        let _guard = TempDir(dir.parent().unwrap().to_path_buf());
        assert!(!dir.exists());
        let lock = try_lock_workspace(&dir).expect("lock on a fresh directory");
        assert_eq!(lock.path(), dir.join(LOCK_FILE_NAME));
        drop(lock);
    }
}
