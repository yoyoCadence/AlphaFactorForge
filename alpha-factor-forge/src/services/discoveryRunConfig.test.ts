import { describe, expect, it } from 'vitest';
import {
  DISCOVERY_DEFAULT_BASE_ID,
  DISCOVERY_DEFAULT_START_EQUITY,
  DISCOVERY_ENVELOPE_KEYS,
  buildDiscoveryConfig,
  randomRootSeed,
  type BuildDiscoveryConfigInput,
  type DiscoveryRunOptions,
} from './discoveryRunConfig';
import {
  DISCOVERY_CONFIG_VERSION,
  DISCOVERY_CONTRACT_VERSIONS,
  DISCOVERY_DEFAULT_CANDIDATE_CAP,
  DISCOVERY_PRESET_VERSION,
  parseDiscoveryConfig,
  type DiscoveryAxis,
} from './discoveryConfig';
import { DEFAULT_GATE_CONFIG } from './gate';
import { DEFAULT_SCORE_CONFIG } from './score';
import { DEFAULT_RANDOM_ENTRY_RUNS } from './randomEntry';
import { MAX_U32 } from './discoverySeed';
import { defaultStrategy, type ParamsStrategy } from './strategy';

const DATASET = { id: 7, hash: `dataset-content-v2:${'a'.repeat(64)}` };
const AXIS: DiscoveryAxis = { key: 'fastMA', min: 5, max: 11, step: 3 };

function input(overrides: {
  strategy?: Partial<ParamsStrategy>;
  options?: Partial<DiscoveryRunOptions>;
  dataset?: Partial<typeof DATASET>;
  logicalCores?: number;
} = {}): BuildDiscoveryConfigInput {
  return {
    dataset: { ...DATASET, ...overrides.dataset },
    strategy: { ...defaultStrategy(), ...overrides.strategy },
    options: {
      axes: [AXIS],
      holdingAllowanceBars: 0,
      rootSeed: 123_456_789,
      ...overrides.options,
    },
    logicalCores: overrides.logicalCores ?? 8,
  };
}

describe('buildDiscoveryConfig', () => {
  it('produces an envelope the shared admission parser accepts', () => {
    const { envelope, resolved } = buildDiscoveryConfig(input());
    // Built once, validated by the same parser the backend mirrors — and
    // re-parsed here so the returned envelope, not just an internal value, is
    // proven admissible.
    expect(() => parseDiscoveryConfig(envelope, { logicalCores: 8 })).not.toThrow();
    expect(resolved.envelopeVersion).toBe(DISCOVERY_CONFIG_VERSION);
    expect(resolved.dataset).toEqual({ id: 7, contentHash: DATASET.hash });
    expect(resolved.bases).toHaveLength(1);
    expect(resolved.bases[0].id).toBe(DISCOVERY_DEFAULT_BASE_ID);
    expect(resolved.bases[0].presetVersion).toBe(DISCOVERY_PRESET_VERSION);
    expect(resolved.bases[0].axes).toEqual([AXIS]);
  });

  // The envelope is an exact-key contract: an added or missing key is rejected
  // wholesale, so the key set is pinned rather than assumed.
  it('emits exactly the thirteen envelope keys', () => {
    const { envelope } = buildDiscoveryConfig(input());
    expect(Object.keys(envelope).sort()).toEqual([...DISCOVERY_ENVELOPE_KEYS].sort());
  });

  it('copies the pinned contract versions instead of restating them', () => {
    const { envelope } = buildDiscoveryConfig(input());
    expect(envelope.contracts).toEqual(DISCOVERY_CONTRACT_VERSIONS);
    // A copy, not the live object: mutating the envelope must not corrupt the
    // module-level constant for every later run.
    (envelope.contracts as Record<string, string>).metrics = 'metrics-v1';
    expect(DISCOVERY_CONTRACT_VERSIONS.metrics).toBe('metrics-v2');
  });

  it('takes Gate and Score from their owning modules', () => {
    const { resolved } = buildDiscoveryConfig(input());
    expect(resolved.gateConfig).toEqual(DEFAULT_GATE_CONFIG);
    expect(resolved.scoreConfig).toEqual(DEFAULT_SCORE_CONFIG);
  });

  // Benchmark costs must equal the base preset's, so they are derived. An
  // independent input could only ever drift out of agreement.
  it('derives benchmark costs from the base strategy', () => {
    const { envelope, resolved } = buildDiscoveryConfig(
      input({ strategy: { feePct: 0.1, slipPct: 0.03 } }),
    );
    expect(envelope.benchmarkCosts).toEqual({ feePct: 0.1, slipPct: 0.03 });
    expect(resolved.benchmarkCosts).toEqual({ feePct: 0.1, slipPct: 0.03 });
    expect(resolved.bases[0].strategy.feePct).toBe(0.1);
  });

  it('applies the documented defaults and honours explicit overrides', () => {
    const { resolved } = buildDiscoveryConfig(input());
    expect(resolved.execution.startEquity).toBe(DISCOVERY_DEFAULT_START_EQUITY);
    expect(resolved.randomEntry.runs).toBe(DEFAULT_RANDOM_ENTRY_RUNS);
    expect(resolved.caps.candidates).toBe(DISCOVERY_DEFAULT_CANDIDATE_CAP);
    // v1 never sends an explicit concurrency: the backend resolves it with its
    // own core count, so a locally validated number could still be rejected.
    expect(resolved.concurrency).toEqual({ requested: null, resolved: 7, logicalCores: 8 });

    const custom = buildDiscoveryConfig(input({
      options: {
        axes: [AXIS],
        holdingAllowanceBars: 12,
        rootSeed: 1,
        randomEntryRuns: 50,
        candidateCap: 32,
        startEquity: 25_000,
        baseId: 'ma-cross',
      },
    })).resolved;
    expect(custom.embargo.holdingAllowanceBars).toBe(12);
    expect(custom.randomEntry.runs).toBe(50);
    expect(custom.caps.candidates).toBe(32);
    expect(custom.execution.startEquity).toBe(25_000);
    expect(custom.bases[0].id).toBe('ma-cross');
  });

  it('detaches the strategy so a later edit cannot rewrite a submitted run', () => {
    const strategy = defaultStrategy();
    const { envelope, resolved } = buildDiscoveryConfig({
      dataset: DATASET,
      strategy,
      options: { axes: [AXIS], holdingAllowanceBars: 0, rootSeed: 7 },
      logicalCores: 8,
    });

    strategy.fastMA = 99;
    strategy.entryRules[0].r = 'ema';

    const sent = (envelope.bases as { strategy: ParamsStrategy }[])[0].strategy;
    expect(sent.fastMA).toBe(9);
    expect(sent.entryRules[0].r).toBe('maSlow');
    expect(resolved.bases[0].strategy.fastMA).toBe(9);
  });

  it('stays JSON-serializable, because the envelope crosses the invoke boundary', () => {
    const { envelope } = buildDiscoveryConfig(input());
    expect(JSON.parse(JSON.stringify(envelope))).toEqual(envelope);
  });
});

describe('rejection before any invoke', () => {
  // Every case below would otherwise have been a backend rejection AFTER the
  // command was called.
  // Not a rejection, and deliberately pinned as such: a base with no axes is a
  // legal single-candidate run (validate this exact strategy end to end). The
  // builder must not invent a rule the admission contract does not have — if
  // the panel wants to require an axis, that is a product decision in the UI.
  it('accepts an empty axis list as a single-candidate run', () => {
    const { resolved } = buildDiscoveryConfig(
      input({ options: { axes: [], holdingAllowanceBars: 0, rootSeed: 1 } }),
    );
    expect(resolved.bases[0].axes).toEqual([]);
  });

  it('rejects a fractional bound on an integer axis', () => {
    expect(() => buildDiscoveryConfig(input({
      options: { axes: [{ key: 'fastMA', min: 5, max: 11.5, step: 3 }], holdingAllowanceBars: 0, rootSeed: 1 },
    }))).toThrow(/integer axis/);
  });

  it('rejects a non-positive axis step', () => {
    expect(() => buildDiscoveryConfig(input({
      options: { axes: [{ key: 'fastMA', min: 5, max: 11, step: 0 }], holdingAllowanceBars: 0, rootSeed: 1 },
    }))).toThrow(/step must be > 0/);
  });

  // The STRATEGY-VALIDATION-001 rule set reaches discovery for free: the
  // envelope parser runs `checkNumericParam` over the base preset.
  it('rejects a base strategy whose indicator period cannot produce a series', () => {
    expect(() => buildDiscoveryConfig(input({ strategy: { fastMA: 0 } }))).toThrow(/fastMA/);
    expect(() => buildDiscoveryConfig(input({ strategy: { bbMult: 0 } }))).toThrow(/bbMult/);
  });

  it('rejects a non-params base strategy', () => {
    expect(() => buildDiscoveryConfig(input({ strategy: { mode: 'blocks' } }))).toThrow(/params/);
  });

  it('rejects a root seed outside the u32 range', () => {
    expect(() => buildDiscoveryConfig(input({ options: { axes: [AXIS], holdingAllowanceBars: 0, rootSeed: -1 } })))
      .toThrow(/rootSeed/);
    expect(() => buildDiscoveryConfig(input({
      options: { axes: [AXIS], holdingAllowanceBars: 0, rootSeed: MAX_U32 + 1 },
    }))).toThrow(/rootSeed/);
  });

  it('rejects a negative holding allowance', () => {
    expect(() => buildDiscoveryConfig(input({
      options: { axes: [AXIS], holdingAllowanceBars: -1, rootSeed: 1 },
    }))).toThrow(/holdingAllowanceBars/);
  });

  it('rejects a dataset identity that is not a dataset-content-v2 hash', () => {
    expect(() => buildDiscoveryConfig(input({ dataset: { hash: 'sha256:abc' } }))).toThrow(/contentHash/);
  });

  // A KNOWN BOUNDARY of this module, pinned so nobody assumes otherwise: the
  // candidate cap is enforced by `enumerateCandidates`, not by envelope
  // admission, so an over-budget grid builds here and is rejected by the
  // backend — before any candidate or job row is created, per RUNNER-CONFIG-001.
  // Surfacing the projected count in the panel is slice b-2's job; inventing a
  // second cap check here would create a second authority.
  it('still builds an over-cap grid, because the enumerator owns that check', () => {
    const overBudget = input({
      options: {
        axes: [
          { key: 'fastMA', min: 1, max: 64, step: 1 },
          { key: 'slowMA', min: 1, max: 64, step: 1 },
        ],
        holdingAllowanceBars: 0,
        rootSeed: 1,
        candidateCap: 256,
      },
    });
    const { resolved } = buildDiscoveryConfig(overBudget);
    expect(resolved.caps.candidates).toBe(256);
    // 64 x 64 = 4096 raw combinations against a 256 cap: admissible envelope,
    // impossible run.
    expect(resolved.bases[0].axes).toHaveLength(2);
  });

  // The per-axis limit IS an envelope rule, so that one does fail here.
  it('rejects an axis with more values than the per-axis limit', () => {
    expect(() => buildDiscoveryConfig(input({
      options: {
        axes: [{ key: 'fastMA', min: 1, max: 500, step: 1 }],
        holdingAllowanceBars: 0,
        rootSeed: 1,
      },
    }))).toThrow(RangeError);
  });
});

describe('randomRootSeed', () => {
  it('stays inside the u32 range at both extremes of the generator', () => {
    expect(randomRootSeed(() => 0)).toBe(0);
    expect(randomRootSeed(() => 0.9999999999)).toBe(MAX_U32);
    // Math.random() is exclusive of 1, but a caller-supplied generator might
    // not be: the seed must remain admissible either way.
    expect(randomRootSeed(() => 1)).toBeLessThanOrEqual(MAX_U32);
  });

  it('produces a seed the envelope accepts', () => {
    const rootSeed = randomRootSeed(() => 0.5);
    expect(() => buildDiscoveryConfig(input({ options: { axes: [AXIS], holdingAllowanceBars: 0, rootSeed } })))
      .not.toThrow();
  });
});
