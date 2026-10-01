// Recomputes the `expected` blocks and PRNG vectors of
// fixtures/rs-core/research-confirmation-statistics-v1.json from the
// independent reference in src/parity/confirmationStatisticsFixture.ts.
// The hand-written inputs (declarations, family sizes, return series) are
// kept as they are; only the derived values are rewritten.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import {
  referenceConfirmationTests,
  splitmix64Outputs,
  type ConfirmationFixtureCase,
} from '../src/parity/confirmationStatisticsFixture';

const outputPath = resolve(
  process.cwd(),
  'fixtures/rs-core/research-confirmation-statistics-v1.json',
);

const fixture = JSON.parse(await readFile(outputPath, 'utf8')) as {
  prngVectors: { state: string; outputs: string[] }[];
  cases: (ConfirmationFixtureCase & { expected: unknown })[];
};
for (const vector of fixture.prngVectors) {
  vector.outputs = splitmix64Outputs(vector.state, vector.outputs.length);
}
for (const testCase of fixture.cases) {
  testCase.expected = { tests: referenceConfirmationTests(testCase) };
}
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
