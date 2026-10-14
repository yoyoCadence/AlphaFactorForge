import { describe, expect, it } from 'vitest';
import type { Metrics } from '../core/metrics';
import { METRIC_ROWS, metricColumns } from './MetricsTable';

function metrics(tradeCount: number): Metrics {
  return {
    netReturn: 0,
    cagr: 0,
    maxDrawdown: 0,
    sharpe: 0,
    sortino: 0,
    calmar: 0,
    winRate: 0,
    tradeCount,
    profitFactor: 0,
    avgTradeReturn: 0,
    medianTradeReturn: 0,
    avgHoldingBars: 0,
    exposure: 0,
    turnover: 0,
    largestWin: 0,
    largestLoss: 0,
    consecutiveLosses: 0,
    monthlyReturns: {},
  };
}

describe('metricColumns', () => {
  it('uses a single unlabeled full-period column without a complete holdout split', () => {
    const full = metrics(10);
    expect(metricColumns({ full })).toEqual([{ label: '', metrics: full }]);
    expect(metricColumns({ full, inSample: metrics(6) })).toEqual([{ label: '', metrics: full }]);
  });

  it('orders full, in-sample, and out-of-sample columns for a complete split', () => {
    const full = metrics(10);
    const inSample = metrics(6);
    const outSample = metrics(4);
    expect(metricColumns({ full, inSample, outSample })).toEqual([
      { label: '全期', metrics: full },
      { label: '樣本內', metrics: inSample },
      { label: '樣本外', metrics: outSample },
    ]);
  });
});

// METRIC-DISPLAY-001 — a non-finite metric reads as what it is, in every row.
describe('metric row formatting', () => {
  const cell = (label: string, value: Partial<Metrics>): string => {
    const row = METRIC_ROWS.find((entry) => entry.label === label);
    if (!row) throw new Error(`no metric row labelled ${label}`);
    return row.fmt({ ...metrics(3), ...value });
  };

  it('formats finite values as before', () => {
    expect(cell('淨報酬', { netReturn: 0.1448 })).toBe('14.48%');
    expect(cell('Sharpe', { sharpe: 5.554 })).toBe('5.55');
    expect(cell('Profit Factor', { profitFactor: 2.37 })).toBe('2.37');
    expect(cell('交易數', { tradeCount: 11 })).toBe('11');
  });

  it('shows an infinite ratio as the same symbol in every row', () => {
    expect(cell('Sortino', { sortino: Number.POSITIVE_INFINITY })).toBe('∞');
    expect(cell('Calmar', { calmar: Number.POSITIVE_INFINITY })).toBe('∞');
    expect(cell('Profit Factor', { profitFactor: Number.POSITIVE_INFINITY })).toBe('∞');
    expect(cell('Sharpe', { sharpe: Number.NEGATIVE_INFINITY })).toBe('−∞');
  });

  it('never prints NaN', () => {
    // CAGR is NaN when an unleveraged short loses more than its collateral.
    expect(cell('CAGR', { cagr: Number.NaN })).toBe('—');
    expect(cell('Sharpe', { sharpe: Number.NaN })).toBe('—');
    for (const row of METRIC_ROWS) {
      const allNaN = Object.fromEntries(Object.keys(metrics(1)).map((key) => [key, Number.NaN])) as unknown as Metrics;
      expect(row.fmt({ ...allNaN, tradeCount: 0, monthlyReturns: {} })).not.toMatch(/NaN/);
    }
  });

  it('shows a dash for a value that arrived as null', () => {
    expect(cell('Sortino', { sortino: null as unknown as number })).toBe('—');
    expect(cell('淨報酬', { netReturn: null as unknown as number })).toBe('—');
  });
});
