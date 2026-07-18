// METRIC-001 locks (SCORE-001 handoff Resolution): downside deviation over ALL
// bar returns, Sortino/Calmar infinity semantics, and zero-denominator cases.

import { describe, it, expect } from 'vitest';
import { computeMetrics, type EquityPoint } from './index';

const eq = (values: number[]): EquityPoint[] =>
  values.map((equity, time) => ({ time, equity }));

/** One bar per equity point; barsPerYear = totalBars so years = 1 and the
 *  annualization factor for Sharpe/Sortino is sqrt(totalBars). */
const compute = (equityValues: number[], startEquity = 100) =>
  computeMetrics({
    trades: [],
    equity: eq(equityValues),
    startEquity,
    totalBars: equityValues.length,
    barsPerYear: equityValues.length,
  });

describe('METRIC-001 — Sortino downside deviation', () => {
  it('a single downside observation still yields a finite Sortino', () => {
    // returns from 100: +10%, -10%, +10% -> mean 1/30,
    // downside = sqrt(mean([0, 0.01, 0])) = 0.1/sqrt(3),
    // sortino = (1/30)/(0.1/sqrt(3)) * sqrt(3) = 1/sqrt(3) * sqrt(3)... see below
    const m = compute([110, 99, 108.9]);
    const ratio = 1 / 30 / (0.1 / Math.sqrt(3)); // = sqrt(3)/3
    expect(m.sortino).toBeCloseTo(ratio * Math.sqrt(3), 10);
    expect(Number.isFinite(m.sortino)).toBe(true);
    expect(m.sortino).toBeGreaterThan(0);
  });

  it('no downside at all with positive mean excess -> +Infinity', () => {
    const m = compute([110, 121]);
    expect(m.sortino).toBe(Infinity);
  });

  it('flat returns (zero denominator, zero mean) -> 0', () => {
    const m = compute([100, 100, 100]);
    expect(m.sortino).toBe(0);
    expect(m.sharpe).toBe(0);
  });
});

describe('METRIC-001 — Calmar zero-drawdown semantics', () => {
  it('positive CAGR with zero drawdown -> +Infinity', () => {
    const m = compute([110, 121]);
    expect(m.maxDrawdown).toBe(0);
    expect(m.cagr).toBeGreaterThan(0);
    expect(m.calmar).toBe(Infinity);
  });

  it('flat equity (zero drawdown, zero CAGR) -> 0', () => {
    const m = compute([100, 100]);
    expect(m.calmar).toBe(0);
  });

  it('a real drawdown keeps the normal finite ratio', () => {
    const m = compute([120, 90, 130]);
    expect(m.maxDrawdown).toBeCloseTo((120 - 90) / 120, 10);
    expect(Number.isFinite(m.calmar)).toBe(true);
    expect(m.calmar).toBeCloseTo(m.cagr / m.maxDrawdown, 10);
  });
});
