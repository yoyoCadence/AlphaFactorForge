import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import { canonicalizeFixtureSource } from '../src/parity/indicatorFixture';
import { buildBenchmarkParityFixture } from '../src/parity/benchmarkFixture';

const projectRoot = process.cwd();
const sources = {
  generator: 'src/parity/benchmarkFixture.ts',
  parityEncode: 'src/parity/parityEncode.ts',
  nonFinite: 'src/services/nonFinite.ts',
  benchmarks: 'src/services/benchmarks.ts',
  randomEntry: 'src/services/randomEntry.ts',
  backtestRunner: 'src/services/backtestRunner.ts',
  strategySignals: 'src/services/strategySignals.ts',
  strategy: 'src/services/strategy.ts',
  indicators: 'src/core/indicators/index.ts',
  backtest: 'src/core/backtest/index.ts',
  metrics: 'src/core/metrics/index.ts',
  sampleData: 'src/services/sampleData.ts',
} as const;
const outputPath = resolve(projectRoot, 'fixtures/rs-core/benchmark-v1.json');

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

const fixture = buildBenchmarkParityFixture(hashes);

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(`wrote ${outputPath}`);
