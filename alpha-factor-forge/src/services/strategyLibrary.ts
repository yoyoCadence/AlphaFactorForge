// Safe strategy_def -> editable form conversion for the SQLite strategy library.
// Persisted JSON is an IO boundary: validate it before it reaches React state or
// the signal/backtest pipeline. Defaults are not silently applied because every
// strategy saved by this app contains the complete ParamsStrategy shape.

import type { StrategyDef } from '../tauri-client/commands';
import {
  OPERAND_IDS,
  type OperandId,
  type ParamsStrategy,
  type Rule,
  type RuleOp,
  type SignalId,
} from './strategy';

const MODES: ParamsStrategy['mode'][] = ['params', 'blocks', 'code'];
const SIGNALS: SignalId[] = [
  'maCrossUp', 'maCrossDown', 'emaCrossUp', 'emaCrossDown',
  'priceAboveSlow', 'priceBelowSlow', 'rsiOversold', 'rsiOverbought',
  'macdCrossUp', 'macdCrossDown', 'bbLowerTouch', 'bbUpperTouch',
  'stochOversold', 'stochOverbought',
];
const RULE_OPS: RuleOp[] = ['>', '<', '>=', '<=', 'crossUp', 'crossDown'];
const NUMBER_KEYS = [
  'fastMA', 'slowMA', 'emaPeriod', 'rsiPeriod', 'rsiBuy', 'rsiSell',
  'macdFast', 'macdSlow', 'macdSignal', 'bbPeriod', 'bbMult',
  'slPct', 'tpPct', 'feePct', 'slipPct', 'sizePct',
] as const satisfies readonly (keyof ParamsStrategy)[];

function isRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value);
}

function isOneOf<T extends string>(value: unknown, values: readonly T[]): value is T {
  return typeof value === 'string' && values.includes(value as T);
}

function parseRules(value: unknown, field: string): Rule[] {
  if (!Array.isArray(value)) throw new Error(`${field} 必須是陣列`);
  return value.map((item, index) => {
    if (!isRecord(item)) throw new Error(`${field}[${index}] 格式錯誤`);
    if (!isOneOf(item.l, OPERAND_IDS)) throw new Error(`${field}[${index}].l 無效`);
    if (!isOneOf(item.op, RULE_OPS)) throw new Error(`${field}[${index}].op 無效`);
    if (typeof item.r !== 'string') throw new Error(`${field}[${index}].r 必須是字串`);
    return { l: item.l as OperandId, op: item.op, r: item.r };
  });
}

/** Parse and validate one persisted manual strategy for loading into the form. */
export function strategyFromDef(def: StrategyDef): ParamsStrategy {
  if (!isOneOf(def.type, MODES)) {
    throw new Error(`策略類型 ${def.type} 尚不能載入此編輯器`);
  }

  let value: unknown;
  try {
    value = JSON.parse(def.original_definition_json);
  } catch {
    throw new Error('策略定義不是有效 JSON');
  }
  if (!isRecord(value)) throw new Error('策略定義必須是物件');
  if (!isOneOf(value.mode, MODES)) throw new Error('策略 mode 無效');
  if (value.mode !== def.type) throw new Error('策略 type 與定義 mode 不一致');

  const numbers = {} as Pick<ParamsStrategy, (typeof NUMBER_KEYS)[number]>;
  for (const key of NUMBER_KEYS) {
    const n = value[key];
    if (typeof n !== 'number' || !Number.isFinite(n)) throw new Error(`策略欄位 ${key} 必須是有限數字`);
    numbers[key] = n;
  }

  if (!isOneOf(value.entrySig, SIGNALS) || !isOneOf(value.exitSig, SIGNALS)) {
    throw new Error('策略訊號識別碼無效');
  }
  if (typeof value.entryCode !== 'string' || typeof value.exitCode !== 'string') {
    throw new Error('策略程式碼欄位必須是字串');
  }
  if (!isOneOf(value.fillMode, ['close', 'nextOpen'] as const)) throw new Error('策略成交模式無效');
  if (!isOneOf(value.direction, ['long', 'short', 'both'] as const)) throw new Error('策略方向無效');

  return {
    mode: value.mode,
    ...numbers,
    entrySig: value.entrySig,
    exitSig: value.exitSig,
    entryRules: parseRules(value.entryRules, 'entryRules'),
    exitRules: parseRules(value.exitRules, 'exitRules'),
    entryCode: value.entryCode,
    exitCode: value.exitCode,
    fillMode: value.fillMode,
    direction: value.direction,
  };
}
