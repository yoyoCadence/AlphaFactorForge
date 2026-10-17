import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-binding-snapshot-v5.json';
import { sha256BytesHex } from '../core/hashing';

function sortedJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(sortedJson).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    const object = value as Record<string, unknown>;
    return `{${Object.keys(object).sort().map(key => `${JSON.stringify(key)}:${sortedJson(object[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

describe('independent v5 binding byte oracle', () => {
  it('matches canonical JSON and both original/canonical UTF-8 hashes', async () => {
    const utf8 = new TextEncoder();
    expect(sortedJson(fixture.binding)).toBe(fixture.canonicalJson);
    expect(await sha256BytesHex(utf8.encode(fixture.raw))).toBe(fixture.rawSha256);
    expect(await sha256BytesHex(utf8.encode(fixture.canonicalJson))).toBe(fixture.canonicalSha256);
    expect(fixture.rawSha256).not.toBe(fixture.canonicalSha256);
  });

  it('derives the empty-prefix chain independently', async () => {
    expect(await sha256BytesHex(new TextEncoder().encode(
      `trial-ledger-v1:genesis:${fixture.binding.registryId}`,
    ))).toBe(fixture.binding.chainHead);
    expect(fixture.binding.seq).toBe(0);
  });
});
