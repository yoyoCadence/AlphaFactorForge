// P12e-6a — independent reference for the DRAFT confirmation candidates S1 and
// S2 (docs/research-confirmation-candidates-draft-v1.md), written from that
// document rather than ported from the Rust module. Resampling, exact
// p-values and Holm come from the v1 reference; only the rule that makes a
// resample extreme is new. It produces the `expected` blocks of
// fixtures/rs-core/research-confirmation-candidates-draft-v1.json.
// Test support only: the candidates are experimental and no product path
// may use them.

import {
  below,
  candidateStream,
  orderedSum,
  referenceHolm,
  type ConfirmationFixtureCandidate,
  type ConfirmationFixtureCase,
  type ConfirmationFixtureDeclaration,
  type ConfirmationFixtureTest,
  type Observed,
} from './confirmationStatisticsFixture';

export const CANDIDATE_S1 = 'research-confirmation-candidate-s1-v1';
export const CANDIDATE_S2 = 'research-confirmation-candidate-s2-v1';
export type CandidateStatistic = typeof CANDIDATE_S1 | typeof CANDIDATE_S2;

/** `B(ℓ)`: the block variance over all `n` circular blocks (draft §2). */
export function referenceBlockVariance(series: readonly number[], sum: number, block: number): number {
  const n = series.length;
  const centre = block * (sum / n);
  let total = 0;
  for (let start = 0; start < n; start += 1) {
    let blockSum = 0;
    for (let offset = 0; offset < block; offset += 1) blockSum += series[(start + offset) % n];
    const deviation = blockSum - centre;
    total += deviation * deviation;
  }
  return total / (n * block);
}

/** `a·√p ≥ s·√q` without a square root (draft §2). */
export function referenceScaledAtLeast(a: number, p: number, s: number, q: number): boolean {
  const left = a * a * p;
  const right = s * s * q;
  if (a >= 0) return s >= 0 ? left >= right : true;
  return s >= 0 ? left === 0 && right === 0 : left <= right;
}

interface DrawnBlock {
  sums: [number, number];
  take: number;
}

interface Resample {
  totals: [number, number];
  blocks: DrawnBlock[];
}

/** Every resample of one candidate: v1's block draws, with per-block sums. */
function* resamples(
  declaration: ConfirmationFixtureDeclaration,
  candidateIndex: number,
  series: readonly [readonly number[], readonly number[]],
): Generator<Resample> {
  const n = series[0].length;
  const next = candidateStream(declaration.seed, candidateIndex);
  const blockCount = Math.ceil(n / declaration.blockLength);
  for (let sample = 0; sample < declaration.bootstrapSamples; sample += 1) {
    const totals: [number, number] = [0, 0];
    const blocks: DrawnBlock[] = [];
    let filled = 0;
    for (let block = 0; block < blockCount; block += 1) {
      const start = below(next, n);
      const take = Math.min(declaration.blockLength, n - filled);
      const sums: [number, number] = [0, 0];
      for (let offset = 0; offset < take; offset += 1) {
        const bar = (start + offset) % n;
        for (const test of [0, 1] as const) {
          totals[test] += series[test][bar];
          sums[test] += series[test][bar];
        }
      }
      blocks.push({ sums, take });
      filled += take;
    }
    yield { totals, blocks };
  }
}

function candidateObserved(
  statistic: CandidateStatistic,
  declaration: ConfirmationFixtureDeclaration,
  candidate: ConfirmationFixtureCandidate,
): Observed[] {
  const returns = candidate.returns;
  const n = returns.length;
  const length = declaration.blockLength;
  const widest = statistic === CANDIDATE_S2 ? 2 * length : length;
  if (
    n !== candidate.benchmarkReturns.length ||
    n < 2 ||
    widest * widest > n ||
    ![...returns, ...candidate.benchmarkReturns].every(Number.isFinite)
  ) {
    throw new Error('series is outside the candidate draft contract');
  }
  const excess = returns.map((value, bar) => value - candidate.benchmarkReturns[bar]);
  const series = [returns, excess] as const;
  const sums = [orderedSum(returns), orderedSum(excess)] as const;

  // The left and right scales of §2's comparison that do not depend on the
  // resample: S1's observed block variance, S2's stretch.
  const narrow = series.map((values, test) => referenceBlockVariance(values, sums[test], length));
  const stretch = series.map((values, test): [number, number] => {
    if (statistic !== CANDIDATE_S2) return [1, 1];
    const flatTop = 2 * referenceBlockVariance(values, sums[test], 2 * length) - narrow[test];
    return narrow[test] > 0 && flatTop > narrow[test] ? [flatTop, narrow[test]] : [1, 1];
  });

  const extreme = [0, 0];
  for (const resample of resamples(declaration, candidate.candidateIndex, series)) {
    for (const test of [0, 1] as const) {
      const deviation = resample.totals[test] - sums[test];
      let isExtreme: boolean;
      if (statistic === CANDIDATE_S1) {
        // v*: the variance of the blocks that were drawn.
        const mean = resample.totals[test] / n;
        let spread = 0;
        for (const block of resample.blocks) {
          const difference = block.sums[test] - block.take * mean;
          spread += difference * difference;
        }
        isExtreme = referenceScaledAtLeast(deviation, narrow[test], sums[test], spread / n);
      } else {
        isExtreme = referenceScaledAtLeast(
          deviation,
          stretch[test][0],
          sums[test],
          stretch[test][1],
        );
      }
      if (isExtreme) extreme[test] += 1;
    }
  }
  return (['net_return', 'benchmark_excess'] as const).map((test, position) => ({
    candidateIndex: candidate.candidateIndex,
    test,
    observations: n,
    observedMean: sums[position] / n,
    extremeCount: extreme[position],
  }));
}

/** The `expected.tests` block of one candidate fixture case. */
export function referenceCandidateTests(
  statistic: CandidateStatistic,
  testCase: ConfirmationFixtureCase,
): ConfirmationFixtureTest[] {
  return referenceHolm(
    testCase,
    testCase.candidates.flatMap((candidate) =>
      candidateObserved(statistic, testCase.declaration, candidate),
    ),
  );
}
