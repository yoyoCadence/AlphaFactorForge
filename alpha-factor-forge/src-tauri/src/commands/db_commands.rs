// SKELETON — Phase A database commands. The frontend reaches the DB ONLY
// through these. (Frontend never opens SQLite directly.)

use tauri::State;

use crate::db::repositories::{
    self, BacktestSummary, Candle, Dataset, StrategyDef, TradeRow, ValidationRecordRow,
};
use crate::error::{AppError, AppResult};
use crate::AppState;

/// Migrations run automatically at startup (db::initialize). These two are
/// exposed for explicit re-trigger / health-check from Settings.
#[tauri::command]
pub fn init_database(_state: State<AppState>) -> AppResult<String> {
    Ok("database already initialized at startup".into())
}

#[tauri::command]
pub fn run_migrations(state: State<AppState>) -> AppResult<String> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    crate::db::apply_migrations(&conn)?;
    Ok("migrations up to date".into())
}

#[tauri::command]
pub fn get_datasets(state: State<AppState>) -> AppResult<Vec<Dataset>> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::list_datasets(&conn)
}

#[tauri::command]
pub fn get_candles(
    state: State<AppState>,
    dataset_id: i64,
    from: i64,
    to: i64,
) -> AppResult<Vec<Candle>> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::get_candles(&conn, dataset_id, from, to)
}

/// Import a batch of candles. The frontend computes `dataset_hash` with the
/// shared core/hashing module and passes the dataset meta + rows.
#[tauri::command]
pub fn import_candles(
    state: State<AppState>,
    dataset: Dataset,
    candles: Vec<Candle>,
) -> AppResult<i64> {
    let mut conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    let dataset_id = repositories::insert_dataset(&conn, &dataset)?;
    repositories::insert_candles(&mut conn, dataset_id, &candles)?;
    Ok(dataset_id)
}

#[tauri::command]
pub fn save_strategy(state: State<AppState>, strategy: StrategyDef) -> AppResult<i64> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::insert_strategy(&conn, &strategy)
}

#[tauri::command]
pub fn get_strategies(state: State<AppState>) -> AppResult<Vec<StrategyDef>> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::list_strategies(&conn)
}

/// Persist one backtest summary and its closed trades atomically.
/// Phase A stores the metric columns; gate/score/benchmark stay null until
/// Phase B. Re-saving the same summary key replaces its prior trade rows.
#[tauri::command]
pub fn save_backtest_result(
    state: State<AppState>,
    summary: BacktestSummary,
    trades: Vec<TradeRow>,
) -> AppResult<i64> {
    let mut conn = state
        .db
        .lock()
        .map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::save_backtest_result(&mut conn, &summary, &trades)
}

#[tauri::command]
pub fn get_backtest_results(
    state: State<AppState>,
    strategy_id: Option<i64>,
) -> AppResult<Vec<BacktestSummary>> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::list_backtest_summaries(&conn, strategy_id)
}

/// PERSIST-001 (PR #64 handoff Resolution): atomically persist one validation
/// bundle — Train summary + trades, Validation summary + trades, and the
/// immutable append-only validation record — in ONE transaction. The bundle
/// is fully validated BEFORE the transaction opens; any write failure rolls
/// everything back. Returns the new record id.
#[tauri::command]
pub fn save_validation_record(
    state: State<AppState>,
    train_summary: BacktestSummary,
    train_trades: Vec<TradeRow>,
    validation_summary: BacktestSummary,
    validation_trades: Vec<TradeRow>,
    record: ValidationRecordRow,
) -> AppResult<i64> {
    repositories::validate_validation_bundle(&train_summary, &validation_summary, &record)?;
    let mut conn = state
        .db
        .lock()
        .map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::save_validation_bundle(
        &mut conn,
        &train_summary,
        &train_trades,
        &validation_summary,
        &validation_trades,
        &record,
    )
}

#[tauri::command]
pub fn list_validation_records(
    state: State<AppState>,
    strategy_id: Option<i64>,
) -> AppResult<Vec<ValidationRecordRow>> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::list_validation_records(&conn, strategy_id)
}

#[tauri::command]
pub fn get_validation_record(state: State<AppState>, id: i64) -> AppResult<ValidationRecordRow> {
    let conn = state.db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::get_validation_record(&conn, id)
}
