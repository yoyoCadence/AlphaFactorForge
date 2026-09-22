// `strategy-dsl-v1`: the executable, data-only strategy expression contract.
//
// The executable whitelist is intentionally the intersection of the current
// TypeScript and Rust indicator cores. Multi-output indicators (MACD and
// BBANDS) stay out until the DSL has an explicit output selector in both
// languages. The tree has no calls, loops, IO, imports, or source strings.

export const STRATEGY_DSL_VERSION = 'strategy-dsl-v1' as const;

export const INDICATOR_WHITELIST = [
  'EMA', 'SMA', 'WMA', 'RSI', 'ROC', 'ATR', 'STDDEV', 'HIGHEST', 'LOWEST',
  'CLOSE', 'OPEN', 'HIGH', 'LOW', 'HLC3',
] as const;

export const OPERATOR_WHITELIST = [
  'ADD', 'SUB', 'MUL', 'DIV', 'ABS', 'MIN', 'MAX', 'CLAMP',
  'GT', 'LT', 'GTE', 'LTE', 'CROSS_UP', 'CROSS_DOWN',
  'AND', 'OR', 'NOT', 'SHIFT', 'RISING', 'FALLING', 'CONST',
] as const;

export type IndicatorName = (typeof INDICATOR_WHITELIST)[number];
export type OperatorName = (typeof OPERATOR_WHITELIST)[number];

export const PRICE_SOURCES = ['CLOSE', 'OPEN', 'HIGH', 'LOW', 'HLC3', 'VOLUME'] as const;
export type PriceSource = (typeof PRICE_SOURCES)[number];
export type Scalar = number | string;

export interface IndicatorNode {
  ind: IndicatorName;
  src?: PriceSource;
  len?: Scalar;
}

export interface OperatorNode {
  op: OperatorName;
  args?: ExprNode[];
  v?: Scalar;
  n?: Scalar;
}

export type ExprNode = IndicatorNode | OperatorNode;

export interface ParamSpec {
  type: 'int' | 'float';
  min: number;
  max: number;
  default: number;
}

export interface StrategyDSL {
  version: typeof STRATEGY_DSL_VERSION;
  name: string;
  params: Record<string, ParamSpec | number>;
  entry: ExprNode;
  exit: ExprNode;
}

export interface ValidatorLimits {
  maxDepth: number;
  maxNodes: number;
  minLen: number;
  maxLen: number;
  maxConstAbs: number;
}

export const DEFAULT_LIMITS: ValidatorLimits = {
  maxDepth: 8,
  maxNodes: 64,
  minLen: 2,
  maxLen: 400,
  maxConstAbs: 1e9,
};

export type ExpressionType = 'number' | 'boolean';
