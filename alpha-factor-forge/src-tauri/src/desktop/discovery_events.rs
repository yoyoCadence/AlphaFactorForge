//! The desktop's `DiscoveryEventSink`: post-commit runner events onto the
//! Tauri event bus for the main window (RUNNER-EXEC-001, moved out of the
//! runner in P02 with no behavior change — same event names, same target
//! window, same payloads).
//!
//! P04b: the same adapter is the desktop's `LedgerEventSink` in connect
//! mode — a ledger row forwarded from the background service is posted
//! under the channel name it was stored with (`discovery://progress`,
//! `discovery://result`, `discovery://done`), with the very payload the
//! service's runner emitted, so the window cannot tell the two modes
//! apart. It also posts `runtime://host` when the desktop's host mode
//! changes or the service stops/starts answering.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::discovery_runner::{
    DiscoveryEvent, DiscoveryEventSink, DISCOVERY_DONE_EVENT, DISCOVERY_PROGRESS_EVENT,
    DISCOVERY_RESULT_EVENT,
};
use crate::runtime::connect::LedgerEventSink;

/// The window label events are addressed to. Matches the WebView the
/// TypeScript client subscribes from (`src/tauri-client/events.ts`).
const MAIN_WINDOW_LABEL: &str = "main";

/// P04b: the host-mode notification (`src/tauri-client/events.ts`).
pub const HOST_EVENT: &str = "runtime://host";
/// P04b: "your view may be behind the database; re-read your run"
/// (contract §3). Posted by the forwarder for a ledger gap, a state version
/// that moved without a row, a row it could not deliver, or a reconnect.
pub const RESNAPSHOT_EVENT: &str = "runtime://resnapshot";

/// The payload of `runtime://resnapshot`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResnapshotEvent<'a> {
    pub reason: &'a str,
    pub state_version: i64,
}

/// The payload of `runtime://host`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostEvent<'a> {
    pub host_mode: &'a str,
    pub service_reachable: bool,
    pub reason: Option<&'a str>,
}

#[derive(Clone)]
pub struct TauriDiscoveryEventSink {
    app: AppHandle,
}

impl TauriDiscoveryEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn host_changed(&self, host_mode: &str, service_reachable: bool, reason: Option<&str>) {
        let payload = HostEvent { host_mode, service_reachable, reason };
        if let Err(error) = self.app.emit_to(MAIN_WINDOW_LABEL, HOST_EVENT, payload) {
            eprintln!("host event emission failed: {error}");
        }
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

impl LedgerEventSink for TauriDiscoveryEventSink {
    fn emit(&self, channel: &str, payload: &Value) -> Result<(), String> {
        // Only the runner's own channels are forwarded; a ledger row on any
        // other channel (none exist today) is not something the window
        // subscribed to.
        if ![DISCOVERY_PROGRESS_EVENT, DISCOVERY_RESULT_EVENT, DISCOVERY_DONE_EVENT].contains(&channel) {
            return Err(format!("unknown ledger channel {channel:?}"));
        }
        self.app
            .emit_to(MAIN_WINDOW_LABEL, channel, payload)
            .map_err(|error| error.to_string())
    }

    fn connection_lost(&self, reason: &str) {
        self.host_changed(crate::runtime::host::DESKTOP_CONNECT, false, Some(reason));
    }

    fn connection_restored(&self) {
        self.host_changed(crate::runtime::host::DESKTOP_CONNECT, true, None);
    }

    fn resnapshot_needed(&self, reason: &str, state_version: i64) {
        let payload = ResnapshotEvent { reason, state_version };
        if let Err(error) = self.app.emit_to(MAIN_WINDOW_LABEL, RESNAPSHOT_EVENT, payload) {
            eprintln!("resnapshot event emission failed: {error}");
        }
    }
}
