// SKELETON — Phase A database commands. The frontend reaches the DB ONLY
// through these. (Frontend never opens SQLite directly.)

use tauri::State;

use crate::db::repositories::{
    self, BacktestResultDetail, BacktestSummary, Candle, Dataset, PreparedSavedStrategy,
    StrategyDef, TradeRow, ValidationRecordRow,
};
use crate::error::{AppError, AppResult};
use crate::AppState;

/// Database work owns the existing DB mutex on a blocking worker, never
/// on the thread polling the Tauri command. Repository transactions stay intact.
/// Sibling command modules reuse this boundary for their SQLite reads.
pub(super) async fn database_task<T: Send + 'static>(
    db: crate::runtime::SharedDb,
    operation: impl FnOnce(&mut rusqlite::Connection) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut conn = db
            .lock()
            .map_err(|_| AppError::Other("db lock poisoned".into()))?;
        operation(&mut conn)
    })
    .await
    .map_err(|error| AppError::Other(format!("database command task failed: {error}")))?
}

/// Migrations run automatically at startup (runtime::open_workspace). These two are
/// exposed for explicit re-trigger / health-check from Settings.
#[tauri::command]
pub fn init_database(_state: State<AppState>) -> AppResult<String> {
    Ok("database already initialized at startup".into())
}

#[tauri::command]
pub async fn run_migrations(state: State<'_, AppState>) -> AppResult<String> {
    // P04b: only the lease holder migrates (contract §1.1); in connect mode
    // the service already did, and this build was checked against it.
    if state.mode_kind()? != crate::runtime::host::DESKTOP_EMBEDDED {
        return Ok("connected to a background service; its build migrated the workspace".into());
    }
    let _admitted = state.admission.admit()?;
    database_task(state.db()?, |conn| {
        crate::db::apply_migrations(conn)?;
        Ok("migrations up to date".into())
    })
    .await
}

#[tauri::command]
pub async fn get_datasets(state: State<'_, AppState>) -> AppResult<Vec<Dataset>> {
    database_task(state.db()?, |conn| repositories::list_datasets(conn)).await
}

#[tauri::command]
pub async fn get_candles(
    state: State<'_, AppState>,
    dataset_id: i64,
    from: i64,
    to: i64,
) -> AppResult<Vec<Candle>> {
    database_task(state.db()?, move |conn| {
        repositories::get_candles(conn, dataset_id, from, to)
    })
    .await
}

/// Import a batch of candles. Rust recomputes the v2 content identity and owns
/// the single transaction for the dataset row plus every candle.
#[tauri::command]
pub async fn import_candles(
    state: State<'_, AppState>,
    dataset: Dataset,
    candles: Vec<Candle>,
) -> AppResult<i64> {
    let _admitted = state.admission.admit()?;
    database_task(state.db()?, move |conn| {
        repositories::import_dataset_with_candles(conn, &dataset, &candles)
    })
    .await
}

#[tauri::command]
pub async fn save_strategy(state: State<'_, AppState>, strategy: StrategyDef) -> AppResult<i64> {
    let _admitted = state.admission.admit()?;
    database_task(state.db()?, move |conn| {
        repositories::insert_verified_strategy(conn, &strategy)
    })
    .await
}

#[tauri::command]
pub async fn get_strategies(state: State<'_, AppState>) -> AppResult<Vec<StrategyDef>> {
    database_task(state.db()?, |conn| repositories::list_strategies(conn)).await
}

/// Prepare a verified definition under its declared numeric policy. A string
/// carries the resolved binary64 values through the frontend JSON boundary.
#[tauri::command]
pub async fn prepare_saved_strategy(
    state: State<'_, AppState>,
    strategy_id: i64,
) -> AppResult<PreparedSavedStrategy> {
    database_task(state.db()?, move |conn| {
        repositories::prepare_saved_strategy(conn, strategy_id)
    })
    .await
}

/// Persist one backtest summary and its closed trades atomically.
/// Phase A stores the metric columns; gate/score/benchmark stay null until
/// Phase B. Re-saving the same summary key replaces its prior trade rows.
#[tauri::command]
pub async fn save_backtest_result(
    state: State<'_, AppState>,
    summary: BacktestSummary,
    trades: Vec<TradeRow>,
) -> AppResult<i64> {
    let _admitted = state.admission.admit()?;
    database_task(state.db()?, move |conn| {
        repositories::save_backtest_result(conn, &summary, &trades)
    })
    .await
}

#[tauri::command]
pub async fn get_backtest_results(
    state: State<'_, AppState>,
    strategy_id: Option<i64>,
) -> AppResult<Vec<BacktestSummary>> {
    database_task(state.db()?, move |conn| {
        repositories::list_backtest_summaries(conn, strategy_id)
    })
    .await
}

/// P01 Results Explorer: one summary row and its stored trades, read in one
/// transaction so the reader can check the pair against the row it displays
/// (see `repositories::get_backtest_result_detail`). Read only; None when the
/// summary no longer exists.
#[tauri::command]
pub async fn get_backtest_result_detail(
    state: State<'_, AppState>,
    summary_id: i64,
) -> AppResult<Option<BacktestResultDetail>> {
    database_task(state.db()?, move |conn| {
        repositories::get_backtest_result_detail(conn, summary_id)
    })
    .await
}

/// PERSIST-001 (PR #64 handoff Resolution): atomically persist one validation
/// bundle — Train summary + trades, Validation summary + trades, and the
/// immutable append-only validation record — in ONE transaction. The bundle
/// is fully validated BEFORE the transaction opens; any write failure rolls
/// everything back. Returns the new record id.
#[tauri::command]
pub async fn save_validation_record(
    state: State<'_, AppState>,
    train_summary: BacktestSummary,
    train_trades: Vec<TradeRow>,
    validation_summary: BacktestSummary,
    validation_trades: Vec<TradeRow>,
    record: ValidationRecordRow,
) -> AppResult<i64> {
    let _admitted = state.admission.admit()?;
    database_task(state.db()?, move |conn| {
        repositories::validate_validation_bundle(&train_summary, &validation_summary, &record)?;
        repositories::save_validation_bundle(
            conn,
            &train_summary,
            &train_trades,
            &validation_summary,
            &validation_trades,
            &record,
        )
    })
    .await
}

#[tauri::command]
pub async fn list_validation_records(
    state: State<'_, AppState>,
    strategy_id: Option<i64>,
) -> AppResult<Vec<ValidationRecordRow>> {
    database_task(state.db()?, move |conn| {
        repositories::list_validation_records(conn, strategy_id)
    })
    .await
}

#[tauri::command]
pub async fn get_validation_record(
    state: State<'_, AppState>,
    id: i64,
) -> AppResult<ValidationRecordRow> {
    database_task(state.db()?, move |conn| {
        repositories::get_validation_record(conn, id)
    })
    .await
}

#[cfg(test)]
#[path = "db_commands/tests.rs"]
mod tests;
