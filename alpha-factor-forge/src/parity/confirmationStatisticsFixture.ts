// Independent reference for `research-confirmation-statistics-v1`
// (docs/research-confirmation-statistics-v1.md), used only to author and
// guard `fixtures/rs-core/research-confirmation-statistics-v1.json`.
//
// It is written from the contract text, not ported from the Rust module:
// BigInt SplitMix64, a circular block bootstrap over per-bar returns, and Holm
// in exact integers. The fixture's `expected` blocks are this file's output;
// the Rust tests have to reproduce them, and the Vitest beside this file
// fails if the committed fixture ever stops matching.
//
// Not a product path: the backend calculation is the Rust module.

const MASK = (1n << 64n) - 1n;
const GOLDEN_GAMMA = 0x9e3779b97f4a7c15n;

export interface ConfirmationFixtureDeclaration {
  alphaPpm: number;
  blockLength: number;
  bootstrapSamples: number;
  seed: number;
}

export interface ConfirmationFixtureCandidate {
  candidateIndex: number;
  returns: number[];
  benchmarkReturns: number[];
}

export interface ConfirmationFixtureCase {
  declaration: ConfirmationFixtureDeclaration;
  familyTests: number;
  candidates: ConfirmationFixtureCandidate[];
}

export type ConfirmationTestName = 'net_return' | 'benchmark_excess';

export interface ConfirmationFixtureTest {
  candidateIndex: number;
  test: ConfirmationTestName;
  observations: number;
  observedMean: number;
  extremeCount: number;
  rawP: { numerator: number; denominator: number };
  rawPValue: number;
  holmRank: number;
  adjustedP: { numerator: number; denominator: number };
  adjustedPValue: number;
  rejectsNull: boolean;
}

const TEST_ORDER: Record<ConfirmationTestName, number> = { net_return: 0, benchmark_excess: 1 };

function mix64(input: bigint): bigint {
  let z = input;
  z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
  z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
  return z ^ (z >> 31n);
}

function splitmix64(initialState: bigint): () => bigint {
  let state = initialState & MASK;
  return () => {
    state = (state + GOLDEN_GAMMA) & MASK;
    return mix64(state);
  };
}

/** The first `count` raw outputs for a starting state, as decimal strings. */
export function splitmix64Outputs(state: string, count: number): string[] {
  const next = splitmix64(BigInt(state));
  return Array.from({ length: count }, () => next().toString());
}

/** Uniform in `[0, n)`: draws below `2^64 mod n` are discarded. */
export function below(next: () => bigint, n: number): number {
  const size = BigInt(n);
  const threshold = (1n << 64n) % size;
  for (;;) {
    const draw = next();
    if (draw >= threshold) return Number(draw % size);
  }
}

/** Left-to-right sum: the order is part of the contract. */
export function orderedSum(values: number[]): number {
  let total = 0;
  for (const value of values) total += value;
  return total;
}

export interface Observed {
  candidateIndex: number;
  test: ConfirmationTestName;
  observations: number;
  observedMean: number;
  extremeCount: number;
}

/** One SplitMix64 stream per candidate, keyed by its index. */
export function candidateStream(seed: number, candidateIndex: number): () => bigint {
  return splitmix64(mix64(BigInt(seed) ^ mix64((BigInt(candidateIndex) + 1n) & MASK)));
}

function bootstrapCandidate(
  declaration: ConfirmationFixtureDeclaration,
  candidate: ConfirmationFixtureCandidate,
): Observed[] {
  const returns = candidate.returns;
  const n = returns.length;
  const excess = returns.map((value, bar) => value - candidate.benchmarkReturns[bar]);
  const observed = [orderedSum(returns), orderedSum(excess)];
  const series = [returns, excess];
  const next = candidateStream(declaration.seed, candidate.candidateIndex);
  const blocks = Math.ceil(n / declaration.blockLength);
  const extreme = [0, 0];
  for (let sample = 0; sample < declaration.bootstrapSamples; sample += 1) {
    const totals = [0, 0];
    let filled = 0;
    for (let block = 0; block < blocks; block += 1) {
      const start = below(next, n);
      const take = Math.min(declaration.blockLength, n - filled);
      for (let offset = 0; offset < take; offset += 1) {
        const bar = (start + offset) % n;
        totals[0] += series[0][bar];
        totals[1] += series[1][bar];
      }
      filled += take;
    }
    // Centered null: (mean* - mean) >= mean  <=>  sum* >= 2 * sum.
    if (totals[0] >= 2 * observed[0]) extreme[0] += 1;
    if (totals[1] >= 2 * observed[1]) extreme[1] += 1;
  }
  return (['net_return', 'benchmark_excess'] as const).map((test, position) => ({
    candidateIndex: candidate.candidateIndex,
    test,
    observations: n,
    observedMean: observed[position] / n,
    extremeCount: extreme[position],
  }));
}

/** The `expected.tests` block of one fixture case. */
export function referenceConfirmationTests(testCase: ConfirmationFixtureCase): ConfirmationFixtureTest[] {
  return referenceHolm(
    testCase,
    testCase.candidates.flatMap((candidate) => bootstrapCandidate(testCase.declaration, candidate)),
  );
}

/**
 * Exact p-values and Holm against the whole family, for the extreme counts of
 * any statistic that keeps the `(1 + extreme) / (B + 1)` form.
 */
export function referenceHolm(
  testCase: Pick<ConfirmationFixtureCase, 'declaration' | 'familyTests'>,
  observed: Observed[],
): ConfirmationFixtureTest[] {
  const denominator = BigInt(testCase.declaration.bootstrapSamples) + 1n;
  const order = observed
    .map((_, position) => position)
    .sort(
      (a, b) =>
        observed[a].extremeCount - observed[b].extremeCount ||
        observed[a].candidateIndex - observed[b].candidateIndex ||
        TEST_ORDER[observed[a].test] - TEST_ORDER[observed[b].test],
    );
  const results = new Array<ConfirmationFixtureTest>(observed.length);
  let running = 0n;
  order.forEach((position, sorted) => {
    const row = observed[position];
    const rank = sorted + 1;
    const raw = BigInt(row.extremeCount) + 1n;
    let numerator = (BigInt(testCase.familyTests) - BigInt(rank) + 1n) * raw;
    if (numerator > denominator) numerator = denominator;
    if (numerator > running) running = numerator;
    results[position] = {
      ...row,
      rawP: { numerator: Number(raw), denominator: Number(denominator) },
      rawPValue: Number(raw) / Number(denominator),
      holmRank: rank,
      adjustedP: { numerator: Number(running), denominator: Number(denominator) },
      adjustedPValue: Number(running) / Number(denominator),
      rejectsNull: running * 1_000_000n <= BigInt(testCase.declaration.alphaPpm) * denominator,
    };
  });
  return results.sort(
    (a, b) => a.candidateIndex - b.candidateIndex || TEST_ORDER[a.test] - TEST_ORDER[b.test],
  );
}
