import { describe, expect, it } from 'vitest';
import fixture from '../../../fixtures/rs-core/strategy-dsl-v1.json';
import { evaluateDSL } from './evaluator';
import { STRATEGY_DSL_VERSION } from './schema';
import { validateDSL } from './validator';

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
    }
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
