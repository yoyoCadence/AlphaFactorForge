import { describe, expect, it } from 'vitest';
import { planValidationSplit, type InclusiveBarRange } from './split';

describe('planValidationSplit', () => {
  it('excludes both embargo gaps before allocating the default 60/20/20 split', () => {
    expect(planValidationSplit(100, 2)).toEqual({
      totalBars: 100,
      usableBars: 96,
      embargoBars: 2,
      train: { from: 0, to: 57, count: 58 },
      trainValidationEmbargo: { from: 58, to: 59, count: 2 },
      validation: { from: 60, to: 78, count: 19 },
      validationTestEmbargo: { from: 79, to: 80, count: 2 },
      test: { from: 81, to: 99, count: 19 },
    });
  });

  it.each([
    {
      totalBars: 101,
      embargoBars: 0,
      expectedCounts: [61, 20, 20],
    },
    {
      totalBars: 99,
      embargoBars: 1,
      expectedCounts: [58, 20, 19],
    },
    {
      totalBars: 5,
      embargoBars: 0,
      expectedCounts: [3, 1, 1],
    },
  ])(
    'rounds deterministically for $totalBars bars with a $embargoBars-bar embargo',
    ({ totalBars, embargoBars, expectedCounts }) => {
      const plan = planValidationSplit(totalBars, embargoBars);
      expect([plan.train.count, plan.validation.count, plan.test.count]).toEqual(expectedCounts);
      expect(planValidationSplit(totalBars, embargoBars)).toEqual(plan);
    },
  );

  it.each([
    [5, [3, 1, 1]],
    [6, [4, 1, 1]],
    [7, [4, 2, 1]],
    [8, [5, 2, 1]],
    [9, [5, 2, 2]],
  ])('covers the complete largest-remainder residue table for %s usable bars', (totalBars, expectedCounts) => {
    const plan = planValidationSplit(totalBars, 0);
    expect([plan.train.count, plan.validation.count, plan.test.count]).toEqual(expectedCounts);
  });

  it('keeps largest-remainder allocation exact at the safe-integer boundary', () => {
    const plan = planValidationSplit(Number.MAX_SAFE_INTEGER, 0);
    expect([plan.train.count, plan.validation.count, plan.test.count]).toEqual([
      5_404_319_552_844_595,
      1_801_439_850_948_198,
      1_801_439_850_948_198,
    ]);
    expect(plan.test.to).toBe(Number.MAX_SAFE_INTEGER - 1);
  });

  it('uses adjacent segments and no synthetic ranges when embargo is zero', () => {
    const plan = planValidationSplit(10, 0);
    expect(plan.trainValidationEmbargo).toBeNull();
    expect(plan.validationTestEmbargo).toBeNull();
    expect(plan.validation.from).toBe(plan.train.to + 1);
    expect(plan.test.from).toBe(plan.validation.to + 1);
  });

  it('accounts for every bar exactly once in ascending non-overlapping ranges', () => {
    const plan = planValidationSplit(137, 7);
    const ranges: InclusiveBarRange[] = [
      plan.train,
      plan.trainValidationEmbargo,
      plan.validation,
      plan.validationTestEmbargo,
      plan.test,
    ].filter((range): range is InclusiveBarRange => range !== null);

    let expectedFrom = 0;
    for (const range of ranges) {
      expect(range.from).toBe(expectedFrom);
      expect(range.to - range.from + 1).toBe(range.count);
      expectedFrom = range.to + 1;
    }
    expect(expectedFrom).toBe(plan.totalBars);
    expect(plan.test.to).toBe(plan.totalBars - 1);
  });

  it.each([
    [0, 0],
    [4, 0],
    [8, 2],
    [10, 3],
  ])('fails closed when %s total bars and %s embargo bars leave too little data', (totalBars, embargoBars) => {
    expect(() => planValidationSplit(totalBars, embargoBars)).toThrow(RangeError);
  });

  it.each([
    [-1, 0],
    [10.5, 0],
    [Number.NaN, 0],
    [Number.POSITIVE_INFINITY, 0],
    [Number.MAX_SAFE_INTEGER + 1, 0],
    [10, -1],
    [10, 1.5],
    [10, Number.NaN],
  ])('rejects invalid integer inputs (%s, %s)', (totalBars, embargoBars) => {
    expect(() => planValidationSplit(totalBars, embargoBars)).toThrow(RangeError);
  });
});
