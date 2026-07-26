import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import { canonicalizeFixtureSource } from '../src/parity/indicatorFixture';
import {
  buildRunnerConfigParityFixture,
  type RunnerConfigSourceKey,
} from '../src/parity/runnerConfigFixture';

const projectRoot = process.cwd();
const sources = {
  generator: 'src/parity/runnerConfigFixture.ts',
  discoveryConfig: 'src/services/discoveryConfig.ts',
  candidateEnumeration: 'src/services/candidateEnumeration.ts',
  discoverySeed: 'src/services/discoverySeed.ts',
  hashing: 'src/core/hashing/index.ts',
  strategy: 'src/services/strategy.ts',
  gate: 'src/services/gate.ts',
  score: 'src/services/score.ts',
  randomEntry: 'src/services/randomEntry.ts',
} as const satisfies Record<RunnerConfigSourceKey, string>;
const outputPath = resolve(projectRoot, 'fixtures/rs-core/runner-config-v1.json');

async function sourceHash(relativePath: string): Promise<string> {
  const contents = canonicalizeFixtureSource(
    await readFile(resolve(projectRoot, relativePath), 'utf8'),
  );
  return `sha256:${createHash('sha256').update(contents).digest('hex')}`;
}

const hashes = {} as Record<RunnerConfigSourceKey, string>;
for (const [key, path] of Object.entries(sources) as [RunnerConfigSourceKey, string][]) {
  hashes[key] = await sourceHash(path);
}

const fixture = await buildRunnerConfigParityFixture(hashes);

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(`wrote ${outputPath}`);
