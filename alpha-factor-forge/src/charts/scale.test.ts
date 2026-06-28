import { describe, it, expect } from 'vitest';
import { extentOf, padExtent, valueToY } from './scale';

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
