import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-contracts-v1.json';
import { canonicalBytes, sha256BytesHex } from '../core/hashing';

async function identity(version: string, body: unknown): Promise<string> {
  const prefix = new TextEncoder().encode(version + '\0');
  const bytes = canonicalBytes(body);
  const input = new Uint8Array(prefix.length + bytes.length);
  input.set(prefix);
  input.set(bytes, prefix.length);
  return sha256BytesHex(input);
}

describe('P13c-1 independent document identities', () => {
  for (const name of ['validation', 'selection', 'test', 'validationOperation', 'testOperation'] as const) {
    it(`reproduces ${name} with the existing TypeScript byte encoder`, async () => {
      const record = fixture[name];
      expect(await identity(record.body.contractVersion, record.body)).toBe(record.id);
    });
  }

  it('binds the original report bytes, family JSON and existing alpha reservation', async () => {
    const reportHash = await sha256BytesHex(new TextEncoder().encode(fixture.reportJson));
    expect(reportHash).toBe(fixture.selection.body.reportSha256);
    expect(await identity(fixture.alphaReservation.body.contractVersion, fixture.alphaReservation.body))
      .toBe(fixture.alphaReservation.reservationId);
    const familyJson = JSON.stringify({
      instrumentId: fixture.validation.body.source.instrumentId, version: 'trial-family-v1',
    });
    expect('trial-family-v1:' + await sha256BytesHex(new TextEncoder().encode(familyJson)))
      .toBe(fixture.validation.body.source.familyId);
    expect(await identity(fixture.allocationDeclaration.contractVersion, fixture.allocationDeclaration))
      .toBe(fixture.validation.body.statistics.allocationId);
  });

  it('pins the hand-calculated selection without inferring calibration', () => {
    const report = JSON.parse(fixture.reportJson);
    expect(report.familyTests).toBe(8);
    expect(report.tests.map((row: { adjustedP: { numerator: number } }) => row.adjustedP.numerator))
      .toEqual([8, 14, 60, 150]);
    expect(fixture.selection.body.selectedCandidates).toEqual([0]);
    expect(fixture.test.body.candidates).toEqual([fixture.validation.body.candidates[0]]);
    expect(fixture.test.body.statistics).toEqual(fixture.validation.body.statistics);
  });
});
