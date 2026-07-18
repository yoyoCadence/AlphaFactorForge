import { describe, it, expect } from 'vitest';
import { buildReport, reportToJson, tradesToCsv, suggestedFilename, type ReportInput } from './reportExport';
import { defaultStrategy } from './strategy';
import type { ClosedTrade, Metrics } from '../core/metrics';
import type { BacktestResult } from '../core/backtest';

const trades: ClosedTrade[] = [
  { entryTime: 1_000_000, exitTime: 2_000_000, side: 'LONG', entryPrice: 100, exitPrice: 110, pnl: 10, pnlPct: 0.1, bars: 5 },
  { entryTime: 3_000_000, exitTime: 4_000_000, side: 'SHORT', entryPrice: 120, exitPrice: 118, pnl: 2, pnlPct: 0.016, bars: 3 },
];
// export just passes metrics through, so a partial cast is enough for these tests
const metrics = { netReturn: 0.12, tradeCount: 2 } as unknown as Metrics;
const result: BacktestResult = { trades, equity: [], metrics };

const strat = defaultStrategy(); // defaultStrategy is a factory
const AT = Date.UTC(2026, 6, 1, 8, 30); // 2026-07-01T08:30:00Z (month is 0-based)
const input: ReportInput = {
  strategyName: '  demo  ',
  strategy: strat,
  dataset: { symbol: 'SAMPLE', interval: '1h', startTime: 1_000_000, endTime: 4_000_000 },
  result,
  exportedAt: AT,
};

describe('buildReport / reportToJson', () => {
  it('produces a parseable report with the expected shape', () => {
    const r = JSON.parse(reportToJson(input));
    expect(r.app).toBe('AlphaFactorForge');
    expect(r.schema).toBe(2);
    expect(r.exportedAt).toBe('2026-07-01T08:30:00.000Z');
    expect(r.strategyName).toBe('demo'); // trimmed
    expect(r.dataset).toEqual({ symbol: 'SAMPLE', interval: '1h', startTime: 1_000_000, endTime: 4_000_000 });
    expect(r.metrics.tradeCount).toBe(2); // passed through
    expect(r.tradeCount).toBe(2);
    expect(r.trades).toHaveLength(2);
    expect(r.strategy.mode).toBe(strat.mode);
  });

  it('falls back to a placeholder name when blank', () => {
    expect(buildReport({ ...input, strategyName: '   ' }).strategyName).toBe('(未命名)');
    expect(buildReport({ ...input, strategyName: undefined }).strategyName).toBe('(未命名)');
  });

  it('encodes non-finite metrics as null + an explicit status (METRIC-001)', () => {
    const withInf = {
      ...input,
      result: {
        ...result,
        metrics: { ...metrics, sortino: Infinity, calmar: -Infinity, sharpe: NaN } as Metrics,
      },
    };
    // JSON round-trip must preserve the statuses, not silently null them.
    const r = JSON.parse(reportToJson(withInf));
    expect(r.metrics.sortino).toBeNull();
    expect(r.metrics.calmar).toBeNull();
    expect(r.metrics.sharpe).toBeNull();
    expect(r.metricsNonFinite).toEqual({
      sortino: 'positive_infinity',
      calmar: 'negative_infinity',
      sharpe: 'nan',
    });
    // finite fields stay numeric and produce no status entry
    expect(r.metrics.netReturn).toBe(0.12);
    const clean = buildReport(input);
    expect(clean.metricsNonFinite).toEqual({});
  });
});

describe('tradesToCsv', () => {
  it('emits a header + one row per trade', () => {
    const lines = tradesToCsv(trades).split('\n');
    expect(lines).toHaveLength(3); // header + 2 trades
    expect(lines[0]).toBe('entry_time,entry_time_iso,exit_time,exit_time_iso,side,entry_price,exit_price,pnl,pnl_pct,bars');
    expect(lines[1]).toContain('LONG');
    expect(lines[1].split(',')[1]).toBe(new Date(1_000_000).toISOString()); // entry_time_iso
    expect(lines[2]).toContain('SHORT');
  });

  it('emits just the header for no trades', () => {
    expect(tradesToCsv([]).split('\n')).toHaveLength(1);
  });
});

describe('suggestedFilename', () => {
  it('sanitizes symbol/interval and stamps the date + extension', () => {
    expect(suggestedFilename({ symbol: 'BTC/USDT', interval: '1h' }, 'json', AT)).toBe('AlphaFactorForge_BTC-USDT_1h_2026-07-01.json');
    expect(suggestedFilename({ symbol: 'SAMPLE', interval: '1d' }, 'csv', AT)).toBe('AlphaFactorForge_SAMPLE_1d_2026-07-01.csv');
  });
});
