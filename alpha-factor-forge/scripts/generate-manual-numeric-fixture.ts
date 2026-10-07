// Additive manual-save evidence. Never regenerate the historical audit fixture.
import { readFile, writeFile } from 'node:fs/promises';
import { canonicalBytes, strategyHashFromDefinitionJson } from '../src/core/hashing';
import { defaultStrategy } from '../src/services/strategy';
import { manualStrategyDefinition } from '../src/services/manualStrategyDefinition';

const source = JSON.parse(await readFile('fixtures/research/numeric-json-audit-v1.json', 'utf8')) as {
  cases: { id: string; literal: string; expectedJsBits: string }[];
};
const cases = await Promise.all(source.cases.flatMap((row) => (['params', 'blocks', 'code'] as const).map(async (mode) => {
  const definition = manualStrategyDefinition({ ...defaultStrategy(), mode, feePct: JSON.parse(row.literal) as number });
  const definitionJson = JSON.stringify(definition);
  return {
    id: `${row.id}-${mode}`,
    sourceAuditId: row.id,
    literal: row.literal,
    definitionJson,
    expectedJsBits: row.expectedJsBits,
    expectedJsCanonical: Array.from(canonicalBytes(JSON.parse(definitionJson)), (byte) => byte.toString(16).padStart(2, '0')).join(''),
    expectedJsStrategyHash: await strategyHashFromDefinitionJson(definitionJson),
  };
})));
await writeFile('fixtures/research/numeric-json-manual-v1.json', `${JSON.stringify({ version: 'numeric-json-manual-v1', cases }, null, 2)}\n`);
