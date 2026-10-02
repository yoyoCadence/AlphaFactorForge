// P12e-3 — independent reference for `research-noise-simulation-v1`, written
// from docs/research-noise-simulation-v1.md rather than ported from the Rust
// module: BigInt SplitMix64 streams, the AR(1) noise, and one simulated family
// life per simulation built on the P12e-1 and P12e-2 references. It produces
// the `expected` reports of the small `cases` in
// fixtures/rs-core/research-noise-simulation-v1.json; the Rust tests must
// reproduce them. It is far too slow for the 2,000-simulation acceptance runs,
// whose counts come from Rust — `referenceNoiseReport` still re-derives every
// derived field from those counts. Test support only.

import { referenceAllocationId, type AlphaAllocationDeclaration } from './alphaAllocationFixture';
import { referenceConfirmationTests } from './confirmationStatisticsFixture';

const MASK = (1n << 64n) - 1n;
const GOLDEN_GAMMA = 0x9e3779b97f4a7c15n;
const WARMUP_BARS = 64;
// A series starts at zero, so its first kept bar has 1 − φ^130 of the
// stationary variance. 64 warm-up bars are enough only up to this coefficient
// (shortfall about 1.1 ppm); the contract refuses anything larger (§3).
export const MAX_AUTOCORRELATION_PPM = 900_000;
const PPM = 1_000_000n;

export interface NoiseSimulationDeclaration {
  contractVersion: string;
  confirmationContract: string;
  noiseModel: string;
  autocorrelationPpm: number;
  bars: number;
  candidatesPerConfirmation: number;
  priorTrials: number;
  blockLength: number;
  bootstrapSamples: number;
  simulations: number;
  seed: number;
  tolerancePpm: number;
  allocation: AlphaAllocationDeclaration;
}

export interface NoiseConfirmationCounts {
  falsePositives: number;
  netReturnRejections: number;
  benchmarkExcessRejections: number;
}

export interface NoiseSimulationCounts {
  familyFalsePositives: number;
  confirmations: NoiseConfirmationCounts[];
}

export interface NoiseSimulationReport {
  contractVersion: string;
  confirmationContract: string;
  allocationId: string;
  status: 'WITHIN_TOLERANCE' | 'EXCEEDS_TOLERANCE';
  simulations: number;
  nominalAlphaPpm: number;
  tolerancePpm: number;
  limitFalsePositives: number;
  familyFalsePositives: number;
  familyFalsePositiveRate: { numerator: number; denominator: number };
  familyFalsePositiveRatePpm: number;
  nominalStandardErrorPpm: number;
  confirmations: (NoiseConfirmationCounts & {
    confirmationNumber: number;
    alphaPpm: number;
    familyTests: number;
    falsePositiveRatePpm: number;
  })[];
}

function mix64(input: bigint): bigint {
  let z = input & MASK;
  z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
  z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
  return z ^ (z >> 31n);
}

/** A SplitMix64 stream keyed by a list of integers (contract §3). */
export function stream(seed: number, parts: readonly number[]): () => bigint {
  let state = BigInt(seed);
  for (const part of parts) state = mix64(state ^ mix64(BigInt(part) + 1n));
  return () => {
    state = (state + GOLDEN_GAMMA) & MASK;
    return mix64(state);
  };
}

export function noiseSeries(next: () => bigint, phi: number, bars: number): number[] {
  const uniform = () => Number(next() >> 11n) * 2 ** -53;
  const series: number[] = [];
  let value = 0;
  for (let bar = 0; bar < WARMUP_BARS + bars; bar += 1) {
    const innovation = uniform() + uniform() + uniform() + uniform() - 2;
    value = phi * value + innovation;
    if (bar >= WARMUP_BARS) series.push(value);
  }
  return series;
}

function familyTests(declaration: NoiseSimulationDeclaration, confirmationNumber: number): number {
  return 2 * (declaration.priorTrials + confirmationNumber * declaration.candidatesPerConfirmation);
}

function floorSqrt(value: bigint): bigint {
  if (value < 2n) return value;
  let low = 1n;
  let high = value;
  while (low < high) {
    const middle = (low + high + 1n) / 2n;
    if (middle * middle <= value) low = middle;
    else high = middle - 1n;
  }
  return low;
}

/** Runs every simulated family life and counts the false positives. */
export function referenceNoiseCounts(declaration: NoiseSimulationDeclaration): NoiseSimulationCounts {
  const { schedule } = declaration.allocation;
  const candidates = declaration.candidatesPerConfirmation;
  if (
    !Number.isInteger(declaration.autocorrelationPpm) ||
    declaration.autocorrelationPpm < 0 ||
    declaration.autocorrelationPpm > MAX_AUTOCORRELATION_PPM
  ) {
    throw new Error('autocorrelationPpm is outside research-noise-simulation-v1');
  }
  const phi = declaration.autocorrelationPpm / 1_000_000;
  const confirmations: NoiseConfirmationCounts[] = schedule.map(() => ({
    falsePositives: 0,
    netReturnRejections: 0,
    benchmarkExcessRejections: 0,
  }));
  let familyFalsePositives = 0;
  for (let simulation = 0; simulation < declaration.simulations; simulation += 1) {
    let familyRejected = false;
    schedule.forEach((alphaPpm, position) => {
      // The k-th confirmation of a family that reserved the k−1 before it
      // gets the k-th scheduled share.
      const confirmationNumber = position + 1;
      const series = Array.from({ length: candidates }, (_, candidate) => ({
        candidateIndex: position * candidates + candidate,
        returns: noiseSeries(
          stream(declaration.seed, [0, simulation, confirmationNumber, candidate, 0]),
          phi,
          declaration.bars,
        ),
        benchmarkReturns: noiseSeries(
          stream(declaration.seed, [0, simulation, confirmationNumber, candidate, 1]),
          phi,
          declaration.bars,
        ),
      }));
      const tests = referenceConfirmationTests({
        declaration: {
          alphaPpm,
          blockLength: declaration.blockLength,
          bootstrapSamples: declaration.bootstrapSamples,
          seed: Number(stream(declaration.seed, [1, simulation, confirmationNumber])() >> 11n),
        },
        familyTests: familyTests(declaration, confirmationNumber),
        candidates: series,
      });
      const rejected = tests.filter((test) => test.rejectsNull);
      const counts = confirmations[position];
      counts.netReturnRejections += rejected.filter((test) => test.test === 'net_return').length;
      counts.benchmarkExcessRejections += rejected.filter(
        (test) => test.test === 'benchmark_excess',
      ).length;
      if (rejected.length > 0) {
        counts.falsePositives += 1;
        familyRejected = true;
      }
    });
    if (familyRejected) familyFalsePositives += 1;
  }
  return { familyFalsePositives, confirmations };
}

/** The report for given counts: every derived field in exact integers. */
export async function referenceNoiseReport(
  declaration: NoiseSimulationDeclaration,
  counts: NoiseSimulationCounts,
): Promise<NoiseSimulationReport> {
  const simulations = BigInt(declaration.simulations);
  const nominal = declaration.allocation.schedule.reduce((total, alpha) => total + alpha, 0);
  const allowed = BigInt(nominal + declaration.tolerancePpm);
  const ratePpm = (count: number) => Number((BigInt(count) * PPM) / simulations);
  const limit = (simulations * allowed) / PPM;
  return {
    contractVersion: declaration.contractVersion,
    confirmationContract: declaration.confirmationContract,
    allocationId: await referenceAllocationId(declaration.allocation),
    status:
      BigInt(counts.familyFalsePositives) * PPM <= simulations * allowed
        ? 'WITHIN_TOLERANCE'
        : 'EXCEEDS_TOLERANCE',
    simulations: declaration.simulations,
    nominalAlphaPpm: nominal,
    tolerancePpm: declaration.tolerancePpm,
    limitFalsePositives: Number(limit < simulations ? limit : simulations),
    familyFalsePositives: counts.familyFalsePositives,
    familyFalsePositiveRate: {
      numerator: counts.familyFalsePositives,
      denominator: declaration.simulations,
    },
    familyFalsePositiveRatePpm: ratePpm(counts.familyFalsePositives),
    nominalStandardErrorPpm: Number(
      floorSqrt((BigInt(nominal) * (PPM - BigInt(nominal))) / simulations),
    ),
    confirmations: declaration.allocation.schedule.map((alphaPpm, position) => ({
      confirmationNumber: position + 1,
      alphaPpm,
      familyTests: familyTests(declaration, position + 1),
      falsePositives: counts.confirmations[position].falsePositives,
      falsePositiveRatePpm: ratePpm(counts.confirmations[position].falsePositives),
      netReturnRejections: counts.confirmations[position].netReturnRejections,
      benchmarkExcessRejections: counts.confirmations[position].benchmarkExcessRejections,
    })),
  };
}

/** The whole report of a small declaration, computed by the reference. */
export function referenceNoiseSimulation(
  declaration: NoiseSimulationDeclaration,
): Promise<NoiseSimulationReport> {
  return referenceNoiseReport(declaration, referenceNoiseCounts(declaration));
}
