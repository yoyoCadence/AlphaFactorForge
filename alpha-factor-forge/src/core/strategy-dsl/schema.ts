// FULL — Strategy DSL schema (types + whitelists).
// The DSL is a pure expression TREE. It has NO loops, NO IO, NO function
// strings, NO recursion. Anything outside these whitelists is rejected by the
// validator (see validator.ts). This is what makes AI-generated strategies safe.

/** Indicators an expression node may reference (ind nodes). */
export const INDICATOR_WHITELIST = [
  'EMA', 'SMA', 'WMA', 'MACD', 'ADX',
  'RSI', 'STOCH', 'CCI', 'ROC', 'MOM',
  'ATR', 'BBANDS', 'STDDEV', 'KELTNER',
  'OBV', 'VOL_SMA', 'MFI',
  'CLOSE', 'OPEN', 'HIGH', 'LOW', 'HLC3', 'HIGHEST', 'LOWEST',
] as const;

/** Operators an expression node may use (op nodes). */
export const OPERATOR_WHITELIST = [
  'ADD', 'SUB', 'MUL', 'DIV', 'ABS', 'MIN', 'MAX', 'CLAMP',
  'GT', 'LT', 'GTE', 'LTE', 'CROSS_UP', 'CROSS_DOWN',
  'AND', 'OR', 'NOT',
  'SHIFT', 'RISING', 'FALLING', 'CONST',
] as const;

export type IndicatorName = (typeof INDICATOR_WHITELIST)[number];
export type OperatorName = (typeof OPERATOR_WHITELIST)[number];

/** Price sources usable as `src` on an indicator node. */
export const PRICE_SOURCES = ['CLOSE', 'OPEN', 'HIGH', 'LOW', 'HLC3', 'VOLUME'] as const;
export type PriceSource = (typeof PRICE_SOURCES)[number];

// ---------- Expression nodes (the only allowed shapes) ----------

export interface IndicatorNode {
  ind: IndicatorName;
  src?: PriceSource; // for price-derived indicators
  len?: number | string; // number or "$param"
  // extra named params (e.g. MACD fast/slow/signal); each must be number|"$param"
  [k: string]: unknown;
}

export interface OperatorNode {
  op: OperatorName;
  args?: ExprNode[];
  v?: number | string; // for CONST: literal number or "$param"
  n?: number | string; // for SHIFT/RISING/FALLING: lookback
}

export type ExprNode = IndicatorNode | OperatorNode;

/** Parameter schema entry — constrains a `$param` referenced in the tree. */
export interface ParamSpec {
  type: 'int' | 'float';
  min: number;
  max: number;
  default: number;
}

/** A complete strategy in DSL form. */
export interface StrategyDSL {
  name: string;
  params: Record<string, ParamSpec | number>;
  entry: ExprNode; // must evaluate to boolean
  exit: ExprNode; // must evaluate to boolean
}

// ---------- Validation limits (configurable) ----------

export interface ValidatorLimits {
  maxDepth: number; // AST depth limit
  maxNodes: number; // AST node count limit
  minLen: number; // min indicator length
  maxLen: number; // max indicator length
  maxConstAbs: number; // |CONST| ceiling
}

export const DEFAULT_LIMITS: ValidatorLimits = {
  maxDepth: 8,
  maxNodes: 64,
  minLen: 2,
  maxLen: 400,
  maxConstAbs: 1e9,
};

/** Boolean-returning operators (entry/exit roots should be one of these). */
export const BOOLEAN_OPS = new Set<OperatorName>([
  'GT', 'LT', 'GTE', 'LTE', 'CROSS_UP', 'CROSS_DOWN', 'AND', 'OR', 'NOT', 'RISING', 'FALLING',
]);

/** Fields allowed on a node, by kind — anything else is "unknown field" -> reject. */
export const ALLOWED_IND_FIELDS = new Set(['ind', 'src', 'len', 'fast', 'slow', 'signal', 'mult', 'k', 'd']);
export const ALLOWED_OP_FIELDS = new Set(['op', 'args', 'v', 'n']);
