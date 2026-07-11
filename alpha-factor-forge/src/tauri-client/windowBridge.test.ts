import { describe, expect, it } from 'vitest';
import { defaultStrategy } from '../services/strategy';
import { mergeChartSnapshot, type ChartWindowSnapshot } from './windowBridge';

function snapshot(datasetKey: string, close: number): ChartWindowSnapshot {
  return {
    datasetKey,
    title: datasetKey,
    candles: [{ t: 1, o: close, h: close, l: close, c: close, v: 1 }],
    strat: defaultStrategy(),
    show: { ma: true, ema: false, bb: false, rsi: true, vol: true, trades: true },
    trades: [],
  };
}

describe('mergeChartSnapshot', () => {
  it('preserves candle identity for updates to the same dataset', () => {
    const current = snapshot('same', 10);
    const incoming = { ...snapshot('same', 99), title: 'updated title', upto: 0 };
    const merged = mergeChartSnapshot(current, incoming);
    expect(merged.candles).toBe(current.candles);
    expect(merged.title).toBe('updated title');
    expect(merged.upto).toBe(0);
  });

  it('accepts a new candle array when the dataset changes', () => {
    const incoming = snapshot('new', 20);
    expect(mergeChartSnapshot(snapshot('old', 10), incoming)).toBe(incoming);
    expect(mergeChartSnapshot(null, incoming)).toBe(incoming);
  });
});
