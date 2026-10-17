// Independent JSON/byte oracle; no product imports or protected execution.
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';

const sha256 = text => createHash('sha256').update(text, 'utf8').digest('hex');
const canonical = value => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.keys(value).sort()
    .map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
  return JSON.stringify(value);
};
const registryId = '1'.repeat(32);
const family = `trial-family-v1:${'a'.repeat(64)}`;
const sections = ['batches', 'selections', 'operations', 'exposures', 'admissions',
  'events', 'blobs', 'conflicts', 'unknownOrigins'];
const binding = {
  version: 'trial-ledger-binding-v5', registryId, seq: 0,
  chainHead: sha256(`trial-ledger-v1:genesis:${registryId}`),
  evidence: { quarantinedFamilies: [family], conflictedOrigins: ['2'.repeat(32)],
    familyTests: { [family]: 2 } },
  alpha: { budgets: [{ familyId: family, allocationId: 'b'.repeat(64) }],
    reservations: ['c'.repeat(64)], conflicts: ['d'.repeat(64)] },
  alphaUnknownOrigins: ['3'.repeat(32)],
  confirmation: Object.fromEntries(sections.map((section, index) => [section,
    [(index + 4).toString(16).repeat(section === 'unknownOrigins' ? 32 : 64)]])),
};
const raw = JSON.stringify(binding, null, 2) + '\n';
const canonicalJson = canonical(binding);
const fixture = {
  description: 'Format/confirmation-ID coverage only; synthetic IDs do not prove record existence, source authenticity, freshness or execution rights.',
  binding, raw, rawSha256: sha256(raw), canonicalJson,
  canonicalSha256: sha256(canonicalJson),
};
writeFileSync(new URL('../fixtures/rs-core/research-binding-snapshot-v5.json', import.meta.url),
  JSON.stringify(fixture, null, 2) + '\n');
