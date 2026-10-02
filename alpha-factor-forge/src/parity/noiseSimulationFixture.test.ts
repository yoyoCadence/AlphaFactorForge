// P12e-3 — the committed fixture is what the independent reference computes.
// Small cases are recomputed in full; the 2,000-simulation acceptance runs
// are too slow for the BigInt reference, so their counts are taken as given
// (the Rust tests recompute them on every run) and every derived field is
// re-derived here.

import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-noise-simulation-v1.json';
import {
  referenceNoiseReport,
  referenceNoiseSimulation,
  type NoiseSimulationDeclaration,
  type NoiseSimulationReport,
} from './noiseSimulationFixture';

interface FixtureRun {
  id: string;
  declaration: NoiseSimulationDeclaration;
  expected: NoiseSimulationReport;
}

const cases = fixture.cases as FixtureRun[];
const acceptance = fixture.acceptance as FixtureRun[];

describe('research-noise-simulation-v1 fixture', () => {
  it.each(cases.map((testCase) => [testCase.id, testCase] as const))(
    '%s matches the reference',
    async (_, testCase) => {
      expect(await referenceNoiseSimulation(testCase.declaration)).toEqual(testCase.expected);
    },
  );

  it.each(acceptance.map((run) => [run.id, run] as const))(
    'acceptance run %s is the declared one and its derived fields follow from its counts',
    async (_, run) => {
      // Docs §7: only the autocorrelation differs between the two runs.
      const { autocorrelationPpm, ...declared } = run.declaration;
      expect([0, 300000]).toContain(autocorrelationPpm);
      expect(declared).toEqual({
        contractVersion: 'research-noise-simulation-v1',
        confirmationContract: 'research-confirmation-statistics-v1',
        noiseModel: 'ar1-uniform-sum',
        bars: 256,
        candidatesPerConfirmation: 1,
        priorTrials: 0,
        blockLength: 6,
        bootstrapSamples: 799,
        simulations: 2000,
        seed: 20261002,
        tolerancePpm: 10000,
        allocation: {
          contractVersion: 'research-alpha-allocation-v1',
          rule: 'declared-schedule',
          scope: 'trial-family',
          totalAlphaPpm: 50000,
          schedule: [25000, 25000],
        },
      });
      const { expected } = run;
      expect(await referenceNoiseReport(run.declaration, expected)).toEqual(expected);
      expect(expected.limitFalsePositives).toBe(120);
      expect(expected.nominalStandardErrorPpm).toBe(4873);
      expect(expected.status).toBe(
        expected.familyFalsePositives <= 120 ? 'WITHIN_TOLERANCE' : 'EXCEEDS_TOLERANCE',
      );
    },
  );

  it('covers both declared kinds of noise exactly once', () => {
    expect(acceptance.map((run) => [run.id, run.declaration.autocorrelationPpm])).toEqual([
      ['independent-noise', 0],
      ['serially-correlated-noise', 300000],
    ]);
  });
});
