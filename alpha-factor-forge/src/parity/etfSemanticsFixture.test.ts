import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/etf-semantics-v1.json';
import { accrueDividend, applySplit, assessEtfSemantics, estimateEtfCosts, etfSessionDates,
  payDividend, splitAdjustedSignals, valueEtfPosition, type DividendAction, type EtfCostProfile,
  type EtfMarket, type EtfPosition, type NativeCurrency, type SplitAction } from '../core/market-data/etf';
import { computeEtfMetrics } from '../core/metrics/etf';
import { computeMetrics } from '../core/metrics';
import { barsPerYear } from '../services/backtestRunner';
import type { MarketDataCandle } from '../core/market-data/quality';
import type { Suspension } from '../core/market-data/foundation';

type Input = Parameters<typeof computeEtfMetrics>[0] & {
  market: EtfMarket; from: string; toExclusive: string; suspensions: Suspension[];
  costs: EtfCostProfile; currency: NativeCurrency; side: 'buy' | 'sell'; quantity: number; rawPrice: number;
  state: EtfPosition; action: DividendAction & SplitAction; actionId: string; paymentDate: string;
  raw: MarketDataCandle[]; instrumentId: string; splits: SplitAction[]; asOfMs: number;
  evidence: Parameters<typeof assessEtfSemantics>[2];
};
function run(op: string, i: Input): unknown {
  switch (op) {
    case 'sessions': return etfSessionDates(i.market, i.from, i.toExclusive, i.suspensions);
    case 'costs': return estimateEtfCosts(i.costs, i.currency, i.side, i.quantity, i.rawPrice);
    case 'accrue': return accrueDividend(i.state, i.action);
    case 'pay': return payDividend(i.state, i.actionId, i.paymentDate);
    case 'value': return valueEtfPosition(i.state, i.rawPrice);
    case 'split': return applySplit(i.state, i.action);
    case 'signals': return splitAdjustedSignals(i.raw, i.instrumentId, i.splits, i.asOfMs);
    case 'assess': return assessEtfSemantics(i.market, i.costs, i.evidence, i.from, i.toExclusive);
    case 'metrics': {
      const { metrics, ...context } = computeEtfMetrics(i);
      return { ...context, netReturn: metrics.netReturn, cagr: metrics.cagr,
        maxDrawdown: metrics.maxDrawdown, calmar: metrics.calmar, sharpe: metrics.sharpe };
    }
    default: throw new Error(`Unknown fixture operation: ${op}`);
  }
}
function compare(actual: unknown, expected: unknown, path: string): void {
  if (typeof expected === 'number') {
    if (path.endsWith('.timestamp')) { expect(actual, path).toBe(expected); return; }
    expect(typeof actual, path).toBe('number');
    const value = actual as number;
    expect(Number.isFinite(value), path).toBe(true);
    expect(Math.abs(value - expected) <= fixture.numericPolicy.absolute
      || Math.abs(value - expected) <= fixture.numericPolicy.relative * Math.max(Math.abs(value), Math.abs(expected)), path).toBe(true);
  } else if (Array.isArray(expected)) {
    expect(Array.isArray(actual), path).toBe(true);
    expect((actual as unknown[]).length, path).toBe(expected.length);
    expected.forEach((value, index) => compare((actual as unknown[])[index], value, `${path}[${index}]`));
  } else if (expected !== null && typeof expected === 'object') {
    expect(Object.keys(actual as object).sort(), path).toEqual(Object.keys(expected).sort());
    for (const [key, value] of Object.entries(expected)) compare((actual as Record<string, unknown>)[key], value, `${path}.${key}`);
  } else expect(actual, path).toEqual(expected);
}

describe('P08 shared, independently authored ETF specification', () => {
  it('pins the case inventory and version', () => {
    expect(fixture.fixtureVersion).toBe('etf-semantics-parity-v1');
    expect(fixture.cases).toHaveLength(55);
    expect(new Set(fixture.cases.map(c => c.id)).size).toBe(55);
  });
  for (const entry of fixture.cases) it(entry.id, () => {
    const input = structuredClone(entry.input) as unknown as Input;
    const before = structuredClone(input);
    if ('error' in entry) expect(() => run(entry.op, input)).toThrow(new RangeError(entry.error));
    else if (entry.op === 'sessions') expect(run(entry.op, input)).toEqual(entry.expected);
    else compare(run(entry.op, input), entry.expected, entry.id);
    expect(input).toEqual(before); // Pure operations never mutate original prices, actions or account state.
  });
  it('retains metrics-v2 and the legacy daily factor', () => {
    expect(barsPerYear('1d')).toBe(365);
    const input = fixture.cases.find(c => c.id === 'elapsed_year_cagr_daily_volatility')!.input as unknown as Input;
    const legacy = computeMetrics({ ...input, barsPerYear: 365 });
    expect(legacy.cagr).toBeCloseTo(Math.pow(.99, 365 / 2) - 1, 12);
    expect(computeEtfMetrics(input).metrics.cagr).toBeCloseTo(-.01, 12);
  });
  it('rejects non-finite values instead of minting plausible results', () => {
    const i = fixture.cases.find(c => c.op === 'split')!.input as unknown as Input;
    for (const ratio of [NaN, Infinity, -Infinity]) expect(() => applySplit(i.state, { ...i.action, ratio })).toThrow('invalid_split');
    const m = fixture.cases.find(c => c.op === 'metrics')!.input as unknown as Input;
    expect(() => computeEtfMetrics({ ...m, riskFreePerBar: NaN })).toThrow('invalid_metrics_input');
    expect(() => computeEtfMetrics({ ...m, equity: [{ time: m.periodEndMs, equity: Infinity }], totalBars: 1 })).toThrow('invalid_equity');
  });
});
