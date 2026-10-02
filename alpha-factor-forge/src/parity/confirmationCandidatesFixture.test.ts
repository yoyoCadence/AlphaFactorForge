// P12e-6a — the committed fixture of the DRAFT candidates S1 and S2 is what
// the independent reference computes, and the reference's building blocks do
// what the draft contract says.

import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-candidates-draft-v1.json';
import {
  CANDIDATE_S1,
  CANDIDATE_S2,
  referenceBlockVariance,
  referenceCandidateTests,
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

  it('refuses a series too short for the candidate', () => {
    const base = cases.find((testCase) => testCase.id === 's2-two-candidates') as FixtureCase;
    const wider = { ...base, declaration: { ...base.declaration, blockLength: 3 } };
    // Sixteen bars: (2 × 3)² = 36 is too wide for S2, 3² = 9 is fine for S1.
    expect(() => referenceCandidateTests(CANDIDATE_S2, wider)).toThrow();
    expect(() => referenceCandidateTests(CANDIDATE_S1, wider)).not.toThrow();
  });
});
