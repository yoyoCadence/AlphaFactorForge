// FULL — whitelist compiler / validator for the Strategy DSL.
// Rejects anything not explicitly allowed. This is the security boundary for
// AI-generated strategies: a hostile AI can at worst produce an INVALID tree,
// never executable code. Mirror this logic in the Rust validate_strategy_dsl
// command for defense in depth.

import {
  ALLOWED_IND_FIELDS,
  ALLOWED_OP_FIELDS,
  BOOLEAN_OPS,
  DEFAULT_LIMITS,
  INDICATOR_WHITELIST,
  OPERATOR_WHITELIST,
  PRICE_SOURCES,
  type ExprNode,
  type IndicatorNode,
  type OperatorNode,
  type ParamSpec,
  type StrategyDSL,
  type ValidatorLimits,
} from './schema';

export interface ValidationResult {
  ok: boolean;
  errors: string[];
  nodeCount: number;
  depth: number;
}

const IND = new Set<string>(INDICATOR_WHITELIST);
const OP = new Set<string>(OPERATOR_WHITELIST);
const SRC = new Set<string>(PRICE_SOURCES);

// Substrings that must never appear anywhere in the serialized DSL — defense
// against attempts to smuggle code through string fields.
const SUSPICIOUS = [
  'eval', 'function', 'new function', 'import', 'require', 'fetch', 'xmlhttp',
  'process', 'child_process', 'fs.', 'readfile', 'writefile', 'localstorage',
  'sessionstorage', 'document', 'window', 'globalthis', '=>', 'while', 'for(',
  'constructor', '__proto__', 'prototype', 'settimeout', 'setinterval', '`',
];

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return !!v && typeof v === 'object' && !Array.isArray(v);
}

/** Validate a complete DSL. Pure; never throws on bad input. */
export function validateDSL(
  input: unknown,
  limits: ValidatorLimits = DEFAULT_LIMITS,
): ValidationResult {
  const errors: string[] = [];

  // 0. suspicious-string scan over the WHOLE payload (cheap, catches smuggling)
  try {
    const raw = JSON.stringify(input).toLowerCase();
    for (const bad of SUSPICIOUS) {
      if (raw.includes(bad)) errors.push(`suspicious token in payload: "${bad}"`);
    }
  } catch {
    errors.push('payload is not JSON-serializable');
    return { ok: false, errors, nodeCount: 0, depth: 0 };
  }

  if (!isPlainObject(input)) {
    errors.push('DSL must be an object');
    return { ok: false, errors, nodeCount: 0, depth: 0 };
  }
  const dsl = input as Partial<StrategyDSL>;

  if (typeof dsl.name !== 'string' || !dsl.name.trim()) errors.push('missing name');
  if (!isPlainObject(dsl.params)) errors.push('missing params object');
  if (!dsl.entry) errors.push('missing entry expression');
  if (!dsl.exit) errors.push('missing exit expression');

  // 1. param specs
  const paramNames = new Set<string>();
  if (isPlainObject(dsl.params)) {
    for (const [k, spec] of Object.entries(dsl.params)) {
      paramNames.add(k);
      if (typeof spec === 'number') continue; // fixed literal param
      const s = spec as ParamSpec;
      if (!isPlainObject(spec) || (s.type !== 'int' && s.type !== 'float')) {
        errors.push(`param ${k}: type must be int|float`);
        continue;
      }
      if (!Number.isFinite(s.min) || !Number.isFinite(s.max) || s.min > s.max) {
        errors.push(`param ${k}: invalid min/max`);
      }
      if (!Number.isFinite(s.default) || s.default < s.min || s.default > s.max) {
        errors.push(`param ${k}: default out of range`);
      }
    }
  }

  let nodeCount = 0;
  let maxDepth = 0;

  const refParam = (val: number | string | undefined, label: string): void => {
    if (typeof val === 'string') {
      if (!val.startsWith('$')) errors.push(`${label}: string must be a $param ref`);
      else if (!paramNames.has(val.slice(1))) errors.push(`${label}: unknown param ${val}`);
    }
  };

  const checkLen = (val: number | string | undefined, label: string): void => {
    if (val === undefined) return;
    if (typeof val === 'string') return refParam(val, label);
    if (!Number.isInteger(val) || val < limits.minLen || val > limits.maxLen) {
      errors.push(`${label}: len must be int in [${limits.minLen}, ${limits.maxLen}]`);
    }
  };

  const walk = (node: unknown, depth: number, path: string): void => {
    nodeCount += 1;
    maxDepth = Math.max(maxDepth, depth);
    if (depth > limits.maxDepth) {
      errors.push(`${path}: exceeds max depth ${limits.maxDepth}`);
      return;
    }
    if (nodeCount > limits.maxNodes) {
      errors.push(`exceeds max node count ${limits.maxNodes}`);
      return;
    }
    if (!isPlainObject(node)) {
      errors.push(`${path}: node must be an object`);
      return;
    }

    const hasInd = 'ind' in node;
    const hasOp = 'op' in node;
    if (hasInd === hasOp) {
      errors.push(`${path}: node must have exactly one of "ind" | "op"`);
      return;
    }

    if (hasInd) {
      const n = node as IndicatorNode;
      for (const f of Object.keys(n)) {
        if (!ALLOWED_IND_FIELDS.has(f)) errors.push(`${path}: unknown field "${f}"`);
      }
      if (!IND.has(n.ind)) errors.push(`${path}: indicator "${n.ind}" not in whitelist`);
      if (n.src !== undefined && !SRC.has(n.src)) errors.push(`${path}: bad src "${n.src}"`);
      checkLen(n.len, `${path}.len`);
      for (const f of ['fast', 'slow', 'signal', 'mult', 'k', 'd'] as const) {
        if (n[f] !== undefined) {
          if (typeof n[f] === 'string') refParam(n[f] as string, `${path}.${f}`);
          else if (!Number.isFinite(n[f] as number)) errors.push(`${path}.${f}: not finite`);
        }
      }
      return; // indicator nodes are leaves
    }

    // operator node
    const n = node as unknown as OperatorNode;
    for (const f of Object.keys(n)) {
      if (!ALLOWED_OP_FIELDS.has(f)) errors.push(`${path}: unknown field "${f}"`);
    }
    if (!OP.has(n.op)) {
      errors.push(`${path}: operator "${n.op}" not in whitelist`);
      return;
    }
    if (n.op === 'CONST') {
      if (typeof n.v === 'string') refParam(n.v, `${path}.v`);
      else if (!Number.isFinite(n.v) || Math.abs(n.v as number) > limits.maxConstAbs) {
        errors.push(`${path}.v: CONST must be finite and |v| <= ${limits.maxConstAbs}`);
      }
      return;
    }
    if (n.op === 'SHIFT' || n.op === 'RISING' || n.op === 'FALLING') {
      checkLen(n.n, `${path}.n`);
    }
    if (!Array.isArray(n.args) || n.args.length === 0) {
      errors.push(`${path}: operator "${n.op}" requires args[]`);
      return;
    }
    n.args.forEach((a: ExprNode, i: number) => walk(a, depth + 1, `${path}.args[${i}]`));
  };

  if (dsl.entry) walk(dsl.entry, 1, 'entry');
  if (dsl.exit) walk(dsl.exit, 1, 'exit');

  // 2. entry/exit roots should be boolean-returning
  for (const [k, root] of [['entry', dsl.entry], ['exit', dsl.exit]] as const) {
    if (root && isPlainObject(root) && 'op' in root) {
      const op = (root as unknown as OperatorNode).op;
      if (!BOOLEAN_OPS.has(op)) errors.push(`${k}: root op "${op}" is not boolean-returning`);
    } else if (root) {
      errors.push(`${k}: root must be a boolean operator node`);
    }
  }

  return { ok: errors.length === 0, errors, nodeCount, depth: maxDepth };
}
