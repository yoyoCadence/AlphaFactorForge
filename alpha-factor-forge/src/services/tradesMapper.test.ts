import { describe, expect, it } from 'vitest';
import type { ClosedTrade } from '../core/metrics';
import { tradeToRow, tradesToRows } from './tradesMapper';

const longTrade: ClosedTrade = {
  entryTime: 100,
  exitTime: 200,
  side: 'LONG',
  entryPrice: 10,
  exitPrice: 12,
  pnl: 20,
  pnlPct: 0.2,
  bars: 4,
};

describe('tradesMapper', () => {
  it('maps every persisted field to the snake_case TradeRow shape', () => {
    expect(tradeToRow(longTrade)).toEqual({
      entry_time: 100,
      exit_time: 200,
      side: 'LONG',
      entry_price: 10,
      exit_price: 12,
      pnl: 20,
      pnl_pct: 0.2,
      reason: null,
    });
  });

  it('preserves trade order and side while omitting unsupported holding bars', () => {
    const shortTrade: ClosedTrade = {
      ...longTrade,
      entryTime: 300,
      exitTime: 400,
      side: 'SHORT',
      pnl: -5,
      pnlPct: -0.05,
      bars: 9,
    };
    const rows = tradesToRows([longTrade, shortTrade]);

    expect(rows.map((row) => [row.entry_time, row.side])).toEqual([
      [100, 'LONG'],
      [300, 'SHORT'],
    ]);
    expect(rows[1]).toMatchObject({ pnl: -5, pnl_pct: -0.05 });
    expect(rows[1]).not.toHaveProperty('bars');
    expect(rows[1]).not.toHaveProperty('fee');
    expect(rows[1]).not.toHaveProperty('slippage');
  });

  it('maps an empty result to an empty persistence batch', () => {
    expect(tradesToRows([])).toEqual([]);
  });
});
