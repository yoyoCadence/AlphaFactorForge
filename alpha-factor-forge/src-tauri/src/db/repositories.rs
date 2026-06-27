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
// TODO(local): insert_backtest_summary + list_backtest_results with the full
// metric column set. Phase A can store the JSON-serialized summary; Phase B
// fills gate_passed / score / breakdown / benchmark columns.
