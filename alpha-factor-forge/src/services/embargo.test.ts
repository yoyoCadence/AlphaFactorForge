import { describe, it, expect } from 'vitest';
import { planValidationSplit } from '../core/validation/split';
import { defaultStrategy, type ParamsStrategy } from './strategy';
import { deriveEmbargoBars, maxSignalLookbackBars } from './embargo';

const base = defaultStrategy();

describe('maxSignalLookbackBars — params mode', () => {
  it('MA cross reads max(fast, slow) + 1 bars', () => {
    // defaults: fastMA 9 / slowMA 21, maCrossUp / maCrossDown.
    expect(maxSignalLookbackBars(base)).toBe(22);
  });

  it('takes the max of entry and exit signals', () => {
    const strat: ParamsStrategy = { ...base, exitSig: 'macdCrossDown' };
    // macd signal warm-up max(12, 26) + 9 - 1 = 34, + 1 for the cross.
    expect(maxSignalLookbackBars(strat)).toBe(35);
  });

  it('uses each signal family’s real warm-up convention', () => {
    expect(
      maxSignalLookbackBars({ ...base, entrySig: 'rsiOversold', exitSig: 'rsiOverbought' }),
    ).toBe(16); // rsi(14) needs 15 bars, + 1 for the threshold cross
    expect(
      maxSignalLookbackBars({ ...base, entrySig: 'bbLowerTouch', exitSig: 'bbUpperTouch' }),
    ).toBe(20); // plain compare: bbands(20), no cross bonus
    expect(
      maxSignalLookbackBars({ ...base, entrySig: 'priceAboveSlow', exitSig: 'priceBelowSlow' }),
    ).toBe(21); // plain compare against sma(21)
  });

  it('is usage-aware: unused configured periods never count or throw', () => {
    // emaPeriod 0 would be invalid — but MA-cross signals never read it.
    expect(maxSignalLookbackBars({ ...base, emaPeriod: 0 })).toBe(22);
    expect(maxSignalLookbackBars({ ...base, emaPeriod: 500 })).toBe(22);
  });

  it('fails closed on unsupported stoch signals and invalid used periods', () => {
    expect(() => maxSignalLookbackBars({ ...base, entrySig: 'stochOversold' })).toThrow(/stoch/i);
    expect(() => maxSignalLookbackBars({ ...base, fastMA: 0 })).toThrow(RangeError);
    expect(() => maxSignalLookbackBars({ ...base, slowMA: 2.5 })).toThrow(RangeError);
  });
});

describe('maxSignalLookbackBars — blocks mode', () => {
  const blocks = (entryRules: ParamsStrategy['entryRules'], exitRules: ParamsStrategy['exitRules']): ParamsStrategy =>
    ({ ...base, mode: 'blocks', entryRules, exitRules });

  it('reads only the operands the rules reference; constants contribute 0', () => {
    const strat = blocks(
      [{ l: 'maFast', op: 'crossUp', r: 'maSlow' }],
      [{ l: 'rsi', op: '<', r: '30' }],
    );
    expect(maxSignalLookbackBars(strat)).toBe(22); // max(9,21)+1 vs rsi 15
  });

  it('macdSignal/macdHist operands include the signal-line warm-up', () => {
    const strat = blocks([{ l: 'macdHist', op: '>', r: '0' }], []);
    expect(maxSignalLookbackBars(strat)).toBe(34); // max(12,26)+9-1, no cross
  });

  it('ignores unknown right operands and clamps empty rule lists to 1', () => {
    expect(maxSignalLookbackBars(blocks([{ l: 'price', op: '>', r: 'nonsense' }], []))).toBe(1);
    expect(maxSignalLookbackBars(blocks([], []))).toBe(1);
  });
});

describe('maxSignalLookbackBars — code mode', () => {
  const code = (entryCode: string, exitCode = '0'): ParamsStrategy =>
    ({ ...base, mode: 'code', entryCode, exitCode });

  it('walks the AST: cross of macd lines includes the signal warm-up + 1', () => {
    // default code expressions: crossUp/crossDown(macd, macdSignal).
    expect(maxSignalLookbackBars({ ...base, mode: 'code' })).toBe(35);
  });

  it('prev() adds one bar to its argument’s lookback', () => {
    expect(maxSignalLookbackBars(code('prev(rsi) > 50'))).toBe(16);
    expect(maxSignalLookbackBars(code('price > bbUpper'))).toBe(20);
  });

  it('fails closed on invalid expressions', () => {
    expect(() => maxSignalLookbackBars(code('rsi >'))).toThrow(/invalid expression/);
  });
});

describe('deriveEmbargoBars', () => {
  it('adds the explicit holding allowance and returns a recordable breakdown', () => {
    expect(deriveEmbargoBars(base, 0)).toEqual({
      embargoBars: 22,
      maxSignalLookbackBars: 22,
      holdingAllowanceBars: 0,
    });
    expect(deriveEmbargoBars(base, 8).embargoBars).toBe(30);
  });

  it('rejects a negative or non-integer allowance', () => {
    expect(() => deriveEmbargoBars(base, -1)).toThrow(RangeError);
    expect(() => deriveEmbargoBars(base, 1.5)).toThrow(RangeError);
  });

  it('produces an embargo planValidationSplit accepts (integration sanity)', () => {
    const { embargoBars } = deriveEmbargoBars(base, 5);
    const plan = planValidationSplit(600, embargoBars);
    expect(plan.embargoBars).toBe(27);
    expect(plan.trainValidationEmbargo?.count).toBe(27);
  });
});
