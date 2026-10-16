// Independent Node encoder: no Rust, TypeScript encoder or provider calls.
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';

function canonical(value) {
  const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32BE(n); return b; };
  const str = s => { const b = Buffer.from(s); return Buffer.concat([u32(b.length), b]); };
  if (value === null) return Buffer.from([0]);
  if (typeof value === 'boolean') return Buffer.from([value ? 2 : 1]);
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value)) throw new Error('Fixture integer outside safe domain');
    const b = Buffer.alloc(9); b[0] = 3; b.writeDoubleBE(value, 1); return b;
  }
  if (typeof value === 'string') return Buffer.concat([Buffer.from([4]), str(value)]);
  if (Array.isArray(value)) return Buffer.concat([Buffer.from([5]), u32(value.length), ...value.map(canonical)]);
  const keys = Object.keys(value).sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return Buffer.concat([Buffer.from([6]), u32(keys.length), ...keys.flatMap(k => [str(k), canonical(value[k])])]);
}
function record(body) {
  const id = createHash('sha256').update(body.contractVersion).update(Buffer.from([0]))
    .update(canonical(body)).digest('hex');
  return { id, body };
}
const t = 946684800000;
const scope = record({
  contractVersion: 'research-train-scope-v1', workspaceId: '1'.repeat(32), runId: 42,
  campaignId: '3'.repeat(64), instrumentId: 'crypto:binance:BTCUSDT',
  trainDataHash: 'dataset-content-v2:' + '2'.repeat(64),
  trainWindow: { fromMs: t, toExclusiveMs: t + 10000 }, engineManifestHash: '4'.repeat(64),
});
const origins = [
  { kind: 'summary', observations: 256, closedTrades: 12, netReturnPpm: -12500 },
  { kind: 'hypothesis', mechanism: 'mean_reversion', invalidation: 'costs_dominate' },
  { kind: 'failure', class: 'no_trades' },
].map(fact => record({
  contractVersion: 'research-train-origin-v1', scopeId: scope.id, provenance: 'train_only',
  inputs: [{ provenance: 'train_only', instrumentId: scope.body.instrumentId,
    trainDataHash: scope.body.trainDataHash, support: [{ fromMs: t + 100, toExclusiveMs: t + 9000 }] }], fact,
})).sort((a, b) => a.id.localeCompare(b.id));
const request = { contractVersion: 'research-train-context-request-v1', scopeId: scope.id,
  originIds: origins.map(v => v.id) };
const context = record({ contractVersion: 'research-train-context-v1', scopeId: scope.id,
  scope: scope.body, facts: origins.map(v => ({ originId: v.id, fact: v.body.fact })) });
const fixture = {
  description: 'Synthetic Train-only allowlist and independent canonical identities; no provider or calibration claim.',
  scope, origins, request, context,
};
writeFileSync(new URL('../fixtures/rs-core/research-train-context-v1.json', import.meta.url), JSON.stringify(fixture, null, 2) + '\n');
console.log(JSON.stringify({ scope: scope.id, origins: origins.map(v => v.id), context: context.id }));
