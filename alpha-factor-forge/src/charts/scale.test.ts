import { describe, it, expect } from 'vitest';
import { extentOf, padExtent, valueToY, tradeLegs, replayWindow, replayTick, positionAtTime, barAtX, reconcileBarWindow, zoomBarWindow } from './scale';

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

describe('reconcileBarWindow', () => {
  it('clamps a normal window without changing its bar count', () => {
    expect(reconcileBarWindow({ start: 950, end: 1049 }, 1000)).toEqual({ start: 900, end: 999 });
    expect(reconcileBarWindow({ start: -20, end: 79 }, 1000)).toEqual({ start: 0, end: 99 });
  });

  it('follows the replay cursor and never reveals future bars', () => {
    expect(reconcileBarWindow({ start: 100, end: 199 }, 1000, 350)).toEqual({ start: 251, end: 350 });
    expect(reconcileBarWindow({ start: 100, end: 199 }, 1000, 40)).toEqual({ start: 0, end: 40 });
  });
});

describe('zoomBarWindow', () => {
  it('zooms in around the anchor bar and preserves its relative position', () => {
    const before = { start: 100, end: 199 }; // 100 bars; anchor 149 is near centre
    const after = zoomBarWindow(before, 149, -100, 999, 10, 500);
    expect(after).toEqual({ start: 110, end: 189 }); // 80 bars
    const beforeRatio = (149 - before.start + 0.5) / 100;
    const afterRatio = (149 - after.start + 0.5) / 80;
    expect(Math.abs(afterRatio - beforeRatio)).toBeLessThan(0.01);
  });

  it('zooms out and clamps at the data boundary / configured cap', () => {
    expect(zoomBarWindow({ start: 100, end: 199 }, 149, 100, 219, 10, 500)).toEqual({ start: 88, end: 212 });
    expect(zoomBarWindow({ start: 0, end: 499 }, 250, 100, 999, 10, 500)).toEqual({ start: 0, end: 499 });
  });

  it('honours the minimum and handles empty bounds', () => {
    expect(zoomBarWindow({ start: 0, end: 9 }, 5, -100, 99, 10, 500)).toEqual({ start: 0, end: 9 });
    expect(zoomBarWindow({ start: 0, end: -1 }, 0, -100, -1)).toEqual({ start: 0, end: -1 });
  });
});

describe('replayTick', () => {
  it('advances one bar and flags the end', () => {
    expect(replayTick(100, 600)).toEqual({ cursor: 101, atEnd: false });
    expect(replayTick(598, 600)).toEqual({ cursor: 599, atEnd: true }); // reaches last bar
    expect(replayTick(599, 600)).toEqual({ cursor: 599, atEnd: true }); // clamps at last bar
  });

  it('handles a single-bar / empty series', () => {
    expect(replayTick(0, 1)).toEqual({ cursor: 0, atEnd: true });
    expect(replayTick(0, 0)).toEqual({ cursor: 0, atEnd: true });
  });
});

describe('barAtX', () => {
  // padL 6, plotW 600, start 0, n 60 -> bar width 10
  it('maps an x pixel to the bar under it', () => {
    expect(barAtX(6, 6, 600, 0, 60)).toBe(0);
    expect(barAtX(15, 6, 600, 0, 60)).toBe(0); // within bar 0's [6,16)
    expect(barAtX(16, 6, 600, 0, 60)).toBe(1);
    expect(barAtX(106, 6, 600, 0, 60)).toBe(10);
  });

  it('clamps to the visible window and honours a start offset', () => {
    expect(barAtX(0, 6, 600, 0, 60)).toBe(0); // left of the plot -> first bar
    expect(barAtX(9999, 6, 600, 0, 60)).toBe(59); // right of the plot -> last bar
    expect(barAtX(6, 6, 500, 100, 50)).toBe(100); // start offset
    expect(barAtX(9999, 6, 500, 100, 50)).toBe(149);
  });

  it('returns start for an empty window', () => {
    expect(barAtX(50, 6, 600, 7, 0)).toBe(7);
  });
});

describe('positionAtTime', () => {
  const trades = [
    { entryTime: 20, exitTime: 40, side: 'LONG' as const },
    { entryTime: 60, exitTime: 80, side: 'SHORT' as const },
  ];

  it('returns the covering trade side (bounds inclusive)', () => {
    expect(positionAtTime(trades, 30)).toBe('LONG');
    expect(positionAtTime(trades, 20)).toBe('LONG'); // entry bar
    expect(positionAtTime(trades, 40)).toBe('LONG'); // exit bar
    expect(positionAtTime(trades, 70)).toBe('SHORT');
  });

  it('returns FLAT before, between, and after trades', () => {
    expect(positionAtTime(trades, 10)).toBe('FLAT');
    expect(positionAtTime(trades, 50)).toBe('FLAT');
    expect(positionAtTime(trades, 99)).toBe('FLAT');
    expect(positionAtTime([], 30)).toBe('FLAT');
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
