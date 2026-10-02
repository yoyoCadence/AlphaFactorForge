// Recomputes the `expected` reports of the small `cases` in
// fixtures/rs-core/research-noise-simulation-v1.json from the independent
// reference in src/parity/noiseSimulationFixture.ts. The hand-written
// declarations are kept as they are. The `acceptance` runs are not touched:
// at 2,000 simulations the BigInt reference is too slow, so their reports
// come from the Rust module (docs/research-noise-simulation-v1.md §8).
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import {
  referenceNoiseSimulation,
  type NoiseSimulationDeclaration,
} from '../src/parity/noiseSimulationFixture';

const outputPath = resolve(process.cwd(), 'fixtures/rs-core/research-noise-simulation-v1.json');

const fixture = JSON.parse(await readFile(outputPath, 'utf8')) as {
  cases: { declaration: NoiseSimulationDeclaration; expected?: unknown }[];
};
for (const testCase of fixture.cases) {
  testCase.expected = await referenceNoiseSimulation(testCase.declaration);
}
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
