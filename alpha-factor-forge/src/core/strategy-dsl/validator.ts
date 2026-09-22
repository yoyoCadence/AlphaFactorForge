// Pure structural + semantic admission gate for `strategy-dsl-v1`.
// The Rust runner mirrors this contract and re-validates every DSL before it
// can be enumerated, persisted, or executed.

import {
  DEFAULT_LIMITS,
  INDICATOR_WHITELIST,
  OPERATOR_WHITELIST,
  PRICE_SOURCES,
  STRATEGY_DSL_VERSION,
  type ExpressionType,
  type ParamSpec,
  type ValidatorLimits,
} from './schema';

export interface ValidationResult {
  ok: boolean;
  errors: string[];
  nodeCount: number;
  depth: number;
  maxLookbackBars: number;
  parameterCount: number;
}

interface Analysis {
  type: ExpressionType;
  lookback: number;
}

const INDICATORS = new Set<string>(INDICATOR_WHITELIST);
const OPERATORS = new Set<string>(OPERATOR_WHITELIST);
const SOURCES = new Set<string>(PRICE_SOURCES);
const PARAM_NAME = /^[A-Za-z][A-Za-z0-9_]{0,63}$/;
const ROOT_KEYS = ['version', 'name', 'params', 'entry', 'exit'] as const;
const PARAM_SPEC_KEYS = ['type', 'min', 'max', 'default'] as const;
const RAW_INDICATORS = new Set(['CLOSE', 'OPEN', 'HIGH', 'LOW', 'HLC3']);
const SOURCE_INDICATORS = new Set(['EMA', 'SMA', 'WMA', 'RSI', 'ROC', 'STDDEV', 'HIGHEST', 'LOWEST']);

const SUSPICIOUS = [
  'eval', 'function', 'new function', 'import', 'require', 'fetch', 'xmlhttp',
  'process', 'child_process', 'fs.', 'readfile', 'writefile', 'localstorage',
  'sessionstorage', 'document', 'window', 'globalthis', '=>', 'while', 'for(',
  'constructor', '__proto__', 'prototype', 'settimeout', 'setinterval', '`',
];

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactFields(
  object: Record<string, unknown>,
  fields: readonly string[],
  path: string,
  errors: string[],
): void {
  for (const key of Object.keys(object).sort()) {
    if (!fields.includes(key)) errors.push(`${path}: unknown field "${key}"`);
  }
  for (const field of fields) {
    if (!(field in object)) errors.push(`${path}: missing field "${field}"`);
  }
}

/** Validate a complete DSL. Pure and total: malformed input never throws. */
export function validateDSL(
  input: unknown,
  limits: ValidatorLimits = DEFAULT_LIMITS,
): ValidationResult {
  const errors: string[] = [];
  let nodeCount = 0;
  let maxDepth = 0;
  let maxLookbackBars = 0;
  const params = new Map<string, number>();
  const result = (): ValidationResult => ({
    ok: errors.length === 0,
    errors,
    nodeCount,
    depth: maxDepth,
    maxLookbackBars,
    parameterCount: params.size,
  });

  try {
    const encoded = JSON.stringify(input);
    if (encoded === undefined) throw new TypeError('not serializable');
    const raw = encoded.toLowerCase();
    for (const token of SUSPICIOUS) {
      if (raw.includes(token)) errors.push(`suspicious token in payload: "${token}"`);
    }
  } catch {
    errors.push('payload is not JSON-serializable');
    return result();
  }

  if (!isPlainObject(input)) {
    errors.push('DSL must be an object');
    return result();
  }
  exactFields(input, ROOT_KEYS, 'DSL', errors);
  if (input.version !== STRATEGY_DSL_VERSION) {
    errors.push(`DSL.version must be "${STRATEGY_DSL_VERSION}"`);
  }
  if (typeof input.name !== 'string' || input.name.trim().length === 0) errors.push('missing name');

  if (!isPlainObject(input.params)) {
    errors.push('missing params object');
  } else {
    for (const [name, rawSpec] of Object.entries(input.params)) {
      if (!PARAM_NAME.test(name)) errors.push(`param ${name}: invalid name`);
      if (typeof rawSpec === 'number') {
        if (!Number.isFinite(rawSpec)) errors.push(`param ${name}: fixed value must be finite`);
        else params.set(name, rawSpec);
        continue;
      }
      if (!isPlainObject(rawSpec)) {
        errors.push(`param ${name}: type must be int|float`);
        continue;
      }
      exactFields(rawSpec, PARAM_SPEC_KEYS, `param ${name}`, errors);
      const spec = rawSpec as unknown as ParamSpec;
      if (spec.type !== 'int' && spec.type !== 'float') {
        errors.push(`param ${name}: type must be int|float`);
        continue;
      }
      if (!Number.isFinite(spec.min) || !Number.isFinite(spec.max) || spec.min > spec.max) {
        errors.push(`param ${name}: invalid min/max`);
      }
      if (!Number.isFinite(spec.default) || spec.default < spec.min || spec.default > spec.max) {
        errors.push(`param ${name}: default out of range`);
      }
      if (spec.type === 'int' && ![spec.min, spec.max, spec.default].every(Number.isSafeInteger)) {
        errors.push(`param ${name}: int bounds/default must be safe integers`);
      }
      if (Number.isFinite(spec.default)) params.set(name, spec.default);
    }
  }

  const scalar = (
    value: unknown,
    path: string,
    constraint: 'number' | 'period',
  ): number | undefined => {
    let resolved: number | undefined;
    if (typeof value === 'number') resolved = value;
    else if (typeof value === 'string' && value.startsWith('$')) {
      const name = value.slice(1);
      if (!params.has(name)) errors.push(`${path}: unknown param ${value}`);
      else resolved = params.get(name);
    } else if (typeof value === 'string') {
      errors.push(`${path}: string must be a $param ref`);
    } else {
      errors.push(`${path}: must be a number or $param ref`);
    }
    if (resolved === undefined) return undefined;
    if (!Number.isFinite(resolved)) {
      errors.push(`${path}: value must be finite`);
      return undefined;
    }
    if (constraint === 'period'
      && (!Number.isSafeInteger(resolved) || resolved < limits.minLen || resolved > limits.maxLen)) {
      errors.push(`${path}: lookback must be int in [${limits.minLen}, ${limits.maxLen}]`);
      return undefined;
    }
    if (constraint === 'number' && Math.abs(resolved) > limits.maxConstAbs) {
      errors.push(`${path}: value must have |v| <= ${limits.maxConstAbs}`);
      return undefined;
    }
    return resolved;
  };

  const walk = (node: unknown, currentDepth: number, path: string): Analysis | undefined => {
    nodeCount += 1;
    maxDepth = Math.max(maxDepth, currentDepth);
    if (currentDepth > limits.maxDepth) {
      errors.push(`${path}: exceeds max depth ${limits.maxDepth}`);
      return undefined;
    }
    if (nodeCount > limits.maxNodes) {
      errors.push(`exceeds max node count ${limits.maxNodes}`);
      return undefined;
    }
    if (!isPlainObject(node)) {
      errors.push(`${path}: node must be an object`);
      return undefined;
    }
    const hasInd = 'ind' in node;
    const hasOp = 'op' in node;
    if (hasInd === hasOp) {
      errors.push(`${path}: node must have exactly one of "ind" | "op"`);
      return undefined;
    }

    if (hasInd) {
      if (typeof node.ind !== 'string' || !INDICATORS.has(node.ind)) {
        errors.push(`${path}: indicator "${String(node.ind)}" not executable in ${STRATEGY_DSL_VERSION}`);
        return undefined;
      }
      if (RAW_INDICATORS.has(node.ind)) {
        exactFields(node, ['ind'], path, errors);
        maxLookbackBars = Math.max(maxLookbackBars, 1);
        return { type: 'number', lookback: 1 };
      }
      if (node.ind === 'ATR') {
        exactFields(node, ['ind', 'len'], path, errors);
      } else if (SOURCE_INDICATORS.has(node.ind)) {
        exactFields(node, ['ind', 'src', 'len'], path, errors);
        if (typeof node.src !== 'string' || !SOURCES.has(node.src)) {
          errors.push(`${path}.src: unsupported price source "${String(node.src)}"`);
        }
      }
      const period = scalar(node.len, `${path}.len`, 'period');
      const lookback = period === undefined ? 1 : period + (node.ind === 'RSI' || node.ind === 'ROC' ? 1 : 0);
      maxLookbackBars = Math.max(maxLookbackBars, lookback);
      return { type: 'number', lookback };
    }

    if (typeof node.op !== 'string' || !OPERATORS.has(node.op)) {
      errors.push(`${path}: operator "${String(node.op)}" not in whitelist`);
      return undefined;
    }
    const op = node.op;
    if (op === 'CONST') {
      exactFields(node, ['op', 'v'], path, errors);
      scalar(node.v, `${path}.v`, 'number');
      maxLookbackBars = Math.max(maxLookbackBars, 1);
      return { type: 'number', lookback: 1 };
    }

    const temporal = op === 'SHIFT' || op === 'RISING' || op === 'FALLING';
    exactFields(node, temporal ? ['op', 'args', 'n'] : ['op', 'args'], path, errors);
    const expectedArity = op === 'ABS' || op === 'NOT' || temporal ? 1 : op === 'CLAMP' ? 3 : 2;
    if (!Array.isArray(node.args) || node.args.length !== expectedArity) {
      errors.push(`${path}: operator "${op}" requires exactly ${expectedArity} args`);
      return undefined;
    }
    const children = node.args.map((child, index) => walk(child, currentDepth + 1, `${path}.args[${index}]`));
    const lookback = Math.max(1, ...children.map((child) => child?.lookback ?? 1));
    const requireType = (expected: ExpressionType): void => {
      children.forEach((child, index) => {
        if (child && child.type !== expected) {
          errors.push(`${path}.args[${index}]: expected ${expected}, received ${child.type}`);
        }
      });
    };

    let analysis: Analysis;
    if (op === 'AND' || op === 'OR' || op === 'NOT') {
      requireType('boolean');
      analysis = { type: 'boolean', lookback };
    } else if (['GT', 'LT', 'GTE', 'LTE'].includes(op)) {
      requireType('number');
      analysis = { type: 'boolean', lookback };
    } else if (op === 'CROSS_UP' || op === 'CROSS_DOWN') {
      requireType('number');
      analysis = { type: 'boolean', lookback: lookback + 1 };
    } else if (op === 'SHIFT') {
      const offset = scalar(node.n, `${path}.n`, 'period') ?? 0;
      analysis = { type: children[0]?.type ?? 'number', lookback: lookback + offset };
    } else if (op === 'RISING' || op === 'FALLING') {
      requireType('number');
      const periods = scalar(node.n, `${path}.n`, 'period') ?? 0;
      analysis = { type: 'boolean', lookback: lookback + periods };
    } else {
      requireType('number');
      analysis = { type: 'number', lookback };
    }
    maxLookbackBars = Math.max(maxLookbackBars, analysis.lookback);
    return analysis;
  };

  const entry = walk(input.entry, 1, 'entry');
  const exit = walk(input.exit, 1, 'exit');
  if (entry && entry.type !== 'boolean') errors.push('entry: root must return boolean');
  if (exit && exit.type !== 'boolean') errors.push('exit: root must return boolean');
  return result();
}
