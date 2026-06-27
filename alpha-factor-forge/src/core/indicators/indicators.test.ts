// FULL — unit tests for indicators. Run: npm test
import { describe, it, expect } from 'vitest';
import { sma, ema, rsi, macd, atr, bbands, highest, lowest, roc } from './index';

describe('sma', () => {
  it('averages a flat series to the same value', () => {
    expect(sma([5, 5, 5, 5], 2).slice(1)).toEqual([5, 5, 5]);
  });
  it('warms up with NaN', () => {
    expect(Number.isNaN(sma([1, 2, 3], 3)[0])).toBe(true);
    expect(sma([1, 2, 3], 3)[2]).toBe(2);
  });
});

describe('ema', () => {
  it('matches SMA seed at the first valid index', () => {
    const e = ema([1, 2, 3, 4], 2);
    expect(e[1]).toBeCloseTo(1.5, 10); // sma of [1,2]
    expect(Number.isNaN(e[0])).toBe(true);
  });
  it('is deterministic', () => {
    const a = ema([1, 3, 2, 5, 4, 6], 3);
    const b = ema([1, 3, 2, 5, 4, 6], 3);
    expect(a).toEqual(b);
  });
});

describe('rsi', () => {
  it('returns 100 for a strictly rising series', () => {
    const r = rsi([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16], 14);
    expect(r[15]).toBeCloseTo(100, 6);
  });
  it('stays within [0,100]', () => {
    const r = rsi([5, 4, 6, 3, 7, 2, 8, 1, 9, 0, 10, 11, 9, 12, 8, 13], 14).filter((x) => !Number.isNaN(x));
    for (const v of r) {
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(100);
    }
  });
});

describe('macd', () => {
  it('produces aligned arrays of the input length', () => {
    const v = Array.from({ length: 60 }, (_, i) => 100 + Math.sin(i / 3) * 5);
    const m = macd(v, 12, 26, 9);
    expect(m.macd.length).toBe(60);
    expect(m.signal.length).toBe(60);
    expect(m.hist.length).toBe(60);
  });
});

describe('atr / bbands / channels', () => {
  it('atr is non-negative where defined', () => {
    const h = [10, 11, 12, 13, 14];
    const l = [9, 9, 10, 11, 12];
    const c = [9.5, 10.5, 11.5, 12.5, 13.5];
    const a = atr(h, l, c, 2).filter((x) => !Number.isNaN(x));
    for (const v of a) expect(v).toBeGreaterThanOrEqual(0);
  });
  it('bbands upper >= middle >= lower', () => {
    const v = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    const b = bbands(v, 5, 2);
    for (let i = 4; i < v.length; i++) {
      expect(b.upper[i]).toBeGreaterThanOrEqual(b.middle[i]);
      expect(b.middle[i]).toBeGreaterThanOrEqual(b.lower[i]);
    }
  });
  it('highest/lowest track window extremes', () => {
    expect(highest([1, 3, 2, 5, 4], 2)[3]).toBe(5);
    expect(lowest([1, 3, 2, 5, 4], 2)[2]).toBe(2);
  });
  it('roc computes percent change', () => {
    expect(roc([100, 110], 1)[1]).toBeCloseTo(10, 10);
  });
});
