// Numeric policy belongs to the persisted manual definition, never the editor
// or discovery config. These markers are covered by the existing strategy-v2
// binary hash; unmarked documents retain their declared legacy interpretation.
import type { ParamsStrategy } from './strategy';

export const MANUAL_DEFINITION_VERSION = 'manual-strategy-definition-v1';
export const MANUAL_NUMERIC_POLICY = 'json-f64-roundtrip-v1';
export const LEGACY_NUMERIC_POLICY = 'serde-json-default-v1';
export type ManualNumericPolicy = typeof MANUAL_NUMERIC_POLICY | typeof LEGACY_NUMERIC_POLICY;

export type ManualStrategyDefinition = ParamsStrategy & {
  definitionVersion: typeof MANUAL_DEFINITION_VERSION;
  numericPolicy: typeof MANUAL_NUMERIC_POLICY;
};

export function manualStrategyDefinition(strategy: ParamsStrategy): ManualStrategyDefinition {
  return {
    ...strategy,
    definitionVersion: MANUAL_DEFINITION_VERSION,
    numericPolicy: MANUAL_NUMERIC_POLICY,
  };
}

/** Missing BOTH markers means legacy; partial/unknown declarations fail closed. */
export function manualNumericPolicy(value: unknown): ManualNumericPolicy {
  if (value == null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('策略定義必須是物件');
  }
  const document = value as Record<string, unknown>;
  const hasVersion = Object.prototype.hasOwnProperty.call(document, 'definitionVersion');
  const hasPolicy = Object.prototype.hasOwnProperty.call(document, 'numericPolicy');
  if (!hasVersion && !hasPolicy) return LEGACY_NUMERIC_POLICY;
  if (document.definitionVersion !== MANUAL_DEFINITION_VERSION || document.numericPolicy !== MANUAL_NUMERIC_POLICY) {
    throw new Error('策略數值版本不受支援或宣告不完整');
  }
  return MANUAL_NUMERIC_POLICY;
}

/** The browser mock has no Rust legacy parser. Only admit literals already
 * pinned by the old identity/audit fixtures, plus safe integer JSON tokens.
 * Everything else requires the native preparation command, not a guessed hash. */
export function assertMockLegacyNumericSupport(definitionJson: string): void {
  const controls = new Set(['0.05', '0.02', '0.1', '0.125', '1.25', '0.3333333333333333', '1.2345678901234567']);
  const tokens = /"(?:\\.|[^"\\])*"|(-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)/g;
  for (const match of definitionJson.matchAll(tokens)) {
    const literal = match[1];
    if (literal == null) continue; // Numbers inside string values are not JSON numbers.
    if (/^-?(?:0|[1-9]\d*)$/.test(literal) && Number.isSafeInteger(Number(literal))) continue;
    if (controls.has(literal)) continue;
    throw new Error('瀏覽器測試模式無法驗證此舊策略的數值解讀；請使用桌面程式載入');
  }
}
