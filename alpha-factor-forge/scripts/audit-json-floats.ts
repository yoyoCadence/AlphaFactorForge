// NUMERIC-JSON-001: isolated inputs and independent frontend identity evidence.
// This never rewrites established strategy/calibration fixtures or user data.
import { writeFile } from 'node:fs/promises';
import { canonicalBytes, strategyHashFromDefinitionJson } from '../src/core/hashing';

const literals = ['0', '0.125', '0.1', '0.0036944444444444438', '0.3333333333333333', '1.2345678901234567'];
const bits = (value: number) => {
  const bytes = new ArrayBuffer(8);
  new DataView(bytes).setFloat64(0, value, false);
  return new DataView(bytes).getBigUint64(0, false).toString(16).padStart(16, '0');
};
const cases = await Promise.all(literals.map(async (literal, index) => {
  const definitionJson = `{"mode":"params","feePct":${literal},"slipPct":0,"fastMA":5,"slowMA":20}`;
  return {
    id: `number-${index}`, literal, definitionJson,
    expectedJsBits: bits(JSON.parse(literal)),
    expectedJsCanonical: Array.from(canonicalBytes(JSON.parse(definitionJson)), (byte) => byte.toString(16).padStart(2, '0')).join(''),
    expectedJsStrategyHash: await strategyHashFromDefinitionJson(definitionJson),
  };
}));
await writeFile('fixtures/research/numeric-json-audit-v1.json', `${JSON.stringify({ version: 'numeric-json-audit-v1', cases }, null, 2)}\n`);
