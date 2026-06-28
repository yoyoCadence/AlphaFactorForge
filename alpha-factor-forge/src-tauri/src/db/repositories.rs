// SKELETON — data-access layer. Phase A implements datasets + candles +
// strategy_def + backtest_summary CRUD enough to satisfy the v1 delivery.
// Functions marked `todo!()` need local completion (see TODO.md).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::AppResult;

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
    conn.execute(
        "INSERT INTO strategy_def
            (name, type, dsl_json, original_definition_json, param_schema_json,
             source, ai_prompt_hash, strategy_hash, lifecycle, parent_strategy_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(strategy_hash) DO UPDATE SET updated_at = datetime('now')",
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
