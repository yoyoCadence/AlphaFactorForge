import { describe, expect, it } from 'vitest';
import { canonicalBytes, strategyHash, strategyHashFromDefinitionJson } from '../core/hashing';
import { defaultStrategy } from './strategy';
import { buildStrategyDef } from './strategyRecord';
import { strategyFromDef, strategyFromPrepared } from './strategyLibrary';
import {
  assertMockLegacyNumericSupport, LEGACY_NUMERIC_POLICY, MANUAL_DEFINITION_VERSION,
  MANUAL_NUMERIC_POLICY, manualNumericPolicy, manualStrategyDefinition,
} from './manualStrategyDefinition';
import fixture from '../../fixtures/research/numeric-json-manual-v1.json';
import type { PreparedSavedStrategy } from '../tauri-client/commands';

describe('versioned manual numeric definitions', () => {
  it.each(fixture.cases)('$id locks the numeric bits, marked bytes and durable hash without changing legacy evidence', async (row) => {
    const definition = JSON.parse(row.definitionJson);
    const bytes = new ArrayBuffer(8);
    new DataView(bytes).setFloat64(0, definition.feePct, false);
    expect(new DataView(bytes).getBigUint64(0, false).toString(16).padStart(16, '0')).toBe(row.expectedJsBits);
    expect(manualNumericPolicy(definition)).toBe(MANUAL_NUMERIC_POLICY);
    expect(definition.definitionVersion).toBe(MANUAL_DEFINITION_VERSION);
    expect(Array.from(canonicalBytes(definition), (byte) => byte.toString(16).padStart(2, '0')).join('')).toBe(row.expectedJsCanonical);
    expect(await strategyHashFromDefinitionJson(row.definitionJson)).toBe(row.expectedJsStrategyHash);
    const strat = strategyFromDef({
      name: '', type: definition.mode, source: 'manual', lifecycle: 'candidate',
      original_definition_json: row.definitionJson, strategy_hash: row.expectedJsStrategyHash,
    });
    const saved = await buildStrategyDef(strat, 'Exact');
    expect(JSON.parse(saved.original_definition_json)).toEqual(definition);
    expect(saved.strategy_hash).toBe(row.expectedJsStrategyHash);
    expect(Object.keys(strat)).not.toContain('numericPolicy');
    expect(Object.keys(strat)).not.toContain('definitionVersion');
  });

  it('binds both markers to the hash and rejects partial or unknown policies', async () => {
    const strategy = defaultStrategy();
    const marked = manualStrategyDefinition(strategy);
    const exec = { feePct: strategy.feePct, slippagePct: strategy.slipPct };
    expect(await strategyHash(marked, exec)).not.toBe(await strategyHash(strategy, exec));
    expect(manualNumericPolicy(strategy)).toBe(LEGACY_NUMERIC_POLICY);
    for (const bad of [
      { ...strategy, definitionVersion: MANUAL_DEFINITION_VERSION },
      { ...strategy, numericPolicy: MANUAL_NUMERIC_POLICY },
      { ...marked, definitionVersion: 'future-version' },
      { ...marked, numericPolicy: 'future-policy' },
      { ...strategy, definitionVersion: null, numericPolicy: null },
    ]) expect(() => manualNumericPolicy(bad)).toThrow(/數值版本/);
  });

  it('preserves a marked same-hash row verbatim, including its immutable parent and lifecycle', async () => {
    const original = await buildStrategyDef(defaultStrategy(), 'Original');
    const reordered = JSON.stringify(Object.fromEntries(Object.entries(JSON.parse(original.original_definition_json)).reverse()));
    const source = { ...original, id: 7, original_definition_json: reordered, parent_strategy_id: 2, lifecycle: 'validated' as const };
    expect(await buildStrategyDef(defaultStrategy(), 'Renamed', source)).toEqual({ ...source, name: 'Renamed', source: 'manual' });
    const changed = await buildStrategyDef({ ...defaultStrategy(), fastMA: 11 }, 'Changed', source);
    expect(changed.strategy_hash).not.toBe(source.strategy_hash);
    expect(changed.parent_strategy_id).toBe(7);
    expect(changed.id).toBeUndefined();
  });

  it('converts a legacy source additively, retains the original row and refuses an invalid source id', async () => {
    const strat = defaultStrategy();
    const json = JSON.stringify(strat);
    const source = {
      ...await buildStrategyDef(strat, 'Legacy'), id: 9, original_definition_json: json,
      strategy_hash: await strategyHashFromDefinitionJson(json), parent_strategy_id: 3,
    };
    const before = structuredClone(source);
    const next = await buildStrategyDef(strat, 'Converted', source);
    expect(next.strategy_hash).not.toBe(source.strategy_hash);
    expect(next.parent_strategy_id).toBe(9);
    expect(next.original_definition_json).not.toBe(json);
    expect(source).toEqual(before);
    await expect(buildStrategyDef(strat, 'Bad source', { ...source, id: 0 })).rejects.toThrow(/識別碼/);
    await expect(buildStrategyDef(strat, 'Bad source', { ...next, id: 7, original_definition_json: json })).rejects.toThrow(/不一致/);
  });

  it('requires the prepared view to identify the selected source and its declared policy', async () => {
    const source = { ...await buildStrategyDef(defaultStrategy(), 'Saved'), id: 12 };
    const prepared: PreparedSavedStrategy = {
      sourceStrategyId: 12, sourceStrategyHash: source.strategy_hash,
      numericPolicy: MANUAL_NUMERIC_POLICY, interpretedDefinitionJson: source.original_definition_json,
    };
    expect(strategyFromPrepared(source, prepared)).toEqual(defaultStrategy());
    expect(() => strategyFromPrepared(source, { ...prepared, sourceStrategyId: 13 })).toThrow(/來源策略不一致/);
    expect(() => strategyFromPrepared(source, { ...prepared, sourceStrategyHash: 'wrong' })).toThrow(/來源策略不一致/);
    expect(() => strategyFromPrepared(source, { ...prepared, numericPolicy: LEGACY_NUMERIC_POLICY })).toThrow(/數值版本不一致/);
    const legacy = { ...source, original_definition_json: JSON.stringify(defaultStrategy()) };
    // The backend's legacy interpretation is used, even when a decimal's
    // correctly rounded interpretation would be a different number.
    const interpreted = { ...defaultStrategy(), feePct: 0.003694444444444444 };
    const loaded = strategyFromPrepared(legacy, {
      ...prepared, numericPolicy: LEGACY_NUMERIC_POLICY,
      interpretedDefinitionJson: JSON.stringify(interpreted),
    });
    expect(loaded.feePct).toBe(interpreted.feePct);
  });

  it('keeps the mock honest about legacy decimals and ignores JSON string contents', () => {
    expect(() => assertMockLegacyNumericSupport('{"period":20,"feePct":0.05,"code":"0.0036944444444444438"}')).not.toThrow();
    expect(() => assertMockLegacyNumericSupport('{"feePct":0.0036944444444444438}')).toThrow(/桌面程式/);
    expect(() => assertMockLegacyNumericSupport('{"feePct":3.6944444444444438e-3}')).toThrow(/桌面程式/);
  });
});
