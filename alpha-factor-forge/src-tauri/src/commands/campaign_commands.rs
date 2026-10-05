//! P12d-2d: the desktop's campaign authoring commands
//! (docs/research-campaign-declaration-v1.md).
//!
//! Reads work in both host modes from the mode's connection: the P06 market
//! data a campaign binds to, a validated preview, the stored campaigns, and a
//! run's admission decision. Writes are the workspace owner's: embedded mode
//! writes directly under its epoch; connected mode proxies `campaign.freeze` /
//! `campaign.start` to the background service. The frontend never supplies a
//! campaign ID or an admission status — the backend derives both.

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use alpha_factor_forge::discovery_core::campaign::freeze_campaign;

use super::db_commands::database_task;
use crate::db::campaign::{self, CampaignSummary, StoredCampaignAdmission};
use crate::db::repositories;
use crate::desktop::discovery_events::TauriDiscoveryEventSink;
use crate::error::{AppError, AppResult};
use crate::market::{registry, snapshot};
use crate::research::campaign_snapshot::resolve_campaign_instrument;
use crate::runtime::commands::LedgerSink;
use crate::{AppState, HostSnapshot};

/// Newest snapshots offered for binding; older ones stay addressable by ID.
const SNAPSHOT_LIST_LIMIT: usize = 500;

fn locked(
    db: &crate::runtime::SharedDb,
) -> AppResult<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    db.lock()
        .map_err(|_| AppError::Other("db lock poisoned".into()))
}

fn join_error(error: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("campaign command task failed: {error}"))
}

/// The dataset facts a binding needs next to its snapshot.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDataset {
    pub id: i64,
    pub interval: String,
    pub start_time: i64,
    pub end_time: i64,
    pub candle_count: i64,
}

/// One P06 snapshot as the authoring form offers it. `qualificationEligible`
/// is the snapshot's own rule (status ok, not demo); the preview still runs
/// the full P12d-2a verification before anything is frozen.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotOption {
    pub snapshot: snapshot::SnapshotRow,
    pub qualification_eligible: bool,
    pub dataset: SnapshotDataset,
}

#[tauri::command]
pub async fn list_market_instruments(
    state: State<'_, AppState>,
) -> AppResult<Vec<registry::InstrumentRow>> {
    database_task(state.db()?, |conn| registry::list_instruments(conn)).await
}

#[tauri::command]
pub async fn list_market_snapshots(state: State<'_, AppState>) -> AppResult<Vec<SnapshotOption>> {
    database_task(state.db()?, |conn| {
        snapshot::list_snapshots(conn, SNAPSHOT_LIST_LIMIT)?
            .into_iter()
            .map(|row| {
                let dataset = repositories::get_dataset_by_id(conn, row.dataset_id)?;
                Ok(SnapshotOption {
                    qualification_eligible: row.qualification_eligible(),
                    dataset: SnapshotDataset {
                        id: row.dataset_id,
                        interval: dataset.interval,
                        start_time: dataset.start_time,
                        end_time: dataset.end_time,
                        candle_count: dataset.candle_count,
                    },
                    snapshot: row,
                })
            })
            .collect()
    })
    .await
}

/// One declared instrument checked against its exact snapshot now.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentPreview {
    pub instrument_id: String,
    pub resolved: bool,
    pub bar_count: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignPreview {
    pub campaign_id: String,
    pub instruments: Vec<InstrumentPreview>,
}

/// Validate a draft without storing it: the campaign ID `freeze` would give
/// and, per instrument, whether its declared snapshot resolves (P12d-2a). A
/// declaration that does not freeze is an error.
///
/// Async (FU-5, `DB-ASYNC-001`): resolving re-reads and re-hashes every
/// declared dataset, and Tauri runs a non-async command on the main thread.
#[tauri::command]
pub async fn preview_research_campaign(
    state: State<'_, AppState>,
    declaration: Value,
) -> AppResult<CampaignPreview> {
    let db = state.db()?;
    tauri::async_runtime::spawn_blocking(move || preview_campaign(&db, &declaration))
        .await
        .map_err(join_error)?
}

/// The preview itself, off the main thread. The shared DB mutex is taken per
/// instrument, so another command waits for one instrument's verification at
/// most, not for the whole campaign. Each instrument is still resolved inside
/// one lock; `freeze` and `start` re-verify everything, so a write landing
/// between two instruments cannot reach a frozen or started campaign.
fn preview_campaign(
    db: &crate::runtime::SharedDb,
    declaration: &Value,
) -> AppResult<CampaignPreview> {
    let frozen =
        freeze_campaign(declaration).map_err(|error| AppError::Other(error.to_string()))?;
    let ids: Vec<String> = frozen.document()["instruments"]
        .as_array()
        .map(|instruments| {
            instruments
                .iter()
                .filter_map(|instrument| instrument["instrumentId"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut instruments = Vec::with_capacity(ids.len());
    for instrument_id in ids {
        let resolved = {
            let conn = locked(db)?;
            resolve_campaign_instrument(&conn, &frozen, &instrument_id)
        };
        instruments.push(match resolved {
            Ok(resolved) => InstrumentPreview {
                instrument_id,
                resolved: true,
                bar_count: Some(resolved.bar_count),
                error: None,
            },
            Err(error) => InstrumentPreview {
                instrument_id,
                resolved: false,
                bar_count: None,
                error: Some(error.to_string()),
            },
        });
    }
    Ok(CampaignPreview {
        campaign_id: frozen.campaign_id().to_string(),
        instruments,
    })
}

/// Freeze and store a declaration (maintainer decision 2026-09-30: freezing
/// saves). Returns the backend-derived campaign ID.
#[tauri::command]
pub async fn freeze_research_campaign(
    state: State<'_, AppState>,
    declaration: Value,
) -> AppResult<String> {
    let _admitted = state.admission.admit()?;
    match state.snapshot()? {
        HostSnapshot::Embedded { db, epoch, .. } => {
            tauri::async_runtime::spawn_blocking(move || {
                let conn = locked(&db)?;
                campaign::freeze_and_store_campaign(&conn, Some(epoch), &declaration)
            })
            .await
            .map_err(join_error)?
        }
        HostSnapshot::Connected(proxy) => {
            tauri::async_runtime::spawn_blocking(move || proxy.campaign_freeze(declaration))
                .await
                .map_err(join_error)?
        }
    }
}

#[tauri::command]
pub async fn list_research_campaigns(
    state: State<'_, AppState>,
) -> AppResult<Vec<CampaignSummary>> {
    database_task(state.db()?, |conn| campaign::list_campaigns(conn)).await
}

/// Start one declared instrument's run of a stored campaign with a
/// `discovery-config-v3` config.
#[tauri::command]
pub async fn start_campaign_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
    config: Value,
    campaign_id: String,
    instrument_id: String,
) -> AppResult<i64> {
    let _admitted = state.admission.admit()?;
    match state.snapshot()? {
        HostSnapshot::Embedded {
            db,
            discovery,
            epoch,
            ..
        } => {
            let sink = std::sync::Arc::new(LedgerSink::new(
                db.clone(),
                epoch,
                std::sync::Arc::new(TauriDiscoveryEventSink::new(app)),
            ));
            tauri::async_runtime::spawn_blocking(move || {
                discovery.start_stored_campaign_for_request(
                    db,
                    sink,
                    config,
                    &campaign_id,
                    &instrument_id,
                    None,
                )
            })
            .await
            .map_err(join_error)?
        }
        HostSnapshot::Connected(proxy) => tauri::async_runtime::spawn_blocking(move || {
            proxy.campaign_start(config, &campaign_id, &instrument_id)
        })
        .await
        .map_err(join_error)?,
    }
}

/// A run's admission decision, or `null` for a run that is not campaign-bound.
#[tauri::command]
pub async fn get_campaign_admission(
    state: State<'_, AppState>,
    run_id: i64,
) -> AppResult<Option<StoredCampaignAdmission>> {
    database_task(state.db()?, move |conn| {
        campaign::get_campaign_admission(conn, run_id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The extracted preview resolves each declared instrument on its own and
    /// reports a missing snapshot per instrument instead of failing the call.
    #[test]
    fn the_preview_reports_each_instrument_without_holding_the_whole_campaign() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::apply_migrations(&conn).unwrap();
        let db: crate::runtime::SharedDb = std::sync::Arc::new(std::sync::Mutex::new(conn));
        let declaration: Value = serde_json::from_str(include_str!(
            "../../../fixtures/rs-core/research-campaign-declaration-v1.json"
        ))
        .unwrap();
        let preview = preview_campaign(&db, &declaration).unwrap();
        assert_eq!(
            preview.campaign_id,
            freeze_campaign(&declaration).unwrap().campaign_id()
        );
        assert!(!preview.instruments.is_empty());
        for instrument in &preview.instruments {
            assert!(!instrument.resolved, "{}", instrument.instrument_id);
            assert!(
                instrument
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("snapshot")),
                "{:?}",
                instrument.error
            );
        }
        // The lock is free again once the preview returns.
        assert!(db.try_lock().is_ok());
        // A declaration that does not freeze is still an error.
        assert!(preview_campaign(&db, &serde_json::json!({})).is_err());
    }
}
