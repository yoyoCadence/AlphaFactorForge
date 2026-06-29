import { describe, it, expect } from 'vitest';
import { compileExpression, isTruthy } from './exprInterpreter';

/** Compile + evaluate a constant (no variables) expression. */
const evalConst = (src: string): number => compileExpression(src, []).evaluate({}, 1);
/** Compile + evaluate with variable series at bar i. */
const evalAt = (src: string, vars: string[], series: Record<string, number[]>, i: number): number =>
  compileExpression(src, vars).evaluate(series, i);

describe('exprInterpreter — arithmetic & precedence', () => {
  it('respects operator precedence and parens', () => {
    expect(evalConst('1 + 2 * 3')).toBe(7);
    expect(evalConst('(1 + 2) * 3')).toBe(9);
    expect(evalConst('10 / 2 - 1')).toBe(4);
    expect(evalConst('2 * -3')).toBe(-6);
  });

  it('handles unary - and !', () => {
    expect(evalConst('-5')).toBe(-5);
    expect(evalConst('!0')).toBe(1);
    expect(evalConst('!5')).toBe(0);
    expect(evalConst('!(1 > 2)')).toBe(1);
  });
});

describe('exprInterpreter — comparisons & logic (1/0)', () => {
  it('comparisons yield 1/0', () => {
    expect(evalConst('2 > 1')).toBe(1);
    expect(evalConst('2 < 1')).toBe(0);
    expect(evalConst('3 == 3')).toBe(1);
    expect(evalConst('3 != 3')).toBe(0);
    expect(evalConst('2 >= 2')).toBe(1);
    expect(evalConst('1 <= 0')).toBe(0);
  });

  it('&& and || short-circuit on truthiness', () => {
    expect(evalConst('1 && 1')).toBe(1);
    expect(evalConst('1 && 0')).toBe(0);
    expect(evalConst('0 || 0')).toBe(0);
    expect(evalConst('0 || 5')).toBe(1);
    expect(evalConst('!0 && 1')).toBe(1);
  });
});

describe('exprInterpreter — variables & NaN', () => {
  it('reads variable series at the bar index', () => {
    expect(evalAt('rsi < 30', ['rsi'], { rsi: [NaN, 25, 40] }, 1)).toBe(1);
    expect(evalAt('rsi < 30', ['rsi'], { rsi: [NaN, 25, 40] }, 2)).toBe(0);
  });

  it('treats NaN operands in comparisons as false', () => {
    expect(evalAt('rsi > 0', ['rsi'], { rsi: [NaN, 1] }, 0)).toBe(0);
  });

  it('non-finite results are not truthy', () => {
    expect(isTruthy(evalConst('1 / 0'))).toBe(false);
    expect(isTruthy(evalConst('0'))).toBe(false);
    expect(isTruthy(evalConst('3'))).toBe(true);
  });
});

describe('exprInterpreter — whitelisted functions', () => {
  it('prev(x) reads one bar back; out of range -> NaN', () => {
    expect(evalAt('prev(price)', ['price'], { price: [10, 20, 30] }, 2)).toBe(20);
    expect(Number.isNaN(evalAt('prev(price)', ['price'], { price: [10, 20, 30] }, 0))).toBe(true);
  });

  it('crossUp / crossDown match the cross semantics', () => {
    expect(evalAt('crossUp(a, b)', ['a', 'b'], { a: [1, 1, 3], b: [2, 2, 2] }, 2)).toBe(1);
    expect(evalAt('crossUp(a, b)', ['a', 'b'], { a: [1, 1, 3], b: [2, 2, 2] }, 1)).toBe(0);
    expect(evalAt('crossDown(a, b)', ['a', 'b'], { a: [3, 3, 1], b: [2, 2, 2] }, 2)).toBe(1);
  });
});

describe('exprInterpreter — security: rejects everything off-whitelist', () => {
  const rejected = [
    'rsi = 1', // assignment
    'foo(1)', // unknown function
    'bar', // unknown variable
    'rsi.x', // member access
    'rsi[0]', // indexing
    '1 ? 2 : 3', // ternary
    '"abc"', // string
    'eval(1)', // unknown function (eval is not whitelisted)
    'NaN', // unknown variable (no NaN literal)
    'Infinity', // unknown variable (no Infinity literal)
    '1 & 1', // bitwise (lone &)
    '1 | 1', // bitwise (lone |)
    '1 +', // unexpected end
    '1 2', // trailing token
    '', // empty
  ];
  for (const src of rejected) {
    it(`rejects ${JSON.stringify(src)}`, () => {
      expect(() => compileExpression(src, ['rsi'])).toThrow(/invalid expression/);
    });
  }

  it('rejects wrong function arity', () => {
    expect(() => compileExpression('prev(price, 2)', ['price'])).toThrow(/invalid expression/);
    expect(() => compileExpression('crossUp(a)', ['a'])).toThrow(/invalid expression/);
  });

  it('rejects nested time-shift (max 1-bar lookback)', () => {
    expect(() => compileExpression('prev(prev(price))', ['price'])).toThrow(/1-bar|nested/);
    expect(() => compileExpression('crossUp(prev(a), b)', ['a', 'b'])).toThrow(/1-bar|nested/);
  });
});

describe('exprInterpreter — caps', () => {
  it('rejects over-long source', () => {
    const long = '1' + '+1'.repeat(600); // ~1201 chars
    expect(() => compileExpression(long, [])).toThrow(/too long/);
  });

  it('rejects too many nodes', () => {
    expect(() => compileExpression('1+1+1', [], { maxLen: 1000, maxNodes: 2, maxDepth: 16 })).toThrow(/too many nodes/);
  });

  it('rejects too-deep nesting', () => {
    const deep = '!'.repeat(20) + '1'; // depth ~21 > 16
    expect(() => compileExpression(deep, [])).toThrow(/too deeply nested/);
  });
});
