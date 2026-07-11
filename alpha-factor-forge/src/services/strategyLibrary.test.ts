import { describe, expect, it } from 'vitest';
import type { StrategyDef } from '../tauri-client/commands';
import { defaultStrategy } from './strategy';
import { strategyFromDef } from './strategyLibrary';

function def(overrides: Partial<StrategyDef> = {}): StrategyDef {
  const strategy = defaultStrategy();
  return {
    name: 'Saved MA',
    type: strategy.mode,
    original_definition_json: JSON.stringify(strategy),
    source: 'manual',
    strategy_hash: 'hash',
    lifecycle: 'candidate',
    ...overrides,
  };
}

describe('strategyFromDef', () => {
  it('round-trips a complete saved strategy without changing its values', () => {
    const strategy = { ...defaultStrategy(), fastMA: 7, direction: 'both' as const };
    expect(strategyFromDef(def({ original_definition_json: JSON.stringify(strategy) }))).toEqual(strategy);
  });

  it('rejects malformed JSON and non-finite numeric fields', () => {
    expect(() => strategyFromDef(def({ original_definition_json: '{' }))).toThrow(/有效 JSON/);
    const bad = { ...defaultStrategy(), fastMA: '7' };
    expect(() => strategyFromDef(def({ original_definition_json: JSON.stringify(bad) }))).toThrow(/fastMA/);
  });

  it('rejects unsupported DSL rows and mismatched stored types', () => {
    expect(() => strategyFromDef(def({ type: 'dsl' }))).toThrow(/尚不能載入/);
    expect(() => strategyFromDef(def({ type: 'blocks' }))).toThrow(/不一致/);
  });

  it('loads legacy params rows saved before rule and code fields existed', () => {
    const legacy = { ...defaultStrategy(), fastMA: 7 } as Partial<ReturnType<typeof defaultStrategy>>;
    delete legacy.entryRules;
    delete legacy.exitRules;
    delete legacy.entryCode;
    delete legacy.exitCode;

    expect(strategyFromDef(def({ original_definition_json: JSON.stringify(legacy) }))).toEqual({
      ...defaultStrategy(),
      fastMA: 7,
    });
  });

  it('loads legacy blocks rows saved before code fields existed', () => {
    const legacy = {
      ...defaultStrategy(),
      mode: 'blocks' as const,
      entryRules: [{ l: 'price' as const, op: '>' as const, r: '100' }],
      exitRules: [{ l: 'rsi' as const, op: '>' as const, r: '70' }],
    } as Partial<ReturnType<typeof defaultStrategy>>;
    delete legacy.entryCode;
    delete legacy.exitCode;

    const loaded = strategyFromDef(def({
      type: 'blocks',
      original_definition_json: JSON.stringify(legacy),
    }));
    expect(loaded.entryRules).toEqual(legacy.entryRules);
    expect(loaded.exitRules).toEqual(legacy.exitRules);
    expect(loaded.entryCode).toBe(defaultStrategy().entryCode);
    expect(loaded.exitCode).toBe(defaultStrategy().exitCode);
  });

  it('still rejects missing active fields and partial historical shapes', () => {
    const blocksWithoutRules = { ...defaultStrategy(), mode: 'blocks' as const } as Partial<ReturnType<typeof defaultStrategy>>;
    delete blocksWithoutRules.entryRules;
    delete blocksWithoutRules.exitRules;
    expect(() => strategyFromDef(def({
      type: 'blocks',
      original_definition_json: JSON.stringify(blocksWithoutRules),
    }))).toThrow(/entryRules/);

    const codeWithoutExpressions = { ...defaultStrategy(), mode: 'code' as const } as Partial<ReturnType<typeof defaultStrategy>>;
    delete codeWithoutExpressions.entryCode;
    delete codeWithoutExpressions.exitCode;
    expect(() => strategyFromDef(def({
      type: 'code',
      original_definition_json: JSON.stringify(codeWithoutExpressions),
    }))).toThrow(/程式碼欄位/);

    const paramsWithOneCodeField = { ...defaultStrategy(), entryCode: undefined };
    expect(() => strategyFromDef(def({
      original_definition_json: JSON.stringify(paramsWithOneCodeField),
    }))).toThrow(/程式碼欄位/);
  });
});
