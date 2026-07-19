import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import {
  buildIndicatorParityFixture,
  canonicalizeFixtureSource,
} from '../src/parity/indicatorFixture';

const projectRoot = process.cwd();
const sources = {
  generator: 'src/parity/indicatorFixture.ts',
  indicators: 'src/core/indicators/index.ts',
  sampleData: 'src/services/sampleData.ts',
} as const;
const outputPath = resolve(projectRoot, 'fixtures/rs-core/indicators-v1.json');

async function sourceHash(relativePath: string): Promise<string> {
  const contents = canonicalizeFixtureSource(
    await readFile(resolve(projectRoot, relativePath), 'utf8'),
  );
  return `sha256:${createHash('sha256').update(contents).digest('hex')}`;
}

const fixture = buildIndicatorParityFixture({
  generator: await sourceHash(sources.generator),
  indicators: await sourceHash(sources.indicators),
  sampleData: await sourceHash(sources.sampleData),
});

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(`wrote ${outputPath}`);
