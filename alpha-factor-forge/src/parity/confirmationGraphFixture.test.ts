import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-graph-v1.json';
import { canonicalBytes, sha256BytesHex } from '../core/hashing';

async function identity(body: { contractVersion: string }): Promise<string> {
  const prefix = new TextEncoder().encode(body.contractVersion + '\0');
  const bytes = canonicalBytes(body);
  const input = new Uint8Array(prefix.length + bytes.length);
  input.set(prefix); input.set(bytes, prefix.length);
  return sha256BytesHex(input);
}
describe('independent complete confirmation graph fixture', () => {
  for (const [name, record] of Object.entries(fixture.records)) {
    it(`reproduces ${name} identity without trusting its stored ID`, async () => {
      expect(await identity(record.body)).toBe(record.id);
    });
  }
  for (const [stage, reservation] of Object.entries(fixture.reservations)) {
    it(`reproduces ${stage} original alpha identity`, async () => {
      expect(await identity(reservation.body)).toBe(reservation.reservationId);
    });
  }
  for (const [index, blob] of fixture.blobs.entries()) {
    it(`hashes blob ${index} from original bytes, including decimals and whitespace`, async () => {
      expect(await sha256BytesHex(new TextEncoder().encode(blob.text))).toBe(blob.sha256);
      expect(blob.text).not.toBe(JSON.stringify(JSON.parse(blob.text)));
    });
  }
  it('keeps full registered-batch and family counts for the smaller Test subset', () => {
    expect(fixture.records.test.body.candidates).toHaveLength(1);
    for (const admission of [fixture.records.validationAdmission, fixture.records.testAdmission]) {
      const p = admission.body.precisionPlan;
      const proof = admission.body.precisionProof;
      const m = BigInt(p.priorTrials + p.plannedTrials) * BigInt(p.testsPerTrial);
      const ceil = (a: bigint, b: bigint) => (a + b - 1n) / b;
      expect(m).toBe(8n);
      expect(proof.familyTests).toBe(Number(m));
      expect(proof.requiredSamplesForResolution).toBe(Number(ceil(m * 1_000_000n, BigInt(p.alphaPpm)) - 1n));
      expect(proof.requiredSamplesForPrecision).toBe(Number(ceil((1_000_000n * m - BigInt(p.alphaPpm)) * 1_000_000_000_000n,
        BigInt(p.alphaPpm) * BigInt(p.maxRelativeStandardErrorPpm) ** 2n)));
      expect(p.plannedTrials).toBe(fixture.registeredSource.candidates.length);
    }
  });
});
