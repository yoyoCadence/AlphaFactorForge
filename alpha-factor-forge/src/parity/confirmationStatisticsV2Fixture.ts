// P12e-7a — independent reference for `research-confirmation-statistics-v2`,
// written from docs/research-confirmation-statistics-v2.md: the draft S2
// reference with block rule R3 checked first. R3 is found here by scanning
// odd cubes in BigInt, not by the Rust module's bisection, so the two check
// each other. It produces the `expected` blocks of
// fixtures/rs-core/research-confirmation-statistics-v2.json. Test support only.

import { CANDIDATE_S2, referenceCandidateTests } from './confirmationCandidatesFixture';
import type {
  ConfirmationFixtureCase,
  ConfirmationFixtureTest,
} from './confirmationStatisticsFixture';

export const STATISTICS_V2 = 'research-confirmation-statistics-v2';

/** Block rule R3, `round(n^(1/3))`: the first `L ≥ 1` with `(2L + 1)³ > 8n`. */
export function referenceR3BlockLength(n: number): number {
  if (!Number.isSafeInteger(n) || n < 1) throw new Error('n must be a positive safe integer');
  const eightN = 8n * BigInt(n);
  let length = 1n;
  while ((2n * length + 1n) ** 3n <= eightN) length += 1n;
  return Number(length);
}

/** The `expected.tests` block of one v2 fixture case. */
export function referenceConfirmationV2Tests(
  testCase: ConfirmationFixtureCase,
): ConfirmationFixtureTest[] {
  for (const candidate of testCase.candidates) {
    const n = candidate.returns.length;
    if (
      n === candidate.benchmarkReturns.length &&
      n >= 2 &&
      testCase.declaration.blockLength !== referenceR3BlockLength(n)
    ) {
      throw new Error(`blockLength is not round(cbrt(${n}))`);
    }
  }
  return referenceCandidateTests(CANDIDATE_S2, testCase);
}
