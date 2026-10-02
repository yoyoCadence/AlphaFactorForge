// Recomputes the `expected` reports of
// fixtures/rs-core/research-noise-simulation-v2.json from the independent
// reference in src/parity/noiseSimulationV2Fixture.ts. The hand-written
// declarations are kept as they are.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import {
  referenceNoiseSimulationV2,
  type NoiseSimulationV2Declaration,
} from '../src/parity/noiseSimulationV2Fixture';

const outputPath = resolve(process.cwd(), 'fixtures/rs-core/research-noise-simulation-v2.json');

const fixture = JSON.parse(await readFile(outputPath, 'utf8')) as {
  cases: { declaration: NoiseSimulationV2Declaration; expected?: unknown }[];
};
for (const testCase of fixture.cases) {
  testCase.expected = await referenceNoiseSimulationV2(testCase.declaration);
}
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
