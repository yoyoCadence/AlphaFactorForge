// SKELETON — data-access layer. Phase A implements datasets + candles +
// strategy_def + backtest_summary CRUD enough to satisfy the v1 delivery.
// Functions marked `todo!()` need local completion (see TODO.md).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

// ---------- DTOs (mirror the SQLite schema; shared shapes with frontend TS) ----------

#[derive(Debug, Serialize, Deserialize)]
pub struct Dataset {
    pub id: Option<i64>,
    pub exchange: String,
    pub symbol: String,
    pub interval: String,
    pub start_time: i64,
    pub end_time: i64,
    pub candle_count: i64,
    pub source: String,
    pub dataset_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyDef {
    pub id: Option<i64>,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub dsl_json: Option<String>,
    pub original_definition_json: String,
    pub param_schema_json: Option<String>,
    pub source: String,
    pub ai_prompt_hash: Option<String>,
    pub strategy_hash: String,
    pub lifecycle: String, // candidate | validated | rejected (v1)
    pub parent_strategy_id: Option<i64>,
}

/// One row of `backtest_summary`. Mirrors the SQLite column set (snake_case).
/// The identity quad `(strategy_id, dataset_id, segment)` is the unique key.
/// Phase A persists `net_return..consecutive_losses` (from core/metrics, which
/// the frontend maps from its camelCase `Metrics` shape); `gate_passed / score
/// / *_json` stay `None` until Phase B fills them.
#[derive(Debug, Serialize, Deserialize)]
pub struct BacktestSummary {
    #[serde(default)]
    pub id: Option<i64>,
    pub strategy_id: i64,
    pub dataset_id: i64,
    pub segment: String, // train | validation | test | full (CHECK enforced)
    pub start_time: i64,
    pub end_time: i64,
    #[serde(default)]
    pub net_return: Option<f64>,
    #[serde(default)]
    pub cagr: Option<f64>,
    #[serde(default)]
    pub max_drawdown: Option<f64>,
    #[serde(default)]
    pub sharpe: Option<f64>,
    #[serde(default)]
    pub sortino: Option<f64>,
    #[serde(default)]
    pub calmar: Option<f64>,
    #[serde(default)]
    pub win_rate: Option<f64>,
    #[serde(default)]
    pub trade_count: Option<i64>,
    #[serde(default)]
    pub profit_factor: Option<f64>,
    #[serde(default)]
    pub avg_trade_return: Option<f64>,
    #[serde(default)]
    pub median_trade_return: Option<f64>,
    #[serde(default)]
    pub exposure: Option<f64>,
    #[serde(default)]
    pub turnover: Option<f64>,
    #[serde(default)]
    pub largest_win: Option<f64>,
    #[serde(default)]
    pub largest_loss: Option<f64>,
    #[serde(default)]
    pub consecutive_losses: Option<i64>,
    // ---- Phase B (stay None in v1) ----
    #[serde(default)]
    pub gate_passed: Option<bool>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub score_breakdown_json: Option<String>,
    #[serde(default)]
    pub benchmark_result_json: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>, // set by DB default; read-only
}

/// One closed trade persisted under a `backtest_summary` row.
///
/// Phase A does not expose per-trade fee/slippage, so those SQLite columns
/// intentionally remain NULL. Holding bars are not part of the current schema.
#[derive(Debug, Serialize, Deserialize)]
pub struct TradeRow {
    pub entry_time: i64,
    pub exit_time: i64,
    pub side: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub reason: Option<String>,
}

/// One `validation_records` row (PERSIST-001, PR #64 handoff Resolution):
/// an append-only immutable decision audit snapshot. `record_json` is the
/// self-contained `validation-record-v1` envelope. There is NO update or
/// delete path in v1; `backtest_summary` stays the mutable "latest" view.
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationRecordRow {
    #[serde(default)]
    pub id: Option<i64>,
    pub strategy_id: i64,
    pub dataset_id: i64,
    pub record_version: String,
    pub gate_passed: bool,
    #[serde(default)]
    pub score: Option<f64>,
    pub record_json: String,
    #[serde(default)]
    pub created_at: Option<String>, // set by DB default; read-only
}

// ---------- datasets ----------

pub fn insert_dataset(conn: &Connection, d: &Dataset) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO datasets
            (exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(dataset_hash) DO UPDATE SET
            candle_count = excluded.candle_count,
            end_time     = excluded.end_time,
            updated_at   = datetime('now')",
        params![
            d.exchange, d.symbol, d.interval, d.start_time, d.end_time,
            d.candle_count, d.source, d.dataset_hash
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM datasets WHERE dataset_hash = ?1",
        [&d.dataset_hash],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn list_datasets(conn: &Connection) -> AppResult<Vec<Dataset>> {
    let mut stmt = conn.prepare(
        "SELECT id, exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash
         FROM datasets ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Dataset {
                id: Some(r.get(0)?),
                exchange: r.get(1)?,
                symbol: r.get(2)?,
                interval: r.get(3)?,
                start_time: r.get(4)?,
                end_time: r.get(5)?,
                candle_count: r.get(6)?,
                source: r.get(7)?,
                dataset_hash: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------- candles ----------

/// Bulk insert candles for a dataset inside a single transaction.
pub fn insert_candles(conn: &mut Connection, dataset_id: i64, candles: &[Candle]) -> AppResult<usize> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO candles
                (dataset_id, timestamp, open, high, low, close, volume)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
        )?;
        for c in candles {
            stmt.execute(params![
                dataset_id, c.timestamp, c.open, c.high, c.low, c.close, c.volume
            ])?;
        }
    }
    tx.commit()?;
    Ok(candles.len())
}

pub fn get_candles(conn: &Connection, dataset_id: i64, from: i64, to: i64) -> AppResult<Vec<Candle>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, open, high, low, close, volume
         FROM candles
         WHERE dataset_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
         ORDER BY timestamp ASC",
    )?;
    let rows = stmt
        .query_map(params![dataset_id, from, to], |r| {
            Ok(Candle {
                timestamp: r.get(0)?,
                open: r.get(1)?,
                high: r.get(2)?,
                low: r.get(3)?,
                close: r.get(4)?,
                volume: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------- strategy_def ----------

pub fn insert_strategy(conn: &Connection, s: &StrategyDef) -> AppResult<i64> {
    // A hash conflict represents the same strategy definition/execution model,
    // so refresh only mutable presentation/provenance fields. `lifecycle` is
    // deliberately preserved because validation owns that review state; a
    // routine re-save must never demote a validated/rejected row to candidate.
    conn.execute(
        "INSERT INTO strategy_def
            (name, type, dsl_json, original_definition_json, param_schema_json,
             source, ai_prompt_hash, strategy_hash, lifecycle, parent_strategy_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(strategy_hash) DO UPDATE SET
             name       = excluded.name,
             source     = excluded.source,
             updated_at = datetime('now')",
        params![
            s.name, s.kind, s.dsl_json, s.original_definition_json, s.param_schema_json,
            s.source, s.ai_prompt_hash, s.strategy_hash, s.lifecycle, s.parent_strategy_id
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM strategy_def WHERE strategy_hash = ?1",
        [&s.strategy_hash],
        |r| r.get(0),
    )?;
    Ok(id)
}

pub fn list_strategies(conn: &Connection) -> AppResult<Vec<StrategyDef>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, dsl_json, original_definition_json, param_schema_json,
                source, ai_prompt_hash, strategy_hash, lifecycle, parent_strategy_id
         FROM strategy_def ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(StrategyDef {
                id: Some(r.get(0)?),
                name: r.get(1)?,
                kind: r.get(2)?,
                dsl_json: r.get(3)?,
                original_definition_json: r.get(4)?,
                param_schema_json: r.get(5)?,
                source: r.get(6)?,
                ai_prompt_hash: r.get(7)?,
                strategy_hash: r.get(8)?,
                lifecycle: r.get(9)?,
                parent_strategy_id: r.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------- backtest_summary ----------

/// Upsert one summary row. Re-running the same `(strategy_id, dataset_id,
/// segment)` overwrites the metrics (so a re-backtest refreshes in place rather
/// than duplicating). Returns the row id.
pub fn insert_backtest_summary(conn: &Connection, s: &BacktestSummary) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO backtest_summary
            (strategy_id, dataset_id, segment, start_time, end_time,
             net_return, cagr, max_drawdown, sharpe, sortino, calmar, win_rate,
             trade_count, profit_factor, avg_trade_return, median_trade_return,
             exposure, turnover, largest_win, largest_loss, consecutive_losses,
             gate_passed, score, score_breakdown_json, benchmark_result_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25)
         ON CONFLICT(strategy_id, dataset_id, segment) DO UPDATE SET
             start_time            = excluded.start_time,
             end_time              = excluded.end_time,
             net_return            = excluded.net_return,
             cagr                  = excluded.cagr,
             max_drawdown          = excluded.max_drawdown,
             sharpe                = excluded.sharpe,
             sortino               = excluded.sortino,
             calmar                = excluded.calmar,
             win_rate              = excluded.win_rate,
             trade_count           = excluded.trade_count,
             profit_factor         = excluded.profit_factor,
             avg_trade_return      = excluded.avg_trade_return,
             median_trade_return   = excluded.median_trade_return,
             exposure              = excluded.exposure,
             turnover              = excluded.turnover,
             largest_win           = excluded.largest_win,
             largest_loss          = excluded.largest_loss,
             consecutive_losses    = excluded.consecutive_losses,
             gate_passed           = excluded.gate_passed,
             score                 = excluded.score,
             score_breakdown_json  = excluded.score_breakdown_json,
             benchmark_result_json = excluded.benchmark_result_json",
        params![
            s.strategy_id, s.dataset_id, s.segment, s.start_time, s.end_time,
            s.net_return, s.cagr, s.max_drawdown, s.sharpe, s.sortino, s.calmar, s.win_rate,
            s.trade_count, s.profit_factor, s.avg_trade_return, s.median_trade_return,
            s.exposure, s.turnover, s.largest_win, s.largest_loss, s.consecutive_losses,
            s.gate_passed, s.score, s.score_breakdown_json, s.benchmark_result_json
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM backtest_summary
         WHERE strategy_id = ?1 AND dataset_id = ?2 AND segment = ?3",
        params![s.strategy_id, s.dataset_id, s.segment],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Upsert a summary and replace its trade rows on the CURRENT connection.
/// Callers own the transaction boundary: `save_backtest_result` wraps this in
/// its own transaction, and `save_validation_bundle` runs it (twice) inside
/// the whole-bundle transaction.
fn write_backtest_result(
    conn: &Connection,
    summary: &BacktestSummary,
    trades: &[TradeRow],
) -> AppResult<i64> {
    let summary_id = insert_backtest_summary(conn, summary)?;

    conn.execute(
        "DELETE FROM trades WHERE backtest_summary_id = ?1",
        params![summary_id],
    )?;

    {
        let mut stmt = conn.prepare(
            "INSERT INTO trades
                (backtest_summary_id, entry_time, exit_time, side,
                 entry_price, exit_price, pnl, pnl_pct, fee, slippage, reason)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,NULL,?9)",
        )?;
        for trade in trades {
            stmt.execute(params![
                summary_id,
                trade.entry_time,
                trade.exit_time,
                trade.side,
                trade.entry_price,
                trade.exit_price,
                trade.pnl,
                trade.pnl_pct,
                trade.reason,
            ])?;
        }
    }

    Ok(summary_id)
}

/// Atomically upsert a summary and replace all trade rows attached to it.
///
/// Re-running the same strategy/dataset/segment must never accumulate stale
/// trades. Keeping the upsert, delete, and inserts in one transaction also
/// preserves the previous complete result if any replacement row fails.
pub fn save_backtest_result(
    conn: &mut Connection,
    summary: &BacktestSummary,
    trades: &[TradeRow],
) -> AppResult<i64> {
    let tx = conn.transaction()?;
    let summary_id = write_backtest_result(&tx, summary, trades)?;
    tx.commit()?;
    Ok(summary_id)
}

/// List summaries, newest first. Pass `strategy_id` to scope to one strategy.
pub fn list_backtest_summaries(
    conn: &Connection,
    strategy_id: Option<i64>,
) -> AppResult<Vec<BacktestSummary>> {
    const COLS: &str = "id, strategy_id, dataset_id, segment, start_time, end_time,
             net_return, cagr, max_drawdown, sharpe, sortino, calmar, win_rate,
             trade_count, profit_factor, avg_trade_return, median_trade_return,
             exposure, turnover, largest_win, largest_loss, consecutive_losses,
             gate_passed, score, score_breakdown_json, benchmark_result_json, created_at";

    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<BacktestSummary> {
        Ok(BacktestSummary {
            id: Some(r.get(0)?),
            strategy_id: r.get(1)?,
            dataset_id: r.get(2)?,
            segment: r.get(3)?,
            start_time: r.get(4)?,
            end_time: r.get(5)?,
            net_return: r.get(6)?,
            cagr: r.get(7)?,
            max_drawdown: r.get(8)?,
            sharpe: r.get(9)?,
            sortino: r.get(10)?,
            calmar: r.get(11)?,
            win_rate: r.get(12)?,
            trade_count: r.get(13)?,
            profit_factor: r.get(14)?,
            avg_trade_return: r.get(15)?,
            median_trade_return: r.get(16)?,
            exposure: r.get(17)?,
            turnover: r.get(18)?,
            largest_win: r.get(19)?,
            largest_loss: r.get(20)?,
            consecutive_losses: r.get(21)?,
            gate_passed: r.get(22)?,
            score: r.get(23)?,
            score_breakdown_json: r.get(24)?,
            benchmark_result_json: r.get(25)?,
            created_at: Some(r.get(26)?),
        })
    };

    let rows = match strategy_id {
        Some(sid) => {
            let sql = format!(
                "SELECT {COLS} FROM backtest_summary
                 WHERE strategy_id = ?1 ORDER BY created_at DESC, segment ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![sid], map_row)?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
        None => {
            let sql = format!(
                "SELECT {COLS} FROM backtest_summary
                 ORDER BY created_at DESC, segment ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], map_row)?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
    };
    Ok(rows)
}

// ---------- validation records (PERSIST-001) ----------

/// Pre-transaction validation of the whole bundle (Resolution D5). Pure over
/// its inputs so the command can reject bad bundles BEFORE any write starts;
/// the SQLite CHECKs remain the second line of defense.
pub fn validate_validation_bundle(
    train_summary: &BacktestSummary,
    validation_summary: &BacktestSummary,
    record: &ValidationRecordRow,
) -> AppResult<()> {
    let fail = |msg: &str| Err(AppError::Other(format!("invalid validation bundle: {msg}")));

    if train_summary.segment != "train" {
        return fail("first summary must be the train segment");
    }
    if validation_summary.segment != "validation" {
        return fail("second summary must be the validation segment");
    }
    for s in [train_summary, validation_summary] {
        if s.strategy_id != record.strategy_id || s.dataset_id != record.dataset_id {
            return fail("summary identity must match the record");
        }
    }
    if train_summary.gate_passed.is_some()
        || train_summary.score.is_some()
        || train_summary.score_breakdown_json.is_some()
        || train_summary.benchmark_result_json.is_some()
    {
        return fail("train summary Phase B fields must be null");
    }
    if validation_summary.gate_passed != Some(record.gate_passed) {
        return fail("validation summary gate_passed must match the record");
    }
    if validation_summary.benchmark_result_json.is_none() {
        return fail("validation summary requires the benchmark record");
    }
    if record.gate_passed {
        // PR #65 review: non-null was not enough — the latest view and the
        // immutable row must agree on the SAME finite score.
        let row_score = match record.score {
            Some(score) if score.is_finite() => score,
            _ => return fail("a passing gate requires a finite score"),
        };
        match validation_summary.score {
            Some(score) if score.is_finite() && score == row_score => {}
            _ => return fail("validation summary score must equal the record score"),
        }
        if validation_summary.score_breakdown_json.is_none() {
            return fail("a passing gate requires validation score + breakdown");
        }
    } else {
        if record.score.is_some()
            || validation_summary.score.is_some()
            || validation_summary.score_breakdown_json.is_some()
        {
            return fail("a failing gate forbids any score fields");
        }
    }

    // The record_json envelope must agree with the row AND with the summary's
    // latest-view snapshots — otherwise a self-contradictory audit record
    // would be appended forever (PR #65 review).
    let envelope: serde_json::Value = serde_json::from_str(&record.record_json)?;
    if envelope.get("version").and_then(|v| v.as_str()) != Some(record.record_version.as_str()) {
        return fail("record_version must match the record_json envelope version");
    }
    if envelope.get("strategyId").and_then(|v| v.as_i64()) != Some(record.strategy_id)
        || envelope.get("datasetId").and_then(|v| v.as_i64()) != Some(record.dataset_id)
    {
        return fail("record_json identity must match the record row");
    }
    if envelope.get("gatePassed").and_then(|v| v.as_bool()) != Some(record.gate_passed) {
        return fail("record_json gatePassed must match the record row");
    }
    let env_score = envelope.get("score");
    if record.gate_passed {
        let env_score_value = env_score.and_then(|s| s.get("score")).and_then(|v| v.as_f64());
        if env_score_value != record.score {
            return fail("record_json score must equal the record row score");
        }
        let breakdown: serde_json::Value = serde_json::from_str(
            validation_summary
                .score_breakdown_json
                .as_deref()
                .expect("checked above"),
        )?;
        if env_score != Some(&breakdown) {
            return fail("validation summary breakdown must equal the record snapshot");
        }
    } else if env_score.map(|v| !v.is_null()).unwrap_or(true) {
        return fail("a failing gate requires a null record_json score");
    }
    // PR #65 second review: the benchmark must be a REAL bench-record-v1
    // object — JSON null / non-objects / wrong versions must never
    // impersonate the required benchmark evidence.
    let benchmark: serde_json::Value = serde_json::from_str(
        validation_summary
            .benchmark_result_json
            .as_deref()
            .expect("checked above"),
    )?;
    let bench_shape_ok = benchmark
        .as_object()
        .map(|o| {
            o.get("version").and_then(|v| v.as_str()) == Some("bench-record-v1")
                && o.get("benchmarks").map(|b| b.is_array()).unwrap_or(false)
                && o.get("randomEntry").map(|r| r.is_object()).unwrap_or(false)
        })
        .unwrap_or(false);
    if !bench_shape_ok {
        return fail("validation summary benchmark must be a bench-record-v1 object");
    }
    if envelope.get("benchmark") != Some(&benchmark) {
        return fail("validation summary benchmark must equal the record snapshot");
    }
    Ok(())
}

/// Append one immutable record (plain INSERT — never an upsert).
fn insert_validation_record(conn: &Connection, r: &ValidationRecordRow) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO validation_records
            (strategy_id, dataset_id, record_version, gate_passed, score, record_json)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            r.strategy_id,
            r.dataset_id,
            r.record_version,
            r.gate_passed,
            r.score,
            r.record_json
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Atomically persist one validation bundle: Train summary + trades,
/// Validation summary + trades, and the immutable record — all in ONE
/// transaction. Any failure rolls the whole bundle back (Resolution D5).
/// Callers must run `validate_validation_bundle` first.
pub fn save_validation_bundle(
    conn: &mut Connection,
    train_summary: &BacktestSummary,
    train_trades: &[TradeRow],
    validation_summary: &BacktestSummary,
    validation_trades: &[TradeRow],
    record: &ValidationRecordRow,
) -> AppResult<i64> {
    let tx = conn.transaction()?;
    write_backtest_result(&tx, train_summary, train_trades)?;
    write_backtest_result(&tx, validation_summary, validation_trades)?;
    let record_id = insert_validation_record(&tx, record)?;
    tx.commit()?;
    Ok(record_id)
}

const VALIDATION_RECORD_COLS: &str =
    "id, strategy_id, dataset_id, record_version, gate_passed, score, record_json, created_at";

fn map_validation_record(r: &rusqlite::Row) -> rusqlite::Result<ValidationRecordRow> {
    Ok(ValidationRecordRow {
        id: Some(r.get(0)?),
        strategy_id: r.get(1)?,
        dataset_id: r.get(2)?,
        record_version: r.get(3)?,
        gate_passed: r.get(4)?,
        score: r.get(5)?,
        record_json: r.get(6)?,
        created_at: Some(r.get(7)?),
    })
}

/// List records newest first. Pass `strategy_id` to scope to one strategy.
pub fn list_validation_records(
    conn: &Connection,
    strategy_id: Option<i64>,
) -> AppResult<Vec<ValidationRecordRow>> {
    let rows = match strategy_id {
        Some(sid) => {
            let sql = format!(
                "SELECT {VALIDATION_RECORD_COLS} FROM validation_records
                 WHERE strategy_id = ?1 ORDER BY created_at DESC, id DESC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![sid], map_validation_record)?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
        None => {
            let sql = format!(
                "SELECT {VALIDATION_RECORD_COLS} FROM validation_records
                 ORDER BY created_at DESC, id DESC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], map_validation_record)?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        }
    };
    Ok(rows)
}

pub fn get_validation_record(conn: &Connection, id: i64) -> AppResult<ValidationRecordRow> {
    let sql = format!("SELECT {VALIDATION_RECORD_COLS} FROM validation_records WHERE id = ?1");
    Ok(conn.query_row(&sql, params![id], map_validation_record)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A migrated in-memory DB — no temp files, isolated per test.
    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        crate::db::apply_migrations(&conn).expect("apply migrations");
        conn
    }

    fn blocks_strategy(hash: &str) -> StrategyDef {
        StrategyDef {
            id: None,
            name: "blocks test".into(),
            kind: "blocks".into(),
            dsl_json: None,
            original_definition_json:
                r#"{"mode":"blocks","entryRules":[{"l":"price","op":"<","r":"bbLower"}]}"#.into(),
            param_schema_json: None,
            source: "manual".into(),
            ai_prompt_hash: None,
            strategy_hash: hash.into(),
            lifecycle: "candidate".into(),
            parent_strategy_id: None,
        }
    }

    fn saved_parent_rows(conn: &Connection) -> (i64, i64) {
        let dataset_id = insert_dataset(
            conn,
            &Dataset {
                id: None,
                exchange: "test".into(),
                symbol: "BTCUSDT".into(),
                interval: "1h".into(),
                start_time: 1,
                end_time: 10,
                candle_count: 10,
                source: "test".into(),
                dataset_hash: "trade-test-dataset".into(),
            },
        )
        .unwrap();
        let strategy_id = insert_strategy(conn, &blocks_strategy("trade-test-strategy")).unwrap();
        (strategy_id, dataset_id)
    }

    fn summary(strategy_id: i64, dataset_id: i64, net_return: f64) -> BacktestSummary {
        BacktestSummary {
            id: None,
            strategy_id,
            dataset_id,
            segment: "full".into(),
            start_time: 1,
            end_time: 10,
            net_return: Some(net_return),
            cagr: None,
            max_drawdown: None,
            sharpe: None,
            sortino: None,
            calmar: None,
            win_rate: None,
            trade_count: None,
            profit_factor: None,
            avg_trade_return: None,
            median_trade_return: None,
            exposure: None,
            turnover: None,
            largest_win: None,
            largest_loss: None,
            consecutive_losses: None,
            gate_passed: None,
            score: None,
            score_breakdown_json: None,
            benchmark_result_json: None,
            created_at: None,
        }
    }

    fn trade(entry_time: i64, exit_time: i64, reason: Option<&str>) -> TradeRow {
        TradeRow {
            entry_time,
            exit_time,
            side: "LONG".into(),
            entry_price: 100.0,
            exit_price: 110.0,
            pnl: 10.0,
            pnl_pct: 0.1,
            reason: reason.map(str::to_owned),
        }
    }

    #[test]
    fn insert_strategy_persists_blocks_type_and_definition() {
        let conn = mem_db();
        let id = insert_strategy(&conn, &blocks_strategy("hash-blocks-1")).unwrap();

        let (kind, def): (String, String) = conn
            .query_row(
                "SELECT type, original_definition_json FROM strategy_def WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "blocks");
        assert!(def.contains("\"mode\":\"blocks\""));

        // and it reads back through the repository as a blocks strategy
        let listed = list_strategies(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, "blocks");
    }

    #[test]
    fn insert_strategy_upserts_on_hash_without_duplicating() {
        let conn = mem_db();
        let id1 = insert_strategy(&conn, &blocks_strategy("dup-hash")).unwrap();
        conn.execute(
            "UPDATE strategy_def SET lifecycle = 'validated' WHERE id = ?1",
            params![id1],
        )
        .unwrap();

        let mut resaved = blocks_strategy("dup-hash");
        resaved.name = "renamed blocks strategy".into();
        resaved.source = "sweep".into();
        // The frontend currently submits candidate on each manual save. The DB
        // must retain the validation-owned lifecycle already on the row.
        resaved.lifecycle = "candidate".into();
        let id2 = insert_strategy(&conn, &resaved).unwrap();
        assert_eq!(id1, id2, "same strategy_hash must not create a second row");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM strategy_def WHERE strategy_hash = 'dup-hash'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let (name, source, lifecycle): (String, String, String) = conn
            .query_row(
                "SELECT name, source, lifecycle FROM strategy_def WHERE id = ?1",
                params![id1],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "renamed blocks strategy");
        assert_eq!(source, "sweep");
        assert_eq!(lifecycle, "validated");
    }

    #[test]
    fn insert_strategy_persists_rename_for_same_hash() {
        let conn = mem_db();
        let mut original = blocks_strategy("rename-hash");
        original.name = "old name".into();
        insert_strategy(&conn, &original).unwrap();

        let mut renamed = blocks_strategy("rename-hash");
        renamed.name = "new name".into();
        insert_strategy(&conn, &renamed).unwrap();

        let listed = list_strategies(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "new name");
    }

    #[test]
    fn save_backtest_result_replaces_trades_for_same_summary() {
        let mut conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        let original = summary(strategy_id, dataset_id, 0.1);
        let summary_id = save_backtest_result(
            &mut conn,
            &original,
            &[trade(1, 2, None), trade(3, 4, None)],
        )
        .unwrap();

        let replacement = summary(strategy_id, dataset_id, 0.2);
        let replacement_id =
            save_backtest_result(&mut conn, &replacement, &[trade(5, 6, Some("signal"))])
                .unwrap();

        assert_eq!(replacement_id, summary_id);
        let (count, entry_time, reason): (i64, i64, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), MIN(entry_time), MAX(reason)
                 FROM trades WHERE backtest_summary_id = ?1",
                params![summary_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(count, 1, "replacement must not accumulate old trades");
        assert_eq!(entry_time, 5);
        assert_eq!(reason.as_deref(), Some("signal"));

        let net_return: f64 = conn
            .query_row(
                "SELECT net_return FROM backtest_summary WHERE id = ?1",
                params![summary_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(net_return, 0.2);
    }

    #[test]
    fn save_backtest_result_rolls_back_summary_and_trades_together() {
        let mut conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        let original = summary(strategy_id, dataset_id, 0.1);
        let summary_id =
            save_backtest_result(&mut conn, &original, &[trade(1, 2, None)]).unwrap();

        conn.execute_batch(
            "CREATE TRIGGER reject_marked_trade
             BEFORE INSERT ON trades
             WHEN NEW.reason = 'reject'
             BEGIN SELECT RAISE(ABORT, 'rejected test trade'); END;",
        )
        .unwrap();

        let replacement = summary(strategy_id, dataset_id, 0.9);
        let result = save_backtest_result(
            &mut conn,
            &replacement,
            &[trade(3, 4, None), trade(7, 8, Some("reject"))],
        );
        assert!(result.is_err());

        let (net_return, count, entry_time): (f64, i64, i64) = conn
            .query_row(
                "SELECT s.net_return, COUNT(t.id), MIN(t.entry_time)
                 FROM backtest_summary s
                 JOIN trades t ON t.backtest_summary_id = s.id
                 WHERE s.id = ?1",
                params![summary_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(net_return, 0.1, "failed replacement must roll back summary");
        assert_eq!(count, 1, "failed replacement must retain prior trades");
        assert_eq!(entry_time, 1);
    }

    #[test]
    fn deleting_strategy_cascades_to_summary_and_trades() {
        let mut conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        save_backtest_result(
            &mut conn,
            &summary(strategy_id, dataset_id, 0.1),
            &[trade(1, 2, None)],
        )
        .unwrap();

        conn.execute("DELETE FROM strategy_def WHERE id = ?1", params![strategy_id])
            .unwrap();

        let summaries: i64 = conn
            .query_row("SELECT COUNT(*) FROM backtest_summary", [], |row| row.get(0))
            .unwrap();
        let trades: i64 = conn
            .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))
            .unwrap();
        assert_eq!(summaries, 0);
        assert_eq!(trades, 0);
    }

    // ---------- PERSIST-001: validation records ----------

    fn seg_summary(strategy_id: i64, dataset_id: i64, segment: &str) -> BacktestSummary {
        let mut s = summary(strategy_id, dataset_id, 0.1);
        s.segment = segment.into();
        s
    }

    const TEST_BREAKDOWN: &str = r#"{"formulaVersion":"score-v1","score":2.65}"#;
    const TEST_BENCHMARK: &str =
        r#"{"version":"bench-record-v1","benchmarks":[],"randomEntry":{"runs":20}}"#;

    fn record_json_for(strategy_id: i64, dataset_id: i64, gate_passed: bool) -> String {
        let score = if gate_passed { TEST_BREAKDOWN.to_string() } else { "null".to_string() };
        format!(
            r#"{{"version":"validation-record-v1","strategyId":{strategy_id},"datasetId":{dataset_id},"gatePassed":{gate_passed},"score":{score},"benchmark":{TEST_BENCHMARK}}}"#
        )
    }

    fn passing_bundle(
        strategy_id: i64,
        dataset_id: i64,
    ) -> (BacktestSummary, BacktestSummary, ValidationRecordRow) {
        let train = seg_summary(strategy_id, dataset_id, "train");
        let mut validation = seg_summary(strategy_id, dataset_id, "validation");
        validation.gate_passed = Some(true);
        validation.score = Some(2.65);
        validation.score_breakdown_json = Some(TEST_BREAKDOWN.into());
        validation.benchmark_result_json = Some(TEST_BENCHMARK.into());
        let record = ValidationRecordRow {
            id: None,
            strategy_id,
            dataset_id,
            record_version: "validation-record-v1".into(),
            gate_passed: true,
            score: Some(2.65),
            record_json: record_json_for(strategy_id, dataset_id, true),
            created_at: None,
        };
        (train, validation, record)
    }

    fn failing_bundle(
        strategy_id: i64,
        dataset_id: i64,
    ) -> (BacktestSummary, BacktestSummary, ValidationRecordRow) {
        let (train, mut validation, mut record) = passing_bundle(strategy_id, dataset_id);
        validation.gate_passed = Some(false);
        validation.score = None;
        validation.score_breakdown_json = None;
        record.gate_passed = false;
        record.score = None;
        record.record_json = record_json_for(strategy_id, dataset_id, false);
        (train, validation, record)
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn migrations_apply_0002_and_rerun_idempotently() {
        let conn = mem_db();
        assert_eq!(count(&conn, "validation_records"), 0, "table must exist");
        crate::db::apply_migrations(&conn).expect("re-run must be a no-op");
        assert_eq!(count(&conn, "schema_migrations"), 2);
    }

    #[test]
    fn migration_0002_upgrades_an_existing_0001_database_preserving_data() {
        // Simulate a DB created before 0002 existed.
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version    TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        conn.execute_batch(include_str!("../../migrations/0001_init.sql"))
            .unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version) VALUES ('0001_init')",
            [],
        )
        .unwrap();
        let _ = saved_parent_rows(&conn);

        crate::db::apply_migrations(&conn).expect("upgrade to 0002");

        assert_eq!(count(&conn, "strategy_def"), 1, "existing data survives");
        assert_eq!(count(&conn, "datasets"), 1);
        assert_eq!(count(&conn, "validation_records"), 0, "new table exists");
        assert_eq!(count(&conn, "schema_migrations"), 2);
    }

    #[test]
    fn validation_record_checks_enforce_gate_score_invariant_and_fks() {
        let conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        let insert = |sid: i64, gate: i64, score: Option<f64>| {
            conn.execute(
                "INSERT INTO validation_records
                    (strategy_id, dataset_id, record_version, gate_passed, score, record_json)
                 VALUES (?1,?2,'validation-record-v1',?3,?4,'{}')",
                params![sid, dataset_id, gate, score],
            )
        };
        assert!(insert(strategy_id, 2, None).is_err(), "gate_passed must be 0/1");
        assert!(insert(strategy_id, 0, Some(1.0)).is_err(), "fail + score violates the D3 CHECK");
        assert!(insert(strategy_id, 1, None).is_err(), "pass without score violates the D3 CHECK");
        assert!(insert(strategy_id, 0, None).is_ok());
        assert!(insert(strategy_id, 1, Some(2.5)).is_ok());
        assert!(insert(9999, 0, None).is_err(), "unknown strategy_id must violate the FK");
    }

    #[test]
    fn validate_validation_bundle_rejects_inconsistent_bundles() {
        let (train, validation, record) = passing_bundle(1, 2);
        assert!(validate_validation_bundle(&train, &validation, &record).is_ok());

        // swapped segments
        assert!(validate_validation_bundle(&validation, &train, &record).is_err());

        // identity mismatch
        let (t, v, mut r) = passing_bundle(1, 2);
        r.strategy_id = 9;
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // train row must keep Phase B fields null
        let (mut t, v, r) = passing_bundle(1, 2);
        t.gate_passed = Some(false);
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // validation row must carry the benchmark record
        let (t, mut v, r) = passing_bundle(1, 2);
        v.benchmark_result_json = None;
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // a consistent failing bundle is legal…
        let (t, v, r) = failing_bundle(1, 2);
        assert!(validate_validation_bundle(&t, &v, &r).is_ok());

        // …but a failing gate with any score field is not
        let (t, mut v, mut r) = passing_bundle(1, 2);
        r.gate_passed = false;
        v.gate_passed = Some(false);
        r.record_json = record_json_for(1, 2, false);
        assert!(validate_validation_bundle(&t, &v, &r).is_err(), "record.score still set");
        r.score = None;
        assert!(validate_validation_bundle(&t, &v, &r).is_err(), "summary score still set");

        // passing gate requires a FINITE score
        let (t, mut v, mut r) = passing_bundle(1, 2);
        r.score = Some(f64::INFINITY);
        v.score = Some(f64::INFINITY);
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // record_version must match the JSON envelope
        let (t, v, mut r) = passing_bundle(1, 2);
        r.record_json = r#"{"version":"something-else"}"#.into();
        assert!(validate_validation_bundle(&t, &v, &r).is_err());
    }

    #[test]
    fn validate_validation_bundle_rejects_contradictory_scores_and_envelopes() {
        // summary score finite but DIFFERENT from the record row (PR #65 P1)
        let (t, mut v, r) = passing_bundle(1, 2);
        v.score = Some(999.0);
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // envelope identity contradicts the row
        let (t, v, mut r) = passing_bundle(1, 2);
        r.record_json = record_json_for(9, 2, true);
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // envelope gatePassed contradicts the row
        let (t, v, mut r) = passing_bundle(1, 2);
        r.record_json = record_json_for(1, 2, false);
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // envelope score value contradicts the row score
        let (t, v, mut r) = passing_bundle(1, 2);
        r.record_json = r.record_json.replace("2.65", "9.99");
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // summary breakdown snapshot differs from the envelope's
        let (t, mut v, r) = passing_bundle(1, 2);
        v.score_breakdown_json = Some(r#"{"formulaVersion":"score-v1","score":2.65,"x":1}"#.into());
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // summary benchmark snapshot differs from the envelope's (valid shape)
        let (t, mut v, r) = passing_bundle(1, 2);
        v.benchmark_result_json =
            Some(r#"{"version":"bench-record-v1","benchmarks":[1],"randomEntry":{"runs":20}}"#.into());
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // key-order differences alone are NOT a mismatch (structural compare)
        let (t, mut v, r) = passing_bundle(1, 2);
        v.score_breakdown_json = Some(r#"{"score":2.65,"formulaVersion":"score-v1"}"#.into());
        assert!(validate_validation_bundle(&t, &v, &r).is_ok());
    }

    #[test]
    fn validate_validation_bundle_requires_a_real_benchmark_object() {
        // PR #65 second review: even a CONSISTENT null/non-object/wrong-version
        // pair (summary + envelope agreeing) must be rejected — no audit
        // record may exist without real benchmark evidence.
        let cases = [
            "null",
            "[]",
            "{}",
            r#"{"version":"bench-record-v999","benchmarks":[],"randomEntry":{}}"#,
            r#"{"version":"bench-record-v1","benchmarks":{},"randomEntry":{}}"#,
            r#"{"version":"bench-record-v1","benchmarks":[]}"#,
        ];
        for bogus in cases {
            let (t, mut v, mut r) = passing_bundle(1, 2);
            v.benchmark_result_json = Some(bogus.into());
            r.record_json = r.record_json.replace(TEST_BENCHMARK, bogus);
            assert!(
                validate_validation_bundle(&t, &v, &r).is_err(),
                "benchmark impersonation must be rejected: {bogus}"
            );
        }
    }

    #[test]
    fn save_validation_bundle_commits_and_appends_immutably() {
        let mut conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        let (train, validation, record) = passing_bundle(strategy_id, dataset_id);

        let record_id = save_validation_bundle(
            &mut conn,
            &train,
            &[trade(1, 2, None)],
            &validation,
            &[trade(3, 4, None), trade(5, 6, None)],
            &record,
        )
        .unwrap();

        assert_eq!(count(&conn, "backtest_summary"), 2);
        assert_eq!(count(&conn, "trades"), 3);
        let read = get_validation_record(&conn, record_id).unwrap();
        assert_eq!(read.record_json, record.record_json, "exact JSON reads back");
        assert!(read.gate_passed);
        assert_eq!(read.score, Some(2.65));

        // Append-only: a re-run appends a SECOND record while the summaries
        // upsert (latest view) and the trades replace.
        let id2 = save_validation_bundle(&mut conn, &train, &[], &validation, &[], &record).unwrap();
        assert_ne!(record_id, id2);
        assert_eq!(list_validation_records(&conn, Some(strategy_id)).unwrap().len(), 2);
        assert_eq!(list_validation_records(&conn, None).unwrap().len(), 2);
        assert_eq!(count(&conn, "backtest_summary"), 2, "summaries stay the latest view");
        assert_eq!(count(&conn, "trades"), 0, "trade rows replaced by the re-run");
    }

    #[test]
    fn save_validation_bundle_rolls_back_the_whole_bundle_on_failure() {
        let mut conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        let (train, mut validation, record) = passing_bundle(strategy_id, dataset_id);
        // Bypass command-level pre-validation on purpose: an illegal segment
        // makes the SECOND summary insert violate the schema CHECK after the
        // train summary already wrote inside the transaction.
        validation.segment = "bogus".into();

        let result = save_validation_bundle(
            &mut conn,
            &train,
            &[trade(1, 2, None)],
            &validation,
            &[],
            &record,
        );
        assert!(result.is_err());
        assert_eq!(count(&conn, "backtest_summary"), 0, "train summary must roll back");
        assert_eq!(count(&conn, "trades"), 0);
        assert_eq!(count(&conn, "validation_records"), 0);
    }
}
