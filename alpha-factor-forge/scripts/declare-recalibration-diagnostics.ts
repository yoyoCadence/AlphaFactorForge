// P12e-6b — writes the DECLARATIONS of the diagnostic grid of
// docs/plans/confirmation-recalibration-plan-v1.md §6 to
// fixtures/research/recalibration-plan-v1-diagnostics.json, from the plan's
// own tables. Nothing is run here. The file is committed before any of its
// declarations is executed; the reports are added afterwards by
// scripts/record-recalibration-diagnostics.ts.
import { writeFile, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';

const outputPath = resolve(
  process.cwd(),
  'fixtures/research/recalibration-plan-v1-diagnostics.json',
);

// Plan §6.
const SEED = 20261005;
const SIMULATIONS = 4000;
// Plan §4: the baseline and the two draft candidates, in tie-break order.
const STATISTICS = [
  ['V1', 'research-confirmation-statistics-v1'],
  ['S1', 'research-confirmation-candidate-s1-v1'],
  ['S2', 'research-confirmation-candidate-s2-v1'],
] as const;
// Plan §3: block length by rule and length, written out as integers.
const BLOCK_LENGTH: Record<'R3' | 'R4', Record<number, number>> = {
  R3: { 256: 6, 512: 8, 1024: 10 },
  R4: { 256: 4, 512: 5, 1024: 6 },
};
const BARS = [256, 512, 1024] as const;
const AUTOCORRELATION = [0, 300000] as const;
// Plan §5: the power shift in millionths, by length and autocorrelation.
const EFFECT: Record<number, Record<number, number>> = {
  256: { 0: 108253, 300000: 154647 },
  512: { 0: 76547, 300000: 109352 },
  1024: { 0: 54127, 300000: 77324 },
};

function declaration(
  statistic: string,
  rule: 'R3' | 'R4',
  bars: number,
  autocorrelationPpm: number,
  power: boolean,
) {
  return {
    contractVersion: 'research-noise-simulation-v2',
    statistic,
    noiseModel: 'ar1-uniform-sum',
    autocorrelationPpm,
    effectMillionths: power ? EFFECT[bars][autocorrelationPpm] : 0,
    bars,
    candidatesPerConfirmation: 1,
    priorTrials: 0,
    blockLength: BLOCK_LENGTH[rule][bars],
    bootstrapSamples: 799,
    simulations: SIMULATIONS,
    seed: SEED,
    // Plan §6's screen for size; power runs only count.
    check: power ? { rule: 'none' } : { rule: 'point-screen', limitMultiplierPpm: 1200000 },
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

const runs = (power: boolean) =>
  STATISTICS.flatMap(([candidate, statistic]) =>
    (['R3', 'R4'] as const).flatMap((rule) =>
      AUTOCORRELATION.flatMap((autocorrelationPpm) =>
        BARS.map((bars) => ({
          id: `${power ? 'power' : 'size'}-${candidate}-${rule}-phi${autocorrelationPpm / 1000}-n${bars}`,
          candidate,
          rule,
          declaration: declaration(statistic, rule, bars, autocorrelationPpm, power),
        })),
      ),
    ),
  );

const document = {
  plan: 'confirmation-recalibration-plan-v1',
  phase: 'diagnostics (§6)',
  description:
    'Declarations of the diagnostic grid, written from the plan tables and committed before any of them was run. Size runs are all executed. Power runs are executed only for candidate/rule pairs that are eligible on the size screen, in the cells at and above their lowest supported tested length (plan §6); the others stay declared and unrun. Reports are added under `reports` by scripts/record-recalibration-diagnostics.ts after the runs; nothing here may be edited in response to a result.',
  screen: {
    confirmationLimitCount: 120,
    familyLimitCount: 240,
  },
  size: runs(false),
  power: runs(true),
};
await mkdir(resolve(process.cwd(), 'fixtures/research'), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(document, null, 2)}\n`, 'utf8');
console.log(`declared ${document.size.length} size and ${document.power.length} power runs`);
