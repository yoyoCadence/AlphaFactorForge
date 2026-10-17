import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-witness-graph-v1.json';
import sourceFixture from '../../fixtures/rs-core/research-source-witness-v1.json';
import { canonicalBytes, sha256BytesHex } from '../core/hashing';

async function identity(body: { contractVersion: string }): Promise<string> {
  const prefix = new TextEncoder().encode(body.contractVersion + '\0');
  const bytes = canonicalBytes(body);
  const input = new Uint8Array(prefix.length + bytes.length);
  input.set(prefix); input.set(bytes, prefix.length);
  return sha256BytesHex(input);
}
describe('independent strict source-witness graph fixture', () => {
  it('reproduces every original record and alpha reservation identity', async () => {
    for (const record of Object.values(fixture.records)) expect(await identity(record.body)).toBe(record.id);
    for (const reservation of Object.values(fixture.reservations)) expect(await identity(reservation.body)).toBe(reservation.reservationId);
  });
  it('hashes every kind from its original bytes and ties the source to the independent chain', async () => {
    for (const blob of fixture.blobs) expect(await sha256BytesHex(new TextEncoder().encode(blob.text))).toBe(blob.sha256);
    const witness = fixture.blobs.find(b => b.kind === 'source_witness')!;
    expect(witness.text).toBe(sourceFixture.cases[0].witnessText);
    expect(fixture.registeredSource).toEqual(sourceFixture.registeredSource);
    const binding = JSON.parse(fixture.blobs.find(b => b.kind === 'binding')!.text);
    expect(binding.seq).toBe(4);
    expect(binding.chainHead).toBe(sourceFixture.cases[0].anchor.chainHead);
    expect(binding.evidence).toEqual(sourceFixture.cases[0].anchor.evidence);
  });
  it('keeps complete source counts and exact original binding on both admission stages', () => {
    const binding = fixture.blobs.find(b => b.kind === 'binding')!;
    for (const admission of [fixture.records.validationAdmission, fixture.records.testAdmission]) {
      expect(admission.body.registryObservation.seq).toBe(4);
      expect(admission.body.registryObservation.chainHead).toBe(sourceFixture.cases[0].anchor.chainHead);
      expect(admission.body.registryObservation.bindingSha256).toBe(binding.sha256);
      expect(admission.body.registryObservation.familyEffectiveTrials).toBe(4);
      expect(admission.body.registryObservation.registeredBatchEffectiveTrials).toBe(2);
      expect(admission.body.precisionProof.familyTests).toBe(8);
    }
    expect(fixture.records.test.body.candidates).toHaveLength(1);
    expect(fixture.registeredSource.candidates).toHaveLength(2);
  });
});
