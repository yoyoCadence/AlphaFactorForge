// Recomputes the `expected` blocks of
// fixtures/rs-core/research-confirmation-statistics-v2.json from the
// independent reference in src/parity/confirmationStatisticsV2Fixture.ts. The
// fixed inputs are kept as they are.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { referenceConfirmationV2Tests } from '../src/parity/confirmationStatisticsV2Fixture';
import type { ConfirmationFixtureCase } from '../src/parity/confirmationStatisticsFixture';

const outputPath = resolve(
  process.cwd(),
  'fixtures/rs-core/research-confirmation-statistics-v2.json',
);

const fixture = JSON.parse(await readFile(outputPath, 'utf8')) as {
  cases: (ConfirmationFixtureCase & { expected?: unknown })[];
};
for (const testCase of fixture.cases) {
  testCase.expected = { tests: referenceConfirmationV2Tests(testCase) };
}
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
