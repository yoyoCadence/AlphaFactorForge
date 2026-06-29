// SAFE expression interpreter for code-mode strategies (Slice 4b-1).
//
// Tokenizer -> recursive-descent parser -> restricted AST evaluator. It never
// turns a string into code (no dynamic dispatch, no runtime code generation) —
// an expression is only ever turned into a small whitelisted AST walked
// numerically. (A unit test scans this file to keep it that way.)
// Anything outside the whitelist (member access, indexing, assignment, strings,
// ternary, unknown identifiers/functions, non-finite literals) is REJECTED at
// parse time. Pure: no IO, no state. (STRATEGY_DISCOVERY §0.3.)
//
// Whitelist:
//   operators : + - * /  > < >= <= == !=  && || !  (and parentheses, unary -/!)
//   variables : caller-supplied (the blocks operand series)
//   functions : prev(x) [1-bar lookback], crossUp(a,b), crossDown(a,b)
//   literals  : finite numbers only
// Caps: source length, AST node count, AST depth, and NO nested time-shift
// (prev/crossUp/crossDown may not appear inside another's arguments => max 1 bar).

export type BinOp = '+' | '-' | '*' | '/' | '>' | '<' | '>=' | '<=' | '==' | '!=' | '&&' | '||';
export type FnName = 'prev' | 'crossUp' | 'crossDown';

export type ExprNode =
  | { kind: 'num'; value: number }
  | { kind: 'var'; name: string }
  | { kind: 'unary'; op: '-' | '!'; arg: ExprNode }
  | { kind: 'binary'; op: BinOp; left: ExprNode; right: ExprNode }
  | { kind: 'call'; name: FnName; args: ExprNode[] };

export interface InterpreterLimits {
  maxLen: number;
  maxNodes: number;
  maxDepth: number;
}

export const DEFAULT_LIMITS: InterpreterLimits = { maxLen: 1000, maxNodes: 128, maxDepth: 16 };

const FN_ARITY: Record<FnName, number> = { prev: 1, crossUp: 2, crossDown: 2 };

export interface CompiledExpr {
  ast: ExprNode;
  /** Evaluate to a number at bar `i` (booleans are 1/0). vars resolved from `series`. */
  evaluate(series: Record<string, number[]>, i: number): number;
}

/** Truthiness used for &&/||/! and the final signal: finite and non-zero. */
export function isTruthy(x: number): boolean {
  return Number.isFinite(x) && x !== 0;
}

function fail(msg: string): never {
  throw new Error(`invalid expression: ${msg}`);
}

// ---------- tokenizer ----------

type Tok =
  | { t: 'num'; v: number }
  | { t: 'id'; v: string }
  | { t: 'op'; v: BinOp | '!' }
  | { t: 'lparen' }
  | { t: 'rparen' }
  | { t: 'comma' };

const isDigit = (c: string) => c >= '0' && c <= '9';
const isAlpha = (c: string) => (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c === '_';
const isAlnum = (c: string) => isAlpha(c) || isDigit(c);

function tokenize(src: string): Tok[] {
  const toks: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === ' ' || c === '\t' || c === '\n' || c === '\r') {
      i++;
      continue;
    }
    if (isDigit(c) || (c === '.' && isDigit(src[i + 1] ?? ''))) {
      let j = i;
      while (j < src.length && isDigit(src[j])) j++;
      if (src[j] === '.') {
        j++;
        while (j < src.length && isDigit(src[j])) j++;
      }
      if (src[j] === 'e' || src[j] === 'E') {
        j++;
        if (src[j] === '+' || src[j] === '-') j++;
        if (!isDigit(src[j] ?? '')) fail('malformed numeric literal');
        while (j < src.length && isDigit(src[j])) j++;
      }
      const text = src.slice(i, j);
      const v = Number(text);
      if (!Number.isFinite(v)) fail(`non-finite numeric literal "${text}"`);
      toks.push({ t: 'num', v });
      i = j;
      continue;
    }
    if (isAlpha(c)) {
      let j = i;
      while (j < src.length && isAlnum(src[j])) j++;
      toks.push({ t: 'id', v: src.slice(i, j) });
      i = j;
      continue;
    }
    const two = src.slice(i, i + 2);
    if (two === '>=' || two === '<=' || two === '==' || two === '!=' || two === '&&' || two === '||') {
      toks.push({ t: 'op', v: two });
      i += 2;
      continue;
    }
    if (c === '+' || c === '-' || c === '*' || c === '/' || c === '>' || c === '<' || c === '!') {
      toks.push({ t: 'op', v: c });
      i++;
      continue;
    }
    if (c === '(') {
      toks.push({ t: 'lparen' });
      i++;
      continue;
    }
    if (c === ')') {
      toks.push({ t: 'rparen' });
      i++;
      continue;
    }
    if (c === ',') {
      toks.push({ t: 'comma' });
      i++;
      continue;
    }
    // Everything else (= & | . [ ] ? : " ' etc.) is forbidden.
    fail(`unexpected character ${JSON.stringify(c)}`);
  }
  return toks;
}

// ---------- parser (recursive descent) ----------

function parse(toks: Tok[], allowedVars: Set<string>): ExprNode {
  let pos = 0;
  const peek = (): Tok | undefined => toks[pos];
  const isOp = (v: BinOp | '!') => {
    const t = peek();
    return t !== undefined && t.t === 'op' && t.v === v;
  };

  const bin = (op: BinOp, left: ExprNode, right: ExprNode): ExprNode => ({ kind: 'binary', op, left, right });

  const parseExpr = (): ExprNode => parseOr();

  const parseOr = (): ExprNode => {
    let n = parseAnd();
    while (isOp('||')) {
      pos++;
      n = bin('||', n, parseAnd());
    }
    return n;
  };
  const parseAnd = (): ExprNode => {
    let n = parseEquality();
    while (isOp('&&')) {
      pos++;
      n = bin('&&', n, parseEquality());
    }
    return n;
  };
  const parseEquality = (): ExprNode => {
    let n = parseCompare();
    while (isOp('==') || isOp('!=')) {
      const op = (peek() as { t: 'op'; v: BinOp }).v;
      pos++;
      n = bin(op, n, parseCompare());
    }
    return n;
  };
  const parseCompare = (): ExprNode => {
    let n = parseAdd();
    while (isOp('>') || isOp('<') || isOp('>=') || isOp('<=')) {
      const op = (peek() as { t: 'op'; v: BinOp }).v;
      pos++;
      n = bin(op, n, parseAdd());
    }
    return n;
  };
  const parseAdd = (): ExprNode => {
    let n = parseMul();
    while (isOp('+') || isOp('-')) {
      const op = (peek() as { t: 'op'; v: BinOp }).v;
      pos++;
      n = bin(op, n, parseMul());
    }
    return n;
  };
  const parseMul = (): ExprNode => {
    let n = parseUnary();
    while (isOp('*') || isOp('/')) {
      const op = (peek() as { t: 'op'; v: BinOp }).v;
      pos++;
      n = bin(op, n, parseUnary());
    }
    return n;
  };
  const parseUnary = (): ExprNode => {
    if (isOp('!') || isOp('-')) {
      const op = (peek() as { t: 'op'; v: '-' | '!' }).v;
      pos++;
      return { kind: 'unary', op, arg: parseUnary() };
    }
    return parsePrimary();
  };
  const parsePrimary = (): ExprNode => {
    const t = peek();
    if (t === undefined) fail('unexpected end of expression');
    if (t.t === 'num') {
      pos++;
      return { kind: 'num', value: t.v };
    }
    if (t.t === 'lparen') {
      pos++;
      const e = parseExpr();
      if (peek()?.t !== 'rparen') fail('expected ")"');
      pos++;
      return e;
    }
    if (t.t === 'id') {
      pos++;
      if (peek()?.t === 'lparen') {
        const name = t.v;
        if (!(name in FN_ARITY)) fail(`unknown function "${name}"`);
        pos++; // consume "("
        const args: ExprNode[] = [];
        if (peek()?.t !== 'rparen') {
          args.push(parseExpr());
          while (peek()?.t === 'comma') {
            pos++;
            args.push(parseExpr());
          }
        }
        if (peek()?.t !== 'rparen') fail('expected ")"');
        pos++;
        const arity = FN_ARITY[name as FnName];
        if (args.length !== arity) fail(`"${name}" expects ${arity} argument(s), got ${args.length}`);
        return { kind: 'call', name: name as FnName, args };
      }
      if (!allowedVars.has(t.v)) fail(`unknown variable "${t.v}"`);
      return { kind: 'var', name: t.v };
    }
    fail('unexpected token');
  };

  const ast = parseExpr();
  if (pos !== toks.length) fail('unexpected trailing token');
  return ast;
}

// ---------- validation (caps + no nested time-shift) ----------

function validate(node: ExprNode, depth: number, inShift: boolean, limits: InterpreterLimits): number {
  if (depth > limits.maxDepth) fail(`expression too deeply nested (> ${limits.maxDepth})`);
  let count = 1;
  switch (node.kind) {
    case 'num':
    case 'var':
      break;
    case 'unary':
      count += validate(node.arg, depth + 1, inShift, limits);
      break;
    case 'binary':
      count += validate(node.left, depth + 1, inShift, limits);
      count += validate(node.right, depth + 1, inShift, limits);
      break;
    case 'call':
      // prev/crossUp/crossDown all look back one bar; nesting them would compound
      // the lookback, so forbid a time-shift inside another's arguments.
      if (inShift) fail('nested time-shift (prev/crossUp/crossDown) not allowed — max 1-bar lookback');
      for (const a of node.args) count += validate(a, depth + 1, true, limits);
      break;
  }
  return count;
}

// ---------- evaluator ----------

function valueAt(series: Record<string, number[]>, name: string, j: number): number {
  const arr = series[name];
  if (!arr || j < 0 || j >= arr.length) return Number.NaN;
  return arr[j];
}

function evaluate(node: ExprNode, series: Record<string, number[]>, i: number): number {
  switch (node.kind) {
    case 'num':
      return node.value;
    case 'var':
      return valueAt(series, node.name, i);
    case 'unary': {
      const v = evaluate(node.arg, series, i);
      return node.op === '-' ? -v : isTruthy(v) ? 0 : 1;
    }
    case 'binary': {
      const op = node.op;
      if (op === '&&') {
        const l = evaluate(node.left, series, i);
        return isTruthy(l) ? (isTruthy(evaluate(node.right, series, i)) ? 1 : 0) : 0;
      }
      if (op === '||') {
        const l = evaluate(node.left, series, i);
        return isTruthy(l) ? 1 : isTruthy(evaluate(node.right, series, i)) ? 1 : 0;
      }
      const l = evaluate(node.left, series, i);
      const r = evaluate(node.right, series, i);
      switch (op) {
        case '+':
          return l + r;
        case '-':
          return l - r;
        case '*':
          return l * r;
        case '/':
          return l / r;
        default:
          // comparisons: NaN operand -> false
          if (!Number.isFinite(l) || !Number.isFinite(r)) return 0;
          switch (op) {
            case '>':
              return l > r ? 1 : 0;
            case '<':
              return l < r ? 1 : 0;
            case '>=':
              return l >= r ? 1 : 0;
            case '<=':
              return l <= r ? 1 : 0;
            case '==':
              return l === r ? 1 : 0;
            case '!=':
              return l !== r ? 1 : 0;
          }
          return 0;
      }
    }
    case 'call': {
      if (node.name === 'prev') return evaluate(node.args[0], series, i - 1);
      const ai = evaluate(node.args[0], series, i);
      const bi = evaluate(node.args[1], series, i);
      const ap = evaluate(node.args[0], series, i - 1);
      const bp = evaluate(node.args[1], series, i - 1);
      if (![ai, bi, ap, bp].every(Number.isFinite)) return 0;
      return node.name === 'crossUp' ? (ap <= bp && ai > bi ? 1 : 0) : ap >= bp && ai < bi ? 1 : 0;
    }
  }
}

/**
 * Parse + validate an expression against the whitelist and caps, returning a
 * compiled evaluator. Throws `Error` (message prefixed "invalid expression:")
 * on any violation — callers decide how to surface it.
 */
export function compileExpression(
  src: string,
  allowedVars: readonly string[],
  limits: InterpreterLimits = DEFAULT_LIMITS,
): CompiledExpr {
  if (src.length > limits.maxLen) fail(`expression too long (> ${limits.maxLen} chars)`);
  const toks = tokenize(src);
  if (toks.length === 0) fail('empty expression');
  const ast = parse(toks, new Set(allowedVars));
  const nodes = validate(ast, 1, false, limits);
  if (nodes > limits.maxNodes) fail(`expression has too many nodes (> ${limits.maxNodes})`);
  return {
    ast,
    evaluate: (series, i) => evaluate(ast, series, i),
  };
}
