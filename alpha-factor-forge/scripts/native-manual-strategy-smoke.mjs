// NUMERIC-JSON-002a: independent JS number/identity evidence across real IPC,
// SQLite and policy-preserving preparation. The caller owns an isolated app.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';

const input = JSON.parse(await readFile(new URL('../fixtures/research/numeric-json-audit-v1.json', import.meta.url), 'utf8'));
const legacy = JSON.parse(await readFile(new URL('../fixtures/research/numeric-json-default-output.json', import.meta.url), 'utf8'));

function count(value) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32BE(value);
  return bytes;
}

function string(value) {
  const bytes = Buffer.from(value, 'utf8');
  return Buffer.concat([count(bytes.length), bytes]);
}

// Independent reference for the already-fixed strategy-v2 binary contract;
// numeric expectations come from JavaScript and the retained audit fixture.
function canonical(value) {
  if (value === null) return Buffer.from([0]);
  if (typeof value === 'boolean') return Buffer.from([value ? 2 : 1]);
  if (typeof value === 'number') {
    assert(Number.isFinite(value));
    const bytes = Buffer.alloc(9);
    bytes[0] = 3;
    bytes.writeDoubleBE(value === 0 ? 0 : value, 1);
    return bytes;
  }
  if (typeof value === 'string') return Buffer.concat([Buffer.from([4]), string(value)]);
  if (Array.isArray(value)) return Buffer.concat([Buffer.from([5]), count(value.length), ...value.map(canonical)]);
  const keys = Object.keys(value).sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return Buffer.concat([Buffer.from([6]), count(keys.length), ...keys.flatMap(key => [string(key), canonical(value[key])])]);
}

function hash(definition) {
  return 'strategy-v2:' + createHash('sha256').update('strategy-v2\0').update(canonical({
    definition,
    execModel: { feePct: definition.feePct, slippagePct: definition.slipPct },
  })).digest('hex');
}

function bits(value) {
  const bytes = Buffer.alloc(8);
  bytes.writeDoubleBE(value);
  return bytes.toString('hex');
}

function marked(definition) {
  return { ...definition, definitionVersion: 'manual-strategy-definition-v1', numericPolicy: 'json-f64-roundtrip-v1' };
}

function row(definition, name, parent = null) {
  return {
    name, type: definition.mode, dsl_json: null,
    original_definition_json: JSON.stringify(definition), param_schema_json: null,
    source: 'manual', ai_prompt_hash: null, strategy_hash: hash(definition),
    lifecycle: 'candidate', parent_strategy_id: parent,
  };
}

export async function verifyManualStrategyNumbers(page) {
  // Verify the reference against immutable existing identity evidence first.
  for (const item of input.cases) assert.equal(hash(JSON.parse(item.definitionJson)), item.expectedJsStrategyHash);
  const nonce = randomUUID();
  const rows = input.cases.flatMap(item => ['params', 'blocks', 'code'].map(mode => {
    const definition = marked({ ...JSON.parse(item.definitionJson), mode });
    return { row: row(definition, `numeric-${nonce}-${item.id}-${mode}`), bits: item.expectedJsBits };
  }));
  const legacyCase = input.cases.find(item => item.id === 'number-3');
  const legacyReport = legacy.cases.find(item => item.id === legacyCase.id);
  const source = {
    ...row(JSON.parse(legacyCase.definitionJson), `legacy-${nonce}`),
    original_definition_json: legacyCase.definitionJson,
    strategy_hash: legacyReport.strategyHash,
  };
  const result = await page.evaluate(async ({ rows, source }) => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const ids = [];
    const prepared = [];
    for (const item of rows) {
      const id = await invoke('save_strategy', { strategy: item.row });
      ids.push(id);
      prepared.push(await invoke('prepare_saved_strategy', { strategyId: id }));
      if (await invoke('save_strategy', { strategy: item.row }) !== id) throw new Error('re-save changed row id');
    }
    const sourceId = await invoke('save_strategy', { strategy: source });
    const legacyPrepared = await invoke('prepare_saved_strategy', { strategyId: sourceId });
    return { ids, prepared, sourceId, legacyPrepared, saved: await invoke('get_strategies') };
  }, { rows, source });
  assert.equal(new Set(result.ids).size, rows.length);
  for (let index = 0; index < rows.length; index++) {
    const prepared = result.prepared[index];
    const saved = result.saved.find(item => item.id === result.ids[index]);
    assert.equal(prepared.sourceStrategyId, saved.id);
    assert.equal(prepared.sourceStrategyHash, rows[index].row.strategy_hash);
    assert.equal(prepared.numericPolicy, 'json-f64-roundtrip-v1');
    assert.equal(bits(JSON.parse(prepared.interpretedDefinitionJson).feePct), rows[index].bits);
    assert.equal(saved.original_definition_json, rows[index].row.original_definition_json);
  }
  assert.equal(result.legacyPrepared.numericPolicy, 'serde-json-default-v1');
  assert.equal(bits(JSON.parse(result.legacyPrepared.interpretedDefinitionJson).feePct), legacyReport.parsedBits);
  const legacySaved = result.saved.find(item => item.id === result.sourceId);
  assert.equal(legacySaved.original_definition_json, source.original_definition_json);
  assert.equal(legacySaved.strategy_hash, source.strategy_hash);

  const child = row(marked(JSON.parse(result.legacyPrepared.interpretedDefinitionJson)), `copy-${nonce}`, result.sourceId);
  const unknown = row({ ...marked(JSON.parse(legacyCase.definitionJson)), numericPolicy: 'unknown-v99' }, `invalid-${nonce}`);
  const copied = await page.evaluate(async ({ child, unknown, sourceId, otherParentId }) => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const childId = await invoke('save_strategy', { strategy: child });
    const before = await invoke('get_strategies');
    let policyError;
    let parentError;
    let wrongKeyError;
    try { await invoke('save_strategy', { strategy: unknown }); } catch (error) { policyError = String(error); }
    try { await invoke('save_strategy', { strategy: { ...child, parent_strategy_id: otherParentId } }); } catch (error) { parentError = String(error); }
    try { await invoke('prepare_saved_strategy', { strategy_id: sourceId }); } catch (error) { wrongKeyError = String(error); }
    return { childId, before, after: await invoke('get_strategies'), policyError, parentError, wrongKeyError };
  }, { child, unknown, sourceId: result.sourceId, otherParentId: result.ids[0] });
  assert.match(copied.policyError, /unsupported.*(?:numericPolicy|policy)/i);
  assert(copied.parentError, 'a conflicting copy parent must be rejected');
  assert.match(copied.wrongKeyError, /strategyId/);
  assert.deepEqual(copied.after, copied.before, 'rejected policy/lineage writes must leave rows unchanged');
  assert.equal(copied.after.find(item => item.id === copied.childId).parent_strategy_id, result.sourceId);
  assert.equal(copied.after.find(item => item.id === result.sourceId).original_definition_json, source.original_definition_json);
  return {
    status: 'passed', markedCases: rows.length, originalLegacyHash: source.strategy_hash,
    originalLegacyBits: legacyReport.parsedBits, roundedBits: legacyCase.expectedJsBits,
    sourceId: result.sourceId, childId: copied.childId, rejectedPolicy: true, rejectedConflictingParent: true,
  };
}
