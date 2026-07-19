// VAL-003: usage-aware embargo derivation for the validation split.
//
// The VAL-001 contract leaves `embargoBars` to the caller and requires it to be
// derived explicitly and recorded for reproducibility. This module implements
// that derivation: embargoBars = the maximum number of history bars any signal
// the strategy ACTUALLY uses can read (its slowest indicator warm-up, plus one
// bar for prev/cross semantics) + a caller-approved holding-period allowance.
//
// Lookback conventions (bars of history, i.e. the first defined output index
// is lookback - 1, matching core/indicators warm-up):
//   sma(p)/ema(p)/bbands(p)  -> p        (ema seeds with SMA(p); its longer
//                                         exponential tail is accepted as the
//                                         recorded v1 convention)
//   rsi(p)                   -> p + 1    (p changes need p + 1 bars)
//   macd line                -> max(fast, slow)
//   macd signal / hist       -> max(fast, slow) + signalPeriod - 1
//   price/open/high/low/vol  -> 1
//   crossUp/crossDown/prev   -> + 1     (they read bar i - 1)
//
// Only indicators referenced by the active mode's entry/exit signals count, so
// an unused configured period never inflates the embargo. Unsupported signals,
// invalid code expressions, non-positive used periods, and an invalid
// allowance fail closed (throw) instead of guessing. Pure — no IO/UI/state.

import type { ParamsStrategy, SignalId, Rule, OperandId } from './strategy';
import { OPERAND_IDS } from './strategy';
import { compileExpression, type ExprNode } from './exprInterpreter';

export interface EmbargoDerivation {
  /** planValidationSplit-ready gap: maxSignalLookbackBars + holdingAllowanceBars. */
  embargoBars: number;
  /** History bars the strategy's slowest used signal reads (>= 1). */
  maxSignalLookbackBars: number;
  /** Caller-approved allowance for boundary-spanning holding periods. */
  holdingAllowanceBars: number;
}

function period(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new RangeError(`${name} must be a positive integer to derive an embargo (got ${value})`);
  }
  return value;
}

/** Derived lookback arithmetic must stay EXACT: IEEE-754 silently rounds past
 *  Number.MAX_SAFE_INTEGER, which would diverge from the Rust port's exact
 *  i64 math (PR #70 review). Any overflowing derivation fails closed. */
function safeLookback(value: number, context: string): number {
  if (!Number.isSafeInteger(value)) {
    throw new RangeError(`${context} exceeds the safe integer range`);
  }
  return value;
}

/** History bars one named operand series reads (usage-aware; validates only
 *  the periods that operand actually depends on). */
function operandLookback(id: OperandId, strat: ParamsStrategy): number {
  switch (id) {
    case 'price':
    case 'open':
    case 'high':
    case 'low':
    case 'volume':
      return 1;
    case 'maFast':
      return period(strat.fastMA, 'fastMA');
    case 'maSlow':
      return period(strat.slowMA, 'slowMA');
    case 'ema':
      return period(strat.emaPeriod, 'emaPeriod');
    case 'rsi':
      return safeLookback(period(strat.rsiPeriod, 'rsiPeriod') + 1, 'derived signal lookback');
    case 'macd':
      return Math.max(period(strat.macdFast, 'macdFast'), period(strat.macdSlow, 'macdSlow'));
    case 'macdSignal':
    case 'macdHist':
      return safeLookback(
        Math.max(period(strat.macdFast, 'macdFast'), period(strat.macdSlow, 'macdSlow')) +
          period(strat.macdSignal, 'macdSignal') -
          1,
        'derived signal lookback',
      );
    case 'bbUpper':
    case 'bbMid':
    case 'bbLower':
      return period(strat.bbPeriod, 'bbPeriod');
  }
}

// ---------- params mode ----------

function paramsSignalLookback(id: SignalId, strat: ParamsStrategy): number {
  switch (id) {
    case 'maCrossUp':
    case 'maCrossDown':
      return safeLookback(
        Math.max(operandLookback('maFast', strat), operandLookback('maSlow', strat)) + 1,
        'derived signal lookback',
      );
    case 'emaCrossUp':
    case 'emaCrossDown':
      return safeLookback(operandLookback('ema', strat) + 1, 'derived signal lookback');
    case 'priceAboveSlow':
    case 'priceBelowSlow':
      return operandLookback('maSlow', strat);
    case 'rsiOversold':
    case 'rsiOverbought':
      return safeLookback(operandLookback('rsi', strat) + 1, 'derived signal lookback');
    case 'macdCrossUp':
    case 'macdCrossDown':
      return safeLookback(operandLookback('macdSignal', strat) + 1, 'derived signal lookback');
    case 'bbLowerTouch':
    case 'bbUpperTouch':
      return operandLookback('bbUpper', strat);
    default:
      // stochOversold / stochOverbought: buildParamsSignals cannot run these
      // either, so the derivation fails closed with the same story.
      throw new Error(
        `unsupported signal "${id}": stoch* signals await a core STOCH indicator (Phase B)`,
      );
  }
}

// ---------- blocks mode ----------

const KNOWN_OPERANDS = new Set<string>(OPERAND_IDS);

/** Lookback of one rule operand string: named series, numeric constant (0), or
 *  unknown text (0 — blocks mode treats it as NaN and the rule never fires). */
function ruleOperandLookback(name: string, strat: ParamsStrategy): number {
  const key = name.trim();
  return KNOWN_OPERANDS.has(key) ? operandLookback(key as OperandId, strat) : 0;
}

function blocksRuleLookback(rule: Rule, strat: ParamsStrategy): number {
  const base = Math.max(
    operandLookback(rule.l, strat),
    ruleOperandLookback(rule.r, strat),
  );
  return rule.op === 'crossUp' || rule.op === 'crossDown'
    ? safeLookback(base + 1, 'derived signal lookback')
    : base;
}

// ---------- code mode ----------

function astLookback(node: ExprNode, strat: ParamsStrategy): number {
  switch (node.kind) {
    case 'num':
      return 0;
    case 'var':
      // compileExpression already validated the name against OPERAND_IDS.
      return operandLookback(node.name as OperandId, strat);
    case 'unary':
      return astLookback(node.arg, strat);
    case 'binary':
      return Math.max(astLookback(node.left, strat), astLookback(node.right, strat));
    case 'call': {
      const args = node.args.map((a) => astLookback(a, strat));
      // prev/crossUp/crossDown all read i - 1
      return safeLookback(Math.max(...args, 0) + 1, 'derived signal lookback');
    }
  }
}

function codeLookback(source: string, strat: ParamsStrategy): number {
  return astLookback(compileExpression(source, OPERAND_IDS).ast, strat);
}

// ---------- derivation ----------

/** History bars the strategy's slowest used entry/exit signal reads (>= 1). */
export function maxSignalLookbackBars(strat: ParamsStrategy): number {
  let lookback: number;
  if (strat.mode === 'blocks') {
    lookback = Math.max(
      0,
      ...strat.entryRules.map((r) => blocksRuleLookback(r, strat)),
      ...strat.exitRules.map((r) => blocksRuleLookback(r, strat)),
    );
  } else if (strat.mode === 'code') {
    lookback = Math.max(codeLookback(strat.entryCode, strat), codeLookback(strat.exitCode, strat));
  } else {
    lookback = Math.max(
      paramsSignalLookback(strat.entrySig, strat),
      paramsSignalLookback(strat.exitSig, strat),
    );
  }
  return Math.max(1, lookback);
}

/**
 * Derive the VAL-001 `embargoBars` for a strategy. The returned breakdown must
 * be recorded alongside the run so the split stays reproducible.
 *
 * `holdingAllowanceBars` is the caller-approved allowance for trades whose
 * holding period could span a segment boundary; 0 is explicit, never implied.
 */
export function deriveEmbargoBars(
  strat: ParamsStrategy,
  holdingAllowanceBars: number,
): EmbargoDerivation {
  if (!Number.isSafeInteger(holdingAllowanceBars) || holdingAllowanceBars < 0) {
    throw new RangeError('holdingAllowanceBars must be a non-negative safe integer');
  }
  const lookback = maxSignalLookbackBars(strat);
  const embargoBars = safeLookback(
    lookback + holdingAllowanceBars,
    'derived embargoBars',
  );
  return {
    embargoBars,
    maxSignalLookbackBars: lookback,
    holdingAllowanceBars,
  };
}
