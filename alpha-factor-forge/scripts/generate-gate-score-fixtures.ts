import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import { canonicalizeFixtureSource } from '../src/parity/indicatorFixture';
import { buildGateScoreParityFixture } from '../src/parity/gateScoreFixture';

const projectRoot = process.cwd();
const sources = {
  generator: 'src/parity/gateScoreFixture.ts',
  gate: 'src/services/gate.ts',
  score: 'src/services/score.ts',
  strategy: 'src/services/strategy.ts',
  benchmarks: 'src/services/benchmarks.ts',
  validationRecord: 'src/services/validationRecord.ts',
  metricsCodec: 'src/services/metricsCodec.ts',
  nonFinite: 'src/services/nonFinite.ts',
} as const;
const outputPath = resolve(projectRoot, 'fixtures/rs-core/gate-score-v1.json');

async function sourceHash(relativePath: string): Promise<string> {
  const contents = canonicalizeFixtureSource(
    await readFile(resolve(projectRoot, relativePath), 'utf8'),
  );
  return `sha256:${createHash('sha256').update(contents).digest('hex')}`;
}

const hashes = {} as Record<keyof typeof sources, string>;
for (const [key, path] of Object.entries(sources) as [keyof typeof sources, string][]) {
  hashes[key] = await sourceHash(path);
}

const fixture = buildGateScoreParityFixture(hashes);

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(`wrote ${outputPath}`);
