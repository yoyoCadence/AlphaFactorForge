//! P05 — the immutable artifact store (plan §3.3: "artifacts are staged,
//! verified, renamed atomically, and only then referenced from the
//! database; unreferenced files are identifiable, referenced files are never
//! silently deleted").
//!
//! Content-addressed: a file lives at `<root>/<sha256[0..2]>/<sha256>.json`
//! and its name IS its checksum, so a reader can prove the bytes are the
//! ones that were written. Writing is stage → hash → fsync → rename, so a
//! crash leaves either nothing or a complete file (plus, at worst, a stale
//! `staging/` entry, which `unreferenced` reports). There is no delete
//! operation at all.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

use super::sha256_hex;

/// The artifact directory's name inside the workspace data directory.
pub const ARTIFACTS_DIR_NAME: &str = "artifacts";
const STAGING_DIR_NAME: &str = "staging";
const FILE_EXTENSION: &str = "json";

/// What the database references (`research_artifacts` row).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRef {
    pub kind: String,
    pub sha256: String,
    pub byte_len: i64,
    /// Relative to the store root, with `/` separators.
    pub relative_path: String,
}

#[derive(Clone, Debug)]
pub struct ArtifactStore {
    root: PathBuf,
}

impl ArtifactStore {
    /// The store under `<data_dir>/artifacts`.
    pub fn in_data_dir(data_dir: &Path) -> Self {
        Self { root: data_dir.join(ARTIFACTS_DIR_NAME) }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Store `bytes` immutably and return the reference to record. Writing
    /// the same content twice yields the same reference and leaves the
    /// existing file untouched.
    pub fn put(&self, kind: &str, bytes: &[u8]) -> AppResult<ArtifactRef> {
        self.put_with_extension(kind, FILE_EXTENSION, bytes)
    }

    /// `put` for content that is not JSON — P06 keeps raw market responses
    /// (CSV, ZIP, JSON bodies) byte for byte, and a file named `.json` that
    /// holds a ZIP would lie to whoever opens the store. The extension is
    /// part of the path, never part of the identity: the name is still the
    /// checksum.
    pub fn put_with_extension(
        &self,
        kind: &str,
        extension: &str,
        bytes: &[u8],
    ) -> AppResult<ArtifactRef> {
        if extension.is_empty()
            || extension.len() > 8
            || !extension.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(AppError::Other(format!(
                "artifact extension {extension:?} is not a short lowercase alphanumeric suffix"
            )));
        }
        let sha256 = sha256_hex(bytes);
        let relative_path = format!("{}/{sha256}.{extension}", &sha256[..2]);
        let final_path = self.root.join(&sha256[..2]).join(format!("{sha256}.{extension}"));
        if final_path.is_file() {
            // Already stored: verify rather than trust the name.
            let existing = fs::read(&final_path)?;
            if sha256_hex(&existing) != sha256 {
                return Err(AppError::Other(format!(
                    "artifact {} exists but its content does not match its name",
                    final_path.display()
                )));
            }
        } else {
            let staging_dir = self.root.join(STAGING_DIR_NAME);
            fs::create_dir_all(&staging_dir)?;
            fs::create_dir_all(final_path.parent().expect("artifact path has a parent"))?;
            let staged = staging_dir.join(format!("{sha256}-{}.tmp", std::process::id()));
            {
                let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(&staged)?;
                file.write_all(bytes)?;
                file.sync_all()?;
            }
            // Verify what landed on disk before it becomes the artifact.
            let written = fs::read(&staged)?;
            if sha256_hex(&written) != sha256 {
                let _ = fs::remove_file(&staged);
                return Err(AppError::Other(format!("artifact staging at {} did not round-trip", staged.display())));
            }
            match fs::rename(&staged, &final_path) {
                Ok(()) => {}
                // A concurrent writer of the same content won the rename:
                // the file is there and, by construction, identical.
                Err(_) if final_path.is_file() => {
                    let _ = fs::remove_file(&staged);
                }
                Err(error) => return Err(AppError::Io(error)),
            }
        }
        Ok(ArtifactRef {
            kind: kind.to_string(),
            sha256,
            byte_len: bytes.len() as i64,
            relative_path,
        })
    }

    /// Read an artifact and prove it is the content its reference names.
    pub fn read(&self, reference: &ArtifactRef) -> AppResult<Vec<u8>> {
        let path = self.path_of(&reference.relative_path)?;
        let mut bytes = Vec::with_capacity(reference.byte_len.max(0) as usize);
        File::open(&path)
            .map_err(|error| AppError::Other(format!("artifact {} cannot be opened: {error}", path.display())))?
            .read_to_end(&mut bytes)?;
        if sha256_hex(&bytes) != reference.sha256 || bytes.len() as i64 != reference.byte_len {
            return Err(AppError::Other(format!(
                "artifact {} does not match its recorded checksum; it was altered or truncated",
                path.display()
            )));
        }
        Ok(bytes)
    }

    /// The absolute path of a relative artifact path, refusing anything that
    /// would leave the store.
    pub fn path_of(&self, relative_path: &str) -> AppResult<PathBuf> {
        let relative = Path::new(relative_path);
        let escapes = relative.is_absolute()
            || relative.components().any(|component| {
                !matches!(component, std::path::Component::Normal(_))
            });
        if escapes || relative_path.is_empty() {
            return Err(AppError::Other(format!("artifact path {relative_path:?} is not inside the store")));
        }
        Ok(self.root.join(relative))
    }

    /// Every file under the store (relative paths, `/` separators) that is
    /// not in `referenced` — stale staging entries included. Identification
    /// only: nothing here deletes.
    pub fn unreferenced(&self, referenced: &[String]) -> io::Result<Vec<String>> {
        let mut found = Vec::new();
        if !self.root.is_dir() {
            return Ok(found);
        }
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(&self.root)
                    .map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
                    .unwrap_or_default();
                if !referenced.iter().any(|known| known == &relative) {
                    found.push(relative);
                }
            }
        }
        found.sort();
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn fresh_root() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("aff-artifacts-test-{}-{n}", std::process::id()))
    }

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            if self.0.exists() {
                fs::remove_dir_all(&self.0)
                    .unwrap_or_else(|error| panic!("temp dir {} not removed: {error}", self.0.display()));
            }
        }
    }

    #[test]
    fn put_is_content_addressed_idempotent_and_leaves_no_staging_file() {
        let data_dir = fresh_root();
        let _guard = TempDir(data_dir.clone());
        let store = ArtifactStore::in_data_dir(&data_dir);
        let first = store.put("candidate-result-v1", br#"{"a":1}"#).unwrap();
        assert_eq!(first.sha256.len(), 64);
        assert_eq!(first.byte_len, 7);
        assert_eq!(first.relative_path, format!("{}/{}.json", &first.sha256[..2], first.sha256));
        assert!(store.path_of(&first.relative_path).unwrap().is_file());
        let again = store.put("candidate-result-v1", br#"{"a":1}"#).unwrap();
        assert_eq!(again, first, "same content, same reference");
        let other = store.put("candidate-result-v1", br#"{"a":2}"#).unwrap();
        assert_ne!(other.sha256, first.sha256);
        let staging: Vec<_> = fs::read_dir(store.root().join(STAGING_DIR_NAME)).unwrap().collect();
        assert!(staging.is_empty(), "staging is empty after every put");
        assert_eq!(store.read(&first).unwrap(), br#"{"a":1}"#);
    }

    /// P06: raw market responses are not JSON, and the file name must not
    /// claim they are. The extension changes the path, never the identity.
    #[test]
    fn a_raw_artifact_keeps_its_own_extension_and_a_bad_one_is_refused() {
        let data_dir = fresh_root();
        let _guard = TempDir(data_dir.clone());
        let store = ArtifactStore::in_data_dir(&data_dir);
        let csv = store
            .put_with_extension("market-raw-v1", "csv", b"open,high,low,close\n1,2,0.5,1.5\n")
            .unwrap();
        assert!(csv.relative_path.ends_with(".csv"));
        assert_eq!(csv.relative_path, format!("{}/{}.csv", &csv.sha256[..2], csv.sha256));
        assert!(store.path_of(&csv.relative_path).unwrap().is_file());
        assert_eq!(store.read(&csv).unwrap(), b"open,high,low,close\n1,2,0.5,1.5\n");
        // The same bytes under the default extension are a different path and
        // the same checksum: the extension is not part of the identity.
        let as_json = store.put("market-raw-v1", b"open,high,low,close\n1,2,0.5,1.5\n").unwrap();
        assert_eq!(as_json.sha256, csv.sha256);
        assert_ne!(as_json.relative_path, csv.relative_path);
        for bad in ["", "JSON", "tar.gz", "with space", "verylongextension", "j/s"] {
            assert!(store.put_with_extension("market-raw-v1", bad, b"x").is_err(), "{bad:?}");
        }
    }

    #[test]
    fn read_refuses_an_altered_or_truncated_artifact_and_paths_outside_the_store() {
        let data_dir = fresh_root();
        let _guard = TempDir(data_dir.clone());
        let store = ArtifactStore::in_data_dir(&data_dir);
        let reference = store.put("candidate-result-v1", b"original").unwrap();
        fs::write(store.path_of(&reference.relative_path).unwrap(), b"altered!").unwrap();
        let error = store.read(&reference).unwrap_err().to_string();
        assert!(error.contains("altered or truncated"), "{error}");
        for bad in ["../x.json", "/abs.json", "", "a/../../b.json"] {
            assert!(store.path_of(bad).is_err(), "{bad:?}");
        }
        // A file whose name lies about its content is refused on put too.
        let path = store.path_of(&reference.relative_path).unwrap();
        fs::write(&path, b"altered!").unwrap();
        assert!(store.put("candidate-result-v1", b"original").unwrap_err().to_string().contains("does not match"));
    }

    #[test]
    fn unreferenced_lists_files_the_database_does_not_know_including_stale_staging() {
        let data_dir = fresh_root();
        let _guard = TempDir(data_dir.clone());
        let store = ArtifactStore::in_data_dir(&data_dir);
        assert_eq!(store.unreferenced(&[]).unwrap(), Vec::<String>::new(), "no directory yet");
        let kept = store.put("candidate-result-v1", b"kept").unwrap();
        let orphan = store.put("candidate-result-v1", b"orphan").unwrap();
        fs::write(store.root().join(STAGING_DIR_NAME).join("crashed-123.tmp"), b"partial").unwrap();
        let listed = store.unreferenced(std::slice::from_ref(&kept.relative_path)).unwrap();
        assert_eq!(listed, vec![orphan.relative_path.clone(), "staging/crashed-123.tmp".to_string()]);
        assert!(store.path_of(&orphan.relative_path).unwrap().is_file(), "identified, not deleted");
    }
}
