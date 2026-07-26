import { describe, expect, it } from 'vitest';
import { strategyHash } from '../core/hashing';
import { DEFAULT_GATE_CONFIG } from './gate';
import { DEFAULT_SCORE_CONFIG } from './score';
import { defaultStrategy } from './strategy';
import {
  DISCOVERY_CONFIG_VERSION,
  DISCOVERY_CONTRACT_VERSIONS,
  DISCOVERY_PRESET_VERSION,
  parseDiscoveryConfig,
} from './discoveryConfig';
import {
  candidateValidity,
  enumerateCandidates,
  rawCombinationCount,
} from './candidateEnumeration';
import { deriveDiscoverySeed, discoverySeedPreimage } from './discoverySeed';

const DATASET_HASH = `dataset-content-v2:${'b'.repeat(64)}`;
const OTHER_DATASET_HASH = `dataset-content-v2:${'c'.repeat(64)}`;

interface BaseSpec {
  id: string;
  axes: { key: string; min: number; max: number; step: number }[];
  strategy?: Record<string, unknown>;
}

function configWith(bases: BaseSpec[], overrides: Record<string, unknown> = {}) {
  const envelope = {
    envelopeVersion: DISCOVERY_CONFIG_VERSION,
    contracts: { ...DISCOVERY_CONTRACT_VERSIONS },
    dataset: { id: 3, contentHash: DATASET_HASH },
    bases: bases.map((base) => ({
      id: base.id,
      presetVersion: DISCOVERY_PRESET_VERSION,
      strategy: { ...defaultStrategy(), ...base.strategy },
      axes: base.axes,
    })),
    embargo: { holdingAllowanceBars: 5 },
    execution: { startEquity: 10000 },
    benchmarkCosts: { feePct: 0.05, slipPct: 0.02 },
    randomEntry: { runs: 200 },
    gateConfig: { ...DEFAULT_GATE_CONFIG },
    scoreConfig: {
      caps: { ...DEFAULT_SCORE_CONFIG.caps },
      weights: { ...DEFAULT_SCORE_CONFIG.weights },
    },
    rootSeed: 20260726,
    caps: { candidates: 256 },
    maxConcurrency: null,
    ...overrides,
  };
  return parseDiscoveryConfig(envelope, { logicalCores: 8 });
}

describe('discovery-enumeration-v1', () => {
  it('enumerates a single-axis grid and reconciles every counter', async () => {
    const plan = await enumerateCandidates(
      configWith([{ id: 'ma-cross', axes: [{ key: 'fastMA', min: 5, max: 11, step: 3 }] }]),
    );
    expect(plan.contractVersion).toBe('discovery-enumeration-v1');
    expect(plan.counts).toEqual({ raw: 3, prunedInvalid: 0, duplicates: 0, finalUnique: 3 });
    expect(plan.candidates.map((candidate) => candidate.strategy.fastMA).sort((a, b) => a - b))
      .toEqual([5, 8, 11]);
    expect(plan.candidates.map((candidate) => candidate.appliedAxes)).toEqual(
      plan.candidates.map((candidate) => ({ fastMA: candidate.strategy.fastMA })),
    );
    expect(plan.testedCombinations).toEqual({ n: 3, basis: 'lineage-final-unique' });
  });

  it('takes the Cartesian product of two axes with the last axis fastest', async () => {
    const config = configWith([
      {
        id: 'ma-cross',
        axes: [
          { key: 'fastMA', min: 5, max: 9, step: 2 },
          { key: 'emaPeriod', min: 20, max: 40, step: 20 },
        ],
      },
    ]);
    expect(rawCombinationCount(config.bases)).toBe(6);
    const plan = await enumerateCandidates(config);
    expect(plan.counts).toEqual({ raw: 6, prunedInvalid: 0, duplicates: 0, finalUnique: 6 });
    const applied = plan.candidates
      .map((candidate) => `${candidate.strategy.fastMA}/${candidate.strategy.emaPeriod}`)
      .sort();
    expect(applied).toEqual(['5/20', '5/40', '7/20', '7/40', '9/20', '9/40']);
  });

  it('prunes cross-field-invalid combinations instead of rejecting the run', async () => {
    // slowMA stays 21, so fastMA 21 and 24 are not valid hypotheses.
    const plan = await enumerateCandidates(
      configWith([{ id: 'ma-cross', axes: [{ key: 'fastMA', min: 18, max: 24, step: 3 }] }]),
    );
    expect(plan.counts).toEqual({ raw: 3, prunedInvalid: 2, duplicates: 0, finalUnique: 1 });
    expect(plan.candidates[0].strategy.fastMA).toBe(18);
  });

  it('applies the fixed validity rule order', () => {
    const strategy = defaultStrategy();
    expect(candidateValidity(strategy)).toBeNull();
    expect(candidateValidity({ ...strategy, fastMA: 21 })).toBe('fastMA<slowMA');
    expect(candidateValidity({ ...strategy, macdFast: 26 })).toBe('macdFast<macdSlow');
    expect(candidateValidity({ ...strategy, rsiBuy: 70 })).toBe('rsiBuy<rsiSell');
    // First violation in declaration order wins.
    expect(candidateValidity({ ...strategy, fastMA: 21, rsiBuy: 70 })).toBe('fastMA<slowMA');
  });

  it('deduplicates identical hypotheses produced by different bases', async () => {
    const plan = await enumerateCandidates(
      configWith([
        { id: 'grid-low', axes: [{ key: 'fastMA', min: 5, max: 11, step: 3 }] },
        { id: 'grid-high', axes: [{ key: 'fastMA', min: 8, max: 14, step: 3 }] },
      ]),
    );
    // {5,8,11} + {8,11,14} = 6 raw, 2 repeats, 4 distinct hypotheses.
    expect(plan.counts).toEqual({ raw: 6, prunedInvalid: 0, duplicates: 2, finalUnique: 4 });
    expect(plan.candidates.map((candidate) => candidate.strategy.fastMA).sort((a, b) => a - b))
      .toEqual([5, 8, 11, 14]);
    // First occurrence keeps provenance, so the shared values stay on grid-low.
    const eight = plan.candidates.find((candidate) => candidate.strategy.fastMA === 8);
    expect(eight?.baseId).toBe('grid-low');
  });

  it('orders candidates by strategy hash and indexes them afterwards', async () => {
    const forward = await enumerateCandidates(
      configWith([
        { id: 'low', axes: [{ key: 'fastMA', min: 5, max: 6, step: 1 }] },
        { id: 'high', axes: [{ key: 'fastMA', min: 15, max: 16, step: 1 }] },
      ]),
    );
    const reversed = await enumerateCandidates(
      configWith([
        { id: 'high', axes: [{ key: 'fastMA', min: 15, max: 16, step: 1 }] },
        { id: 'low', axes: [{ key: 'fastMA', min: 5, max: 6, step: 1 }] },
      ]),
    );
    expect(forward.candidates.map((candidate) => candidate.strategyHash))
      .toEqual(reversed.candidates.map((candidate) => candidate.strategyHash));
    expect(forward.candidates.map((candidate) => candidate.index)).toEqual([0, 1, 2, 3]);
    expect(forward.candidates.map((candidate) => candidate.baseId))
      .toEqual(reversed.candidates.map((candidate) => candidate.baseId));

    const hashes = forward.candidates.map((candidate) => candidate.strategyHash);
    expect([...hashes].sort()).toEqual(hashes);
    for (const candidate of forward.candidates) {
      expect(candidate.strategyHash).toBe(
        await strategyHash(candidate.strategy, {
          feePct: candidate.strategy.feePct,
          slippagePct: candidate.strategy.slipPct,
        }),
      );
    }
  });

  it('fails closed above the candidate cap before building candidates', async () => {
    const config = configWith(
      [{ id: 'wide', axes: [{ key: 'fastMA', min: 1, max: 20, step: 1 }] }],
      { caps: { candidates: 10 } },
    );
    await expect(enumerateCandidates(config))
      .rejects.toThrow(/raw combination count 20 exceeds the candidate cap 10/);
  });

  it('fails closed when nothing survives pruning', async () => {
    await expect(enumerateCandidates(
      configWith([{ id: 'all-invalid', axes: [{ key: 'fastMA', min: 21, max: 24, step: 3 }] }]),
    )).rejects.toThrow(/enumeration produced no valid candidates/);
  });

  it('derives per-candidate seeds from durable identity only', async () => {
    const base: BaseSpec = { id: 'ma-cross', axes: [{ key: 'fastMA', min: 5, max: 11, step: 3 }] };
    const plan = await enumerateCandidates(configWith([base]));
    const seeds = plan.candidates.map((candidate) => candidate.seeds.randomEntry);
    expect(new Set(seeds).size).toBe(seeds.length);
    for (const seed of seeds) {
      expect(Number.isInteger(seed)).toBe(true);
      expect(seed).toBeGreaterThanOrEqual(0);
      expect(seed).toBeLessThanOrEqual(0xffff_ffff);
    }
    for (const candidate of plan.candidates) {
      expect(candidate.seeds.randomEntry).toBe(await deriveDiscoverySeed({
        rootSeed: plan.rootSeed,
        datasetContentHash: DATASET_HASH,
        strategyHash: candidate.strategyHash,
        purpose: 'random-entry',
      }));
    }

    const rerun = await enumerateCandidates(configWith([base]));
    expect(rerun.candidates).toEqual(plan.candidates);

    const otherSeed = await enumerateCandidates(configWith([base], { rootSeed: 20260727 }));
    expect(otherSeed.candidates.map((candidate) => candidate.strategyHash))
      .toEqual(plan.candidates.map((candidate) => candidate.strategyHash));
    expect(otherSeed.candidates.map((candidate) => candidate.seeds.randomEntry))
      .not.toEqual(seeds);

    const otherDataset = await enumerateCandidates(
      configWith([base], { dataset: { id: 3, contentHash: OTHER_DATASET_HASH } }),
    );
    expect(otherDataset.candidates.map((candidate) => candidate.seeds.randomEntry))
      .not.toEqual(seeds);
  });
});

describe('seed-v1 derivation', () => {
  const args = {
    rootSeed: 1,
    datasetContentHash: DATASET_HASH,
    strategyHash: `strategy-v2:${'d'.repeat(64)}`,
    purpose: 'random-entry',
  } as const;

  it('length-prefixes every field so no two inputs share a preimage', () => {
    const preimage = discoverySeedPreimage(args);
    // "seed-v1\0" + u32 rootSeed + (u32 + 83) + (u32 + 76) + (u32 + 12)
    expect(preimage.length).toBe(8 + 4 + (4 + 83) + (4 + 76) + (4 + 12));
    expect([...preimage.slice(0, 8)]).toEqual([...new TextEncoder().encode('seed-v1\0')]);
    expect([...preimage.slice(8, 12)]).toEqual([0, 0, 0, 1]);
  });

  it('rejects non-durable identities, out-of-range seeds, and unknown purposes', () => {
    expect(() => discoverySeedPreimage({ ...args, rootSeed: -1 }))
      .toThrow(/rootSeed must be an integer in \[0, 4294967295]/);
    expect(() => discoverySeedPreimage({ ...args, datasetContentHash: 'legacy' }))
      .toThrow(/datasetContentHash must be a durable dataset-content-v2 identity/);
    expect(() => discoverySeedPreimage({ ...args, strategyHash: 'ephemeral-fnv1a:0' }))
      .toThrow(/strategyHash must be a durable strategy-v2 identity/);
    expect(() => discoverySeedPreimage({
      ...args,
      purpose: 'gate' as unknown as typeof args.purpose,
    })).toThrow(/unsupported seed purpose "gate"/);
  });

  it('is stable and sensitive to every field', async () => {
    const seed = await deriveDiscoverySeed(args);
    expect(await deriveDiscoverySeed(args)).toBe(seed);
    expect(await deriveDiscoverySeed({ ...args, rootSeed: 2 })).not.toBe(seed);
    expect(await deriveDiscoverySeed({ ...args, datasetContentHash: OTHER_DATASET_HASH }))
      .not.toBe(seed);
    expect(await deriveDiscoverySeed({
      ...args,
      strategyHash: `strategy-v2:${'e'.repeat(64)}`,
    })).not.toBe(seed);
  });
});
