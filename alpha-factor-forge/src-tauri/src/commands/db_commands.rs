// SKELETON — Phase A database commands. The frontend reaches the DB ONLY
// through these. (Frontend never opens SQLite directly.)

use tauri::State;

use crate::db::repositories::{
    self, BacktestResultDetail, BacktestSummary, Candle, Dataset, StrategyDef, TradeRow,
    ValidationRecordRow,
};
use crate::error::{AppError, AppResult};
use crate::AppState;

/// Migrations run automatically at startup (runtime::open_workspace). These two are
/// exposed for explicit re-trigger / health-check from Settings.
#[tauri::command]
pub fn init_database(_state: State<AppState>) -> AppResult<String> {
    Ok("database already initialized at startup".into())
}

#[tauri::command]
pub fn run_migrations(state: State<AppState>) -> AppResult<String> {
    // P04b: only the lease holder migrates (contract §1.1); in connect mode
    // the service already did, and this build was checked against it.
    if state.mode_kind()? != crate::runtime::host::DESKTOP_EMBEDDED {
        return Ok("connected to a background service; its build migrated the workspace".into());
    }
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    crate::db::apply_migrations(&conn)?;
    Ok("migrations up to date".into())
}

#[tauri::command]
pub fn get_datasets(state: State<AppState>) -> AppResult<Vec<Dataset>> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::list_datasets(&conn)
}

#[tauri::command]
pub fn get_candles(
    state: State<AppState>,
    dataset_id: i64,
    from: i64,
    to: i64,
) -> AppResult<Vec<Candle>> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::get_candles(&conn, dataset_id, from, to)
}

/// Import a batch of candles. Rust recomputes the v2 content identity and owns
/// the single transaction for the dataset row plus every candle.
#[tauri::command]
pub fn import_candles(
    state: State<AppState>,
    dataset: Dataset,
    candles: Vec<Candle>,
) -> AppResult<i64> {
    let db = state.db()?;
    let mut conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::import_dataset_with_candles(&mut conn, &dataset, &candles)
}

#[tauri::command]
pub fn save_strategy(state: State<AppState>, strategy: StrategyDef) -> AppResult<i64> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::insert_verified_strategy(&conn, &strategy)
}

#[tauri::command]
pub fn get_strategies(state: State<AppState>) -> AppResult<Vec<StrategyDef>> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
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
    let db = state.db()?;
    let mut conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::save_backtest_result(&mut conn, &summary, &trades)
}

#[tauri::command]
pub fn get_backtest_results(
    state: State<AppState>,
    strategy_id: Option<i64>,
) -> AppResult<Vec<BacktestSummary>> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::list_backtest_summaries(&conn, strategy_id)
}

/// P01 Results Explorer: one summary row and its stored trades, read in one
/// transaction so the reader can check the pair against the row it displays
/// (see `repositories::get_backtest_result_detail`). Read only; None when the
/// summary no longer exists.
#[tauri::command]
pub fn get_backtest_result_detail(
    state: State<AppState>,
    summary_id: i64,
) -> AppResult<Option<BacktestResultDetail>> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::get_backtest_result_detail(&conn, summary_id)
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
    let db = state.db()?;
    let mut conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
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
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::list_validation_records(&conn, strategy_id)
}

#[tauri::command]
pub fn get_validation_record(state: State<AppState>, id: i64) -> AppResult<ValidationRecordRow> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    repositories::get_validation_record(&conn, id)
}
