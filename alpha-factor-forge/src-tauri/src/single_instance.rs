//! Desktop process-ownership guard.
//!
//! The plugin is registered before setup so a secondary process exits before
//! SQLite initialization or orphan recovery can mutate a live primary run.

use tauri::{plugin::TauriPlugin, AppHandle, Manager, Runtime, WebviewWindow};

const MAIN_WINDOW_LABEL: &str = "main";

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Err(error) = focus_main_window(app) {
            eprintln!("failed to restore primary AlphaFactorForge window: {error}");
        }
    })
}

fn focus_main_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| format!("missing webview window {MAIN_WINDOW_LABEL:?}"))?;
    restore_and_focus(&window)
}

trait MainWindowActions {
    fn show_window(&self) -> Result<(), String>;
    fn unminimize_window(&self) -> Result<(), String>;
    fn focus_window(&self) -> Result<(), String>;
}

impl<R: Runtime> MainWindowActions for WebviewWindow<R> {
    fn show_window(&self) -> Result<(), String> {
        self.show().map_err(|error| error.to_string())
    }

    fn unminimize_window(&self) -> Result<(), String> {
        self.unminimize().map_err(|error| error.to_string())
    }

    fn focus_window(&self) -> Result<(), String> {
        self.set_focus().map_err(|error| error.to_string())
    }
}

fn restore_and_focus(window: &impl MainWindowActions) -> Result<(), String> {
    window
        .show_window()
        .map_err(|error| format!("show failed: {error}"))?;
    window
        .unminimize_window()
        .map_err(|error| format!("unminimize failed: {error}"))?;
    window
        .focus_window()
        .map_err(|error| format!("focus failed: {error}"))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{restore_and_focus, MainWindowActions};

    #[derive(Default)]
    struct RecordingWindow {
        calls: RefCell<Vec<&'static str>>,
        fail_at: Option<&'static str>,
    }

    impl MainWindowActions for RecordingWindow {
        fn show_window(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("show");
            self.result_for("show")
        }

        fn unminimize_window(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("unminimize");
            self.result_for("unminimize")
        }

        fn focus_window(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("focus");
            self.result_for("focus")
        }
    }

    impl RecordingWindow {
        fn result_for(&self, stage: &'static str) -> Result<(), String> {
            if self.fail_at == Some(stage) {
                Err("mock failure".into())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn secondary_launch_restores_then_focuses_the_primary_window() {
        let window = RecordingWindow::default();

        restore_and_focus(&window).unwrap();

        assert_eq!(*window.calls.borrow(), ["show", "unminimize", "focus"]);
    }

    #[test]
    fn focus_sequence_stops_at_the_first_window_error() {
        let window = RecordingWindow {
            fail_at: Some("unminimize"),
            ..Default::default()
        };

        let error = restore_and_focus(&window).unwrap_err();

        assert_eq!(*window.calls.borrow(), ["show", "unminimize"]);
        assert_eq!(error, "unminimize failed: mock failure");
    }

    #[test]
    fn single_instance_registration_precedes_setup_and_recovery() {
        let source = include_str!("main.rs");
        let plugin = source
            .find(".plugin(single_instance::plugin())")
            .expect("main must register the process-ownership plugin");
        let setup = source
            .find(".setup(|app|")
            .expect("main must retain its setup boundary");
        let recovery = source
            .find(".recover_orphans(&db)")
            .expect("main must retain startup recovery");

        assert!(plugin < setup);
        assert!(setup < recovery);
    }
}
