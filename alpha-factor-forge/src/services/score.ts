// SCORE-001: §5.2 weighted ranking score (STRATEGY_DISCOVERY.md), implemented
// per the PR #61 handoff Resolution (handoffs/2026-07-19-score-001-design-
// proposal-v1.md) — the Resolution overrides the original proposal.
//
// Pure judgment over an already-computed Validation segment result: runs no
// backtests, reads ONLY `ValidationRunResult.validation` (never Train; Test is
// never executed anywhere in v1). The returned breakdown is fully JSON-safe —
// every numeric field is finite or null, with an explicit `rawStatus` — and
// records the resolved config + formulaVersion so only like-for-like scores
// are ever compared. Gate ordering is NOT enforced here; the future runner
// only scores candidates whose GateVerdict passed.

import type { ParamsStrategy, SignalId, Rule, OperandId } from './strategy';
import { OPERAND_IDS } from './strategy';
import { compileExpression, type ExprNode } from './exprInterpreter';
import type { ValidationRunResult } from './validationRun';

export const SCORE_FORMULA_VERSION = 'score-v1';

// ---------- config ----------

export interface ScoreCaps {
  /** normalized = clamp01(cagr / cagr cap) */
  cagr: number;
  sortino: number;
  calmar: number;
  /** normalized = clamp01((pf - 1) / (cap - 1)); must be > 1 */
  profitFactor: number;
  /** Consistency scale s: normalized = 1 / (1 + s * monthlyStdDev) */
  consistencySigmaScale: number;
  complexityUnits: number;
  turnover: number;
  /** normalized = clamp01(log10(N) / cap) */
  dataMiningLog10: number;
}

export interface ScoreWeights {
  cagr: number;
  sortino: number;
  calmar: number;
  /** MUST stay 0 until REGIME-001 exists (Resolution D2). */
  regime: number;
  profitFactor: number;
  consistency: number;
  complexity: number;
  turnover: number;
  dataMining: number;
}

export interface ScoreConfig {
  caps: ScoreCaps;
  weights: ScoreWeights;
}

/** Resolution defaults (caps provisionally keep the proposal's values). */
export const DEFAULT_SCORE_CONFIG: ScoreConfig = {
  caps: {
    cagr: 1.0,
    sortino: 5,
    calmar: 5,
    profitFactor: 3,
    consistencySigmaScale: 10,
    complexityUnits: 40,
    turnover: 0.1,
    dataMiningLog10: 4,
  },
  weights: {
    cagr: 1,
    sortino: 1,
    calmar: 1,
    regime: 0,
    profitFactor: 1,
    consistency: 1,
    complexity: 0.5,
    turnover: 0.5,
    dataMining: 1,
  },
};

// ---------- breakdown types ----------

export type ScoreComponentId =
  | 'cagr'
  | 'sortino'
  | 'calmar'
  | 'regime'
  | 'profitFactor'
  | 'consistency';
export type ScorePenaltyId = 'complexity' | 'turnover' | 'dataMining';

/** Resolution D5 status vocabulary. NaN / negative Infinity map to `invalid`. */
export type RawStatus = 'finite' | 'positive_infinity' | 'insufficient' | 'invalid' | 'deferred';

export interface ScoreEntry<Id extends string> {
  id: Id;
  /** Finite observed value, or null (non-finite / missing / deferred). */
  raw: number | null;
  rawStatus: RawStatus;
  /** [0, 1] contribution basis; null only for the deferred regime entry. */
  normalized: number | null;
  weight: number;
  /** weight * normalized (0 when normalized is null). Penalties subtract. */
  contribution: number;
  evidence?: Record<string, number | string | null>;
}

export interface ScoreBreakdown {
  formulaVersion: typeof SCORE_FORMULA_VERSION;
  segment: 'validation';
  /** Always finite: sum(component contributions) - sum(penalty contributions). */
  score: number;
  components: ScoreEntry<ScoreComponentId>[];
  penalties: ScoreEntry<ScorePenaltyId>[];
  /** The exact resolved config this score was computed with. */
  config: ScoreConfig;
  /** Data-mining evidence: lineage-final unique hypothesis count. */
  testedCombinations: { n: number; basis: 'lineage-final-unique' };
}

export interface ScoreCandidateArgs {
  /** Only the Validation segment is ever read (Resolution D5). */
  validationRun: Pick<ValidationRunResult, 'validation'>;
  strat: ParamsStrategy;
  /** Lineage-final unique hypotheses, >= 1; manual one-offs pass 1 explicitly. */
  testedCombinations: number;
  config?: { caps?: Partial<ScoreCaps>; weights?: Partial<ScoreWeights> };
}

// ---------- validation ----------

/** JSON has no distinct negative-zero representation. Canonicalize it at the
 * score boundary so a breakdown survives stringify/parse without changing. */
const canonicalZero = (value: number): number => (Object.is(value, -0) ? 0 : value);

function validateConfig(config: ScoreConfig): void {
  for (const [name, value] of Object.entries(config.caps)) {
    if (!Number.isFinite(value) || value <= 0) {
      throw new RangeError(`cap ${name} must be finite and > 0`);
    }
  }
  if (config.caps.profitFactor <= 1) {
    throw new RangeError('cap profitFactor must be > 1 (1 is the break-even floor)');
  }
  for (const [name, value] of Object.entries(config.weights)) {
    if (!Number.isFinite(value) || value < 0) {
      throw new RangeError(`weight ${name} must be finite and >= 0`);
    }
  }
  if (config.weights.regime !== 0) {
    throw new RangeError(
      'regime weight must stay 0 until REGIME-001 implements the regime classifier',
    );
  }
}

// ---------- normalization ----------

const clamp01 = (x: number): number => Math.min(1, Math.max(0, x));

/** Ratio-style component: raw scaled by a transform, with the Resolution's
 *  non-finite semantics (+Inf -> full credit; NaN/-Inf -> invalid, 0). */
function ratioEntry<Id extends string>(
  id: Id,
  raw: number,
  weight: number,
  normalize: (finiteRaw: number) => number,
): ScoreEntry<Id> {
  if (Number.isNaN(raw) || raw === -Infinity) {
    return { id, raw: null, rawStatus: 'invalid', normalized: 0, weight, contribution: 0 };
  }
  if (raw === Infinity) {
    return { id, raw: null, rawStatus: 'positive_infinity', normalized: 1, weight, contribution: weight };
  }
  const finiteRaw = canonicalZero(raw);
  const normalized = clamp01(normalize(finiteRaw));
  return {
    id,
    raw: finiteRaw,
    rawStatus: 'finite',
    normalized,
    weight,
    contribution: weight * normalized,
  };
}

// ---------- consistency (Resolution D3, revised) ----------

function consistencyEntry(
  monthlyReturns: Record<string, number>,
  weight: number,
  sigmaScale: number,
): ScoreEntry<'consistency'> {
  const months = Object.values(monthlyReturns).filter((v) => Number.isFinite(v));
  if (months.length < 3) {
    return {
      id: 'consistency',
      raw: null,
      rawStatus: 'insufficient',
      normalized: 0,
      weight,
      contribution: 0,
      evidence: { monthCount: months.length, monthlyStdDev: null },
    };
  }

  // Scale before calculating the population variance. Summing or squaring
  // large-but-finite monthly returns directly can overflow even when the
  // mathematically correct sigma is finite.
  const scale = months.reduce((largest, value) => Math.max(largest, Math.abs(value)), 0);
  const scaled = scale === 0 ? months : months.map((value) => value / scale);
  const mean = scaled.reduce((sum, value) => sum + value, 0) / scaled.length;
  const variance = scaled.reduce((sum, value) => sum + (value - mean) ** 2, 0) / scaled.length;
  const scaledSigma = Math.sqrt(variance);
  const sigma = canonicalZero(scaledSigma * (scale === 0 ? 1 : scale));
  if (![mean, variance, scaledSigma, sigma].every(Number.isFinite)) {
    return {
      id: 'consistency',
      raw: null,
      rawStatus: 'invalid',
      normalized: 0,
      weight,
      contribution: 0,
      evidence: { monthCount: months.length, monthlyStdDev: null },
    };
  }
  const normalized = 1 / (1 + sigmaScale * sigma);
  return {
    id: 'consistency',
    raw: sigma,
    rawStatus: 'finite',
    normalized,
    weight,
    contribution: weight * normalized,
    evidence: { monthCount: months.length, monthlyStdDev: sigma },
  };
}

// ---------- complexity (Resolution D4: canonical cross-mode units) ----------

/** Strategy fields one operand series depends on (indicator params only). */
const OPERAND_FIELDS: Record<OperandId, (keyof ParamsStrategy)[]> = {
  price: [], open: [], high: [], low: [], volume: [],
  maFast: ['fastMA'],
  maSlow: ['slowMA'],
  ema: ['emaPeriod'],
  rsi: ['rsiPeriod'],
  macd: ['macdFast', 'macdSlow'],
  macdSignal: ['macdFast', 'macdSlow', 'macdSignal'],
  macdHist: ['macdFast', 'macdSlow', 'macdSignal'],
  bbUpper: ['bbPeriod', 'bbMult'],
  bbMid: ['bbPeriod', 'bbMult'],
  bbLower: ['bbPeriod', 'bbMult'],
};

/** Canonical form of each params-mode signal: one operator + two
 *  operands/literals = 3 decision nodes, plus the fields it reads. */
const PARAMS_SIGNAL_SHAPE: Partial<
  Record<SignalId, { fields: (keyof ParamsStrategy)[] }>
> = {
  maCrossUp: { fields: ['fastMA', 'slowMA'] },
  maCrossDown: { fields: ['fastMA', 'slowMA'] },
  emaCrossUp: { fields: ['emaPeriod'] },
  emaCrossDown: { fields: ['emaPeriod'] },
  priceAboveSlow: { fields: ['slowMA'] },
  priceBelowSlow: { fields: ['slowMA'] },
  rsiOversold: { fields: ['rsiPeriod', 'rsiBuy'] },
  rsiOverbought: { fields: ['rsiPeriod', 'rsiSell'] },
  macdCrossUp: { fields: ['macdFast', 'macdSlow', 'macdSignal'] },
  macdCrossDown: { fields: ['macdFast', 'macdSlow', 'macdSignal'] },
  bbLowerTouch: { fields: ['bbPeriod', 'bbMult'] },
  bbUpperTouch: { fields: ['bbPeriod', 'bbMult'] },
};

const KNOWN_OPERANDS = new Set<string>(OPERAND_IDS);

interface ComplexityCount {
  decisionNodes: number;
  fields: Set<keyof ParamsStrategy>;
}

function countBlocksList(rules: Rule[], into: ComplexityCount): void {
  for (const rule of rules) {
    into.decisionNodes += 3; // operator + left operand + right operand/literal
    for (const f of OPERAND_FIELDS[rule.l]) into.fields.add(f);
    const r = rule.r.trim();
    if (KNOWN_OPERANDS.has(r)) {
      for (const f of OPERAND_FIELDS[r as OperandId]) into.fields.add(f);
    }
  }
  if (rules.length > 1) into.decisionNodes += rules.length - 1; // AND connectors
}

function countAst(node: ExprNode, into: ComplexityCount): void {
  into.decisionNodes += 1;
  switch (node.kind) {
    case 'num':
      return;
    case 'var':
      for (const f of OPERAND_FIELDS[node.name as OperandId]) into.fields.add(f);
      return;
    case 'unary':
      countAst(node.arg, into);
      return;
    case 'binary':
      countAst(node.left, into);
      countAst(node.right, into);
      return;
    case 'call':
      for (const a of node.args) countAst(a, into);
      return;
  }
}

export interface ComplexityUnits {
  units: number;
  decisionNodes: number;
  indicatorParams: number;
  riskRules: number;
}

/** complexityUnits = canonical decision nodes + distinct indicator params the
 *  active entry/exit signals reference + enabled risk rules. Semantically
 *  equivalent params/blocks/code strategies yield IDENTICAL units. */
export function complexityUnits(strat: ParamsStrategy): ComplexityUnits {
  const count: ComplexityCount = { decisionNodes: 0, fields: new Set() };
  if (strat.mode === 'blocks') {
    countBlocksList(strat.entryRules, count);
    countBlocksList(strat.exitRules, count);
  } else if (strat.mode === 'code') {
    countAst(compileExpression(strat.entryCode, OPERAND_IDS).ast, count);
    countAst(compileExpression(strat.exitCode, OPERAND_IDS).ast, count);
  } else {
    for (const id of [strat.entrySig, strat.exitSig]) {
      const shape = PARAMS_SIGNAL_SHAPE[id];
      if (!shape) {
        throw new Error(
          `unsupported signal "${id}": stoch* signals await a core STOCH indicator (Phase B)`,
        );
      }
      count.decisionNodes += 3;
      for (const f of shape.fields) count.fields.add(f);
    }
  }
  const riskRules = (strat.slPct > 0 ? 1 : 0) + (strat.tpPct > 0 ? 1 : 0);
  return {
    units: count.decisionNodes + count.fields.size + riskRules,
    decisionNodes: count.decisionNodes,
    indicatorParams: count.fields.size,
    riskRules,
  };
}

// ---------- scoring ----------

/**
 * Compute the §5.2 ranking score for one Validation-segment result.
 * Deterministic; throws RangeError on invalid config or testedCombinations;
 * an uncompilable code-mode strategy propagates the interpreter error.
 */
export function scoreCandidate(args: ScoreCandidateArgs): ScoreBreakdown {
  const config: ScoreConfig = {
    caps: { ...DEFAULT_SCORE_CONFIG.caps, ...args.config?.caps },
    weights: { ...DEFAULT_SCORE_CONFIG.weights, ...args.config?.weights },
  };
  for (const key of Object.keys(config.weights) as (keyof ScoreWeights)[]) {
    config.weights[key] = canonicalZero(config.weights[key]);
  }
  validateConfig(config);

  const n = args.testedCombinations;
  if (!Number.isSafeInteger(n) || n < 1) {
    throw new RangeError('testedCombinations must be a positive safe integer (pass 1 for manual one-offs)');
  }

  const { metrics, equity, trades } = args.validationRun.validation;
  const { caps, weights } = config;

  const components: ScoreEntry<ScoreComponentId>[] = [
    ratioEntry('cagr', metrics.cagr, weights.cagr, (v) => v / caps.cagr),
    ratioEntry('sortino', metrics.sortino, weights.sortino, (v) => v / caps.sortino),
    ratioEntry('calmar', metrics.calmar, weights.calmar, (v) => v / caps.calmar),
    {
      id: 'regime',
      raw: null,
      rawStatus: 'deferred',
      normalized: null,
      weight: weights.regime,
      contribution: 0,
    },
    ratioEntry(
      'profitFactor',
      metrics.profitFactor,
      weights.profitFactor,
      (v) => (v - 1) / (caps.profitFactor - 1),
    ),
    consistencyEntry(metrics.monthlyReturns, weights.consistency, caps.consistencySigmaScale),
  ];

  const complexity = complexityUnits(args.strat);
  const penalties: ScoreEntry<ScorePenaltyId>[] = [
    {
      id: 'complexity',
      raw: complexity.units,
      rawStatus: 'finite',
      normalized: clamp01(complexity.units / caps.complexityUnits),
      weight: weights.complexity,
      contribution: weights.complexity * clamp01(complexity.units / caps.complexityUnits),
      evidence: {
        decisionNodes: complexity.decisionNodes,
        indicatorParams: complexity.indicatorParams,
        riskRules: complexity.riskRules,
      },
    },
    ratioEntry('turnover', metrics.turnover, weights.turnover, (v) => v / caps.turnover),
    {
      id: 'dataMining',
      raw: n,
      rawStatus: 'finite',
      normalized: clamp01(Math.log10(n) / caps.dataMiningLog10),
      weight: weights.dataMining,
      contribution: weights.dataMining * clamp01(Math.log10(n) / caps.dataMiningLog10),
      evidence: { n, basis: 'lineage-final-unique' },
    },
  ];
  // trade-frequency proxy provenance (Resolution D4)
  penalties[1].evidence = {
    proxy: 'closedTrades/totalBars@v1',
    closedTradeCount: trades.length,
    totalBars: equity.length,
  };

  const positive = components.reduce((sum, c) => sum + c.contribution, 0);
  const penalty = penalties.reduce((sum, p) => sum + p.contribution, 0);
  const score = positive - penalty;
  if (![positive, penalty, score].every(Number.isFinite)) {
    throw new RangeError('resolved score weights produce a non-finite score');
  }

  return {
    formulaVersion: SCORE_FORMULA_VERSION,
    segment: 'validation',
    score: canonicalZero(score),
    components,
    penalties,
    config,
    testedCombinations: { n, basis: 'lineage-final-unique' },
  };
}
