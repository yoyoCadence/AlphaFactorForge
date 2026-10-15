import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-alpha-reservation-v1.json';
import { canonicalBytes, sha256BytesHex } from '../core/hashing';

describe('P13a reservation identity cross-runtime fixture', () => {
  it('keeps the hand-authored record identity reproducible by the TypeScript encoder', async () => {
    const prefix = new TextEncoder().encode('research-alpha-reservation-v1\0');
    const body = canonicalBytes(fixture.body);
    const encoded = new Uint8Array(prefix.length + body.length);
    encoded.set(prefix);
    encoded.set(body, prefix.length);
    expect(fixture.body.contractVersion).toBe('research-alpha-reservation-v1');
    expect(await sha256BytesHex(encoded)).toBe(fixture.reservationId);
  });
});
