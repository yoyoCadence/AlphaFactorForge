// SKELETON — data-access layer. Phase A implements datasets + candles +
// strategy_def + backtest_summary CRUD enough to satisfy the v1 delivery.
// Functions marked `todo!()` need local completion (see TODO.md).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use alpha_factor_forge::discovery_core::market_data::{self, CandleFields};

use crate::db::validation_record::parse_new_record;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
/// self-contained versioned validation-record envelope. There is NO update or
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

/// Verify a v2 content identity and import the immutable dataset payload in one
/// transaction. Re-importing byte-identical content is idempotent; a hash row
/// whose metadata or candles disagree is treated as corruption and rejected.
pub fn import_dataset_with_candles(
    conn: &mut Connection,
    dataset: &Dataset,
    candles: &[Candle],
) -> AppResult<i64> {
    let normalized = crate::identity::verify_dataset_identity(dataset, candles)?;
    // DATA-QUALITY-001 mount point 3 — admission runs BEFORE the transaction is
    // opened. That ordering is what makes atomicity provable rather than
    // incidental: a rejected payload never reaches a writable transaction, so no
    // previously imported dataset can be disturbed by it.
    market_data::ensure_admissible(normalized.iter().map(db_candle_fields))
        .map_err(|error| AppError::Other(error.0))?;
    let tx = conn.transaction()?;
    let existing: Option<Dataset> = tx
        .query_row(
            "SELECT id, exchange, symbol, interval, start_time, end_time,
                    candle_count, source, dataset_hash
             FROM datasets WHERE dataset_hash = ?1",
            [&dataset.dataset_hash],
            |row| {
                Ok(Dataset {
                    id: Some(row.get(0)?),
                    exchange: row.get(1)?,
                    symbol: row.get(2)?,
                    interval: row.get(3)?,
                    start_time: row.get(4)?,
                    end_time: row.get(5)?,
                    candle_count: row.get(6)?,
                    source: row.get(7)?,
                    dataset_hash: row.get(8)?,
                })
            },
        )
        .optional()?;

    if let Some(existing) = existing {
        let dataset_id = existing.id.expect("queried dataset id");
        let same_metadata = existing.exchange == dataset.exchange
            && existing.symbol == dataset.symbol
            && existing.interval == dataset.interval
            && existing.start_time == dataset.start_time
            && existing.end_time == dataset.end_time
            && existing.candle_count == dataset.candle_count
            && existing.source == dataset.source;
        let stored = {
            let mut statement = tx.prepare(
                "SELECT timestamp, open, high, low, close, volume
                 FROM candles WHERE dataset_id = ?1 ORDER BY timestamp ASC",
            )?;
            let rows = statement
                .query_map([dataset_id], |row| {
                    Ok(Candle {
                        timestamp: row.get(0)?,
                        open: row.get(1)?,
                        high: row.get(2)?,
                        low: row.get(3)?,
                        close: row.get(4)?,
                        volume: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let same_candles = stored.len() == normalized.len()
            && stored
                .iter()
                .zip(&normalized)
                .all(|(left, right)| candles_equal(left, right));
        if !same_metadata || !same_candles {
            return Err(AppError::Other(
                "dataset hash conflicts with stored immutable payload".into(),
            ));
        }
        tx.commit()?;
        return Ok(dataset_id);
    }

    tx.execute(
        "INSERT INTO datasets
            (exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            dataset.exchange,
            dataset.symbol,
            dataset.interval,
            dataset.start_time,
            dataset.end_time,
            dataset.candle_count,
            dataset.source,
            dataset.dataset_hash
        ],
    )?;
    let dataset_id = tx.last_insert_rowid();
    {
        let mut statement = tx.prepare(
            "INSERT INTO candles
                (dataset_id, timestamp, open, high, low, close, volume)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
        )?;
        for candle in &normalized {
            statement.execute(params![
                dataset_id,
                candle.timestamp,
                candle.open,
                candle.high,
                candle.low,
                candle.close,
                candle.volume
            ])?;
        }
    }
    tx.commit()?;
    Ok(dataset_id)
}

/// The single mapping point from a persisted candle row to validator fields, so
/// the import gate and the discovery-runner gate cannot drift apart.
pub(crate) fn db_candle_fields(candle: &Candle) -> CandleFields {
    CandleFields {
        timestamp: candle.timestamp as f64,
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
        volume: candle.volume,
    }
}

fn candles_equal(left: &Candle, right: &Candle) -> bool {
    left.timestamp == right.timestamp
        && left.open.to_bits() == right.open.to_bits()
        && left.high.to_bits() == right.high.to_bits()
        && left.low.to_bits() == right.low.to_bits()
        && left.close.to_bits() == right.close.to_bits()
        && left.volume.to_bits() == right.volume.to_bits()
}

#[cfg(test)]
fn insert_dataset(conn: &Connection, d: &Dataset) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO datasets
            (exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(dataset_hash) DO UPDATE SET
            candle_count = excluded.candle_count,
            end_time     = excluded.end_time,
            updated_at   = datetime('now')",
        params![
            d.exchange,
            d.symbol,
            d.interval,
            d.start_time,
            d.end_time,
            d.candle_count,
            d.source,
            d.dataset_hash
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

/// Load one dataset by its stable row id.
///
/// Discovery admission carries both the row id and its durable content hash,
/// so the runner needs the exact row rather than a list-and-find read.
pub fn get_dataset_by_id(conn: &Connection, dataset_id: i64) -> AppResult<Dataset> {
    conn.query_row(
        "SELECT id, exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash
         FROM datasets WHERE id = ?1",
        [dataset_id],
        |r| {
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
        },
    )
    .optional()?
    .ok_or_else(|| AppError::Other(format!("dataset {dataset_id} not found")))
}

// ---------- candles ----------

pub fn get_candles(
    conn: &Connection,
    dataset_id: i64,
    from: i64,
    to: i64,
) -> AppResult<Vec<Candle>> {
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

/// Product write boundary: reject legacy/forged hashes before persistence.
pub fn insert_verified_strategy(conn: &Connection, strategy: &StrategyDef) -> AppResult<i64> {
    crate::identity::verify_strategy_identity(strategy)?;
    insert_strategy(conn, strategy)
}

/// Runner write boundary: verify the durable strategy identity, insert a new
/// candidate when absent, and otherwise return the existing row unchanged.
///
/// The manual save path deliberately refreshes mutable presentation fields on
/// a hash conflict. Discovery admission must not do that: enumerating a
/// canonical strategy already present in the user's library cannot overwrite
/// its chosen name/source or validation-owned lifecycle.
pub fn get_or_insert_verified_runner_strategy(
    conn: &Connection,
    strategy: &StrategyDef,
) -> AppResult<i64> {
    crate::identity::verify_strategy_identity(strategy)?;
    conn.execute(
        "INSERT INTO strategy_def
            (name, type, dsl_json, original_definition_json, param_schema_json,
             source, ai_prompt_hash, strategy_hash, lifecycle, parent_strategy_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
         ON CONFLICT(strategy_hash) DO NOTHING",
        params![
            strategy.name,
            strategy.kind,
            strategy.dsl_json,
            strategy.original_definition_json,
            strategy.param_schema_json,
            strategy.source,
            strategy.ai_prompt_hash,
            strategy.strategy_hash,
            strategy.lifecycle,
            strategy.parent_strategy_id
        ],
    )?;
    let id = conn.query_row(
        "SELECT id FROM strategy_def WHERE strategy_hash = ?1",
        [&strategy.strategy_hash],
        |row| row.get(0),
    )?;
    Ok(id)
}

fn insert_strategy(conn: &Connection, s: &StrategyDef) -> AppResult<i64> {
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
            s.name,
            s.kind,
            s.dsl_json,
            s.original_definition_json,
            s.param_schema_json,
            s.source,
            s.ai_prompt_hash,
            s.strategy_hash,
            s.lifecycle,
            s.parent_strategy_id
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
pub(crate) fn write_backtest_result(
    conn: &Connection,
    summary: &BacktestSummary,
    trades: &[TradeRow],
) -> AppResult<i64> {
    // PERSIST-INVARIANT-001 — checked HERE, in the funnel, rather than in each
    // caller. All three writers pass through this function (the manual save, the
    // manual validation bundle, and the runner's candidate commit), so a future
    // fourth writer cannot forget the invariants. Nothing is written before this
    // point in this function, and every caller wraps it in a transaction, so a
    // rejection leaves the previously stored result exactly as it was.
    validate_result_bundle(summary, trades)?;
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

// ---------- result-bundle invariants (PERSIST-INVARIANT-001) ----------

/// The segments the 0001 CHECK constraint allows.
const RESULT_SEGMENTS: [&str; 4] = ["train", "validation", "test", "full"];
/// The side vocabulary the 0001 CHECK constraint allows.
const TRADE_SIDES: [&str; 2] = ["LONG", "SHORT"];

/// Every cross-field invariant a summary + its trades must satisfy before the
/// pair may be written.
///
/// The audit found that `save_backtest_result` accepted any bundle at all: a
/// `trade_count` that disagreed with the rows beside it, a side outside
/// `LONG|SHORT`, an exit before its entry, trades outside the summary's own time
/// range, non-positive prices, or a non-finite metric. None of that is caught by
/// the schema — the CHECKs cover single columns, not agreement BETWEEN a summary
/// and its rows — and a stored contradiction is worse than a rejected write,
/// because every later reader trusts the row.
///
/// Two deliberate limits on strictness, so this gate rejects contradictions
/// rather than unusual-but-real results:
///   - Only definitionally bounded ratios are bounded. `win_rate` and `exposure`
///     are counts over counts, so they must sit in `[0, 1]`; `max_drawdown` and
///     `turnover` must be non-negative but are NOT capped at 1, because a short
///     position can lose more than the starting equity and a real drawdown may
///     exceed 100%. `net_return`, `sharpe`, `calmar`, and friends are only
///     required to be finite.
///   - Every PRESENT numeric field must be finite. That is not an extra rule but
///     the existing persistence contract: the metrics mapper narrows a
///     legitimately infinite value (a profit factor with no losing trade) to
///     NULL, so a non-finite value in a column means the mapper was bypassed.
pub fn validate_result_bundle(summary: &BacktestSummary, trades: &[TradeRow]) -> AppResult<()> {
    let fail = |msg: String| -> AppResult<()> {
        Err(AppError::Other(format!("invalid result bundle: {msg}")))
    };

    if !RESULT_SEGMENTS.contains(&summary.segment.as_str()) {
        return fail(format!(
            "segment must be one of {}, got \"{}\"",
            RESULT_SEGMENTS.join("|"),
            summary.segment
        ));
    }
    if summary.start_time > summary.end_time {
        return fail(format!(
            "summary range is inverted: start {} > end {}",
            summary.start_time, summary.end_time
        ));
    }

    // Finite-or-null: a present non-finite value means the metrics mapper's
    // narrowing was bypassed on the way in.
    for (name, value) in [
        ("net_return", summary.net_return),
        ("cagr", summary.cagr),
        ("max_drawdown", summary.max_drawdown),
        ("sharpe", summary.sharpe),
        ("sortino", summary.sortino),
        ("calmar", summary.calmar),
        ("win_rate", summary.win_rate),
        ("profit_factor", summary.profit_factor),
        ("avg_trade_return", summary.avg_trade_return),
        ("median_trade_return", summary.median_trade_return),
        ("exposure", summary.exposure),
        ("turnover", summary.turnover),
        ("largest_win", summary.largest_win),
        ("largest_loss", summary.largest_loss),
        ("score", summary.score),
    ] {
        if let Some(value) = value {
            if !value.is_finite() {
                return fail(format!("{name} must be finite when present, got {value}"));
            }
        }
    }

    // Ratios that are counts over counts.
    for (name, value) in [("win_rate", summary.win_rate), ("exposure", summary.exposure)] {
        if let Some(value) = value {
            if !(0.0..=1.0).contains(&value) {
                return fail(format!("{name} must be within [0, 1], got {value}"));
            }
        }
    }
    for (name, value) in [
        ("max_drawdown", summary.max_drawdown),
        ("turnover", summary.turnover),
    ] {
        if let Some(value) = value {
            if value < 0.0 {
                return fail(format!("{name} must be non-negative, got {value}"));
            }
        }
    }
    for (name, value) in [
        ("trade_count", summary.trade_count),
        ("consecutive_losses", summary.consecutive_losses),
    ] {
        if let Some(value) = value {
            if value < 0 {
                return fail(format!("{name} must be non-negative, got {value}"));
            }
        }
    }

    // The invariant a reader is most likely to trust blindly: the summary's
    // count and the rows beside it must be the same number.
    if let Some(count) = summary.trade_count {
        if count != trades.len() as i64 {
            return fail(format!(
                "trade_count {count} does not match the {} trade rows in the bundle",
                trades.len()
            ));
        }
    }

    for (index, trade) in trades.iter().enumerate() {
        let at = |msg: String| -> AppResult<()> { fail(format!("trade {index}: {msg}")) };
        if !TRADE_SIDES.contains(&trade.side.as_str()) {
            return at(format!(
                "side must be one of {}, got \"{}\"",
                TRADE_SIDES.join("|"),
                trade.side
            ));
        }
        if trade.entry_time > trade.exit_time {
            return at(format!(
                "exit {} precedes entry {}",
                trade.exit_time, trade.entry_time
            ));
        }
        if trade.entry_time < summary.start_time || trade.exit_time > summary.end_time {
            return at(format!(
                "[{}, {}] falls outside the summary range [{}, {}]",
                trade.entry_time, trade.exit_time, summary.start_time, summary.end_time
            ));
        }
        for (name, price) in [
            ("entry_price", trade.entry_price),
            ("exit_price", trade.exit_price),
        ] {
            if !price.is_finite() || price <= 0.0 {
                return at(format!("{name} must be finite and > 0, got {price}"));
            }
        }
        for (name, value) in [("pnl", trade.pnl), ("pnl_pct", trade.pnl_pct)] {
            if !value.is_finite() {
                return at(format!("{name} must be finite, got {value}"));
            }
        }
    }

    Ok(())
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
    let decoded = parse_new_record(&record.record_version, &record.record_json)
        .map_err(|message| AppError::Other(format!("invalid validation bundle: {message}")))?;
    if decoded.strategy_id != record.strategy_id || decoded.dataset_id != record.dataset_id {
        return fail("record_json identity must match the record row");
    }
    if decoded.gate_passed != record.gate_passed {
        return fail("record_json gatePassed must match the record row");
    }
    decoded
        .train_metrics
        .validate_summary(train_summary, "trainSummary")
        .and_then(|_| {
            decoded
                .validation_metrics
                .validate_summary(validation_summary, "validationSummary")
        })
        .map_err(|message| AppError::Other(format!("invalid validation bundle: {message}")))?;

    let envelope: serde_json::Value = serde_json::from_str(&record.record_json)?;
    let env_score = envelope.get("score");
    if record.gate_passed {
        let decoded_score = decoded.score.0.as_ref().expect("strict DTO checked above");
        if Some(decoded_score.score) != record.score {
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
    } else if env_score.map(|value| !value.is_null()).unwrap_or(true) {
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
    if envelope.get("benchmark") != Some(&benchmark) {
        return fail("validation summary benchmark must equal the record snapshot");
    }
    Ok(())
}

/// Append one immutable record (plain INSERT — never an upsert).
fn insert_validation_record(conn: &Connection, r: &ValidationRecordRow) -> AppResult<i64> {
    insert_validation_record_for_run(conn, r, None)
}

/// Same append, optionally linked to the discovery run that produced it.
/// Manual UI saves pass `None` and stay outside the per-run uniqueness rule
/// (migration 0003); runner assessments pass their run id so at most one
/// assessment can exist per (run, strategy, dataset).
///
/// Takes `&Connection` so a caller-owned `Transaction` can pass itself in and
/// keep the whole candidate commit atomic (Resolution D5).
pub(crate) fn insert_validation_record_for_run(
    conn: &Connection,
    r: &ValidationRecordRow,
    discovery_run_id: Option<i64>,
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO validation_records
            (strategy_id, dataset_id, record_version, gate_passed, score, record_json,
             discovery_run_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            r.strategy_id,
            r.dataset_id,
            r.record_version,
            r.gate_passed,
            r.score,
            r.record_json,
            discovery_run_id
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

    /// DATA-QUALITY-001: the original timestamps `1`/`2` are no longer
    /// admissible market data. These are the committed TypeScript identity
    /// fixture's values. The dataset hash below is computed, never a literal, so
    /// moving the timestamps changes nothing else in these tests — and
    /// `identity.rs`'s own `1`/`2` hashing tests are deliberately left alone as
    /// the mechanical proof that the validator did not enter the identity path.
    const IDENTITY_CANDLE_T0: i64 = 1_721_001_600_000;
    const IDENTITY_CANDLE_T1: i64 = 1_721_005_200_000;

    fn identity_candles() -> Vec<Candle> {
        vec![
            Candle {
                timestamp: IDENTITY_CANDLE_T1,
                open: 101.0,
                high: 103.0,
                low: 100.0,
                close: 102.0,
                volume: 12.0,
            },
            Candle {
                timestamp: IDENTITY_CANDLE_T0,
                open: 100.0,
                high: 102.0,
                low: 99.0,
                close: 101.0,
                volume: 10.0,
            },
        ]
    }

    fn identity_dataset(candles: &[Candle]) -> Dataset {
        let mut dataset = Dataset {
            id: None,
            exchange: "binance".into(),
            symbol: "BTCUSDT".into(),
            interval: "1h".into(),
            start_time: candles.iter().map(|candle| candle.timestamp).min().unwrap(),
            end_time: candles.iter().map(|candle| candle.timestamp).max().unwrap(),
            candle_count: candles.len() as i64,
            source: "test-fixture".into(),
            dataset_hash: String::new(),
        };
        dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, candles).unwrap();
        dataset
    }

    fn verified_blocks_strategy() -> StrategyDef {
        let definition = r#"{"mode":"blocks","feePct":0.05,"slipPct":0.02,"entryRules":[{"l":"price","op":"<","r":"bbLower"}]}"#;
        let hash = crate::identity::strategy_hash_from_definition_json(definition).unwrap();
        let mut strategy = blocks_strategy(&hash);
        strategy.original_definition_json = definition.into();
        strategy
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

    /// Restate a fixture summary's `trade_count` for the slice a test commits.
    /// PERSIST-INVARIANT-001 requires the two to agree, and the fixture
    /// summaries carry a real assessment's count.
    fn with_trade_count(summary: &BacktestSummary, trades: usize) -> BacktestSummary {
        BacktestSummary {
            trade_count: Some(trades as i64),
            ..summary.clone()
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
    fn verified_strategy_rejects_forged_and_legacy_hashes() {
        let conn = mem_db();
        let valid = verified_blocks_strategy();
        assert!(insert_verified_strategy(&conn, &valid).is_ok());

        let mut forged = valid;
        forged.strategy_hash = "legacy-or-forged".into();
        assert!(insert_verified_strategy(&conn, &forged).is_err());
        let mut wrong_mode = verified_blocks_strategy();
        wrong_mode.kind = "params".into();
        assert!(insert_verified_strategy(&conn, &wrong_mode).is_err());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM strategy_def", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn atomic_dataset_import_is_sorted_and_idempotent() {
        let mut conn = mem_db();
        let candles = identity_candles();
        let dataset = identity_dataset(&candles);
        let first_id = import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap();
        let second_id = import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap();
        assert_eq!(first_id, second_id);

        let stored = get_candles(&conn, first_id, IDENTITY_CANDLE_T0, IDENTITY_CANDLE_T1).unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].timestamp, IDENTITY_CANDLE_T0);
        assert_eq!(stored[1].timestamp, IDENTITY_CANDLE_T1);
        let dataset_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM datasets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dataset_count, 1);
    }

    #[test]
    fn dataset_lookup_returns_the_exact_row_and_rejects_an_unknown_id() {
        let mut conn = mem_db();
        let candles = identity_candles();
        let dataset = identity_dataset(&candles);
        let dataset_id = import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap();

        let found = get_dataset_by_id(&conn, dataset_id).unwrap();
        assert_eq!(found.id, Some(dataset_id));
        assert_eq!(found.exchange, dataset.exchange);
        assert_eq!(found.symbol, dataset.symbol);
        assert_eq!(found.interval, dataset.interval);
        assert_eq!(found.start_time, dataset.start_time);
        assert_eq!(found.end_time, dataset.end_time);
        assert_eq!(found.candle_count, dataset.candle_count);
        assert_eq!(found.source, dataset.source);
        assert_eq!(found.dataset_hash, dataset.dataset_hash);

        let error = get_dataset_by_id(&conn, dataset_id + 1).unwrap_err();
        assert!(error.to_string().contains("dataset"));
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn atomic_dataset_import_rejects_forgery_and_conflicting_payload() {
        let mut conn = mem_db();
        let candles = identity_candles();
        let mut forged = identity_dataset(&candles);
        forged.dataset_hash = "dataset-content-v2:forged".into();
        assert!(import_dataset_with_candles(&mut conn, &forged, &candles).is_err());
        let mut wrong_bounds = identity_dataset(&candles);
        wrong_bounds.end_time += 1;
        assert!(import_dataset_with_candles(&mut conn, &wrong_bounds, &candles).is_err());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM datasets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "verification must happen before the first write");

        let dataset = identity_dataset(&candles);
        import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap();
        let mut conflicting = dataset;
        conflicting.source = "contradictory-source".into();
        assert!(import_dataset_with_candles(&mut conn, &conflicting, &candles).is_err());
    }

    #[test]
    fn atomic_dataset_import_rolls_back_dataset_when_a_candle_write_fails() {
        let mut conn = mem_db();
        conn.execute_batch(&format!(
            "CREATE TRIGGER reject_second_identity_candle
             BEFORE INSERT ON candles WHEN NEW.timestamp = {IDENTITY_CANDLE_T1}
             BEGIN SELECT RAISE(ABORT, 'injected candle failure'); END;",
        ))
        .unwrap();
        let candles = identity_candles();
        let dataset = identity_dataset(&candles);
        let error = import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap_err();
        // The rollback must be driven by the injected candle write, not by the
        // DATA-QUALITY-001 admission gate rejecting the payload first.
        assert!(
            error.to_string().contains("injected candle failure"),
            "rollback came from the wrong failure: {error}"
        );
        let dataset_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM datasets", [], |row| row.get(0))
            .unwrap();
        let candle_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM candles", [], |row| row.get(0))
            .unwrap();
        assert_eq!((dataset_count, candle_count), (0, 0));
    }

    /// DATA-QUALITY-001 step 6 — one case per REACHABLE rule id.
    ///
    /// Rule 3 (`timestamp_not_representable`) is unreachable by construction and
    /// is covered by the direct predicate test in `discovery_core::market_data`
    /// instead, so it has no row here.
    ///
    /// Each case starts from a database that already holds one valid imported
    /// dataset, then imports a payload mutated to violate exactly one rule, and
    /// proves all three of: the call fails with the expected rejection, no row
    /// was written, and the pre-existing dataset is still byte-identical.
    ///
    /// Every case but one expects the quality gate to name its rule id. Rule 1
    /// is the documented exception — see the FINDING note on the table below.
    #[test]
    fn market_data_mutations_are_rejected_atomically_without_disturbing_stored_data() {
        let base = || {
            vec![
                Candle {
                    timestamp: 1_735_689_600_000, // 2025-01-01T00:00:00Z
                    open: 100.0,
                    high: 103.0,
                    low: 99.0,
                    close: 102.0,
                    volume: 10.0,
                },
                Candle {
                    timestamp: 1_735_693_200_000,
                    open: 102.0,
                    high: 105.0,
                    low: 101.0,
                    close: 104.0,
                    volume: 12.0,
                },
            ]
        };

        // (rule id, mutation applied to the SECOND candle, which gate rejects it)
        //
        // FINDING (see docs/market-data-quality-contract.md): rule 1 is
        // structurally UNREACHABLE at this mount point. `db::Candle.timestamp`
        // is an `i64`, so a non-integral timestamp cannot exist here, and
        // `identity::normalize_dataset_candles` already rejects magnitudes above
        // `Number.MAX_SAFE_INTEGER` before the quality gate is reached. The case
        // is kept because the atomicity guarantee still has to hold, but it
        // asserts the rejection actually observed rather than one the code
        // cannot produce.
        let cases: Vec<MutationCase> = vec![
            (
                "timestamp_not_integer",
                Box::new(|candle: &mut Candle| candle.timestamp = 9_007_199_254_740_992),
                RejectedBy::IdentitySafeInteger,
            ),
            (
                "timestamp_out_of_range",
                // Epoch SECONDS for 2024-01-01, silently read as milliseconds.
                Box::new(|candle: &mut Candle| candle.timestamp = 1_704_067_200),
                RejectedBy::QualityGate,
            ),
            (
                "price_not_positive",
                Box::new(|candle: &mut Candle| candle.low = 0.0),
                RejectedBy::QualityGate,
            ),
            (
                "volume_negative",
                Box::new(|candle: &mut Candle| candle.volume = -1.0),
                RejectedBy::QualityGate,
            ),
            (
                "high_below_low",
                Box::new(|candle: &mut Candle| {
                    candle.open = 100.0;
                    candle.high = 99.0;
                    candle.low = 100.0;
                    candle.close = 100.0;
                }),
                RejectedBy::QualityGate,
            ),
            (
                "ohlc_out_of_range",
                Box::new(|candle: &mut Candle| candle.open = 106.0),
                RejectedBy::QualityGate,
            ),
        ];

        // Every reachable rule id must appear exactly once in the table.
        let mut covered: Vec<&str> = cases.iter().map(|(rule, _, _)| *rule).collect();
        covered.sort_unstable();
        covered.dedup();
        assert_eq!(covered.len(), cases.len(), "duplicate rule id in the table");
        for rule in market_data::MARKET_DATA_RULE_IDS {
            if rule == "timestamp_not_representable" {
                continue;
            }
            assert!(
                cases.iter().any(|(covered, _, _)| *covered == rule),
                "no atomic-import mutation case for {rule}"
            );
        }

        for (rule, mutate, rejected_by) in &cases {
            let mut conn = mem_db();
            let good = base();
            let good_dataset = identity_dataset(&good);
            let good_id = import_dataset_with_candles(&mut conn, &good_dataset, &good)
                .unwrap_or_else(|error| panic!("{rule}: valid baseline import failed: {error}"));
            let stored_before = get_candles(
                &conn,
                good_id,
                good_dataset.start_time,
                good_dataset.end_time,
            )
            .unwrap();
            let counts_before = row_counts(&conn);

            let mut bad = base();
            mutate(&mut bad[1]);
            let mut bad_dataset = Dataset {
                symbol: "ETHUSDT".into(),
                start_time: bad.iter().map(|candle| candle.timestamp).min().unwrap(),
                end_time: bad.iter().map(|candle| candle.timestamp).max().unwrap(),
                ..identity_dataset(&good)
            };
            // Hash the MUTATED payload wherever hashing accepts it, so identity
            // verification passes and the rejection can only come from the
            // quality gate. Hashing performs no semantic validation.
            bad_dataset.dataset_hash =
                match crate::identity::dataset_content_hash(&bad_dataset, &bad) {
                    Ok(hash) => hash,
                    Err(error) => {
                        // Only the unreachable rule-1 case lands here.
                        assert_eq!(*rejected_by, RejectedBy::IdentitySafeInteger);
                        assert!(
                            error.to_string().contains("JavaScript safe integer"),
                            "{rule}: unexpected hashing failure: {error}"
                        );
                        String::from("dataset-content-v2:unhashable")
                    }
                };

            let message = match import_dataset_with_candles(&mut conn, &bad_dataset, &bad) {
                Ok(_) => panic!("{rule}: mutated import was admitted"),
                Err(error) => error.to_string(),
            };
            match rejected_by {
                RejectedBy::QualityGate => {
                    assert!(
                        message.contains(rule),
                        "{rule}: error did not name the rule: {message}"
                    );
                    assert!(
                        !message.contains("identity") && !message.contains("hash"),
                        "{rule}: rejection came from identity, not the quality gate: {message}"
                    );
                }
                RejectedBy::IdentitySafeInteger => {
                    // Documented deviation: the pre-existing identity rule fires
                    // first, so the quality rule id can never appear here. The
                    // atomicity guarantee below is still asserted in full.
                    assert!(
                        message.contains("JavaScript safe integer"),
                        "{rule}: expected the identity safe-integer rejection: {message}"
                    );
                }
            }

            assert_eq!(
                row_counts(&conn),
                counts_before,
                "{rule}: a rejected import wrote rows"
            );
            let stored_after = get_candles(
                &conn,
                good_id,
                good_dataset.start_time,
                good_dataset.end_time,
            )
            .unwrap();
            assert_eq!(
                stored_after.len(),
                stored_before.len(),
                "{rule}: stored candle count changed"
            );
            for (before, after) in stored_before.iter().zip(&stored_after) {
                assert!(
                    candles_equal(before, after),
                    "{rule}: a previously imported candle is no longer byte-identical"
                );
            }
        }
    }

    /// One atomic-import mutation row: the rule it targets, the single-field
    /// mutation it applies, and which gate actually stops it.
    type MutationCase = (&'static str, Box<dyn Fn(&mut Candle)>, RejectedBy);

    /// Which gate a mutated import is actually stopped by.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RejectedBy {
        QualityGate,
        /// Rule 1 only: `identity::normalize_dataset_candles` rejects the
        /// magnitude before the quality gate can classify it.
        IdentitySafeInteger,
    }

    fn row_counts(conn: &Connection) -> (i64, i64) {
        (
            conn.query_row("SELECT COUNT(*) FROM datasets", [], |row| row.get(0))
                .unwrap(),
            conn.query_row("SELECT COUNT(*) FROM candles", [], |row| row.get(0))
                .unwrap(),
        )
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
    fn runner_strategy_conflict_preserves_user_fields_and_lifecycle() {
        let conn = mem_db();
        let mut user_strategy = verified_blocks_strategy();
        user_strategy.name = "user chosen name".into();
        user_strategy.source = "manual".into();
        user_strategy.lifecycle = "validated".into();
        let existing_id = insert_verified_strategy(&conn, &user_strategy).unwrap();

        let mut runner_candidate = verified_blocks_strategy();
        runner_candidate.name = "generated candidate name".into();
        runner_candidate.source = "traditional".into();
        runner_candidate.lifecycle = "candidate".into();
        let returned_id = get_or_insert_verified_runner_strategy(&conn, &runner_candidate).unwrap();

        assert_eq!(returned_id, existing_id);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM strategy_def WHERE strategy_hash = ?1",
                [&runner_candidate.strategy_hash],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let (name, source, lifecycle): (String, String, String) = conn
            .query_row(
                "SELECT name, source, lifecycle FROM strategy_def WHERE id = ?1",
                [existing_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "user chosen name");
        assert_eq!(source, "manual");
        assert_eq!(lifecycle, "validated");
    }

    #[test]
    fn runner_strategy_insert_still_verifies_the_durable_identity() {
        let conn = mem_db();
        let mut forged = verified_blocks_strategy();
        forged.strategy_hash = "strategy-v2:forged".into();

        assert!(get_or_insert_verified_runner_strategy(&conn, &forged).is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM strategy_def", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
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
            save_backtest_result(&mut conn, &replacement, &[trade(5, 6, Some("signal"))]).unwrap();

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
        let summary_id = save_backtest_result(&mut conn, &original, &[trade(1, 2, None)]).unwrap();

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

        conn.execute(
            "DELETE FROM strategy_def WHERE id = ?1",
            params![strategy_id],
        )
        .unwrap();

        let summaries: i64 = conn
            .query_row("SELECT COUNT(*) FROM backtest_summary", [], |row| {
                row.get(0)
            })
            .unwrap();
        let trades: i64 = conn
            .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))
            .unwrap();
        assert_eq!(summaries, 0);
        assert_eq!(trades, 0);
    }

    // ---------- PERSIST-001: validation records ----------

    fn passing_bundle(
        strategy_id: i64,
        dataset_id: i64,
    ) -> (BacktestSummary, BacktestSummary, ValidationRecordRow) {
        let output = crate::discovery_runner::execution::tests::representative_output();
        assert!(
            output.record.gate_passed,
            "representative fixture must pass Gate: {}",
            serde_json::from_str::<serde_json::Value>(&output.record.record_json).unwrap()["gate"]
        );
        let mut train = output.train_summary;
        let mut validation = output.validation_summary;
        let mut record = output.record;
        train.strategy_id = strategy_id;
        train.dataset_id = dataset_id;
        validation.strategy_id = strategy_id;
        validation.dataset_id = dataset_id;
        record.strategy_id = strategy_id;
        record.dataset_id = dataset_id;
        let mut envelope: serde_json::Value = serde_json::from_str(&record.record_json).unwrap();
        envelope["strategyId"] = serde_json::json!(strategy_id);
        envelope["datasetId"] = serde_json::json!(dataset_id);
        record.record_json = serde_json::to_string(&envelope).unwrap();
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
        let mut envelope: serde_json::Value = serde_json::from_str(&record.record_json).unwrap();
        envelope["gatePassed"] = serde_json::json!(false);
        envelope["gate"]["pass"] = serde_json::json!(false);
        envelope["gate"]["criteria"][0]["pass"] = serde_json::json!(false);
        envelope["gate"]["criteria"][0]["value"] = serde_json::json!(0.0);
        envelope["contracts"]["score"] = serde_json::Value::Null;
        envelope["score"] = serde_json::Value::Null;
        record.record_json = serde_json::to_string(&envelope).unwrap();
        (train, validation, record)
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn migrations_apply_through_0003_and_rerun_idempotently() {
        let conn = mem_db();
        assert_eq!(count(&conn, "validation_records"), 0, "table must exist");
        // 0003 columns must exist on a freshly migrated database.
        assert_eq!(count(&conn, "discovery_runs"), 0);
        conn.query_row(
            "SELECT candidate_index FROM discovery_jobs LIMIT 1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .ok();
        conn.prepare("SELECT discovery_run_id FROM validation_records")
            .expect("0003 adds the run linkage column");
        crate::db::apply_migrations(&conn).expect("re-run must be a no-op");
        assert_eq!(count(&conn, "schema_migrations"), 3);
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
        assert_eq!(count(&conn, "schema_migrations"), 3);
        // The 0001 -> 0003 path must also land 0003's structure, not just 0002.
        conn.prepare("SELECT discovery_run_id FROM validation_records")
            .expect("0003 run linkage column");
        conn.prepare("SELECT candidate_index FROM discovery_jobs")
            .expect("0003 candidate index column");
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
        assert!(
            insert(strategy_id, 2, None).is_err(),
            "gate_passed must be 0/1"
        );
        assert!(
            insert(strategy_id, 0, Some(1.0)).is_err(),
            "fail + score violates the D3 CHECK"
        );
        assert!(
            insert(strategy_id, 1, None).is_err(),
            "pass without score violates the D3 CHECK"
        );
        assert!(insert(strategy_id, 0, None).is_ok());
        assert!(insert(strategy_id, 1, Some(2.5)).is_ok());
        assert!(
            insert(9999, 0, None).is_err(),
            "unknown strategy_id must violate the FK"
        );
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
        let (t, mut v, mut r) = failing_bundle(1, 2);
        r.score = Some(1.0);
        assert!(
            validate_validation_bundle(&t, &v, &r).is_err(),
            "record.score still set"
        );
        r.score = None;
        v.score = Some(1.0);
        assert!(
            validate_validation_bundle(&t, &v, &r).is_err(),
            "summary score still set"
        );

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
        let mut envelope: serde_json::Value = serde_json::from_str(&r.record_json).unwrap();
        envelope["strategyId"] = serde_json::json!(9);
        r.record_json = serde_json::to_string(&envelope).unwrap();
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // envelope gatePassed contradicts the row
        let (t, v, mut r) = passing_bundle(1, 2);
        let mut envelope: serde_json::Value = serde_json::from_str(&r.record_json).unwrap();
        envelope["gatePassed"] = serde_json::json!(false);
        r.record_json = serde_json::to_string(&envelope).unwrap();
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // envelope score value contradicts the row score
        let (t, v, mut r) = passing_bundle(1, 2);
        let mut envelope: serde_json::Value = serde_json::from_str(&r.record_json).unwrap();
        envelope["score"]["score"] = serde_json::json!(999.0);
        r.record_json = serde_json::to_string(&envelope).unwrap();
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // summary breakdown snapshot differs from the envelope's
        let (t, mut v, r) = passing_bundle(1, 2);
        let mut breakdown: serde_json::Value =
            serde_json::from_str(v.score_breakdown_json.as_deref().unwrap()).unwrap();
        breakdown["x"] = serde_json::json!(1);
        v.score_breakdown_json = Some(serde_json::to_string(&breakdown).unwrap());
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // summary benchmark snapshot differs from the envelope's (valid shape)
        let (t, mut v, r) = passing_bundle(1, 2);
        v.benchmark_result_json = Some(
            r#"{"version":"bench-record-v1","benchmarks":[1],"randomEntry":{"runs":20}}"#.into(),
        );
        assert!(validate_validation_bundle(&t, &v, &r).is_err());

        // key-order differences alone are NOT a mismatch (structural compare)
        let (t, mut v, r) = passing_bundle(1, 2);
        let breakdown: serde_json::Value =
            serde_json::from_str(v.score_breakdown_json.as_deref().unwrap()).unwrap();
        v.score_breakdown_json = Some(serde_json::to_string_pretty(&breakdown).unwrap());
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
            let mut envelope: serde_json::Value = serde_json::from_str(&r.record_json).unwrap();
            envelope["benchmark"] = serde_json::from_str(bogus).unwrap();
            r.record_json = serde_json::to_string(&envelope).unwrap();
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
        let expected_score = record.score;
        // PERSIST-INVARIANT-001: a summary's `trade_count` must equal the rows
        // committed with it. The fixture summaries come from a real assessment,
        // so they are restated for the deliberately small slices this test uses
        // to prove that trades attach per summary (1 + 2 = 3).
        let train_1 = with_trade_count(&train, 1);
        let validation_2 = with_trade_count(&validation, 2);
        // Inside each summary's own range: PERSIST-INVARIANT-001 rejects a trade
        // outside it, and these fixtures carry real epoch-millisecond ranges.
        let train_trade = trade(train.start_time, train.start_time + 1, None);
        let validation_trades = [
            trade(validation.start_time, validation.start_time + 1, None),
            trade(validation.start_time + 2, validation.end_time, None),
        ];

        let record_id = save_validation_bundle(
            &mut conn,
            &train_1,
            &[train_trade],
            &validation_2,
            &validation_trades,
            &record,
        )
        .unwrap();

        assert_eq!(count(&conn, "backtest_summary"), 2);
        assert_eq!(count(&conn, "trades"), 3);
        let read = get_validation_record(&conn, record_id).unwrap();
        assert_eq!(
            read.record_json, record.record_json,
            "exact JSON reads back"
        );
        assert!(read.gate_passed);
        assert_eq!(read.score, expected_score);

        // Append-only: a re-run appends a SECOND record while the summaries
        // upsert (latest view) and the trades replace.
        let train_0 = with_trade_count(&train, 0);
        let validation_0 = with_trade_count(&validation, 0);
        let id2 =
            save_validation_bundle(&mut conn, &train_0, &[], &validation_0, &[], &record).unwrap();
        assert_ne!(record_id, id2);
        assert_eq!(
            list_validation_records(&conn, Some(strategy_id))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(list_validation_records(&conn, None).unwrap().len(), 2);
        assert_eq!(
            count(&conn, "backtest_summary"),
            2,
            "summaries stay the latest view"
        );
        assert_eq!(
            count(&conn, "trades"),
            0,
            "trade rows replaced by the re-run"
        );
    }

    #[test]
    fn save_validation_bundle_rolls_back_the_whole_bundle_on_failure() {
        let mut conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        let (train, validation, record) = passing_bundle(strategy_id, dataset_id);
        let train_1 = with_trade_count(&train, 1);
        // The injected failure must be one this crate CANNOT reject up front, or
        // this stops being a rollback proof. It used to be an illegal segment,
        // which PERSIST-INVARIANT-001 now catches at the write funnel before any
        // row is inserted — a rejection proof wearing a rollback proof's name.
        // A dangling dataset reference is invisible to a pure validator and only
        // fails as a foreign-key violation on the SECOND summary insert, after
        // the train summary and its trade row have already written inside the
        // transaction.
        let mut orphan_validation = with_trade_count(&validation, 0);
        orphan_validation.dataset_id = dataset_id + 9_999;

        let result = save_validation_bundle(
            &mut conn,
            &train_1,
            &[trade(train.start_time, train.start_time + 1, None)],
            &orphan_validation,
            &[],
            &record,
        );
        assert!(result.is_err());
        assert_eq!(
            count(&conn, "backtest_summary"),
            0,
            "train summary must roll back"
        );
        assert_eq!(count(&conn, "trades"), 0);
        assert_eq!(count(&conn, "validation_records"), 0);
    }

    // ---------- PERSIST-INVARIANT-001 ----------

    /// A bundle that describes a possible result: `summary()` spans [1, 10] and
    /// the trade sits inside it.
    fn valid_bundle() -> (BacktestSummary, Vec<TradeRow>) {
        let mut s = summary(1, 1, 0.25);
        s.trade_count = Some(1);
        s.win_rate = Some(1.0);
        s.exposure = Some(0.5);
        s.max_drawdown = Some(0.1);
        s.turnover = Some(2.0);
        s.consecutive_losses = Some(0);
        (s, vec![trade(2, 3, Some("signal"))])
    }

    #[test]
    fn validate_result_bundle_accepts_a_possible_result() {
        let (s, trades) = valid_bundle();
        assert!(validate_result_bundle(&s, &trades).is_ok());
        // Absent optional metrics are not the same as invalid ones.
        let mut sparse = summary(1, 1, 0.0);
        sparse.trade_count = None;
        assert!(validate_result_bundle(&sparse, &[]).is_ok());
    }

    /// One mutation per invariant. Each case starts from the SAME valid bundle,
    /// so a rejection can only be caused by the single field it changes.
    #[test]
    fn validate_result_bundle_rejects_every_impossible_bundle() {
        type Mutate = fn(&mut BacktestSummary, &mut Vec<TradeRow>);
        let cases: Vec<(&str, &str, Mutate)> = vec![
            ("unknown segment", "segment must be one of", |s, _| {
                s.segment = "bogus".into();
            }),
            ("inverted summary range", "summary range is inverted", |s, t| {
                s.start_time = 100;
                s.end_time = 1;
                t.clear();
                s.trade_count = Some(0);
            }),
            ("non-finite metric", "must be finite when present", |s, _| {
                s.sharpe = Some(f64::NAN);
            }),
            ("infinite profit factor", "must be finite when present", |s, _| {
                // The mapper narrows a legitimately infinite factor to NULL, so a
                // value here means that narrowing was bypassed.
                s.profit_factor = Some(f64::INFINITY);
            }),
            ("win rate above one", "must be within [0, 1]", |s, _| {
                s.win_rate = Some(1.5);
            }),
            ("negative exposure", "must be within [0, 1]", |s, _| {
                s.exposure = Some(-0.1);
            }),
            ("negative drawdown", "must be non-negative", |s, _| {
                s.max_drawdown = Some(-0.1);
            }),
            ("negative turnover", "must be non-negative", |s, _| {
                s.turnover = Some(-1.0);
            }),
            ("negative trade count", "must be non-negative", |s, t| {
                s.trade_count = Some(-1);
                t.clear();
            }),
            ("negative loss streak", "must be non-negative", |s, _| {
                s.consecutive_losses = Some(-1);
            }),
            ("count disagrees with rows", "does not match", |s, _| {
                s.trade_count = Some(2);
            }),
            ("unknown side", "side must be one of", |_, t| {
                t[0].side = "long".into();
            }),
            ("exit before entry", "precedes entry", |_, t| {
                t[0].entry_time = 5;
                t[0].exit_time = 4;
            }),
            ("trade before the summary", "falls outside the summary range", |_, t| {
                t[0].entry_time = 0;
            }),
            ("trade after the summary", "falls outside the summary range", |_, t| {
                t[0].exit_time = 11;
            }),
            ("zero entry price", "must be finite and > 0", |_, t| {
                t[0].entry_price = 0.0;
            }),
            ("negative exit price", "must be finite and > 0", |_, t| {
                t[0].exit_price = -1.0;
            }),
            ("non-finite exit price", "must be finite and > 0", |_, t| {
                t[0].exit_price = f64::NAN;
            }),
            ("non-finite pnl", "pnl must be finite", |_, t| {
                t[0].pnl = f64::NAN;
            }),
            ("non-finite pnl percent", "pnl_pct must be finite", |_, t| {
                t[0].pnl_pct = f64::INFINITY;
            }),
        ];

        for (label, fragment, mutate) in cases {
            let (mut s, mut trades) = valid_bundle();
            mutate(&mut s, &mut trades);
            let error = validate_result_bundle(&s, &trades)
                .expect_err(&format!("{label} must be rejected"));
            let message = error.to_string();
            assert!(
                message.contains(fragment),
                "{label}: expected a message containing {fragment:?}, got {message:?}"
            );
        }
    }

    /// The invariant is enforced at the write funnel, so an impossible
    /// replacement cannot land — and the previously stored result must survive it
    /// completely, not partially.
    #[test]
    fn an_impossible_replacement_leaves_the_stored_result_intact() {
        let mut conn = mem_db();
        let (strategy_id, dataset_id) = saved_parent_rows(&conn);
        let (mut good, trades) = valid_bundle();
        good.strategy_id = strategy_id;
        good.dataset_id = dataset_id;

        let summary_id = save_backtest_result(&mut conn, &good, &trades).unwrap();
        let before: (f64, i64, i64) = conn
            .query_row(
                "SELECT net_return, trade_count,
                        (SELECT COUNT(*) FROM trades WHERE backtest_summary_id = ?1)
                 FROM backtest_summary WHERE id = ?1",
                params![summary_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(before, (0.25, 1, 1));

        // Same key, so this WOULD upsert the summary and replace its trades —
        // except its count contradicts the single row it carries.
        let mut broken = good.clone();
        broken.net_return = Some(-0.99);
        broken.trade_count = Some(7);
        let error = save_backtest_result(&mut conn, &broken, &trades).expect_err("must reject");
        assert!(error.to_string().contains("does not match"));

        let after: (f64, i64, i64) = conn
            .query_row(
                "SELECT net_return, trade_count,
                        (SELECT COUNT(*) FROM trades WHERE backtest_summary_id = ?1)
                 FROM backtest_summary WHERE id = ?1",
                params![summary_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(after, before, "the stored result is byte-for-byte unchanged");
        assert_eq!(count(&conn, "backtest_summary"), 1, "no extra summary row");
        assert_eq!(count(&conn, "trades"), 1, "no orphaned trade rows");
    }
}
