import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-source-witness-v1.json';
import { sha256BytesHex } from '../core/hashing';

function sortedJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(sortedJson).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    const object = value as Record<string, unknown>;
    return `{${Object.keys(object).sort().map(key => `${JSON.stringify(key)}:${sortedJson(object[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}
const hash = (text: string) => sha256BytesHex(new TextEncoder().encode(text));

describe('independent original source witness oracle', () => {
  it('preserves exact nested source text and canonical outer bytes/SHA', async () => {
    for (const c of fixture.cases) {
      expect(sortedJson({ contractVersion: 'research-confirmation-source-witness-v1',
        registeredSource: fixture.registeredSource, trialExportText: c.trialExportText })).toBe(c.witnessText);
      expect(await hash(c.witnessText)).toBe(c.witnessSha256);
      expect(await hash(c.trialExportText)).toBe(c.archiveSha256);
    }
  });

  it('derives actual event identities, complete batch membership and the nonempty chain', async () => {
    for (const c of fixture.cases) {
      const [headerText, ...lines] = c.trialExportText.trimEnd().split('\n');
      const header = JSON.parse(headerText);
      expect(await hash(lines.join('\n') + '\n')).toBe(header.bodySha256);
      const rows = lines.map(line => JSON.parse(line));
      const events = rows.filter(row => row.type === 'event');
      let chain = await hash(`trial-ledger-v1:genesis:${header.registryId}`);
      for (const [index, row] of events.entries()) {
        expect(row.eventId).toBe(`trial-event-v1:${await hash(sortedJson(row.payload))}`);
        chain = await hash(chain + row.eventId);
        expect(row.seq).toBe(index + 1);
        expect(row.chain).toBe(chain);
      }
      expect(events).toHaveLength(4);
      expect(header.headSeq).toBe(4);
      expect(header.headChain).toBe(chain);
      for (const batch of rows.filter(row => row.type === 'batch')) {
        expect(batch.batchId).toBe(`trial-batch-v1:${await hash(sortedJson({
          version: 'trial-batch-v1', familyId: batch.familyId, members: batch.members,
        }))}`);
        expect(events.filter(e => e.batchId === batch.batchId).map(e => [e.payload.idempotencyKey, e.eventId]))
          .toEqual(batch.members);
      }
    }
  });

  it('separates original anchor facts from audit-byte identity and same-head upgrades', () => {
    const [original, upgraded, audit] = fixture.cases;
    expect(original.anchor).toEqual(audit.anchor);
    expect(original.witnessSha256).not.toBe(audit.witnessSha256);
    expect(original.anchor.chainHead).toBe(upgraded.anchor.chainHead);
    expect(original.anchor.evidence.familyTests).not.toEqual(upgraded.anchor.evidence.familyTests);
    expect(upgraded.expected.testsPerTrial).toBe(3); // archival data, no eligibility claim
  });
});
