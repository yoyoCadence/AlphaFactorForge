// Build a persistable StrategyDef row from a params-mode strategy.
// Centralizes strategy_hash computation (via core/hashing) and the
// ParamsStrategy -> strategy_def field mapping, so save call sites stay thin.

import { strategyHash } from '../core/hashing';
import type { StrategyDef } from '../tauri-client/commands';
import type { ParamsStrategy } from './strategy';

/** Async because durable strategy-v2 identity requires Web Crypto SHA-256. */
export async function buildStrategyDef(strat: ParamsStrategy, name: string): Promise<StrategyDef> {
  const hash = await strategyHash(strat, {
    feePct: strat.feePct,
    slippagePct: strat.slipPct,
  });
  const autoName = strat.mode === 'blocks' ? 'blocks 策略' : `${strat.entrySig} → ${strat.exitSig}`;
  return {
    name: name.trim() || autoName,
    type: strat.mode,
    dsl_json: null,
    original_definition_json: JSON.stringify(strat),
    param_schema_json: null,
    source: 'manual',
    ai_prompt_hash: null,
    strategy_hash: hash,
    lifecycle: 'candidate',
    parent_strategy_id: null,
  };
}
