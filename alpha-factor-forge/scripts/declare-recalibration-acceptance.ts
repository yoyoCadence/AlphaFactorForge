// P12e-7a — writes the DECLARATIONS of the final acceptance of
// docs/plans/confirmation-recalibration-plan-v1.md §7 to
// fixtures/research/recalibration-plan-v1-acceptance.json, from the plan's
// own tables, for the frozen research-confirmation-statistics-v2. Nothing is
// run here. The file is committed and reviewed before the run (P12e-7b),
// which uses seed 20261117 once; its reports are added afterwards and nothing
// here may be edited in response to a result.
import { writeFile, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';

const outputPath = resolve(
  process.cwd(),
  'fixtures/research/recalibration-plan-v1-acceptance.json',
);

// Plan §7.
const SEED = 20261117;
const SIMULATIONS = 20000;
// Plan §6 selection (P12e-6b), frozen in P12e-7a.
const STATISTIC = 'research-confirmation-statistics-v2';
// Plan §3, rule R3, written out as integers.
const BLOCK_LENGTH: Record<number, number> = { 256: 6, 512: 8, 1024: 10 };
const BARS = [256, 512, 1024] as const;
const AUTOCORRELATION = [0, 300000] as const;

function declaration(bars: number, autocorrelationPpm: number) {
  return {
    contractVersion: 'research-noise-simulation-v2',
    statistic: STATISTIC,
    noiseModel: 'ar1-uniform-sum',
    autocorrelationPpm,
    effectMillionths: 0,
    bars,
    candidatesPerConfirmation: 1,
    priorTrials: 0,
    blockLength: BLOCK_LENGTH[bars],
    bootstrapSamples: 799,
    simulations: SIMULATIONS,
    seed: SEED,
    // Plan §7's rule: the 95% Wilson upper bound at most 1.2 × nominal.
    check: { rule: 'wilson-upper-bound', limitMultiplierPpm: 1200000 },
    // Engine contract §6: one checkpoint at 4096 / bars simulations.
    checkpoints: [4096 / bars],
    allocation: {
      contractVersion: 'research-alpha-allocation-v1',
      rule: 'declared-schedule',
      scope: 'trial-family',
      totalAlphaPpm: 50000,
      schedule: [25000, 25000],
    },
  };
}

const document = {
  plan: 'confirmation-recalibration-plan-v1',
  phase: 'final acceptance (§7)',
  description:
    'Declarations of the final acceptance, written from the plan tables for the frozen research-confirmation-statistics-v2 (S2 with block rule R3) and committed before the run. P12e-7b runs all six once on seed 20261117 with the release runner and adds the reports under `reports`; a cell passes when its two confirmation checks and its family check pass, and the supported tested configurations follow from §7 alone. Nothing here may be edited in response to a result.',
  rule: {
    confirmationLimitPpm: 30000,
    familyLimitPpm: 60000,
    maximumConfirmationCount: 552,
    maximumFamilyCount: 1134,
  },
  runs: AUTOCORRELATION.flatMap((autocorrelationPpm) =>
    BARS.map((bars) => ({
      id: `acceptance-v2-phi${autocorrelationPpm / 1000}-n${bars}`,
      declaration: declaration(bars, autocorrelationPpm),
    })),
  ),
};
await mkdir(resolve(process.cwd(), 'fixtures/research'), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(document, null, 2)}\n`, 'utf8');
console.log(`declared ${document.runs.length} acceptance runs`);
