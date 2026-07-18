import { describe, it, expect } from 'vitest';
import type { Candle } from '../core/backtest';
import { planValidationSplit } from '../core/validation/split';
import { runParamsBacktest } from './backtestRunner';
import { defaultStrategy } from './strategy';
import { toCoreCandles } from './candleAdapter';
import { makeSampleCandles } from './sampleData';
import { runValidationBacktests } from './validationRun';

/** Flat candles from a close series; time = index so ranges are assertable. */
const mk = (closes: number[]): Candle[] =>
  closes.map((c, i) => ({ t: i, o: c, h: c, l: c, c, v: 1 }));

const strat = defaultStrategy();

describe('runValidationBacktests', () => {
  it('runs Train and Validation over the exact planned inclusive ranges', () => {
    // 20 bars, embargo 2 -> usable 16 -> train 10 (0..9), gaps 2+2, val 3, test 3.
    const candles = mk(Array.from({ length: 20 }, (_, i) => 100 + i));
    const res = runValidationBacktests({ candles, strat, interval: '1h', embargoBars: 2 });

    expect(res.plan).toEqual(planValidationSplit(20, 2));
    // One equity point per tested bar, timestamped with that bar's time.
    expect(res.train.equity.length).toBe(res.plan.train.count);
    expect(res.train.equity[0].time).toBe(candles[res.plan.train.from].t);
    expect(res.train.equity[res.train.equity.length - 1].time).toBe(candles[res.plan.train.to].t);
    expect(res.validation.equity.length).toBe(res.plan.validation.count);
    expect(res.validation.equity[0].time).toBe(candles[res.plan.validation.from].t);
    expect(res.validation.equity[res.validation.equity.length - 1].time).toBe(
      candles[res.plan.validation.to].t,
    );
  });

  it('never evaluates embargo bars in either segment', () => {
    const candles = mk(Array.from({ length: 25 }, (_, i) => 100 + Math.sin(i) * 5));
    const res = runValidationBacktests({ candles, strat, interval: '1h', embargoBars: 3 });
    const evaluated = new Set(
      [...res.train.equity, ...res.validation.equity].map((p) => p.time),
    );
    for (const gap of [res.plan.trainValidationEmbargo!, res.plan.validationTestEmbargo!]) {
      for (let i = gap.from; i <= gap.to; i++) {
        expect(evaluated.has(candles[i].t)).toBe(false);
      }
    }
  });

  it('matches a direct runParamsBacktest over the same ranges (parity)', () => {
    const candles = toCoreCandles(makeSampleCandles({ seed: 42, count: 120 }));
    const res = runValidationBacktests({ candles, strat, interval: '1h', embargoBars: 5 });
    const direct = (from: number, to: number) =>
      runParamsBacktest({ candles, strat, interval: '1h', from, to });
    expect(res.train).toEqual(direct(res.plan.train.from, res.plan.train.to));
    expect(res.validation).toEqual(direct(res.plan.validation.from, res.plan.validation.to));
  });

  it('is deterministic for identical input', () => {
    const candles = toCoreCandles(makeSampleCandles({ seed: 7, count: 100 }));
    const a = runValidationBacktests({ candles, strat, interval: '4h', embargoBars: 4 });
    const b = runValidationBacktests({ candles, strat, interval: '4h', embargoBars: 4 });
    expect(a).toEqual(b);
  });

  it('exposes no Test result field (hidden-Test discipline)', () => {
    const candles = mk(Array.from({ length: 30 }, (_, i) => 100 + i));
    const res = runValidationBacktests({ candles, strat, interval: '1d', embargoBars: 1 });
    expect('test' in res).toBe(false);
    expect(Object.keys(res).sort()).toEqual(['plan', 'train', 'validation']);
  });

  it('fails closed before any backtest when the split is invalid', () => {
    expect(() =>
      runValidationBacktests({ candles: mk([1, 2, 3]), strat, interval: '1h', embargoBars: 0 }),
    ).toThrow(RangeError);
    expect(() =>
      runValidationBacktests({
        candles: mk(Array.from({ length: 10 }, (_, i) => i + 1)),
        strat,
        interval: '1h',
        embargoBars: -1,
      }),
    ).toThrow(RangeError);
  });
});
