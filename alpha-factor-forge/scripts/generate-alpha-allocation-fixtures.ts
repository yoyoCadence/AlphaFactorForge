// Recomputes the derived values of
// fixtures/rs-core/research-alpha-allocation-v1.json from the independent
// reference in src/parity/alphaAllocationFixture.ts. The hand-written inputs
// (declarations, reserved histories, error messages) are kept as they are;
// only `expected` reports and equal-split `schedule`s are rewritten.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import {
  referenceAllocation,
  referenceEqualSchedule,
  type AlphaAllocationFixtureCase,
} from '../src/parity/alphaAllocationFixture';

const outputPath = resolve(process.cwd(), 'fixtures/rs-core/research-alpha-allocation-v1.json');

const fixture = JSON.parse(await readFile(outputPath, 'utf8')) as {
  cases: (AlphaAllocationFixtureCase & { expected?: unknown; error?: string })[];
  equalSchedules: {
    totalAlphaPpm: number;
    confirmations: number;
    schedule?: number[];
    error?: string;
  }[];
};
for (const testCase of fixture.cases) {
  if (testCase.error === undefined) testCase.expected = await referenceAllocation(testCase);
}
for (const split of fixture.equalSchedules) {
  if (split.error === undefined) {
    split.schedule = referenceEqualSchedule(split.totalAlphaPpm, split.confirmations);
  }
}
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
