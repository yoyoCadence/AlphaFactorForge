import { describe, expect, it } from 'vitest';
import { blurCommitValue } from './NumberInput';

// BUG-SWEEP-BLUR-001 — the value a blur commits. The component reports it to
// its parent only when it differs from the current value, so these cases are
// also the list of blurs that must NOT count as an edit.
describe('blurCommitValue', () => {
  it('returns the current value when the draft already shows it', () => {
    expect(blurCommitValue('5', 5)).toBe(5);
    expect(blurCommitValue('5.0', 5)).toBe(5);
    expect(blurCommitValue('0.05', 0.05)).toBe(0.05);
    expect(blurCommitValue('1e3', 1000)).toBe(1000);
  });

  it('falls back to the current value for a cleared or unparseable draft', () => {
    for (const draft of ['', ' ', '-', '.', 'abc']) {
      expect(blurCommitValue(draft, 7)).toBe(7);
      expect(blurCommitValue(draft, 7, 1, 10)).toBe(7);
    }
  });

  it('keeps the parsed prefix of a partial draft', () => {
    // `parseFloat` semantics, unchanged: what the live path already sent.
    expect(blurCommitValue('5.', 5)).toBe(5);
    expect(blurCommitValue('12abc', 3)).toBe(12);
  });

  it('clamps to the minimum, inclusive', () => {
    expect(blurCommitValue('4', 4, 5, 90)).toBe(5); // MIN - 1
    expect(blurCommitValue('5', 5, 5, 90)).toBe(5); // MIN
    expect(blurCommitValue('6', 6, 5, 90)).toBe(6); // MIN + 1
  });

  it('clamps to the maximum, inclusive', () => {
    expect(blurCommitValue('89', 89, 5, 90)).toBe(89); // MAX - 1
    expect(blurCommitValue('90', 90, 5, 90)).toBe(90); // MAX
    expect(blurCommitValue('91', 91, 5, 90)).toBe(90); // MAX + 1
  });

  it('applies a one-sided bound on its own', () => {
    expect(blurCommitValue('-3', -3, 0)).toBe(0);
    expect(blurCommitValue('1e9', 1e9, 0)).toBe(1e9);
    expect(blurCommitValue('250', 250, undefined, 100)).toBe(100);
    expect(blurCommitValue('-250', -250, undefined, 100)).toBe(-250);
  });

  it('clamps the fallback too, so an out-of-range value is corrected on blur', () => {
    expect(blurCommitValue('', 0, 1)).toBe(1);
    expect(blurCommitValue('', 150, 0, 100)).toBe(100);
  });

  it('never invents a number for a non-finite draft', () => {
    expect(blurCommitValue('Infinity', 3)).toBe(3);
    expect(blurCommitValue('NaN', 3)).toBe(3);
  });
});
