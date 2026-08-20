// File import/export commands. Slice 7-2 writes already-formatted JSON/CSV
// reports from the typed frontend wrapper; broader backup/restore remains later.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

#[tauri::command]
pub fn save_report(
    app: AppHandle,
    suggested_filename: String,
    contents: String,
) -> AppResult<String> {
    let dir = app
        .path()
        .download_dir()
        .map_err(|e| AppError::Other(format!("no downloads dir: {e}")))?;
    std::fs::create_dir_all(&dir)?;

    let file_name = safe_report_filename(&suggested_filename)?;
    let path = write_new_report(&dir, &file_name, &contents)?;
    Ok(path.to_string_lossy().into_owned())
}

/// TODO(local): render a backtest result to a report file (JSON/CSV/HTML) and
/// return the written path. The browser version already builds JSON/CSV reports;
/// port that formatting here.
#[tauri::command]
pub fn export_report(_result_id: i64) -> AppResult<String> {
    Err(AppError::NotImplemented(
        "export_report: port report formatting from the existing UI (TODO.md)",
    ))
}

fn safe_report_filename(input: &str) -> AppResult<String> {
    let raw_name = Path::new(input)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim();
    let cleaned: String = raw_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();

    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return Err(AppError::Other("save_report: empty filename".into()));
    }
    if !(cleaned.ends_with(".json") || cleaned.ends_with(".csv")) {
        return Err(AppError::Other(
            "save_report: filename must end in .json or .csv".into(),
        ));
    }
    Ok(cleaned)
}

/// Numeric suffixes tried before giving up on finding an unused name.
const MAX_REPORT_SUFFIX: u32 = 1000;

/// Create and write a report under the first name that does not already exist.
///
/// The naming and the creation are ONE atomic step, which is the whole point.
/// This used to pick a name with `exists()` and then call `std::fs::write`, and
/// `write` truncates: any file created in the window between the check and the
/// write was silently destroyed, so two exports finishing together could clobber
/// one another and the loser was never told (IO-ROBUSTNESS-001).
///
/// `create_new` closes that window — the OS refuses the open if the path appeared
/// meanwhile — so a collision becomes a retry under the next candidate name
/// instead of data loss. Exhausting the candidates is an error rather than a
/// timestamped last resort: after this many same-named reports the caller needs
/// to hear about it, and a timestamp could collide too.
///
/// Private on purpose. It takes an arbitrary `file_name` and hands it straight
/// to `dir.join`, and `join` REPLACES the base when given an absolute path — so
/// a `pub(crate)` version would let any backend module escape the downloads
/// directory and the `.json`/`.csv` contract that `safe_report_filename`
/// enforces. `save_report` sanitises first and is the only caller; the nested
/// test module can still reach a private parent item. If this ever needs crate
/// visibility, move the filename validation in here instead of widening the
/// boundary (PR #105 review).
fn write_new_report(dir: &Path, file_name: &str, contents: &str) -> AppResult<PathBuf> {
    write_new_report_within(dir, file_name, contents, MAX_REPORT_SUFFIX)
}

fn write_new_report_within(
    dir: &Path,
    file_name: &str,
    contents: &str,
    limit: u32,
) -> AppResult<PathBuf> {
    let as_path = Path::new(file_name);
    let stem = as_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("report");
    let ext = as_path.extension().and_then(|s| s.to_str()).unwrap_or("");

    for attempt in 0..limit {
        let candidate = if attempt == 0 {
            dir.join(file_name)
        } else {
            dir.join(format!("{stem}-{attempt}.{ext}"))
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(contents.as_bytes())?;
                return Ok(candidate);
            }
            // Taken since the last candidate was considered — by an earlier
            // export or by a writer running right now. Both are the same thing
            // here, and neither may be overwritten.
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(AppError::Other(format!(
        "save_report: no unused name for \"{file_name}\" after {limit} attempts"
    )))
}


#[cfg(test)]
mod tests {
    use super::{safe_report_filename, write_new_report, write_new_report_within};
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aff-report-{label}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn name_of(path: &PathBuf) -> String {
        path.file_name().unwrap().to_str().unwrap().to_owned()
    }

    #[test]
    fn safe_report_filename_keeps_only_json_or_csv_basename() {
        assert_eq!(
            safe_report_filename("AlphaFactorForge_BTC-USDT_1h.json").unwrap(),
            "AlphaFactorForge_BTC-USDT_1h.json"
        );
        assert_eq!(
            safe_report_filename("nested/Alpha Factor.csv").unwrap(),
            "Alpha-Factor.csv"
        );
        assert!(safe_report_filename("report.txt").is_err());
        assert!(safe_report_filename("../").is_err());
    }

    #[test]
    fn write_new_report_takes_the_plain_name_then_numbered_suffixes() {
        let dir = temp_dir("suffix");

        let first = write_new_report(&dir, "report.json", "one").unwrap();
        let second = write_new_report(&dir, "report.json", "two").unwrap();
        let third = write_new_report(&dir, "report.json", "three").unwrap();

        assert_eq!(name_of(&first), "report.json");
        assert_eq!(name_of(&second), "report-1.json");
        assert_eq!(name_of(&third), "report-2.json");
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "two");
        assert_eq!(std::fs::read_to_string(&third).unwrap(), "three");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn write_new_report_never_truncates_an_existing_file() {
        let dir = temp_dir("no-truncate");
        std::fs::write(dir.join("report.csv"), "OLD").unwrap();

        let written = write_new_report(&dir, "report.csv", "NEW").unwrap();

        assert_eq!(name_of(&written), "report-1.csv");
        assert_eq!(
            std::fs::read_to_string(dir.join("report.csv")).unwrap(),
            "OLD",
            "the existing report must survive untouched"
        );
        assert_eq!(std::fs::read_to_string(&written).unwrap(), "NEW");

        std::fs::remove_dir_all(dir).unwrap();
    }

    /// The coordinated two-writer regression. A barrier releases every writer at
    /// the same instant, so the check-then-write version this replaced would
    /// have several threads agree on one candidate name and then truncate each
    /// other; `create_new` turns that agreement into retries instead.
    ///
    /// Every assertion is about the OUTCOME, not the interleaving: N distinct
    /// paths, N files, and each file holding exactly what its own writer passed.
    #[test]
    fn concurrent_writers_never_overwrite_each_other() {
        const WRITERS: usize = 8;
        let dir = temp_dir("concurrent");
        let barrier = Arc::new(Barrier::new(WRITERS));

        let handles: Vec<_> = (0..WRITERS)
            .map(|index| {
                let dir = dir.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let contents = format!("writer-{index}");
                    barrier.wait();
                    let path = write_new_report(&dir, "race.json", &contents).unwrap();
                    (path, contents)
                })
            })
            .collect();

        let mut paths = Vec::new();
        for handle in handles {
            let (path, contents) = handle.join().expect("writer thread panicked");
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                contents,
                "each writer's own bytes must survive"
            );
            paths.push(path);
        }

        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), WRITERS, "every writer got its own path");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            WRITERS,
            "one file per writer, nothing merged or lost"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn write_new_report_reports_exhaustion_instead_of_overwriting() {
        let dir = temp_dir("exhausted");
        for name in ["report.json", "report-1.json", "report-2.json"] {
            std::fs::write(dir.join(name), name).unwrap();
        }

        let error = write_new_report_within(&dir, "report.json", "NEW", 3)
            .expect_err("an exhausted name space must fail loudly");
        assert!(error.to_string().contains("no unused name"));

        // Nothing was overwritten on the way to that error.
        for name in ["report.json", "report-1.json", "report-2.json"] {
            assert_eq!(std::fs::read_to_string(dir.join(name)).unwrap(), name);
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);

        std::fs::remove_dir_all(dir).unwrap();
    }
}
