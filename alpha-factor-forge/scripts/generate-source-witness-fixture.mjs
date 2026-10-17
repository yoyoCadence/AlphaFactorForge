// Independent complete trial-chain and canonical witness oracle. No product
// imports, prices, bootstrap, reserved seed or execution authority.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

const hash = text => createHash('sha256').update(text, 'utf8').digest('hex');
const canonical = v => {
  if (Array.isArray(v)) return `[${v.map(canonical).join(',')}]`;
  if (v !== null && typeof v === 'object') return `{${Object.keys(v).sort()
    .map(k => `${JSON.stringify(k)}:${canonical(v[k])}`).join(',')}}`;
  return JSON.stringify(v);
};
const base = JSON.parse(readFileSync(new URL('../fixtures/rs-core/research-confirmation-contracts-v1.json', import.meta.url), 'utf8'));
const source = structuredClone(base.registeredSource);
const origin = base.validationOperation.body.originRegistryId;
const workspace = source.source.workspaceId;
const familyKey = canonical({ version: 'trial-family-v1', instrumentId: source.source.instrumentId });
const family = `trial-family-v1:${hash(familyKey)}`;
if (family !== source.source.familyId) throw Error('Family oracle mismatch');
const batches = []; const events = []; const receipts = [];
let chain = hash(`trial-ledger-v1:genesis:${origin}`);
for (const [request, strategies] of [
  ['prior', [0, 1].map(i => `strategy-v2:${hash(`prior-${i}`)}`)],
  ['original', source.candidates.map(c => c.strategyHash)],
]) {
  const rows = strategies.map((strategy, index) => {
    const candidateIndex = request === 'original' ? source.candidates[index].candidateIndex : index;
    const payload = { version: 'trial-event-v1', familyId: family, kind: 'variant', effective: true,
      idempotencyKey: `${workspace}:request:${request}:candidate:${candidateIndex}`,
      workspaceId: workspace, requestId: request, candidateIndex, legacyAttemptKey: null,
      hypothesisHash: `hypothesis-${request}-${index}`, strategyHash: strategy,
      datasetHash: 'dataset', snapshotId: 'snapshot', splitHash: `trial-split-v1:${'4'.repeat(64)}`,
      seedsHash: 'seeds', engineFingerprintHash: 'engine', benchmarkId: null,
      benchmarkParamsHash: null, reproductionOf: null };
    return { payload, eventId: `trial-event-v1:${hash(canonical(payload))}` };
  }).sort((a, b) => a.payload.idempotencyKey < b.payload.idempotencyKey ? -1 : 1);
  const members = rows.map(r => [r.payload.idempotencyKey, r.eventId]);
  const batchId = `trial-batch-v1:${hash(canonical({ version: 'trial-batch-v1', familyId: family, members }))}`;
  if (request === 'original') source.source.trialBatchId = batchId.slice('trial-batch-v1:'.length);
  batches.push({ type: 'batch', batchId, familyId: family, originRegistryId: origin, members });
  receipts.push({ type: 'receipt', batchId, receiptRegistryId: origin,
    familyEffectiveBefore: events.length, batchEffectiveTrials: rows.length });
  for (const row of rows) {
    chain = hash(chain + row.eventId);
    events.push({ type: 'event', ...row, originRegistryId: origin, batchId, seq: events.length + 1, chain });
  }
}
batches.sort((a, b) => a.batchId < b.batchId ? -1 : 1);
receipts.sort((a, b) => a.batchId < b.batchId ? -1 : 1);
const evidence = tests => ({ quarantinedFamilies: [], conflictedOrigins: [], familyTests: { [family]: tests } });
const cases = [];
for (const [name, tests, exportedAt] of [
  ['pinned', 2, '2026-10-17T00:00:00Z'],
  ['upgraded-same-head', 3, '2026-10-17T00:00:01Z'],
  ['same-facts-new-audit', 2, '2026-10-17T00:00:02Z'],
]) {
  const records = [{ type: 'family', familyId: family, familyKey,
    protocolJson: canonical({ correction: 'holm', testsPerTrial: 2 }) },
  ...(tests === 3 ? [{ type: 'protocolUpgrade', familyId: family, testsPerTrial: 3 }] : []),
  ...batches, ...events, ...receipts];
  const body = records.map(canonical).join('\n') + '\n';
  const header = { version: 'trial-ledger-export-v3', registryId: origin,
    schemaVersion: '0003_family_protocol_upgrades', exportedAt, familyCount: 1,
    batchCount: 2, eventCount: 4, headSeq: 4, headChain: chain, bodySha256: hash(body) };
  const trialExportText = canonical(header) + '\n' + body;
  const anchor = { registryId: origin, seq: 4, chainHead: chain, evidence: evidence(tests) };
  // This synthetic world has only trial rows, no alpha/confirmation records.
  const binding = { version: 'trial-ledger-binding-v5', ...anchor,
    alpha: { budgets: [], reservations: [], conflicts: [] }, alphaUnknownOrigins: [],
    confirmation: { batches: [], selections: [], operations: [], exposures: [],
      admissions: [], events: [], blobs: [], conflicts: [], unknownOrigins: [] } };
  const witnessText = canonical({ contractVersion: 'research-confirmation-source-witness-v1',
    trialExportText, registeredSource: source });
  cases.push({ name, trialExportText, archiveSha256: hash(trialExportText), anchor,
    bindingJson: canonical(binding), witnessText, witnessSha256: hash(witnessText),
    expected: { familyEffectiveTrials: 4, registeredBatchEffectiveTrials: 2,
      testsPerTrial: tests, completeCandidates: 2 } });
}
writeFileSync(new URL('../fixtures/rs-core/research-source-witness-v1.json', import.meta.url),
  JSON.stringify({ description: 'Independent synthetic original-source facts and outer codec bytes. Protocol 3 is archival data only; no statistical support, publisher/file authentication or execution rights.',
    registeredSource: source, cases }, null, 2) + '\n');
