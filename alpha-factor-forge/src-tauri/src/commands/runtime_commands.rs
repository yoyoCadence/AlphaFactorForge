//! P03b — the desktop's entry to the versioned command envelope
//! (`research-command-v1`) and the workspace identity a caller needs to
//! build one. The Tauri bridge is one of the hosts the contract names; the
//! rules (protocol version, workspace check, whitelist, idempotency) live in
//! `runtime::commands`, not here.

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use crate::desktop::discovery_events::TauriDiscoveryEventSink;
use crate::runtime::commands::{CommandError, Dispatcher, COMMAND_PROTOCOL_VERSION, EVENT_PROTOCOL_VERSION};
use crate::AppState;

/// What a caller needs before its first envelope.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub epoch: i64,
    pub holder_kind: &'static str,
    pub instance_id: String,
    pub command_protocol_version: &'static str,
    pub event_protocol_version: &'static str,
}

#[tauri::command]
pub fn get_workspace_info(state: State<'_, AppState>) -> WorkspaceInfo {
    WorkspaceInfo {
        workspace_id: state.workspace_id.clone(),
        epoch: state.ownership.epoch,
        holder_kind: state.ownership.kind.as_str(),
        instance_id: state.ownership.instance_id.clone(),
        command_protocol_version: COMMAND_PROTOCOL_VERSION,
        event_protocol_version: EVENT_PROTOCOL_VERSION,
    }
}

/// Run one `research-command-v1` envelope. A rejection is the structured
/// `CommandError` (`{ code, message, retryable }`), never a bare string, so
/// the caller can act on the code. Blocking work (admission, SQLite) runs off
/// the WebView thread like the discovery commands do.
#[tauri::command]
pub async fn dispatch_research_command(
    app: AppHandle,
    state: State<'_, AppState>,
    envelope: Value,
) -> Result<Value, CommandError> {
    let dispatcher = Dispatcher::new(
        state.db.clone(),
        state.discovery.clone(),
        state.ownership.epoch,
        state.workspace_id.clone(),
        Arc::new(TauriDiscoveryEventSink::new(app)),
        state.in_flight.clone(),
    );
    tauri::async_runtime::spawn_blocking(move || dispatcher.dispatch(envelope))
        .await
        .map_err(|error| CommandError::busy(format!("command task failed: {error}")))?
}
