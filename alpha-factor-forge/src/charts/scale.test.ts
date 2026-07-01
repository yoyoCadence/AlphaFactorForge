import { describe, it, expect } from 'vitest';
import { extentOf, padExtent, valueToY, tradeLegs, replayWindow } from './scale';

describe('extentOf', () => {
  it('ignores NaN/Infinity and returns min/max', () => {
    expect(extentOf([3, NaN, 1, Infinity, 2, -Infinity])).toEqual({ min: 1, max: 3 });
  });

  it('returns a safe span for all-NaN input', () => {
    expect(extentOf([NaN, Infinity])).toEqual({ min: 0, max: 1 });
  });

  it('widens a degenerate (single-value) extent', () => {
    expect(extentOf([5, 5, 5])).toEqual({ min: 4, max: 6 });
  });
});

describe('padExtent', () => {
  it('expands by the given fraction on each side', () => {
    expect(padExtent({ min: 0, max: 10 }, 0.1)).toEqual({ min: -1, max: 11 });
  });
});

describe('valueToY', () => {
  it('maps max to the top and min to the bottom (inverted)', () => {
    const e = { min: 0, max: 100 };
    expect(valueToY(100, e, 0, 200)).toBe(0);
    expect(valueToY(0, e, 0, 200)).toBe(200);
    expect(valueToY(50, e, 0, 200)).toBe(100);
  });

  it('returns top for a zero span', () => {
    expect(valueToY(5, { min: 5, max: 5 }, 10, 200)).toBe(10);
  });
});

describe('replayWindow', () => {
  it('with upto=null shows the last maxBars bars (pre-6-1 behaviour)', () => {
    expect(replayWindow(1000, null, 500)).toEqual({ start: 500, end: 999 });
    // fewer bars than maxBars -> the whole series
    expect(replayWindow(10, null, 500)).toEqual({ start: 0, end: 9 });
  });

  it('ends the window at the cursor and keeps up to maxBars bars', () => {
    // cursor early -> window starts at 0
    expect(replayWindow(600, 100, 500)).toEqual({ start: 0, end: 100 });
    // cursor past the maxBars span -> window slides to end at the cursor
    expect(replayWindow(600, 550, 500)).toEqual({ start: 51, end: 550 });
  });

  it('floors and clamps the cursor into [0, total-1]', () => {
    expect(replayWindow(600, 100.9, 500)).toEqual({ start: 0, end: 100 });
    expect(replayWindow(600, -5, 500)).toEqual({ start: 0, end: 0 });
    expect(replayWindow(600, 999, 500)).toEqual({ start: 100, end: 599 });
  });

  it('handles an empty series and a degenerate maxBars', () => {
    expect(replayWindow(0, null, 500)).toEqual({ start: 0, end: -1 });
    expect(replayWindow(600, 300, 0)).toEqual({ start: 300, end: 300 }); // cap floored to 1
  });
});

describe('tradeLegs', () => {
  // candles at t = 10,20,30,40,50 -> indices 0..4
  const timeToIndex = new Map([10, 20, 30, 40, 50].map((t, i) => [t, i]));

  it('maps a LONG trade to buy@entry / sell@exit at the right indices', () => {
    const legs = tradeLegs([{ entryTime: 20, exitTime: 40, side: 'LONG' }], timeToIndex);
    expect(legs).toEqual([
      { index: 1, kind: 'buy', leg: 'entry' },
      { index: 3, kind: 'sell', leg: 'exit' },
    ]);
  });

  it('flips direction for a SHORT trade (sell@entry / buy@exit)', () => {
    const legs = tradeLegs([{ entryTime: 10, exitTime: 50, side: 'SHORT' }], timeToIndex);
    expect(legs).toEqual([
      { index: 0, kind: 'sell', leg: 'entry' },
      { index: 4, kind: 'buy', leg: 'exit' },
    ]);
  });

  it('drops legs whose time is not a known candle', () => {
    const legs = tradeLegs([{ entryTime: 20, exitTime: 999, side: 'LONG' }], timeToIndex);
    expect(legs).toEqual([{ index: 1, kind: 'buy', leg: 'entry' }]);
  });

  it('flattens multiple trades in order', () => {
    const legs = tradeLegs(
      [
        { entryTime: 10, exitTime: 20, side: 'LONG' },
        { entryTime: 30, exitTime: 40, side: 'LONG' },
      ],
      timeToIndex,
    );
    expect(legs.map((l) => l.index)).toEqual([0, 1, 2, 3]);
  });
});
