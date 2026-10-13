//! APP-STARTUP-ERROR-001 — tell the user why the desktop did not start.
//!
//! A release build is a GUI-subsystem program (`windows_subsystem = "windows"`
//! in `main.rs`): it has no console, so a panic during startup writes to a
//! stderr nobody sees and the user is left with an app that "does not open".
//! The failures that reach this point are ordinary and recoverable by the user
//! — a database written by a newer build, a workspace another host holds, a
//! missing WebView runtime — so they get a message that names the cause and
//! the workspace.
//!
//! The message is always written to stderr first. The modal dialog replaces
//! the console only where there is none: release builds on Windows. A debug
//! build keeps its console and ends at once, which is what the native smoke
//! lane and a developer's terminal rely on. Other platforms have no dialog
//! here yet and get stderr only.

use std::path::Path;

use crate::error::AppError;
use crate::runtime::connect::ConnectError;
use crate::runtime::host::HostError;

const TITLE: &str = "AlphaFactorForge 無法啟動";

/// The message for a workspace that could not be opened or connected to:
/// what the user can do, then the technical reason and the workspace path.
pub fn workspace_failure_message(data_dir: &Path, error: &HostError) -> String {
    format!(
        "{}\n\n原因：{error}\n工作區：{}",
        workspace_advice(error),
        data_dir.display()
    )
}

/// The message for a failure before any workspace is involved (no data
/// directory, or the window runtime could not start).
pub fn runtime_failure_message(advice: &str, reason: &str) -> String {
    format!("{advice}\n\n原因：{reason}")
}

fn workspace_advice(error: &HostError) -> &'static str {
    match error {
        HostError::CannotOpen(AppError::SchemaTooNew(_))
        | HostError::CannotConnect(ConnectError::Schema(AppError::SchemaTooNew(_))) => {
            "這個工作區的資料庫是由較新版本的 AlphaFactorForge 建立的。請改用該版本或更新的版本開啟；本版本不會修改它。"
        }
        HostError::CannotConnect(ConnectError::Schema(_)) => {
            "工作區目前由較舊版本的背景 service 持有，資料庫尚未升級到本版本。請先停止該 service，再重新開啟。"
        }
        HostError::CannotConnect(ConnectError::NotPublished) => {
            "工作區正被另一個程式使用，而它沒有提供連線端點。請關閉另一個 AlphaFactorForge 或背景 service 後再試。"
        }
        HostError::CannotConnect(ConnectError::Unreachable(_)) => {
            "找到背景 service 的連線資訊，但它沒有回應。請確認 service 是否仍在執行，或結束它後再重新開啟。"
        }
        HostError::CannotConnect(ConnectError::IdentityMismatch(_)) => {
            "工作區的連線資訊已過期，屬於另一個執行個體。請結束殘留的背景 service 後再重新開啟。"
        }
        HostError::CannotConnect(ConnectError::Other(_)) => {
            "工作區正被另一個程式使用，而且無法連線到它。請關閉另一個 AlphaFactorForge 或背景 service 後再試。"
        }
        HostError::CannotOpen(AppError::Db(_)) => {
            "無法開啟或升級工作區資料庫。資料庫可能已損毀、被其他程式鎖定，或磁碟空間不足。"
        }
        HostError::CannotOpen(AppError::Io(_)) => {
            "無法讀寫工作區資料夾。請檢查資料夾權限與磁碟空間。"
        }
        HostError::CannotOpen(_) | HostError::SwitchFailed { .. } => "無法使用這個工作區。",
    }
}

/// Report a startup failure and end the process with a failure code.
pub fn fail(message: &str) -> ! {
    eprintln!("{TITLE}\n{message}");
    show_dialog(message);
    std::process::exit(1);
}

/// Release builds on Windows have no console; show the message in a modal
/// box. Blocks until the user dismisses it.
#[cfg(all(windows, not(debug_assertions)))]
fn show_dialog(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
    // SAFETY: both buffers are NUL-terminated UTF-16 that outlive the call;
    // a null owner window is valid and makes the box a top-level window.
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn show_dialog(_message: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dir() -> PathBuf {
        PathBuf::from("C:/data/com.alphafactorforge.desktop")
    }

    #[test]
    fn a_newer_schema_tells_the_user_to_use_the_newer_build() {
        for error in [
            HostError::CannotOpen(AppError::SchemaTooNew("0011_future".into())),
            HostError::CannotConnect(ConnectError::Schema(AppError::SchemaTooNew("0011_future".into()))),
        ] {
            let message = workspace_failure_message(&dir(), &error);
            assert!(message.starts_with("這個工作區的資料庫是由較新版本"), "{message}");
            assert!(message.contains("0011_future"), "the technical reason is kept: {message}");
            assert!(message.contains("com.alphafactorforge.desktop"), "the workspace is named: {message}");
        }
    }

    #[test]
    fn an_owner_running_an_older_build_is_not_reported_as_a_newer_schema() {
        let error = HostError::CannotConnect(ConnectError::Schema(AppError::SchemaPending(
            "0010_campaign_admission".into(),
        )));
        let message = workspace_failure_message(&dir(), &error);
        assert!(message.starts_with("工作區目前由較舊版本的背景 service 持有"), "{message}");
        assert!(message.contains("0010_campaign_admission"));
    }

    #[test]
    fn every_connect_failure_has_its_own_advice() {
        let cases = [
            (ConnectError::NotPublished, "沒有提供連線端點"),
            (ConnectError::Unreachable("connection refused".into()), "沒有回應"),
            (ConnectError::IdentityMismatch("another instance".into()), "已過期"),
            (ConnectError::Other("lock held".into()), "無法連線到它"),
        ];
        for (error, expected) in cases {
            let message = workspace_failure_message(&dir(), &HostError::CannotConnect(error));
            assert!(message.contains(expected), "{expected} missing from: {message}");
            assert!(message.contains("原因："));
            assert!(message.contains("工作區："));
        }
    }

    #[test]
    fn database_and_io_failures_are_told_apart() {
        let db = HostError::CannotOpen(AppError::Db(rusqlite::Error::InvalidQuery));
        assert!(workspace_failure_message(&dir(), &db).starts_with("無法開啟或升級工作區資料庫"));

        let io = HostError::CannotOpen(AppError::Io(std::io::Error::other("access denied")));
        let message = workspace_failure_message(&dir(), &io);
        assert!(message.starts_with("無法讀寫工作區資料夾"), "{message}");
        assert!(message.contains("access denied"));
    }

    #[test]
    fn an_unclassified_failure_still_carries_its_reason() {
        let error = HostError::CannotOpen(AppError::Other("lease file is unreadable".into()));
        let message = workspace_failure_message(&dir(), &error);
        assert!(message.starts_with("無法使用這個工作區。"), "{message}");
        assert!(message.contains("lease file is unreadable"));
    }

    #[test]
    fn a_runtime_failure_message_has_advice_then_reason() {
        let message = runtime_failure_message("視窗元件無法啟動。", "WebView2 runtime not found");
        assert_eq!(message, "視窗元件無法啟動。\n\n原因：WebView2 runtime not found");
    }
}
