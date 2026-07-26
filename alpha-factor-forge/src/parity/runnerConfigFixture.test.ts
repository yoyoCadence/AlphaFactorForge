import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/runner-config-v1.json';
import { sha256Hex } from '../core/hashing';
import candidateEnumerationSource from '../services/candidateEnumeration.ts?raw';
import discoveryConfigSource from '../services/discoveryConfig.ts?raw';
import discoverySeedSource from '../services/discoverySeed.ts?raw';
import gateSource from '../services/gate.ts?raw';
import hashingSource from '../core/hashing/index.ts?raw';
import randomEntrySource from '../services/randomEntry.ts?raw';
import scoreSource from '../services/score.ts?raw';
import strategySource from '../services/strategy.ts?raw';
import generatorSource from './runnerConfigFixture.ts?raw';
import { canonicalizeFixtureSource, FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import {
  EXPECTED_NUMERIC_POLICY,
  buildRunnerConfigParityFixture,
} from './runnerConfigFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

function findById<T extends { id: string }>(cases: T[], id: string): T {
  const found = cases.find((parityCase) => parityCase.id === id);
  if (!found) throw new Error(`missing fixture case ${id}`);
  return found;
}

/** Every expected leaf in this fixture is exact, so no float may be rounded,
 *  non-finite, or negative zero. */
function expectExactJson(value: unknown): void {
  if (typeof value === 'number') {
    expect(Number.isFinite(value)).toBe(true);
    expect(Object.is(value, -0)).toBe(false);
    return;
  }
  if (Array.isArray(value)) {
    for (const child of value) expectExactJson(child);
    return;
  }
  if (value !== null && typeof value === 'object') {
    for (const child of Object.values(value)) expectExactJson(child);
  }
}

describe('RUNNER-CONFIG parity fixture', () => {
  it('is exactly reproducible from canonical current TypeScript reference sources', async () => {
    const regenerated = await buildRunnerConfigParityFixture({
      generator: await hashSource(generatorSource),
      discoveryConfig: await hashSource(discoveryConfigSource),
      candidateEnumeration: await hashSource(candidateEnumerationSource),
      discoverySeed: await hashSource(discoverySeedSource),
      hashing: await hashSource(hashingSource),
      strategy: await hashSource(strategySource),
      gate: await hashSource(gateSource),
      score: await hashSource(scoreSource),
      randomEntry: await hashSource(randomEntrySource),
    });
    expect(JSON.parse(JSON.stringify(regenerated))).toEqual(fixture);
  });

  it('locks the envelope, caps, whitelists, and the exact-only numeric policy', () => {
    expect(fixture.schemaVersion).toBe('rs-core-parity-fixture-v1');
    expect(fixture.fixtureVersion).toBe('runner-config-parity-v1');
    expect(fixture.contracts).toEqual({
      config: 'discovery-config-v1',
      preset: 'discovery-preset-v1',
      enumeration: 'discovery-enumeration-v1',
      seed: 'seed-v1',
      strategyHash: 'strategy-v2',
      datasetHash: 'dataset-content-v2',
    });
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.expectedNumericPolicy).toBe(EXPECTED_NUMERIC_POLICY);
    expect(fixture.caps).toEqual({
      defaultCandidateCap: 256,
      hardCandidateCap: 4096,
      maxAxisValues: 64,
    });
    expect(fixture.validityRuleIds).toEqual([
      'fastMA<slowMA',
      'macdFast<macdSlow',
      'rsiBuy<rsiSell',
    ]);
    expect(fixture.supportedSignalIds).toEqual([
      'maCrossUp', 'maCrossDown',
      'emaCrossUp', 'emaCrossDown',
      'priceAboveSlow', 'priceBelowSlow',
      'rsiOversold', 'rsiOverbought',
      'macdCrossUp', 'macdCrossDown',
      'bbLowerTouch', 'bbUpperTouch',
    ]);
    // Execution/cost fields are never a hypothesis axis.
    expect(fixture.axisKeys).toEqual([
      'fastMA', 'slowMA', 'emaPeriod', 'rsiPeriod', 'rsiBuy', 'rsiSell',
      'macdFast', 'macdSlow', 'macdSignal', 'bbPeriod', 'bbMult', 'slPct', 'tpPct',
    ]);
    for (const excluded of ['feePct', 'slipPct', 'sizePct']) {
      expect(fixture.axisKeys).not.toContain(excluded);
    }
    expectExactJson(fixture.axisCases);
    expectExactJson(fixture.configCases);
    expectExactJson(fixture.enumerationCases);
  });

  it('locks the exact success and TypeScript-held error inventories', () => {
    expect(fixture.seedCases.map((parityCase) => parityCase.id)).toEqual([
      'seed-root-zero',
      'seed-root-max-u32',
      'seed-root-mid',
      'seed-other-strategy',
      'seed-other-dataset',
    ]);
    expect(fixture.seedErrorCases.map((parityCase) => parityCase.id)).toEqual([
      'seed-negative-root',
      'seed-root-above-u32',
      'seed-fractional-root',
      'seed-legacy-dataset-hash',
      'seed-ephemeral-strategy-hash',
      'seed-unknown-purpose',
    ]);
    expect(fixture.axisCases.map((parityCase) => parityCase.id)).toEqual([
      'axis-integer-inclusive',
      'axis-integer-truncated',
      'axis-single-value',
      'axis-float-exact-halves',
      'axis-float-binary-drift',
      'axis-max-values-boundary',
    ]);
    expect(fixture.axisErrorCases.map((parityCase) => parityCase.id))
      .toEqual(['axis-above-value-cap']);
    expect(fixture.concurrencyCases.map((parityCase) => parityCase.id)).toEqual([
      'concurrency-default-single-core',
      'concurrency-default-dual-core',
      'concurrency-default-many-cores',
      'concurrency-override-floor',
      'concurrency-override-all-cores',
    ]);
    expect(fixture.concurrencyErrorCases.map((parityCase) => parityCase.id)).toEqual([
      'concurrency-zero-override',
      'concurrency-above-cores',
      'concurrency-fractional-override',
      'concurrency-zero-cores',
    ]);
    expect(fixture.configCases.map((parityCase) => parityCase.id)).toEqual([
      'config-default-single-base',
      'config-multi-base-overrides',
    ]);
    expect(fixture.enumerationCases.map((parityCase) => parityCase.id)).toEqual([
      'enumerate-single-axis',
      'enumerate-multi-base-product',
      'enumerate-cross-field-prune',
      'enumerate-cross-base-duplicates',
      'enumerate-disjoint-bases',
      'enumerate-disjoint-bases-reversed',
    ]);
    expect(fixture.enumerationErrorCases.map((parityCase) => parityCase.id)).toEqual([
      'enumerate-above-candidate-cap',
      'enumerate-all-pruned',
    ]);
    // 43 admission rejections: envelope/version, dataset identity, candidate
    // mode, signal/domain, axis, base, cost, bound, concurrency, gate, score.
    expect(fixture.configErrorCases).toHaveLength(43);
    expect(new Set(fixture.configErrorCases.map((parityCase) => parityCase.id)).size).toBe(43);
    for (const parityCase of fixture.configErrorCases) {
      expect(parityCase.expectedErrorIncludes.length).toBeGreaterThan(0);
    }
    for (const mode of ['blocks', 'code']) {
      const parityCase = findById(fixture.configErrorCases, `config-${mode}-mode-rejected`);
      expect(parityCase.expectedErrorIncludes).toBe('mode must be "params"');
    }
  });

  it('locks the seed preimage bytes and derived u32 values', () => {
    const seedV1 = [...new TextEncoder().encode('seed-v1\0')]
      .map((byte) => byte.toString(16).padStart(2, '0'))
      .join('');
    for (const parityCase of fixture.seedCases) {
      expect(parityCase.expected.preimageHex.startsWith(seedV1)).toBe(true);
      expect(Number.isInteger(parityCase.expected.seed)).toBe(true);
      expect(parityCase.expected.seed).toBeGreaterThanOrEqual(0);
      expect(parityCase.expected.seed).toBeLessThanOrEqual(0xffff_ffff);
    }
    expect(findById(fixture.seedCases, 'seed-root-zero').expected.preimageHex.slice(16, 24))
      .toBe('00000000');
    expect(findById(fixture.seedCases, 'seed-root-max-u32').expected.preimageHex.slice(16, 24))
      .toBe('ffffffff');
    // Every field participates: no two cases may collide.
    const seeds = fixture.seedCases.map((parityCase) => parityCase.expected.seed);
    expect(new Set(seeds).size).toBe(seeds.length);
  });

  it('locks inclusive axis values including exact binary drift', () => {
    expect(findById(fixture.axisCases, 'axis-integer-inclusive').expected).toEqual([5, 8, 11]);
    expect(findById(fixture.axisCases, 'axis-integer-truncated').expected).toEqual([5, 8, 11]);
    expect(findById(fixture.axisCases, 'axis-single-value').expected).toEqual([9]);
    expect(findById(fixture.axisCases, 'axis-float-exact-halves').expected)
      .toEqual([1.5, 2, 2.5]);
    // `min + i*step`, never `+= step`: the third value is NOT 0.3.
    expect(findById(fixture.axisCases, 'axis-float-binary-drift').expected)
      .toEqual([0, 0.1, 0.2, 0.30000000000000004, 0.4, 0.5]);
    expect(findById(fixture.axisCases, 'axis-max-values-boundary').expected).toHaveLength(64);
  });

  it('locks resolved config output, including concurrency resolution', () => {
    const single = findById(fixture.configCases, 'config-default-single-base');
    expect(single.expected.envelopeVersion).toBe('discovery-config-v1');
    expect(single.expected.concurrency)
      .toEqual({ requested: null, resolved: 7, logicalCores: 8 });
    expect(single.expected.bases[0].strategy.mode).toBe('params');

    const multi = findById(fixture.configCases, 'config-multi-base-overrides');
    expect(multi.expected.bases).toHaveLength(2);
    expect(multi.expected.concurrency).toEqual({ requested: 3, resolved: 3, logicalCores: 4 });
    expect(multi.expected.rootSeed).toBe(0xffff_ffff);
    expect(multi.expected.caps.candidates).toBe(4096);
    expect(multi.expected.randomEntry.runs).toBe(1000);
    expect(multi.expected.embargo.holdingAllowanceBars).toBe(0);
  });

  it('locks enumeration counters, hash ordering, indexes, and derived N', () => {
    for (const parityCase of fixture.enumerationCases) {
      const plan = parityCase.expected;
      expect(plan.contractVersion).toBe('discovery-enumeration-v1');
      const { raw, prunedInvalid, duplicates, finalUnique } = plan.counts;
      expect(prunedInvalid + duplicates + finalUnique).toBe(raw);
      expect(plan.candidates).toHaveLength(finalUnique);
      expect(plan.testedCombinations).toEqual({ n: finalUnique, basis: 'lineage-final-unique' });
      expect(plan.candidates.map((candidate) => candidate.index))
        .toEqual(plan.candidates.map((_, index) => index));
      const hashes = plan.candidates.map((candidate) => candidate.strategyHash);
      expect([...hashes].sort()).toEqual(hashes);
      expect(new Set(hashes).size).toBe(hashes.length);
      for (const candidate of plan.candidates) {
        expect(candidate.strategyHash.startsWith('strategy-v2:')).toBe(true);
        expect(candidate.strategy.mode).toBe('params');
      }
    }

    expect(findById(fixture.enumerationCases, 'enumerate-cross-field-prune').expected.counts)
      .toEqual({ raw: 3, prunedInvalid: 2, duplicates: 0, finalUnique: 1 });
    expect(findById(fixture.enumerationCases, 'enumerate-cross-base-duplicates').expected.counts)
      .toEqual({ raw: 6, prunedInvalid: 0, duplicates: 2, finalUnique: 4 });

    // Base declaration order must not change identity, index, or seed.
    const forward = findById(fixture.enumerationCases, 'enumerate-disjoint-bases').expected;
    const reversed = findById(
      fixture.enumerationCases,
      'enumerate-disjoint-bases-reversed',
    ).expected;
    expect(reversed.candidates).toEqual(forward.candidates);
    expect(reversed.counts).toEqual(forward.counts);
  });

  it('admits no blocks/code candidate and never plans a segment', () => {
    // Blocks/code appear ONLY as rejected inputs, never in an accepted output.
    const accepted = JSON.stringify({
      configCases: fixture.configCases.map((parityCase) => parityCase.expected),
      enumerationCases: fixture.enumerationCases.map((parityCase) => parityCase.expected),
    });
    for (const mode of ['blocks', 'code']) {
      expect(accepted).not.toContain(`"mode":"${mode}"`);
    }
    expect(
      fixture.configErrorCases.filter((parityCase) =>
        parityCase.expectedErrorIncludes.includes('mode must be "params"'),
      ),
    ).toHaveLength(2);

    // This slice enumerates hypotheses only; segments belong to later slices,
    // and the hidden Test segment is never planned or executed anywhere.
    const encoded = JSON.stringify(fixture);
    for (const forbidden of ['"train"', '"validation"', '"test"', 'segment']) {
      expect(encoded).not.toContain(forbidden);
    }
  });
});
