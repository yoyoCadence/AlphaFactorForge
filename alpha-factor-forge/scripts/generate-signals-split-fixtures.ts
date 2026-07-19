import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import { canonicalizeFixtureSource } from '../src/parity/indicatorFixture';
import { buildSignalsSplitParityFixture } from '../src/parity/signalsSplitFixture';

const projectRoot = process.cwd();
const sources = {
  generator: 'src/parity/signalsSplitFixture.ts',
  strategySignals: 'src/services/strategySignals.ts',
  split: 'src/core/validation/split.ts',
  embargo: 'src/services/embargo.ts',
  strategy: 'src/services/strategy.ts',
  indicators: 'src/core/indicators/index.ts',
  sampleData: 'src/services/sampleData.ts',
} as const;
const outputPath = resolve(projectRoot, 'fixtures/rs-core/signals-split-v1.json');

async function sourceHash(relativePath: string): Promise<string> {
  const contents = canonicalizeFixtureSource(
    await readFile(resolve(projectRoot, relativePath), 'utf8'),
  );
  return `sha256:${createHash('sha256').update(contents).digest('hex')}`;
}

const fixture = buildSignalsSplitParityFixture({
  generator: await sourceHash(sources.generator),
  strategySignals: await sourceHash(sources.strategySignals),
  split: await sourceHash(sources.split),
  embargo: await sourceHash(sources.embargo),
  strategy: await sourceHash(sources.strategy),
  indicators: await sourceHash(sources.indicators),
  sampleData: await sourceHash(sources.sampleData),
});

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(`wrote ${outputPath}`);
