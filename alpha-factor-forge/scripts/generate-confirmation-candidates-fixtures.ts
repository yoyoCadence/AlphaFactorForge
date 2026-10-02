// Recomputes the `expected` blocks of
// fixtures/rs-core/research-confirmation-candidates-draft-v1.json from the
// independent reference in src/parity/confirmationCandidatesFixture.ts. The
// fixed inputs and the hand-worked `blockVariances` are kept as they are.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import {
  referenceCandidateTests,
  type CandidateStatistic,
} from '../src/parity/confirmationCandidatesFixture';
import type { ConfirmationFixtureCase } from '../src/parity/confirmationStatisticsFixture';

const outputPath = resolve(
  process.cwd(),
  'fixtures/rs-core/research-confirmation-candidates-draft-v1.json',
);

const fixture = JSON.parse(await readFile(outputPath, 'utf8')) as {
  cases: (ConfirmationFixtureCase & { statistic: CandidateStatistic; expected?: unknown })[];
};
for (const testCase of fixture.cases) {
  testCase.expected = { tests: referenceCandidateTests(testCase.statistic, testCase) };
}
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
