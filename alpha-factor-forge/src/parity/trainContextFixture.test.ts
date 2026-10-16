import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-train-context-v1.json';
import { canonicalBytes, sha256BytesHex } from '../core/hashing';

async function identity(body: { contractVersion: string }): Promise<string> {
  const prefix = new TextEncoder().encode(body.contractVersion + '\0');
  const bytes = canonicalBytes(body);
  const input = new Uint8Array(prefix.length + bytes.length);
  input.set(prefix);
  input.set(bytes, prefix.length);
  return sha256BytesHex(input);
}

describe('P13c-2 independent Train context fixture', () => {
  for (const [name, record] of [
    ['scope', fixture.scope], ['context', fixture.context],
    ...fixture.origins.map((v, i) => [`origin ${i}`, v] as const),
  ] as const) {
    it(`reproduces ${name} using the existing TypeScript encoder`, async () => {
      expect(await identity(record.body)).toBe(record.id);
    });
  }
  it('contains only the exact complete Train facts and scope', () => {
    expect(fixture.request.originIds).toEqual(fixture.origins.map(v => v.id));
    expect(fixture.context.body.facts).toEqual(fixture.origins.map(v => ({ originId: v.id, fact: v.body.fact })));
    expect(fixture.context.body.scope).toEqual(fixture.scope.body);
    expect(fixture.context.body.facts.map(v => v.fact.kind).sort()).toEqual(['failure', 'hypothesis', 'summary']);
  });
});
