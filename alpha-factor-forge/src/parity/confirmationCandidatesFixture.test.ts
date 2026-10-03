// P12e-6a — the committed fixture of the DRAFT candidates S1 and S2 is what
// the independent reference computes, and the reference's building blocks do
// what the draft contract says.

import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-candidates-draft-v1.json';
import {
  CANDIDATE_S1,
  CANDIDATE_S2,
  referenceBinaryExponent,
  referenceBlockVariance,
  referenceCandidateTests,
  referenceNormalized,
  referenceScaledAtLeast,
  type CandidateStatistic,
} from './confirmationCandidatesFixture';
import {
  referenceConfirmationTests,
  type ConfirmationFixtureCase,
  type ConfirmationFixtureTest,
} from './confirmationStatisticsFixture';

type FixtureCase = ConfirmationFixtureCase & {
  id: string;
  statistic: CandidateStatistic;
  expected: { tests: ConfirmationFixtureTest[] };
};

const cases = fixture.cases as FixtureCase[];

describe('research-confirmation-candidates-draft-v1 fixture', () => {
  it.each(cases.map((testCase) => [testCase.id, testCase] as const))(
    '%s matches the reference',
    (_, testCase) => {
      expect(referenceCandidateTests(testCase.statistic, testCase)).toEqual(
        testCase.expected.tests,
      );
    },
  );

  it('covers both candidates and is not a cell of the recalibration plan', () => {
    expect(new Set(cases.map((testCase) => testCase.statistic))).toEqual(
      new Set([CANDIDATE_S1, CANDIDATE_S2]),
    );
    for (const testCase of cases) {
      expect([20261005, 20261117]).not.toContain(testCase.declaration.seed);
      expect(testCase.candidates[0].returns.length).toBeLessThan(64);
    }
  });

  it.each(fixture.blockVariances.map((example) => [example.id, example] as const))(
    'block variance %s is the hand-worked value',
    (_, example) => {
      const sum = example.series.reduce((total, value) => total + value, 0);
      expect(referenceBlockVariance(example.series, sum, example.block)).toBe(example.expected);
    },
  );

  it('compares a·√p with s·√q without a square root', () => {
    const values = [-3, -1.5, -0.25, 0, 0.25, 1.5, 3];
    const scales = [0, 0.0625, 0.25, 1, 4];
    for (const a of values) {
      for (const s of values) {
        for (const p of scales) {
          for (const q of scales) {
            expect(referenceScaledAtLeast(a, p, s, q)).toBe(a * Math.sqrt(p) >= s * Math.sqrt(q));
          }
        }
      }
    }
  });

  it('gives S2 at least as many extreme resamples as v1 when the observed sum is positive', () => {
    for (const testCase of cases.filter((entry) => entry.statistic === CANDIDATE_S2)) {
      const plain = referenceConfirmationTests(testCase);
      testCase.expected.tests.forEach((corrected, position) => {
        if (corrected.observedMean > 0) {
          expect(corrected.extremeCount).toBeGreaterThanOrEqual(plain[position].extremeCount);
        }
      });
    }
  });

  /** 2^k by repeated exact doubling or halving. */
  const power = (k: number) => {
    let value = 1;
    for (let step = 0; step < Math.abs(k); step += 1) value = k > 0 ? value * 2 : value / 2;
    return value;
  };
  const probe = (returns: number[]): ConfirmationFixtureCase => ({
    declaration: { alphaPpm: 25000, blockLength: 2, bootstrapSamples: 799, seed: 17 },
    familyTests: 2,
    candidates: [{ candidateIndex: 0, returns, benchmarkReturns: returns.map(() => 0) }],
  });
  const decisions = (tests: ConfirmationFixtureTest[]) =>
    tests.map((test) => [test.extremeCount, test.rejectsNull]);

  it('decides the same at any exact power-of-two scale (PR #138 review R1)', () => {
    const base = [0.75, 0.375, 1.0, 0.25, 0.625, 0.875, 0.375, 0.5, 0.75, 0.375, 1.0, 0.25, 0.625, 0.875, 0.375, 0.5];
    for (const statistic of [CANDIDATE_S1, CANDIDATE_S2] as const) {
      const reference = decisions(referenceCandidateTests(statistic, probe(base)));
      expect(reference[0]).toEqual([0, true]);
      for (const exponent of [400, -400, 1000, -1000]) {
        const scaled = base.map((value) => value * power(exponent));
        expect(decisions(referenceCandidateTests(statistic, probe(scaled)))).toEqual(reference);
      }
    }
    // The guard behind it: squares that leave the double range are refused.
    const huge = power(400);
    const tiny = power(-400);
    expect(() => referenceScaledAtLeast(huge, huge * huge, 2 * huge, huge * huge)).toThrow(RangeError);
    expect(() => referenceScaledAtLeast(tiny, tiny * tiny, 2 * tiny, tiny * tiny)).toThrow(RangeError);
    expect(referenceScaledAtLeast(0, tiny, 0, 0)).toBe(true);
  });

  it('decides a constant series by its sign (PR #138 review R2)', () => {
    for (const statistic of [CANDIDATE_S1, CANDIDATE_S2] as const) {
      for (const [value, expected] of [
        [0.1, [0, true]],
        [0.125, [0, true]],
        [0, [799, false]],
        [-0.1, [799, false]],
      ] as const) {
        const tests = referenceCandidateTests(statistic, probe(Array(16).fill(value)));
        expect(decisions(tests)).toEqual([expected, expected]);
      }
    }
  });

  it('normalizes exactly into [1, 2) and refuses an underflowing value', () => {
    expect(referenceBinaryExponent(1)).toBe(0);
    expect(referenceBinaryExponent(0.1)).toBe(-4);
    expect(referenceBinaryExponent(Number.MAX_VALUE)).toBe(1023);
    expect(referenceBinaryExponent(Number.MIN_VALUE)).toBe(-1074);
    expect(referenceNormalized([0.75, -0.375, 0])).toEqual([1.5, -0.75, 0]);
    expect(referenceNormalized([Number.MIN_VALUE])).toEqual([1]);
    expect(() => referenceNormalized([Number.MAX_VALUE, 1])).toThrow(RangeError);
    const wide = Array(16).fill(1e-300);
    wide[5] = 1e300;
    expect(() => referenceCandidateTests(CANDIDATE_S1, probe(wide))).toThrow(RangeError);
  });

  it('refuses a series too short for the candidate', () => {
    const base = cases.find((testCase) => testCase.id === 's2-two-candidates') as FixtureCase;
    const wider = { ...base, declaration: { ...base.declaration, blockLength: 3 } };
    // Sixteen bars: (2 × 3)² = 36 is too wide for S2, 3² = 9 is fine for S1.
    expect(() => referenceCandidateTests(CANDIDATE_S2, wider)).toThrow();
    expect(() => referenceCandidateTests(CANDIDATE_S1, wider)).not.toThrow();
  });
});
