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
});
