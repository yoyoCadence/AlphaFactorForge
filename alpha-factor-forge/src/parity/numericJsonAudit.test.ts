// Audit evidence, not a new numeric/identity contract. A repair must preserve
// legacy interpretation explicitly rather than silently changing these hashes.
import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/research/numeric-json-audit-v1.json';
import legacy from '../../fixtures/research/numeric-json-default-output.json';
import roundtrip from '../../fixtures/research/numeric-json-roundtrip-output.json';
import { canonicalBytes, strategyHashFromDefinitionJson } from '../core/hashing';

describe('NUMERIC-JSON-001 frontend audit evidence', () => {
  it('pins both parser modes without treating legacy drift as a repaired contract', () => {
    expect(legacy.cases.map((row) => row.id)).toEqual(fixture.cases.map((row) => row.id));
    expect(roundtrip.cases.map((row) => row.id)).toEqual(fixture.cases.map((row) => row.id));
    for (const [index, row] of fixture.cases.entries()) {
      const correct = roundtrip.cases[index];
      expect([correct.standardBits, correct.parsedBits, correct.roundtripBits]).toEqual(Array(3).fill(row.expectedJsBits));
      expect([correct.strategyHash, correct.rereadCorrectHash]).toEqual(Array(2).fill(row.expectedJsStrategyHash));
      expect(correct.canonical).toBe(row.expectedJsCanonical);
      expect(legacy.cases[index].standardBits).toBe(row.expectedJsBits);
    }
    expect(legacy.cases.filter((row, index) => row.strategyHash !== fixture.cases[index].expectedJsStrategyHash).map((row) => row.id)).toEqual(['number-3']);
  });
  it.each(fixture.cases)('$id preserves native JSON bits and durable identity', async (row) => {
    const bytes = new ArrayBuffer(8);
    new DataView(bytes).setFloat64(0, JSON.parse(row.literal), false);
    expect(new DataView(bytes).getBigUint64(0, false).toString(16).padStart(16, '0')).toBe(row.expectedJsBits);
    expect(JSON.parse(JSON.stringify(JSON.parse(row.literal)))).toBe(JSON.parse(row.literal));
    const encoded = Array.from(canonicalBytes(JSON.parse(row.definitionJson)), (byte) => byte.toString(16).padStart(2, '0')).join('');
    expect(encoded).toBe(row.expectedJsCanonical);
    expect(await strategyHashFromDefinitionJson(row.definitionJson)).toBe(row.expectedJsStrategyHash);
  });
});
