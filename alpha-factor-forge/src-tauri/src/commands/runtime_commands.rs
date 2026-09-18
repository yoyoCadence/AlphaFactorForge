//! P03b — the desktop's entry to the versioned command envelope
//! (`research-command-v1`) and the workspace identity a caller needs to
//! build one. The Tauri bridge is one of the hosts the contract names; the
//! rules (protocol version, workspace check, whitelist, idempotency) live in
//! `runtime::commands`, not here.
//!
//! P04b — plus the desktop's host mode: which host it is, and the two
//! switches (hand the workspace to a background service; take it back).

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::desktop::discovery_events::TauriDiscoveryEventSink;
use crate::runtime::commands::{CommandError, Dispatcher, COMMAND_PROTOCOL_VERSION, EVENT_PROTOCOL_VERSION};
use crate::runtime::host;
use crate::{AppState, HostSnapshot};

/// What a caller needs before its first envelope, and (P04b) which host it
/// is talking to: `hostMode` is the desktop's own mode, `holderKind` names
/// whoever holds the lease (the desktop itself, or the service it proxies).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub epoch: i64,
    pub holder_kind: String,
    pub instance_id: String,
    pub host_mode: &'static str,
    pub command_protocol_version: &'static str,
    pub event_protocol_version: &'static str,
}

#[tauri::command]
pub fn get_workspace_info(state: State<'_, AppState>) -> Result<WorkspaceInfo, CommandError> {
    let info = match state.snapshot()? {
        HostSnapshot::Embedded { epoch, instance_id, workspace_id, .. } => WorkspaceInfo {
            workspace_id,
            epoch,
            holder_kind: crate::db::ownership::HolderKind::DesktopEmbedded.as_str().to_string(),
            instance_id,
            host_mode: host::DESKTOP_EMBEDDED,
            command_protocol_version: COMMAND_PROTOCOL_VERSION,
            event_protocol_version: EVENT_PROTOCOL_VERSION,
        },
        HostSnapshot::Connected(proxy) => WorkspaceInfo {
            workspace_id: proxy.manifest.workspace_id.clone(),
            epoch: proxy.manifest.epoch,
            holder_kind: proxy.manifest.holder_kind.clone(),
            instance_id: proxy.manifest.instance_id.clone(),
            host_mode: host::DESKTOP_CONNECT,
            command_protocol_version: COMMAND_PROTOCOL_VERSION,
            event_protocol_version: EVENT_PROTOCOL_VERSION,
        },
    };
    Ok(info)
}

/// Run one `research-command-v1` envelope. A rejection is the structured
/// `CommandError` (`{ code, message, retryable }`), never a bare string, so
/// the caller can act on the code. Blocking work (admission, SQLite, or the
/// round trip to the service) runs off the WebView thread like the
/// discovery commands do.
#[tauri::command]
pub async fn dispatch_research_command(
    app: AppHandle,
    state: State<'_, AppState>,
    envelope: Value,
) -> Result<Value, CommandError> {
    let _admitted = state.admission.admit()?;
    match state.snapshot()? {
        HostSnapshot::Embedded { db, discovery, epoch, workspace_id, .. } => {
            let dispatcher = Dispatcher::new(
                db,
                discovery,
                epoch,
                workspace_id,
                Arc::new(TauriDiscoveryEventSink::new(app)),
                state.in_flight.clone(),
            );
            tauri::async_runtime::spawn_blocking(move || dispatcher.dispatch(envelope))
                .await
                .map_err(|error| CommandError::busy(format!("command task failed: {error}")))?
        }
        HostSnapshot::Connected(proxy) => tauri::async_runtime::spawn_blocking(move || proxy.dispatch(envelope))
            .await
            .map_err(|error| CommandError::busy(format!("command task failed: {error}")))?,
    }
}

/// P04b: the desktop's host mode and whether a background service is
/// published for this workspace (so the UI can offer the right switch).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    /// `desktop-embedded`, `desktop-connect`, or `switching`.
    pub host_mode: &'static str,
    /// The workspace directory the mode refers to.
    pub data_dir: String,
    /// Where the desktop looks for the service binary, and whether it is there.
    pub service_executable: String,
    pub service_executable_present: bool,
    /// A hand-over that failed and left nothing usable says why here.
    pub detail: Option<String>,
}

#[tauri::command]
pub fn get_host_status(state: State<'_, AppState>) -> Result<HostStatus, CommandError> {
    let (host_mode, detail) = {
        let mode = state
            .host
            .lock()
            .map_err(|_| CommandError::busy("host mode lock poisoned"))?;
        let detail = match &*mode {
            host::HostMode::Switching(reason) => Some(reason.clone()),
            _ => None,
        };
        (mode.kind(), detail)
    };
    let exe = std::env::current_exe()
        .map(|exe| host::service_executable_beside(&exe))
        .unwrap_or_default();
    Ok(HostStatus {
        host_mode,
        data_dir: state.data_dir.display().to_string(),
        service_executable: exe.display().to_string(),
        service_executable_present: exe.is_file(),
        detail,
    })
}

/// Plan §3.1 background mode: stop taking new work, checkpoint, release
/// ownership, start the service, connect to it. Returns the new status.
/// The hand-over drains and waits, so it runs off the WebView thread; the
/// blocking closure reaches the managed state through its own app handle.
#[tauri::command]
pub async fn enter_background_mode(app: AppHandle, state: State<'_, AppState>) -> Result<HostStatus, CommandError> {
    let switched = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || {
            let state = app.state::<AppState>();
            let sink = Arc::new(TauriDiscoveryEventSink::new(app.clone()));
            host::hand_over_to_service(&state.host, &state.admission, state.launcher.as_ref(), sink, &state.data_dir)
        }
    })
    .await
    .map_err(|error| CommandError::busy(format!("hand-over task failed: {error}")))?;
    announce_host(&app, &state);
    switched.map_err(|error| CommandError::busy(error.to_string()))?;
    get_host_status(state)
}

/// The reverse: stop the service (it drains to a checkpoint), take the
/// workspace back, and be embedded again.
#[tauri::command]
pub async fn exit_background_mode(app: AppHandle, state: State<'_, AppState>) -> Result<HostStatus, CommandError> {
    let switched = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || {
            let state = app.state::<AppState>();
            host::take_back_from_service(&state.host, &state.admission, &state.data_dir)
        }
    })
    .await
    .map_err(|error| CommandError::busy(format!("take-back task failed: {error}")))?;
    announce_host(&app, &state);
    switched.map_err(|error| CommandError::busy(error.to_string()))?;
    get_host_status(state)
}

/// Tell the window the mode changed so it can re-snapshot (contract §3) and
/// relabel itself.
fn announce_host(app: &AppHandle, state: &State<'_, AppState>) {
    let sink = TauriDiscoveryEventSink::new(app.clone());
    let kind = state.mode_kind().unwrap_or(host::SWITCHING);
    sink.host_changed(kind, true, None);
}
