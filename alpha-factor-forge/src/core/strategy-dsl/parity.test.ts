import { describe, expect, it } from 'vitest';
import fixture from '../../../fixtures/rs-core/strategy-dsl-v1.json';
import { evaluateDSL } from './evaluator';
import { INDICATOR_WHITELIST, OPERATOR_WHITELIST, STRATEGY_DSL_VERSION } from './schema';
import { validateDSL } from './validator';

function collectVocabulary(node: unknown, indicators: Set<string>, operators: Set<string>): void {
  if (node === null || typeof node !== 'object' || Array.isArray(node)) return;
  const object = node as Record<string, unknown>;
  if (typeof object.ind === 'string') indicators.add(object.ind);
  if (typeof object.op === 'string') operators.add(object.op);
  if (Array.isArray(object.args)) {
    object.args.forEach((child) => collectVocabulary(child, indicators, operators));
  }
}

describe('strategy-dsl-v1 authored TS/Rust parity fixture', () => {
  it('pins the contract and exact executable signal cases', () => {
    expect(fixture.contractVersion).toBe(STRATEGY_DSL_VERSION);
    for (const testCase of fixture.validCases) {
      const validation = validateDSL(testCase.dsl);
      expect(validation, testCase.id).toMatchObject({
        ok: true,
        nodeCount: testCase.expected.nodeCount,
        depth: testCase.expected.depth,
        maxLookbackBars: testCase.expected.maxLookbackBars,
        parameterCount: testCase.expected.parameterCount,
      });
      const evaluated = evaluateDSL(fixture.candles, testCase.dsl);
      expect(evaluated.signals.entry, testCase.id).toEqual(testCase.expected.entry);
      expect(evaluated.signals.exit, testCase.id).toEqual(testCase.expected.exit);
      expect(new Set(evaluated.signals.entry), `${testCase.id} entry`).toEqual(new Set([false, true]));
      expect(new Set(evaluated.signals.exit), `${testCase.id} exit`).toEqual(new Set([false, true]));
    }
  });

  it('covers every executable v1 indicator and operator family', () => {
    const indicators = new Set<string>();
    const operators = new Set<string>();
    fixture.validCases.forEach((testCase) => {
      collectVocabulary(testCase.dsl.entry, indicators, operators);
      collectVocabulary(testCase.dsl.exit, indicators, operators);
    });
    expect([...indicators].sort()).toEqual([...INDICATOR_WHITELIST].sort());
    expect([...operators].sort()).toEqual([...OPERATOR_WHITELIST].sort());
  });

  it('rejects the shared illegal matrix before evaluation', () => {
    for (const testCase of fixture.invalidCases) {
      const validation = validateDSL(testCase.dsl);
      expect(validation.ok, testCase.id).toBe(false);
      expect(validation.errors.some((error) => error.includes(testCase.errorContains)), testCase.id).toBe(true);
      expect(() => evaluateDSL(fixture.candles, testCase.dsl), testCase.id).toThrow();
    }
  });
});
