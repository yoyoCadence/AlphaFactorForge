// P12e-5 — independent reference for `research-noise-simulation-v2`, written
// from docs/research-noise-simulation-v2.md rather than ported from the Rust
// module. The noise and the confirmation statistics come from the v1 and
// P12e-1 references; the Wilson bounds are found here by bisection on the
// score-test inequality, not by the closed form the Rust module evaluates, so
// the two derivations check each other. It produces the `expected` reports of
// fixtures/rs-core/research-noise-simulation-v2.json. Test support only.

import { referenceAllocationId, type AlphaAllocationDeclaration } from './alphaAllocationFixture';
import {
  CANDIDATE_S1,
  CANDIDATE_S2,
  referenceCandidateTests,
} from './confirmationCandidatesFixture';
import { referenceConfirmationTests } from './confirmationStatisticsFixture';
import { referenceR3BlockLength, STATISTICS_V2 } from './confirmationStatisticsV2Fixture';
import { MAX_AUTOCORRELATION_PPM, noiseSeries, stream } from './noiseSimulationFixture';

const PPM = 1_000_000n;
// z = 1.96 = 49/25 (two-sided 95%): z² = 2401/625 = 38416/10000.
const Z2_NUMERATOR = 38_416n;
const Z2_DENOMINATOR = 10_000n;
const STATISTIC_V1 = 'research-confirmation-statistics-v1';

export type NoiseCheckRule = 'none' | 'point-screen' | 'wilson-upper-bound';

export interface NoiseSimulationV2Declaration {
  contractVersion: string;
  statistic: string;
  noiseModel: string;
  autocorrelationPpm: number;
  effectMillionths: number;
  bars: number;
  candidatesPerConfirmation: number;
  priorTrials: number;
  blockLength: number;
  bootstrapSamples: number;
  simulations: number;
  seed: number;
  check: { rule: NoiseCheckRule; limitMultiplierPpm?: number };
  checkpoints: number[];
  allocation: AlphaAllocationDeclaration;
}

export interface NoiseRateCheck {
  nominalPpm: number;
  limitPpm: number | null;
  count: number;
  ratePpm: number;
  wilsonLowerPpm: number;
  wilsonUpperPpm: number;
  withinLimit: boolean | null;
}

export interface NoiseConfirmationCounts {
  rejectingSimulations: number;
  netReturnRejectingSimulations: number;
  benchmarkExcessRejectingSimulations: number;
  netReturnExtremeTotal: number;
  benchmarkExcessExtremeTotal: number;
}

export interface NoiseCheckpoint {
  simulations: number;
  familyRejectingSimulations: number;
  confirmations: NoiseConfirmationCounts[];
}

export interface NoiseSimulationV2Report {
  contractVersion: string;
  statistic: string;
  allocationId: string;
  status: 'MEASURED' | 'WITHIN_LIMITS' | 'EXCEEDS_LIMITS';
  simulations: number;
  effectMillionths: number;
  checkRule: NoiseCheckRule;
  limitMultiplierPpm: number | null;
  family: NoiseRateCheck;
  confirmations: {
    confirmationNumber: number;
    alphaPpm: number;
    familyTests: number;
    check: NoiseRateCheck;
    counts: NoiseConfirmationCounts;
  }[];
  checkpoints: NoiseCheckpoint[];
}

/**
 * True when rate `u` ppm lies outside the Wilson interval of `count / n`:
 * `n·(u − p̂)² ≥ z²·u·(1 − u)`, in integers.
 */
function outsideWilson(count: bigint, n: bigint, u: bigint): boolean {
  const gap = u * n - count * PPM;
  return gap * gap * Z2_DENOMINATOR >= Z2_NUMERATOR * u * (PPM - u) * n;
}

/** The two-sided 95% Wilson interval in ppm, rounded outwards, by bisection. */
export function referenceWilsonBoundsPpm(count: number, simulations: number): [number, number] {
  const x = BigInt(count);
  const n = BigInt(simulations);
  // The observed rate itself can be an end of the interval (count 0 or all),
  // so "above" and "below" are strict and the ends of the scale always count.
  const above = (u: bigint) => u === PPM || (u * n > x * PPM && outsideWilson(x, n, u));
  const below = (u: bigint) => u === 0n || (u * n < x * PPM && outsideWilson(x, n, u));
  // Upper: the smallest whole ppm at or above the rate that is outside.
  let low = (x * PPM + n - 1n) / n;
  let high = PPM;
  while (low < high) {
    const middle = (low + high) / 2n;
    if (above(middle)) high = middle;
    else low = middle + 1n;
  }
  const upper = low;
  // Lower: the largest whole ppm at or below the rate that is outside.
  low = 0n;
  high = (x * PPM) / n;
  while (low < high) {
    const middle = (low + high + 1n) / 2n;
    if (below(middle)) low = middle;
    else high = middle - 1n;
  }
  return [Number(low), Number(upper)];
}

function limitPpm(nominalPpm: number, multiplierPpm: number): number {
  const product = BigInt(nominalPpm) * BigInt(multiplierPpm);
  if (product % PPM !== 0n || product / PPM >= PPM) {
    throw new Error('the limit is not a whole number of ppm below 1000000');
  }
  return Number(product / PPM);
}

function rateCheck(
  nominalPpm: number,
  count: number,
  declaration: NoiseSimulationV2Declaration,
): NoiseRateCheck {
  const { rule, limitMultiplierPpm } = declaration.check;
  const simulations = BigInt(declaration.simulations);
  const limit = rule === 'none' ? null : limitPpm(nominalPpm, limitMultiplierPpm as number);
  let withinLimit: boolean | null = null;
  if (limit !== null) {
    const scaled = BigInt(count) * PPM;
    const bound = BigInt(limit) * simulations;
    withinLimit =
      rule === 'point-screen'
        ? scaled <= bound
        : scaled < bound && outsideWilson(BigInt(count), simulations, BigInt(limit));
  }
  const [wilsonLowerPpm, wilsonUpperPpm] = referenceWilsonBoundsPpm(count, declaration.simulations);
  return {
    nominalPpm,
    limitPpm: limit,
    count,
    ratePpm: Number((BigInt(count) * PPM) / simulations),
    wilsonLowerPpm,
    wilsonUpperPpm,
    withinLimit,
  };
}

function assertDeclared(declaration: NoiseSimulationV2Declaration): void {
  const { check, checkpoints, simulations, effectMillionths, autocorrelationPpm } = declaration;
  const increasing = checkpoints.every(
    (value, index) =>
      Number.isInteger(value) &&
      value < simulations &&
      value > (index === 0 ? 0 : checkpoints[index - 1]),
  );
  if (
    declaration.contractVersion !== 'research-noise-simulation-v2' ||
    ![STATISTIC_V1, CANDIDATE_S1, CANDIDATE_S2, STATISTICS_V2].includes(declaration.statistic) ||
    (declaration.statistic === STATISTICS_V2 &&
      declaration.blockLength !== referenceR3BlockLength(declaration.bars)) ||
    !Number.isInteger(autocorrelationPpm) ||
    autocorrelationPpm < 0 ||
    autocorrelationPpm > MAX_AUTOCORRELATION_PPM ||
    !Number.isInteger(effectMillionths) ||
    effectMillionths < 0 ||
    !increasing ||
    (check.rule === 'none') !== (check.limitMultiplierPpm === undefined) ||
    (check.rule !== 'none' && effectMillionths !== 0)
  ) {
    throw new Error('declaration is outside research-noise-simulation-v2');
  }
}

/** The whole report of a small declaration, computed by the reference. */
export async function referenceNoiseSimulationV2(
  declaration: NoiseSimulationV2Declaration,
): Promise<NoiseSimulationV2Report> {
  assertDeclared(declaration);
  const { schedule } = declaration.allocation;
  const candidates = declaration.candidatesPerConfirmation;
  const phi = declaration.autocorrelationPpm / 1_000_000;
  const shift = declaration.effectMillionths / 1_000_000;
  const familyTests = (confirmationNumber: number) =>
    2 * (declaration.priorTrials + confirmationNumber * candidates);

  const counts: NoiseConfirmationCounts[] = schedule.map(() => ({
    rejectingSimulations: 0,
    netReturnRejectingSimulations: 0,
    benchmarkExcessRejectingSimulations: 0,
    netReturnExtremeTotal: 0,
    benchmarkExcessExtremeTotal: 0,
  }));
  let familyRejecting = 0;
  const checkpoints: NoiseCheckpoint[] = [];
  for (let simulation = 0; simulation < declaration.simulations; simulation += 1) {
    let familyRejected = false;
    schedule.forEach((alphaPpm, position) => {
      const confirmationNumber = position + 1;
      const series = Array.from({ length: candidates }, (_, candidate) => {
        const noise = (role: number) =>
          noiseSeries(
            stream(declaration.seed, [0, simulation, confirmationNumber, candidate, role]),
            phi,
            declaration.bars,
          );
        const returns = noise(0);
        return {
          candidateIndex: position * candidates + candidate,
          // Nothing is added in the null scenario.
          returns:
            declaration.effectMillionths === 0 ? returns : returns.map((value) => value + shift),
          benchmarkReturns: noise(1),
        };
      });
      const confirmation = {
        declaration: {
          alphaPpm,
          blockLength: declaration.blockLength,
          bootstrapSamples: declaration.bootstrapSamples,
          seed: Number(stream(declaration.seed, [1, simulation, confirmationNumber])() >> 11n),
        },
        familyTests: familyTests(confirmationNumber),
        candidates: series,
      };
      // The declared statistic: the baseline, a draft candidate, or v2 — S2
      // with block rule R3, checked above.
      const tests =
        declaration.statistic === CANDIDATE_S1 || declaration.statistic === CANDIDATE_S2
          ? referenceCandidateTests(declaration.statistic, confirmation)
          : declaration.statistic === STATISTICS_V2
            ? referenceCandidateTests(CANDIDATE_S2, confirmation)
            : referenceConfirmationTests(confirmation);
      const row = counts[position];
      const net = tests.filter((test) => test.test === 'net_return');
      const excess = tests.filter((test) => test.test === 'benchmark_excess');
      row.netReturnExtremeTotal += net.reduce((total, test) => total + test.extremeCount, 0);
      row.benchmarkExcessExtremeTotal += excess.reduce(
        (total, test) => total + test.extremeCount,
        0,
      );
      const netRejected = net.some((test) => test.rejectsNull);
      const excessRejected = excess.some((test) => test.rejectsNull);
      if (netRejected) row.netReturnRejectingSimulations += 1;
      if (excessRejected) row.benchmarkExcessRejectingSimulations += 1;
      if (netRejected || excessRejected) {
        row.rejectingSimulations += 1;
        familyRejected = true;
      }
    });
    if (familyRejected) familyRejecting += 1;
    if (declaration.checkpoints.includes(simulation + 1)) {
      checkpoints.push({
        simulations: simulation + 1,
        familyRejectingSimulations: familyRejecting,
        confirmations: counts.map((row) => ({ ...row })),
      });
    }
  }

  const nominal = schedule.reduce((total, alpha) => total + alpha, 0);
  const family = rateCheck(nominal, familyRejecting, declaration);
  const confirmations = schedule.map((alphaPpm, position) => ({
    confirmationNumber: position + 1,
    alphaPpm,
    familyTests: familyTests(position + 1),
    check: rateCheck(alphaPpm, counts[position].rejectingSimulations, declaration),
    counts: counts[position],
  }));
  const { rule } = declaration.check;
  const allWithin =
    family.withinLimit === true && confirmations.every((row) => row.check.withinLimit === true);
  return {
    contractVersion: declaration.contractVersion,
    statistic: declaration.statistic,
    allocationId: await referenceAllocationId(declaration.allocation),
    status: rule === 'none' ? 'MEASURED' : allWithin ? 'WITHIN_LIMITS' : 'EXCEEDS_LIMITS',
    simulations: declaration.simulations,
    effectMillionths: declaration.effectMillionths,
    checkRule: rule,
    limitMultiplierPpm: rule === 'none' ? null : (declaration.check.limitMultiplierPpm as number),
    family,
    confirmations,
    checkpoints,
  };
}
