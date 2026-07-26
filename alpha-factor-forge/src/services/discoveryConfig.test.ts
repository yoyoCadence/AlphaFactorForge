import { describe, expect, it } from 'vitest';
import { DEFAULT_GATE_CONFIG } from './gate';
import { DEFAULT_SCORE_CONFIG } from './score';
import { defaultStrategy } from './strategy';
import { MAX_RANDOM_ENTRY_RUNS } from './randomEntry';
import {
  DISCOVERY_AXIS_KEYS,
  DISCOVERY_CONFIG_VERSION,
  DISCOVERY_CONTRACT_VERSIONS,
  DISCOVERY_DEFAULT_CANDIDATE_CAP,
  DISCOVERY_HARD_CANDIDATE_CAP,
  DISCOVERY_MAX_AXIS_VALUES,
  DISCOVERY_PRESET_VERSION,
  axisValues,
  parseDiscoveryConfig,
  resolveConcurrency,
} from './discoveryConfig';

const DATASET_HASH = `dataset-content-v2:${'a'.repeat(64)}`;

/** A minimal valid envelope; tests mutate a deep clone of it. */
function validConfig(): Record<string, unknown> {
  return {
    envelopeVersion: DISCOVERY_CONFIG_VERSION,
    contracts: { ...DISCOVERY_CONTRACT_VERSIONS },
    dataset: { id: 7, contentHash: DATASET_HASH },
    bases: [
      {
        id: 'ma-cross',
        presetVersion: DISCOVERY_PRESET_VERSION,
        strategy: { ...defaultStrategy() },
        axes: [{ key: 'fastMA', min: 5, max: 11, step: 3 }],
      },
    ],
    embargo: { holdingAllowanceBars: 10 },
    execution: { startEquity: 10000 },
    benchmarkCosts: { feePct: 0.05, slipPct: 0.02 },
    randomEntry: { runs: 200 },
    gateConfig: { ...DEFAULT_GATE_CONFIG },
    scoreConfig: {
      caps: { ...DEFAULT_SCORE_CONFIG.caps },
      weights: { ...DEFAULT_SCORE_CONFIG.weights },
    },
    rootSeed: 123456789,
    caps: { candidates: DISCOVERY_DEFAULT_CANDIDATE_CAP },
    maxConcurrency: null,
  };
}

function parse(mutate: (config: Record<string, unknown>) => void = () => {}, cores = 8) {
  const config = JSON.parse(JSON.stringify(validConfig())) as Record<string, unknown>;
  mutate(config);
  return parseDiscoveryConfig(config, { logicalCores: cores });
}

describe('discovery-config-v1 parsing', () => {
  it('resolves a valid envelope and echoes the pinned contract versions', () => {
    const resolved = parse();
    expect(resolved.envelopeVersion).toBe('discovery-config-v1');
    expect(resolved.contracts).toEqual(DISCOVERY_CONTRACT_VERSIONS);
    expect(resolved.dataset).toEqual({ id: 7, contentHash: DATASET_HASH });
    expect(resolved.bases).toHaveLength(1);
    expect(resolved.bases[0].strategy.mode).toBe('params');
    expect(resolved.gateConfig).toEqual(DEFAULT_GATE_CONFIG);
    expect(resolved.scoreConfig).toEqual(DEFAULT_SCORE_CONFIG);
    expect(resolved.caps.candidates).toBe(DISCOVERY_DEFAULT_CANDIDATE_CAP);
    expect(resolved.concurrency).toEqual({ requested: null, resolved: 7, logicalCores: 8 });
  });

  it('accepts the owning modules current defaults (drift canary)', () => {
    // If gate.ts or score.ts ever ships a default its own validator would
    // reject, this fails here instead of at run time.
    expect(() => parse((config) => {
      config.gateConfig = { ...DEFAULT_GATE_CONFIG };
      config.scoreConfig = {
        caps: { ...DEFAULT_SCORE_CONFIG.caps },
        weights: { ...DEFAULT_SCORE_CONFIG.weights },
      };
    })).not.toThrow();
  });

  it('rejects unknown keys, missing keys, and a version mismatch', () => {
    expect(() => parse((config) => { (config as Record<string, unknown>).extra = 1; }))
      .toThrow(/has unknown key "extra"/);
    expect(() => parse((config) => { delete config.rootSeed; }))
      .toThrow(/is missing key "rootSeed"/);
    expect(() => parse((config) => { config.envelopeVersion = 'discovery-config-v2'; }))
      .toThrow(/envelopeVersion must be "discovery-config-v1"/);
    expect(() => parse((config) => {
      (config.contracts as Record<string, string>).gate = 'gate-v2';
    })).toThrow(/contracts\.gate must be "gate-v1"/);
    expect(() => parse((config) => {
      (config.bases as Record<string, unknown>[])[0].presetVersion = 'preset-v9';
    })).toThrow(/presetVersion must be "discovery-preset-v1"/);
  });

  it('rejects non-params candidate modes here, not deeper in the engine', () => {
    for (const mode of ['blocks', 'code']) {
      expect(() => parse((config) => {
        ((config.bases as Record<string, unknown>[])[0].strategy as Record<string, unknown>).mode =
          mode;
      })).toThrow(/mode must be "params"/);
    }
  });

  it('rejects unsupported signal ids and out-of-domain preset numbers', () => {
    expect(() => parse((config) => {
      ((config.bases as Record<string, unknown>[])[0].strategy as Record<string, unknown>).entrySig =
        'stochOversold';
    })).toThrow(/entrySig must be one of/);
    expect(() => parse((config) => {
      ((config.bases as Record<string, unknown>[])[0].strategy as Record<string, unknown>).fastMA =
        0;
    })).toThrow(/fastMA must be an integer >= 1/);
    expect(() => parse((config) => {
      ((config.bases as Record<string, unknown>[])[0].strategy as Record<string, unknown>).bbMult =
        0;
    })).toThrow(/bbMult must be > 0/);
    expect(() => parse((config) => {
      ((config.bases as Record<string, unknown>[])[0].strategy as Record<string, unknown>).sizePct =
        0;
    })).toThrow(/sizePct must be in \(0, 100]/);
  });

  it('keeps cost and sizing fields off the axis whitelist', () => {
    expect(DISCOVERY_AXIS_KEYS).not.toContain('feePct');
    expect(DISCOVERY_AXIS_KEYS).not.toContain('slipPct');
    expect(DISCOVERY_AXIS_KEYS).not.toContain('sizePct');
    expect(() => parse((config) => {
      (config.bases as Record<string, unknown>[])[0].axes = [
        { key: 'feePct', min: 0, max: 0.1, step: 0.05 },
      ];
    })).toThrow(/key must be one of/);
  });

  it('rejects malformed, repeated, and over-cap axes', () => {
    const withAxes = (axes: unknown[]) => parse((config) => {
      (config.bases as Record<string, unknown>[])[0].axes = axes;
    });
    expect(() => withAxes([{ key: 'fastMA', min: 5, max: 11, step: 0 }])).toThrow(/step must be > 0/);
    expect(() => withAxes([{ key: 'fastMA', min: 11, max: 5, step: 1 }]))
      .toThrow(/max must be >= min/);
    expect(() => withAxes([{ key: 'fastMA', min: 5, max: 11, step: 0.5 }]))
      .toThrow(/must be an integer for the integer axis "fastMA"/);
    expect(() => withAxes([
      { key: 'fastMA', min: 5, max: 11, step: 3 },
      { key: 'fastMA', min: 5, max: 11, step: 3 },
    ])).toThrow(/repeats axis key "fastMA"/);
    expect(() => withAxes([{ key: 'fastMA', min: 1, max: 1000, step: 1 }]))
      .toThrow(new RegExp(`more than ${DISCOVERY_MAX_AXIS_VALUES} values`));
    expect(() => withAxes([{ key: 'rsiBuy', min: 90, max: 110, step: 10 }]))
      .toThrow(/generates an invalid value: rsiBuy must be in \[0, 100]/);
  });

  it('generates inclusive axis values by multiplication', () => {
    expect(axisValues({ key: 'fastMA', min: 5, max: 11, step: 3 })).toEqual([5, 8, 11]);
    expect(axisValues({ key: 'fastMA', min: 5, max: 12, step: 3 })).toEqual([5, 8, 11]);
    expect(axisValues({ key: 'fastMA', min: 9, max: 9, step: 1 })).toEqual([9]);
    expect(axisValues({ key: 'bbMult', min: 1.5, max: 2.5, step: 0.5 })).toEqual([1.5, 2, 2.5]);
  });

  it('requires resolved benchmark costs to match every base preset', () => {
    expect(() => parse((config) => {
      config.benchmarkCosts = { feePct: 0.01, slipPct: 0.02 };
    })).toThrow(/benchmarkCosts must match bases\[0] costs/);
  });

  it('bounds dataset identity, seeds, runs, allowance, equity, and caps', () => {
    expect(() => parse((config) => {
      (config.dataset as Record<string, unknown>).contentHash = 'legacy-unversioned';
    })).toThrow(/must be a durable dataset-content-v2 identity/);
    expect(() => parse((config) => { config.rootSeed = -1; }))
      .toThrow(/rootSeed must be an integer in \[0, 4294967295]/);
    expect(() => parse((config) => { config.rootSeed = 4294967296; }))
      .toThrow(/rootSeed must be an integer in \[0, 4294967295]/);
    expect(() => parse((config) => {
      (config.randomEntry as Record<string, unknown>).runs = MAX_RANDOM_ENTRY_RUNS + 1;
    })).toThrow(/runs must be an integer in \[1, 1000]/);
    expect(() => parse((config) => {
      (config.embargo as Record<string, unknown>).holdingAllowanceBars = -1;
    })).toThrow(/holdingAllowanceBars must be an integer in \[0,/);
    expect(() => parse((config) => {
      (config.execution as Record<string, unknown>).startEquity = 0;
    })).toThrow(/startEquity must be > 0/);
    expect(() => parse((config) => {
      (config.caps as Record<string, unknown>).candidates = DISCOVERY_HARD_CANDIDATE_CAP + 1;
    })).toThrow(new RegExp(`candidates must be an integer in \\[1, ${DISCOVERY_HARD_CANDIDATE_CAP}]`));
  });

  it('rejects non-finite numbers everywhere they can appear', () => {
    // JSON has no Infinity/NaN literal, so these arrive only from an in-memory
    // caller — the parser must still fail closed instead of trusting them.
    const config = validConfig();
    (config.execution as Record<string, unknown>).startEquity = Infinity;
    expect(() => parseDiscoveryConfig(config, { logicalCores: 4 }))
      .toThrow(/startEquity must be a finite number/);
  });

  it('propagates the gate and score validator messages verbatim', () => {
    expect(() => parse((config) => {
      (config.gateConfig as Record<string, unknown>).minTrades = 0;
    })).toThrow('minTrades must be a positive integer');
    expect(() => parse((config) => {
      (config.gateConfig as Record<string, unknown>).maxDrawdown = 1.5;
    })).toThrow('maxDrawdown must be a fraction in (0, 1]');
    expect(() => parse((config) => {
      (config.gateConfig as Record<string, unknown>).minRandomEntryPercentile = 101;
    })).toThrow('minRandomEntryPercentile must be in [0, 100]');
    expect(() => parse((config) => {
      ((config.scoreConfig as Record<string, unknown>).caps as Record<string, unknown>).cagr = 0;
    })).toThrow('cap cagr must be finite and > 0');
    expect(() => parse((config) => {
      ((config.scoreConfig as Record<string, unknown>).caps as Record<string, unknown>)
        .profitFactor = 1;
    })).toThrow('cap profitFactor must be > 1 (1 is the break-even floor)');
    expect(() => parse((config) => {
      ((config.scoreConfig as Record<string, unknown>).weights as Record<string, unknown>).cagr =
        -1;
    })).toThrow('weight cagr must be finite and >= 0');
    expect(() => parse((config) => {
      ((config.scoreConfig as Record<string, unknown>).weights as Record<string, unknown>).regime =
        0.1;
    })).toThrow('regime weight must stay 0 until REGIME-001 implements the regime classifier');
  });

  it('resolves concurrency from logical cores and bounds an override', () => {
    expect(resolveConcurrency(null, 1)).toBe(1);
    expect(resolveConcurrency(null, 2)).toBe(1);
    expect(resolveConcurrency(null, 16)).toBe(15);
    expect(resolveConcurrency(1, 4)).toBe(1);
    expect(resolveConcurrency(4, 4)).toBe(4);
    expect(() => resolveConcurrency(0, 4)).toThrow(/maxConcurrency must be an integer in \[1, 4]/);
    expect(() => resolveConcurrency(5, 4)).toThrow(/maxConcurrency must be an integer in \[1, 4]/);
    expect(() => resolveConcurrency(null, 0)).toThrow(/logicalCores must be an integer >= 1/);
    expect(parse((config) => { config.maxConcurrency = 3; }, 4).concurrency)
      .toEqual({ requested: 3, resolved: 3, logicalCores: 4 });
    expect(() => parse((config) => { config.maxConcurrency = 'auto'; }))
      .toThrow(/maxConcurrency must be a number or null/);
  });

  it('rejects duplicate base ids and an empty base list', () => {
    expect(() => parse((config) => { config.bases = []; }))
      .toThrow(/bases must contain at least one base preset/);
    expect(() => parse((config) => {
      const base = (config.bases as unknown[])[0];
      config.bases = [base, JSON.parse(JSON.stringify(base))];
    })).toThrow(/repeats base id "ma-cross"/);
    expect(() => parse((config) => {
      (config.bases as Record<string, unknown>[])[0].id = 'MA Cross';
    })).toThrow(/id must match/);
  });
});
