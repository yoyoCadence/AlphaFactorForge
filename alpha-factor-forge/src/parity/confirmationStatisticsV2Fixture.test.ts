// P12e-7a — the committed v2 fixture is what the independent reference
// computes, and the reference's block rule R3 is the rounded cube root.

import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-statistics-v2.json';
import { CANDIDATE_S2, referenceCandidateTests } from './confirmationCandidatesFixture';
import type { ConfirmationFixtureCase } from './confirmationStatisticsFixture';
import {
  referenceConfirmationV2Tests,
  referenceR3BlockLength,
  STATISTICS_V2,
} from './confirmationStatisticsV2Fixture';

const cases = fixture.cases as (ConfirmationFixtureCase & {
  id: string;
  declaration: { contractVersion: string };
  expected: { tests: unknown };
})[];

/** `(2L − 1)³ < 8n < (2L + 1)³`, in BigInt. */
function isRoundedCubeRoot(n: number, length: number): boolean {
  const eightN = 8n * BigInt(n);
  const odd = 2n * BigInt(length);
  return length >= 1 && (odd - 1n) ** 3n < eightN && eightN < (odd + 1n) ** 3n;
}

describe('research-confirmation-statistics-v2 fixture', () => {
  it.each(cases.map((testCase) => [testCase.id, testCase] as const))(
    '%s matches the reference',
    (_, testCase) => {
      expect(testCase.declaration.contractVersion).toBe(STATISTICS_V2);
      expect(referenceConfirmationV2Tests(testCase)).toEqual(testCase.expected.tests);
      // v2 is the draft S2 once R3 holds.
      expect(referenceCandidateTests(CANDIDATE_S2, testCase)).toEqual(testCase.expected.tests);
    },
  );

  it('refuses a block length other than R3', () => {
    const testCase = structuredClone(cases[0]);
    testCase.declaration.blockLength = 3;
    expect(() => referenceConfirmationV2Tests(testCase)).toThrow('blockLength is not round(cbrt(64))');
  });
});

describe('block rule R3', () => {
  it('gives the plan table and the rounded cube root everywhere tested', () => {
    expect([256, 512, 1024].map(referenceR3BlockLength)).toEqual([6, 8, 10]);
    for (let n = 1; n <= 50_000; n += 1) {
      expect(isRoundedCubeRoot(n, referenceR3BlockLength(n))).toBe(true);
    }
    for (let length = 1; length <= 500; length += 1) {
      const last = Number((2n * BigInt(length) + 1n) ** 3n / 8n);
      expect(referenceR3BlockLength(last)).toBe(length);
      expect(referenceR3BlockLength(last + 1)).toBe(length + 1);
      expect(referenceR3BlockLength(length ** 3)).toBe(length);
    }
    expect(referenceR3BlockLength(Number.MAX_SAFE_INTEGER)).toBe(208064);
    expect(isRoundedCubeRoot(Number.MAX_SAFE_INTEGER, 208064)).toBe(true);
    expect(() => referenceR3BlockLength(0)).toThrow();
  });
});
