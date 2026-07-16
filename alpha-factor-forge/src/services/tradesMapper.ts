// THE single camelCase ClosedTrade -> snake_case TradeRow mapping.
//
// Persistence callers use this module instead of duplicating field mappings.
// `bars` has no Phase A SQLite column, and the core currently exposes no
// per-trade exit reason, so the persisted reason is explicitly NULL.

import type { ClosedTrade } from '../core/metrics';
import type { TradeRow } from '../tauri-client/commands';

export function tradeToRow(trade: ClosedTrade): TradeRow {
  return {
    entry_time: trade.entryTime,
    exit_time: trade.exitTime,
    side: trade.side,
    entry_price: trade.entryPrice,
    exit_price: trade.exitPrice,
    pnl: trade.pnl,
    pnl_pct: trade.pnlPct,
    reason: null,
  };
}

export function tradesToRows(trades: ClosedTrade[]): TradeRow[] {
  return trades.map(tradeToRow);
}
