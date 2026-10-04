// P12e-7b: attach the single full runner output without changing declarations.
//   npx vite-node scripts/record-recalibration-acceptance.ts
// This records existing reports only; it never runs any simulation.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import type { NoiseSimulationV2Report } from '../src/parity/noiseSimulationV2Fixture';
import { lowestSupportedLength } from '../src/parity/recalibrationDiagnostics';

const path = resolve('fixtures/research/recalibration-plan-v1-acceptance.json');
const fixture = JSON.parse(await readFile(path, 'utf8'));
if ('reports' in fixture) throw new Error('Acceptance reports already recorded; never overwrite them');
const output = JSON.parse(await readFile(resolve('fixtures/research/recalibration-plan-v1-acceptance-output.json'), 'utf8')) as {
  runs: { id: string; report: NoiseSimulationV2Report }[];
};
const ids = fixture.runs.map((run: { id: string }) => run.id);
if (output.runs.length !== 6 || JSON.stringify(output.runs.map((run) => run.id)) !== JSON.stringify(ids)) {
  throw new Error('Runner output must contain exactly the six declared runs, in order');
}
fixture.reports = Object.fromEntries(output.runs.map(({ id, report }) => [id, report]));
const cells = fixture.runs.map((run: { id: string; declaration: { bars: number; autocorrelationPpm: number } }) => ({
  bars: run.declaration.bars,
  autocorrelationPpm: run.declaration.autocorrelationPpm,
  passes: fixture.reports[run.id].status === 'WITHIN_LIMITS',
}));
const lowest = lowestSupportedLength(cells);
fixture.result = {
  lowestSupportedTestedLength: lowest,
  supportedTestedLengths: lowest == null ? [] : [256, 512, 1024].filter((bars) => bars >= lowest),
};
await writeFile(path, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(JSON.stringify({ cells, result: fixture.result }, null, 2));
