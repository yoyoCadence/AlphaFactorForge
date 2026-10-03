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

const MIN_NORMAL = 2.2250738585072014e-308;

/** Draft §2: an intermediate that overflows, or a non-zero one that underflows, is refused. */
function checked(result: number, operandsNonzero: boolean): number {
  if (!Number.isFinite(result) || (operandsNonzero && Math.abs(result) < MIN_NORMAL)) {
    throw new RangeError('outside the candidates\' numeric range');
  }
  return result;
}
const product = (x: number, y: number) => checked(x * y, x !== 0 && y !== 0);
const quotient = (x: number, y: number) => checked(x / y, x !== 0);

/** `floor(log2 m)` for a finite `m > 0`, read from its bits. */
export function referenceBinaryExponent(m: number): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, m);
  const field = (view.getUint32(0) >>> 20) & 0x7ff;
  if (field !== 0) return field - 1023;
  const mantissa = (BigInt(view.getUint32(0) & 0xfffff) << 32n) | BigInt(view.getUint32(4));
  return mantissa.toString(2).length - 1 - 1074;
}

/** `2^k` for `-1022 <= k <= 1023`, built from its bits. */
function powerOfTwo(k: number): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setUint32(0, (k + 1023) * 2 ** 20);
  view.setUint32(4, 0);
  return view.getFloat64(0);
}

function timesPowerOfTwo(x: number, k: number): number {
  if (k >= -1022 && k <= 1023) return x * powerOfTwo(k);
  const half = Math.trunc(k / 2);
  return x * powerOfTwo(half) * powerOfTwo(k - half);
}

/** Draft §2: scaled by an exact power of two so the largest magnitude is in [1, 2). */
export function referenceNormalized(series: readonly number[]): number[] {
  const largest = series.reduce((max, x) => Math.max(max, Math.abs(x)), 0);
  if (largest === 0) return [...series];
  const exponent = referenceBinaryExponent(largest);
  return series.map((x) => checked(timesPowerOfTwo(x, -exponent), x !== 0));
}

/** `B(ℓ)`: the block variance over all `n` circular blocks (draft §2). */
export function referenceBlockVariance(series: readonly number[], sum: number, block: number): number {
  const n = series.length;
  const centre = product(block, quotient(sum, n));
  let total = 0;
  for (let start = 0; start < n; start += 1) {
    let blockSum = 0;
    for (let offset = 0; offset < block; offset += 1) blockSum += series[(start + offset) % n];
    const deviation = blockSum - centre;
    total += product(deviation, deviation);
  }
  return quotient(checked(total, false), n * block);
}

/** `a·√p ≥ s·√q` without a square root (draft §2). */
export function referenceScaledAtLeast(a: number, p: number, s: number, q: number): boolean {
  const left = product(product(a, a), p);
  const right = product(product(s, s), q);
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
  const observedSums = [orderedSum(returns), orderedSum(excess)] as const;
  // Draft §3/§4: a test whose bars are all equal reads no variance.
  const constant = [returns, excess].map((values) =>
    values.every((value) => value === values[0])
      ? values[0] > 0
        ? 0
        : declaration.bootstrapSamples
      : null,
  );
  const series = [referenceNormalized(returns), referenceNormalized(excess)] as const;
  const sums = [orderedSum(series[0]), orderedSum(series[1])] as const;

  // The left and right scales of §2's comparison that do not depend on the
  // resample: S1's observed block variance, S2's stretch.
  const narrow = series.map((values, test) =>
    constant[test] === null ? referenceBlockVariance(values, sums[test], length) : 0,
  );
  const stretch = series.map((values, test): [number, number] => {
    if (statistic !== CANDIDATE_S2 || constant[test] !== null) return [1, 1];
    const flatTop = checked(2 * referenceBlockVariance(values, sums[test], 2 * length) - narrow[test], false);
    return narrow[test] > 0 && flatTop > narrow[test] ? [flatTop, narrow[test]] : [1, 1];
  });

  const extreme = [0, 0];
  for (const resample of resamples(declaration, candidate.candidateIndex, series)) {
    for (const test of [0, 1] as const) {
      if (constant[test] !== null) continue;
      const deviation = resample.totals[test] - sums[test];
      let isExtreme: boolean;
      if (statistic === CANDIDATE_S1) {
        // v*: the variance of the blocks that were drawn.
        const mean = quotient(resample.totals[test], n);
        let spread = 0;
        for (const block of resample.blocks) {
          const difference = block.sums[test] - product(block.take, mean);
          spread += product(difference, difference);
        }
        const resampled = quotient(checked(spread, false), n);
        isExtreme = referenceScaledAtLeast(deviation, narrow[test], sums[test], resampled);
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
    observedMean: observedSums[position] / n,
    extremeCount: constant[position] ?? extreme[position],
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
