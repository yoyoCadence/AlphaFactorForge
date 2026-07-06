// File import/export commands. Slice 7-2 writes already-formatted JSON/CSV
// reports from the typed frontend wrapper; broader backup/restore remains later.

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
    let path = unique_report_path(&dir, &file_name);
    std::fs::write(&path, contents)?;
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

fn unique_report_path(dir: &Path, file_name: &str) -> PathBuf {
    let first = dir.join(file_name);
    if !first.exists() {
        return first;
    }

    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("report");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for n in 1..1000 {
        let candidate = dir.join(format!("{stem}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!(
        "{stem}-{}.{}",
        chrono::Utc::now().timestamp_millis(),
        ext
    ))
}

#[cfg(test)]
mod tests {
    use super::{safe_report_filename, unique_report_path};

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
    fn unique_report_path_adds_suffix_when_needed() {
        let dir = std::env::temp_dir().join(format!(
            "aff-report-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("report.json"), "{}").unwrap();

        let path = unique_report_path(&dir, "report.json");
        assert_eq!(
            path.file_name().and_then(|s| s.to_str()),
            Some("report-1.json")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }
}
