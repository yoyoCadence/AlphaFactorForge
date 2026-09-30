// P12d-2d-2: form data into the exact campaign and discovery declarations.
// The backend remains the authority for freezing, snapshot resolution and
// admission. This module rejects incomplete form input before any command.

import type { MarketInstrument, MarketSnapshotOption } from '../tauri-client/commands';
import type { ParamsStrategy } from './strategy';
import { buildDiscoveryConfig, type BuiltDiscoveryConfig } from './discoveryRunConfig';
import {
  DISCOVERY_CONFIG_VERSION_V3,
  DISCOVERY_CONTRACT_VERSIONS_V3,
  parseDiscoveryConfig,
} from './discoveryConfig';

export const CAMPAIGN_CONTRACTS = {
  datasetIdentity: 'dataset-content-v2',
  marketSnapshot: 'market-snapshot-v1',
  execution: 'backtest-execution-v1',
  metrics: 'metrics-v2',
  split: 'validation-split-v1',
  precision: 'research-precision-v1',
  trialLedger: 'trial-ledger-v1',
  walkForward: 'research-walk-forward-v1',
  walkForwardEvidence: 'walk-forward-evidence-v1',
} as const;

export interface CampaignSamplePolicy {
  minimumTotalBars: number;
  minimumTrainBars: number;
  foldValidationBars: number;
  foldCount: number;
  rationale: string;
}

export interface CampaignSampling {
  alphaPpm: number;
  maxRelativeStandardErrorPpm: number;
  bootstrapSamples: number;
  maxBootstrapSamples: number;
}

export interface SelectedSnapshot {
  option: MarketSnapshotOption;
  instrument: MarketInstrument;
  policy: CampaignSamplePolicy;
}

function positiveInteger(value: number, label: string): void {
  if (!Number.isSafeInteger(value) || value <= 0) throw new RangeError(`${label} 必須是正整數`);
}

export function buildCampaignDeclaration(selected: SelectedSnapshot[], sampling: CampaignSampling) {
  if (selected.length === 0) throw new RangeError('請先選擇至少一個 snapshot');
  for (const [key, value] of Object.entries(sampling)) positiveInteger(value, key);
  if (sampling.alphaPpm >= 1_000_000) throw new RangeError('alphaPpm 必須小於 1000000');
  if (sampling.bootstrapSamples > sampling.maxBootstrapSamples) {
    throw new RangeError('bootstrapSamples 不得超過 maxBootstrapSamples');
  }
  const seen = new Set<string>();
  const instruments = selected.map(({ option, instrument, policy }) => {
    const { snapshot, dataset } = option;
    if (seen.has(snapshot.instrumentId)) throw new RangeError('同一 instrument 只能選一個 snapshot');
    seen.add(snapshot.instrumentId);
    if (snapshot.instrumentRowId !== instrument.id || snapshot.instrumentId !== instrument.instrumentId
      || instrument.listedFrom == null) throw new RangeError(`${snapshot.instrumentId} 缺少對應的上市修訂`);
    for (const key of ['minimumTotalBars', 'minimumTrainBars', 'foldValidationBars', 'foldCount'] as const) {
      positiveInteger(policy[key], `${snapshot.instrumentId} ${key}`);
    }
    if (policy.foldCount < 2 || policy.foldCount > 128) throw new RangeError('foldCount 必須介於 2 到 128');
    if (!policy.rationale.trim()) throw new RangeError(`${snapshot.instrumentId} 需要樣本政策理由`);
    return {
      instrumentId: snapshot.instrumentId,
      listedAtMs: instrument.listedFrom,
      delistedAtMs: instrument.delistedAt,
      snapshotId: snapshot.snapshotId,
      datasetHash: snapshot.datasetHash,
      interval: snapshot.interval,
      fromMs: dataset.startTime,
      toMs: dataset.endTime,
      samplePolicy: { ...policy, rationale: policy.rationale.trim() },
    };
  });
  instruments.sort((a, b) => a.instrumentId < b.instrumentId ? -1 : a.instrumentId > b.instrumentId ? 1 : 0);
  return { contractVersion: 'research-campaign-declaration-v1', contracts: CAMPAIGN_CONTRACTS, sampling, instruments };
}

/** The existing v1 builder owns execution/cost/seed/defaults. Extend its
 * validated params envelope with the v3 pins and the frozen campaign folds. */
export function buildCampaignRunConfig(
  dataset: { id: number; hash: string },
  policy: CampaignSamplePolicy,
  strategy: ParamsStrategy,
  rootSeed: number,
  holdingAllowanceBars: number,
): BuiltDiscoveryConfig {
  const base = buildDiscoveryConfig({
    dataset,
    strategy,
    options: { axes: [], holdingAllowanceBars, rootSeed },
    logicalCores: Math.max(1, globalThis.navigator?.hardwareConcurrency ?? 1),
  });
  const envelope = {
    ...base.envelope,
    envelopeVersion: DISCOVERY_CONFIG_VERSION_V3,
    contracts: { ...DISCOVERY_CONTRACT_VERSIONS_V3 },
    walkForward: {
      minimumTrainBars: policy.minimumTrainBars,
      foldValidationBars: policy.foldValidationBars,
      foldCount: policy.foldCount,
    },
  };
  const resolved = parseDiscoveryConfig(envelope, { logicalCores: Math.max(1, globalThis.navigator?.hardwareConcurrency ?? 1) });
  return { envelope, resolved };
}
