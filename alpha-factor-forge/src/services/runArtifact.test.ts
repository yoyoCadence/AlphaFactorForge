import { describe, it, expect } from 'vitest';
import {
  RUN_CONTEXT_VERSION,
  createRunArtifact,
  datasetCandleKey,
  describeRunContext,
  describeRunRange,
  runContextKey,
  sameRunContext,
  type RunDatasetSnapshot,
} from './runArtifact';
import { holdoutSplitIndex } from './holdout';
import { defaultStrategy, type ParamsStrategy } from './strategy';
import type { BacktestResult } from '../core/backtest';

const DATASET: RunDatasetSnapshot = {
  id: 7,
  hash: 'dataset-content-v2:aa',
  symbol: 'SAMPLE',
  interval: '1h',
  startTime: 1_700_000_000_000,
  endTime: 1_700_003_600_000,
  barCount: 600,
};

function context(overrides: {
  dataset?: Partial<RunDatasetSnapshot>;
  strategy?: Partial<ParamsStrategy>;
  holdout?: boolean;
  holdoutPct?: number;
} = {}) {
  return describeRunContext({
    dataset: { ...DATASET, ...overrides.dataset },
    strategy: { ...defaultStrategy(), ...overrides.strategy },
    holdout: overrides.holdout ?? false,
    holdoutPct: overrides.holdoutPct ?? 30,
  });
}

function result(netReturn = 0.25): BacktestResult {
  return {
    trades: [
      { entryTime: 1, exitTime: 2, side: 'LONG', entryPrice: 100, exitPrice: 110, pnl: 10, pnlPct: 0.1, bars: 3 },
    ],
    equity: [{ time: 1, equity: 10_000 }],
    metrics: {
      netReturn,
      cagr: 0.1,
      maxDrawdown: 0.05,
      sharpe: 1.2,
      sortino: 1.5,
      calmar: 2,
      winRate: 1,
      tradeCount: 1,
      // Legitimately non-finite: no losing trade. structuredClone must keep it.
      profitFactor: Infinity,
      avgTradeReturn: 0.1,
      medianTradeReturn: 0.1,
      avgHoldingBars: 3,
      exposure: 0.5,
      turnover: 1,
      largestWin: 0.1,
      largestLoss: 0,
      consecutiveLosses: 0,
      monthlyReturns: { '2023-11': 0.1 },
    },
  };
}

describe('describeRunRange', () => {
  it('covers every bar and records no split when holdout is off', () => {
    expect(describeRunRange(600, false, 30)).toEqual({ from: 0, to: 599, holdout: null });
  });

  it('derives the split from the shared holdoutSplitIndex', () => {
    const range = describeRunRange(600, true, 30);
    expect(range).toEqual({ from: 0, to: 599, holdout: { pct: 30, splitIndex: holdoutSplitIndex(600, 30) } });
    expect(range.holdout?.splitIndex).toBe(420);
  });

  it('refuses a range with no bars, so an empty load can never become a context', () => {
    expect(() => describeRunRange(0, false, 30)).toThrow(/at least one bar/);
    expect(() => describeRunRange(1.5, false, 30)).toThrow(/at least one bar/);
  });
});

describe('runContextKey / sameRunContext', () => {
  it('treats identical inputs as the same run', () => {
    expect(sameRunContext(context(), context())).toBe(true);
    expect(runContextKey(context())).toContain(RUN_CONTEXT_VERSION);
  });

  it('is independent of object key order', () => {
    const ordered = describeRunContext({
      dataset: DATASET,
      strategy: defaultStrategy(),
      holdout: false,
      holdoutPct: 30,
    });
    // Same fields, reversed insertion order.
    const base = defaultStrategy();
    const shuffled = Object.fromEntries(
      Object.entries(base).reverse(),
    ) as unknown as ParamsStrategy;
    const reordered = describeRunContext({
      dataset: Object.fromEntries(Object.entries(DATASET).reverse()) as unknown as RunDatasetSnapshot,
      strategy: shuffled,
      holdout: false,
      holdoutPct: 30,
    });
    expect(sameRunContext(ordered, reordered)).toBe(true);
  });

  it('never matches a missing context, so "not loaded" and "load failed" fail closed', () => {
    expect(sameRunContext(context(), null)).toBe(false);
    expect(sameRunContext(null, context())).toBe(false);
    expect(sameRunContext(null, null)).toBe(false);
  });

  it.each([
    ['an indicator period', { strategy: { fastMA: 10 } }],
    ['an execution cost', { strategy: { feePct: 0.2 } }],
    ['a risk field', { strategy: { slPct: 2 } }],
    ['the direction', { strategy: { direction: 'short' as const } }],
    ['the fill mode', { strategy: { fillMode: 'nextOpen' as const } }],
    ['the strategy mode', { strategy: { mode: 'blocks' as const } }],
    ['a params signal', { strategy: { entrySig: 'rsiOversold' as const } }],
    ['a nested blocks rule', { strategy: { entryRules: [{ l: 'rsi' as const, op: '>' as const, r: '70' }] } }],
    ['a code expression', { strategy: { entryCode: 'price > maSlow' } }],
    ['the dataset id', { dataset: { id: 8 } }],
    ['the dataset content hash', { dataset: { hash: 'dataset-content-v2:bb' } }],
    ['the interval', { dataset: { interval: '1d' } }],
    ['the symbol', { dataset: { symbol: 'BTCUSDT' } }],
    ['the loaded bar count', { dataset: { barCount: 599 } }],
    ['the dataset time range', { dataset: { startTime: 1 } }],
  ])('invalidates when %s changes', (_label, overrides) => {
    expect(sameRunContext(context(), context(overrides))).toBe(false);
  });

  it('invalidates when holdout is toggled or its percentage changes', () => {
    expect(sameRunContext(context({ holdout: false }), context({ holdout: true }))).toBe(false);
    expect(sameRunContext(context({ holdout: true, holdoutPct: 30 }), context({ holdout: true, holdoutPct: 40 }))).toBe(false);
    expect(sameRunContext(context({ holdout: true, holdoutPct: 30 }), context({ holdout: true, holdoutPct: 30 }))).toBe(true);
  });
});

describe('createRunArtifact', () => {
  it('keeps the snapshot immutable when the live strategy keeps changing', () => {
    const live = defaultStrategy();
    const artifact = createRunArtifact({
      context: describeRunContext({ dataset: DATASET, strategy: live, holdout: false, holdoutPct: 30 }),
      strategyHash: 'strategy-v2:ff',
      result: result(),
      holdoutResult: null,
    });

    // The editor keeps mutating its own object (including the nested rule list).
    live.fastMA = 99;
    live.entryRules[0].r = 'ema';

    expect(artifact.context.strategy.fastMA).toBe(9);
    expect(artifact.context.strategy.entryRules[0].r).toBe('maSlow');
    expect(() => {
      (artifact.context.strategy as ParamsStrategy).fastMA = 42;
    }).toThrow(TypeError);
    expect(() => {
      artifact.result.trades.push(artifact.result.trades[0]);
    }).toThrow(TypeError);
  });

  it('detaches the result so a later run cannot rewrite an earlier artifact', () => {
    const source = result(0.25);
    const artifact = createRunArtifact({
      context: context(),
      strategyHash: 'strategy-v2:ff',
      result: source,
      holdoutResult: { inSample: result(0.1), outSample: result(0.4) },
    });

    source.metrics.netReturn = -1;
    source.trades.length = 0;

    expect(artifact.result.metrics.netReturn).toBe(0.25);
    expect(artifact.result.trades).toHaveLength(1);
    expect(artifact.holdoutResult?.outSample.metrics.netReturn).toBe(0.4);
    // structuredClone (not JSON) — a legitimately infinite profit factor survives.
    expect(artifact.result.metrics.profitFactor).toBe(Infinity);
  });

  it('still describes the context it was built from', () => {
    const built = context({ holdout: true, holdoutPct: 30 });
    const artifact = createRunArtifact({
      context: built,
      strategyHash: 'strategy-v2:ff',
      result: result(),
      holdoutResult: { inSample: result(), outSample: result() },
    });
    expect(sameRunContext(artifact.context, built)).toBe(true);
    expect(artifact.context.range.holdout).toEqual({ pct: 30, splitIndex: holdoutSplitIndex(600, 30) });
  });

  it('refuses an artifact with no durable strategy identity', () => {
    expect(() => createRunArtifact({
      context: context(),
      strategyHash: '',
      result: result(),
      holdoutResult: null,
    })).toThrow(/durable strategy identity/);
  });
});

describe('datasetCandleKey', () => {
  it('separates candle readiness by dataset id AND content hash', () => {
    expect(datasetCandleKey(1, 'dataset-content-v2:aa')).toBe(datasetCandleKey(1, 'dataset-content-v2:aa'));
    expect(datasetCandleKey(1, 'dataset-content-v2:aa')).not.toBe(datasetCandleKey(2, 'dataset-content-v2:aa'));
    expect(datasetCandleKey(1, 'dataset-content-v2:aa')).not.toBe(datasetCandleKey(1, 'dataset-content-v2:bb'));
  });
});
