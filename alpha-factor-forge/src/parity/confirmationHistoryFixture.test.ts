import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-history-records-v1.json';
import { canonicalBytes, sha256BytesHex } from '../core/hashing';

async function identity(body: { contractVersion: string }): Promise<string> {
  const prefix = new TextEncoder().encode(body.contractVersion + '\0');
  const bytes = canonicalBytes(body);
  const input = new Uint8Array(prefix.length + bytes.length);
  input.set(prefix); input.set(bytes, prefix.length);
  return sha256BytesHex(input);
}
describe('independent confirmation history single-record identities', () => {
  for (const [name, record] of Object.entries(fixture.records)) {
    it(`reproduces ${name} with the existing TypeScript encoder`, async () => {
      expect(await identity(record.body)).toBe(record.id);
    });
  }
  for (const [stage, reservation] of Object.entries(fixture.reservations)) {
    it(`reproduces ${stage} reservation without changing its old identity`, async () => {
      expect(await identity(reservation.body)).toBe(reservation.reservationId);
    });
  }
});
