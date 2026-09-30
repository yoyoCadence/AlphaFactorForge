import { describe, expect, it } from 'vitest';
import type { MarketInstrument, MarketSnapshotOption } from '../tauri-client/commands';
import { defaultStrategy } from './strategy';
import { buildCampaignDeclaration, buildCampaignRunConfig, type CampaignSamplePolicy } from './campaignAuthoring';
import { DISCOVERY_CONFIG_VERSION_V3, DISCOVERY_CONTRACT_VERSIONS_V3, parseDiscoveryConfig } from './discoveryConfig';

const policy: CampaignSamplePolicy = {
  minimumTotalBars: 8760, minimumTrainBars: 1000, foldValidationBars: 500,
  foldCount: 3, rationale: 'One year of hourly bars and three Train folds.',
};
const sampling = {
  alphaPpm: 50_000, maxRelativeStandardErrorPpm: 200_000,
  bootstrapSamples: 100_000, maxBootstrapSamples: 200_000,
};
const instrument = { id: 3, instrumentId: 'crypto:binance:BTCUSDT', listedFrom: 1_502_928_000_000, delistedAt: null } as MarketInstrument;
const option = {
  snapshot: {
    snapshotId: 'a'.repeat(64), instrumentRowId: 3,
    instrumentId: instrument.instrumentId, datasetHash: `dataset-content-v2:${'b'.repeat(64)}`, interval: '1h',
  },
  qualificationEligible: true,
  dataset: { id: 11, interval: '1h', startTime: 1_735_689_600_000, endTime: 1_767_222_000_000, candleCount: 8760 },
} as MarketSnapshotOption;

describe('campaign authoring', () => {
  it('binds the selected snapshot, its exact instrument revision and authored policy', () => {
    const document = buildCampaignDeclaration([{ option, instrument, policy }], sampling);
    expect(document.instruments[0]).toEqual({
      instrumentId: instrument.instrumentId, listedAtMs: instrument.listedFrom, delistedAtMs: null,
      snapshotId: option.snapshot.snapshotId, datasetHash: option.snapshot.datasetHash, interval: '1h',
      fromMs: option.dataset.startTime, toMs: option.dataset.endTime, samplePolicy: policy,
    });
    expect(() => buildCampaignDeclaration([{ option, instrument, policy: { ...policy, rationale: ' ' } }], sampling)).toThrow(/理由/);
    expect(() => buildCampaignDeclaration([{ option, instrument: { ...instrument, id: 4 }, policy }], sampling)).toThrow(/修訂/);
  });

  it('starts from the selected dataset with exact v3 folds and frozen costs', () => {
    const strategy = defaultStrategy();
    const { envelope } = buildCampaignRunConfig(
      { id: option.dataset.id, hash: option.snapshot.datasetHash }, policy, strategy, 123, 2,
    );
    const parsed = parseDiscoveryConfig(envelope as { envelopeVersion: typeof DISCOVERY_CONFIG_VERSION_V3 } & Record<string, unknown>, { logicalCores: 4 });
    expect(parsed.envelopeVersion).toBe(DISCOVERY_CONFIG_VERSION_V3);
    expect(parsed.contracts).toEqual(DISCOVERY_CONTRACT_VERSIONS_V3);
    expect(parsed.dataset).toEqual({ id: 11, contentHash: option.snapshot.datasetHash });
    expect(parsed.walkForward).toEqual({ minimumTrainBars: 1000, foldValidationBars: 500, foldCount: 3 });
    expect(parsed.benchmarkCosts).toEqual({ feePct: strategy.feePct, slipPct: strategy.slipPct });
    expect(parsed.embargo.holdingAllowanceBars).toBe(2);
  });
});
