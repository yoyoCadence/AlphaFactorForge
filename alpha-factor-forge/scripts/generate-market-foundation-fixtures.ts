import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import { canonicalizeFixtureSource } from '../src/parity/indicatorFixture';
import { buildMarketFoundationParityFixture } from '../src/parity/marketFoundationFixture';

const projectRoot = process.cwd();
// Only the generator is hashed: this fixture is the authored specification the
// two implementations are measured against, so it must not depend on their
// sources.
const sources = {
  generator: 'src/parity/marketFoundationFixture.ts',
} as const;
const outputPath = resolve(projectRoot, 'fixtures/rs-core/market-foundation-v1.json');

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

const fixture = buildMarketFoundationParityFixture(hashes);

await mkdir(dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
console.log(`wrote ${outputPath}`);
