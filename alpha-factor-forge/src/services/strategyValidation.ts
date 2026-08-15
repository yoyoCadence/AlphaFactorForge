// STRATEGY-VALIDATION-001 — the one runtime admission gate for a manual
// strategy's indicator parameters.
//
// `NumberInput` only guarantees a finite number, so `fastMA: 2.5` reached
// `sma()`, where `values[i - 2.5]` is `undefined` and poisons the running sum;
// `fastMA: 0` took the early-return path. Either way every indicator value
// became NaN, every signal became false, and the backtest reported a confident
// ZERO-TRADE result that could be saved, hashed, and exported. The failure was
// invisible: nothing distinguished "this strategy never triggers" from "this
// parameter cannot produce a series".
//
// What this module guarantees:
//   - ONE rule set for the whole app. The numeric domain table, its
//     `checkNumericParam` predicate, and the three cross-field rules moved here
//     VERBATIM from `discoveryConfig` / `candidateEnumeration`, which now
//     re-export them. This module imports nothing but `./strategy`, which is
//     what lets `backtestRunner` depend on it: the previous owners sit behind
//     `discoveryConfig -> randomEntry -> backtestRunner`, so keeping the rules
//     there and importing them from the runner would form an import cycle.
//   - TWO mount points, both fail-closed: `runParamsBacktest` (every manual
//     execution, including each parameter-sweep variant) and `buildStrategyDef`
//     (before a strategy can acquire a durable `strategy-v2` identity).
//   - A CLOSED classification. Every numeric `ParamsStrategy` field is either
//     hard-validated here or explicitly owned by `toExecCostFractions`' legacy
//     clamping; a test fails if a future field is neither.
//
// Pure: no React, DOM, IO, or persistence.

import type { ParamsStrategy } from './strategy';

/** Bump when the hard-validated key set or its rules change. */
export const STRATEGY_PARAM_RULES_VERSION = 'strategy-params-v1';

// ---------- numeric domains (moved from discoveryConfig, unchanged) ----------

type NumericDomain = 'period' | 'level' | 'positive' | 'percent' | 'sizePercent';

/**
 * Domain of every numeric `ParamsStrategy` field, in declaration order.
 *
 * `percent` is bounded at 100, not merely at 0: `backtestRunner` divides these
 * legacy percent units by 100 and the engine's `assertNormalizedFraction`
 * rejects anything above 1. Admitting `feePct: 101` would queue a run that is
 * GUARANTEED to throw once a job executes — worse than not checking, because
 * the failure would land after jobs exist. `level` shares the same numeric
 * range for an unrelated reason (RSI is defined on 0..100), so the two stay
 * separate domains.
 */
export const NUMERIC_PARAM_DOMAINS = {
  fastMA: 'period',
  slowMA: 'period',
  emaPeriod: 'period',
  rsiPeriod: 'period',
  rsiBuy: 'level',
  rsiSell: 'level',
  macdFast: 'period',
  macdSlow: 'period',
  macdSignal: 'period',
  bbPeriod: 'period',
  bbMult: 'positive',
  slPct: 'percent',
  tpPct: 'percent',
  feePct: 'percent',
  slipPct: 'percent',
  sizePct: 'sizePercent',
} as const satisfies Record<string, NumericDomain>;

export type NumericParamKey = keyof typeof NUMERIC_PARAM_DOMAINS;

/** Domain check shared by base-preset fields and generated axis values, so an
 *  axis can never produce a value the base preset itself could not hold. */
export function checkNumericParam(key: NumericParamKey, value: number): string | null {
  if (!Number.isFinite(value)) return `${key} must be a finite number`;
  switch (NUMERIC_PARAM_DOMAINS[key]) {
    case 'period':
      return Number.isSafeInteger(value) && value >= 1
        ? null
        : `${key} must be an integer >= 1`;
    case 'level':
      return value >= 0 && value <= 100 ? null : `${key} must be in [0, 100]`;
    case 'positive':
      return value > 0 ? null : `${key} must be > 0`;
    case 'percent':
      return value >= 0 && value <= 100 ? null : `${key} must be in [0, 100]`;
    case 'sizePercent':
      return value > 0 && value <= 100 ? null : `${key} must be in (0, 100]`;
  }
}

// ---------- cross-field rules (moved from candidateEnumeration, unchanged) ----------

/**
 * Cross-field validity rules applied to every concrete combination. A grid
 * axis is independent by construction, so a Cartesian product inevitably
 * contains combinations that are not valid hypotheses (a "fast" MA slower than
 * the "slow" one). These are PRUNED and counted, not rejected — pruning is
 * the expected outcome of a legal grid, unlike a malformed config.
 *
 * Rules apply regardless of which signal a base preset selects, so the pruned
 * count stays a property of the grid alone and cannot shift when an unrelated
 * signal id changes.
 */
export const DISCOVERY_VALIDITY_RULE_IDS = [
  'fastMA<slowMA',
  'macdFast<macdSlow',
  'rsiBuy<rsiSell',
] as const;
export type DiscoveryValidityRuleId = (typeof DISCOVERY_VALIDITY_RULE_IDS)[number];

/** The first violated rule id in fixed order, or null when the combination is
 *  a valid hypothesis. */
export function candidateValidity(strategy: ParamsStrategy): DiscoveryValidityRuleId | null {
  if (!(strategy.fastMA < strategy.slowMA)) return 'fastMA<slowMA';
  if (!(strategy.macdFast < strategy.macdSlow)) return 'macdFast<macdSlow';
  if (!(strategy.rsiBuy < strategy.rsiSell)) return 'rsiBuy<rsiSell';
  return null;
}

// ---------- manual-strategy admission gate ----------

/**
 * The indicator-grid parameters, hard-validated before any run or save.
 *
 * These are exactly the fields whose value decides whether an indicator can
 * produce a series at all: the eight `period` fields must be safe integers
 * >= 1, the two RSI `level` fields must be within 0-100, and `bbMult` must be
 * positive. Rules are not restated here — `checkNumericParam` owns them.
 */
export const HARD_VALIDATED_PARAM_KEYS = [
  'fastMA',
  'slowMA',
  'emaPeriod',
  'rsiPeriod',
  'rsiBuy',
  'rsiSell',
  'macdFast',
  'macdSlow',
  'macdSignal',
  'bbPeriod',
  'bbMult',
] as const satisfies readonly NumericParamKey[];
export type HardValidatedParamKey = (typeof HARD_VALIDATED_PARAM_KEYS)[number];

/**
 * The execution-model fields this gate deliberately does NOT police.
 *
 * `toExecCostFractions` owns them through legacy conversions that are committed
 * contracts with their own tests: `sizePct: 0` means 100%, a negative fee clamps
 * to 0 instead of becoming a rebate, and `slPct`/`tpPct` <= 0 mean "off".
 * Applying discovery's stricter percent domains here would contradict those
 * tests and break strategies saved under them; their upper bound is already
 * enforced downstream by the engine's normalized-fraction assertion.
 */
export const LEGACY_CLAMPED_PARAM_KEYS = [
  'slPct',
  'tpPct',
  'feePct',
  'slipPct',
  'sizePct',
] as const satisfies readonly NumericParamKey[];

/** A parameter that cannot produce a meaningful indicator series. Blocks the
 *  run and the save. */
export interface StrategyParamIssue {
  key: HardValidatedParamKey;
  value: number;
  /** zh-TW, naming the field by its key exactly as `strategyLibrary` does. */
  message: string;
}

/** A computable but dubious hypothesis (e.g. a "fast" MA slower than the "slow"
 *  one). Surfaced, never blocking — see the module note below. */
export interface StrategyParamWarning {
  rule: DiscoveryValidityRuleId;
  message: string;
}

export interface StrategyParamValidation {
  /** False only when a hard rule fails; warnings never clear it. */
  ok: boolean;
  issues: StrategyParamIssue[];
  warnings: StrategyParamWarning[];
}

/** zh-TW description of what each hard-validated key accepts. Wording only —
 *  the predicate lives in `checkNumericParam`, so these can never disagree
 *  about validity, only about phrasing. */
const RULE_TEXT: Record<HardValidatedParamKey, string> = {
  fastMA: '必須是 >= 1 的整數',
  slowMA: '必須是 >= 1 的整數',
  emaPeriod: '必須是 >= 1 的整數',
  rsiPeriod: '必須是 >= 1 的整數',
  rsiBuy: '必須介於 0 與 100 之間',
  rsiSell: '必須介於 0 與 100 之間',
  macdFast: '必須是 >= 1 的整數',
  macdSlow: '必須是 >= 1 的整數',
  macdSignal: '必須是 >= 1 的整數',
  bbPeriod: '必須是 >= 1 的整數',
  bbMult: '必須大於 0',
};

/**
 * Cross-field hypothesis rules, reused from the discovery enumerator so the two
 * cannot drift. They are reported as WARNINGS on purpose:
 *   - none of them produces NaN; the strategy is computable, merely dubious;
 *   - the repo's own recorded judgment is that these combinations are pruned as
 *     the expected outcome of a legal grid, not rejected as malformed;
 *   - whether `fastMA` is even read depends on the selected signal;
 *   - blocking them would silently blank the (fastMA=20, slowMA=20) cell of the
 *     default 2-D sweep.
 * Making them fatal later is a one-line change; loosening them afterwards is not.
 */
const WARNING_TEXT: Record<DiscoveryValidityRuleId, string> = {
  'fastMA<slowMA': '快線週期 fastMA 未小於慢線週期 slowMA，交叉訊號可能不是你預期的方向。',
  'macdFast<macdSlow': 'macdFast 未小於 macdSlow，MACD 線的正負號會與慣例相反。',
  'rsiBuy<rsiSell': 'rsiBuy 未小於 rsiSell，超買／超賣門檻已對調。',
};

/** Validate one manual strategy's parameters. Pure; never throws, never
 *  mutates. Callers that must fail closed use `assertStrategyParams`. */
export function validateStrategyParams(strat: ParamsStrategy): StrategyParamValidation {
  const issues: StrategyParamIssue[] = [];
  for (const key of HARD_VALIDATED_PARAM_KEYS) {
    const value = strat[key];
    // `checkNumericParam` decides validity; we only decide how to say it.
    if (checkNumericParam(key, value) != null) {
      issues.push({ key, value, message: `策略欄位 ${key} ${RULE_TEXT[key]}（目前 ${describe(value)}）` });
    }
  }

  const warnings: StrategyParamWarning[] = [];
  // Cross-field rules read several fields at once, so they are only meaningful
  // once every field they read is itself valid.
  if (issues.length === 0) {
    const violated = candidateValidity(strat);
    if (violated != null) warnings.push({ rule: violated, message: WARNING_TEXT[violated] });
  }

  return { ok: issues.length === 0, issues, warnings };
}

/** Fail closed at an execution or persistence boundary. Reports the first
 *  offending field, in declaration order, so the message is deterministic. */
export function assertStrategyParams(strat: ParamsStrategy): void {
  const { issues } = validateStrategyParams(strat);
  if (issues.length > 0) throw new RangeError(issues[0].message);
}

/** Readable rendering for the message: `String(NaN)` is fine, but -0 and huge
 *  values should not surprise the reader. */
function describe(value: number): string {
  return Object.is(value, -0) ? '-0' : String(value);
}
