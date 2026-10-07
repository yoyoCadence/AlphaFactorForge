// Build a persistable StrategyDef row from a params-mode strategy.
// Centralizes strategy_hash computation (via core/hashing) and the
// ParamsStrategy -> strategy_def field mapping, so save call sites stay thin.

import { strategyHash, strategyHashFromDefinitionJson } from '../core/hashing';
import type { StrategyDef } from '../tauri-client/commands';
import { assertStrategyParams } from './strategyValidation';
import type { ParamsStrategy } from './strategy';
import { MANUAL_NUMERIC_POLICY, manualNumericPolicy, manualStrategyDefinition } from './manualStrategyDefinition';

/** Async because durable strategy-v2 identity requires Web Crypto SHA-256. */
export async function buildStrategyDef(strat: ParamsStrategy, name: string, sourceRow?: StrategyDef | null): Promise<StrategyDef> {
  // STRATEGY-VALIDATION-001 — persistence boundary. Rejecting BEFORE the hash is
  // what keeps an unusable strategy from acquiring a durable `strategy-v2`
  // identity (and therefore a row, a library entry, and an exported report).
  // This also covers the load-a-legacy-row-then-save path, which the run gate
  // alone would miss.
  assertStrategyParams(strat);
  const definition = manualStrategyDefinition(strat);
  const hash = await strategyHash(definition, {
    feePct: strat.feePct,
    slippagePct: strat.slipPct,
  });
  const autoName = strat.mode === 'blocks' ? 'blocks 策略' : `${strat.entrySig} → ${strat.exitSig}`;
  const savedName = name.trim() || autoName;
  if (sourceRow != null) {
    if (!Number.isSafeInteger(sourceRow.id) || sourceRow.id! <= 0) {
      throw new Error('來源策略缺少有效識別碼');
    }
    const policy = manualNumericPolicy(JSON.parse(sourceRow.original_definition_json));
    if (sourceRow.strategy_hash === hash) {
      if (policy !== MANUAL_NUMERIC_POLICY || await strategyHashFromDefinitionJson(sourceRow.original_definition_json) !== hash) {
        throw new Error('來源策略與定義識別碼不一致');
      }
      // A same-definition re-save must preserve original JSON and immutable
      // provenance, including the existing parent and validation lifecycle.
      return { ...sourceRow, name: savedName, source: 'manual' };
    }
  }
  return {
    name: savedName,
    type: strat.mode,
    dsl_json: null,
    original_definition_json: JSON.stringify(definition),
    param_schema_json: null,
    source: 'manual',
    ai_prompt_hash: null,
    strategy_hash: hash,
    lifecycle: 'candidate',
    parent_strategy_id: sourceRow?.id ?? null,
  };
}
