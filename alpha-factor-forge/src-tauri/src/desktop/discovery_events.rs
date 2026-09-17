//! The desktop's `DiscoveryEventSink`: post-commit runner events onto the
//! Tauri event bus for the main window (RUNNER-EXEC-001, moved out of the
//! runner in P02 with no behavior change — same event names, same target
//! window, same payloads).

use tauri::{AppHandle, Emitter};

use crate::discovery_runner::{
    DiscoveryEvent, DiscoveryEventSink, DISCOVERY_DONE_EVENT, DISCOVERY_PROGRESS_EVENT,
    DISCOVERY_RESULT_EVENT,
};

/// The window label events are addressed to. Matches the WebView the
/// TypeScript client subscribes from (`src/tauri-client/events.ts`).
const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Clone)]
pub struct TauriDiscoveryEventSink {
    app: AppHandle,
}

impl TauriDiscoveryEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DiscoveryEventSink for TauriDiscoveryEventSink {
    fn emit(&self, event: &DiscoveryEvent) -> Result<(), String> {
        match event {
            DiscoveryEvent::Progress(payload) => self
                .app
                .emit_to(MAIN_WINDOW_LABEL, DISCOVERY_PROGRESS_EVENT, payload)
                .map_err(|error| error.to_string()),
            DiscoveryEvent::Result(payload) => self
                .app
                .emit_to(MAIN_WINDOW_LABEL, DISCOVERY_RESULT_EVENT, payload)
                .map_err(|error| error.to_string()),
            DiscoveryEvent::Done(payload) => self
                .app
                .emit_to(MAIN_WINDOW_LABEL, DISCOVERY_DONE_EVENT, payload)
                .map_err(|error| error.to_string()),
        }
    }
}
