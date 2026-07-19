import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import { canonicalizeFixtureSource } from '../src/parity/indicatorFixture';
import { buildBacktestParityFixture } from '../src/parity/backtestFixture';

const projectRoot = process.cwd();
const sources = {
  generator: 'src/parity/backtestFixture.ts',
  backtest: 'src/core/backtest/index.ts',
  metrics: 'src/core/metrics/index.ts',
  sampleData: 'src/services/sampleData.ts',
} as const;
const outputPath = resolve(projectRoot, 'fixtures/rs-core/backtest-v1.json');

async function sourceHash(relativePath: string): Promise<string> {
  const contents = canonicalizeFixtureSource(
    await readFile(resolve(projectRoot, relativePath), 'utf8'),
  );
  return `sha256:${createHash('sha256').update(contents).digest('hex')}`;
}

const fixture = buildBacktestParityFixture({
  generator: await sourceHash(sources.generator),
  backtest: await sourceHash(sources.backtest),
  metrics: await sourceHash(sources.metrics),
  sampleData: await sourceHash(sources.sampleData),
});

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(`wrote ${outputPath}`);
