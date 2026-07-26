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

/**
 * Every expected leaf in this fixture is exact, so no float may be rounded,
 * non-finite, or negative zero.
 *
 * MUST be run against the IN-MEMORY builder output, never against the parsed
 * artifact: `JSON.stringify(-0)` is `"0"`, so a negative zero is erased the
 * moment the fixture is written and an assertion on the file could never
 * observe one.
 */
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
    // exact-v1 is checked on the LIVE object first. Doing it after the round
    // trip below would be vacuous: stringify turns -0 into 0.
    expectExactJson(regenerated);
    expect(JSON.parse(JSON.stringify(regenerated))).toEqual(fixture);
  });

  it('rejects -0, NaN, and Infinity anywhere in an expected value', () => {
    // Negative controls for the guard above. Without them the guard could be
    // silently vacuous and every "exact" claim in this file would be untested.
    expect(() => expectExactJson({ ok: 0, nested: [1, { deep: 2.5 }] })).not.toThrow();
    for (const bad of [-0, NaN, Infinity, -Infinity]) {
      expect(() => expectExactJson(bad)).toThrow();
      expect(() => expectExactJson({ leaf: bad })).toThrow();
      expect(() => expectExactJson([1, [bad]])).toThrow();
      expect(() => expectExactJson({ nested: { deep: [{ leaf: bad }] } })).toThrow();
    }
    // Proof that the round trip is what hides a -0, i.e. why order matters.
    expect(JSON.stringify(-0)).toBe('0');
    expect(() => expectExactJson(JSON.parse(JSON.stringify({ leaf: -0 })))).not.toThrow();
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
      'seed-empty-dataset-digest',
      'seed-truncated-strategy-digest',
      'seed-uppercase-strategy-digest',
      'seed-non-hex-strategy-digest',
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
    // The EXACT ordered admission-rejection inventory, not just its size: a
    // deleted case must fail here rather than be silently replaced by a new
    // one that happens to keep the count.
    expect(fixture.configErrorCases.map((parityCase) => parityCase.id)).toEqual([
      'config-unknown-envelope-key',
      'config-unknown-key-utf8-order',
      'config-missing-envelope-key',
      'config-envelope-version-mismatch',
      'config-contract-version-mismatch',
      'config-preset-version-mismatch',
      'config-dataset-legacy-hash',
      'config-dataset-empty-digest',
      'config-dataset-uppercase-digest',
      'config-dataset-id-zero',
      'config-blocks-mode-rejected',
      'config-code-mode-rejected',
      'config-unsupported-signal',
      'config-unknown-fill-mode',
      'config-strategy-unknown-key',
      'config-strategy-missing-key',
      'config-period-below-one',
      'config-period-fractional',
      'config-multiplier-not-positive',
      'config-size-out-of-range',
      'config-fee-percent-above-range',
      'config-slippage-percent-above-range',
      'config-stop-loss-percent-above-range',
      'config-take-profit-percent-negative',
      'config-axis-generates-percent-above-range',
      'config-level-out-of-range',
      'config-axis-key-not-whitelisted',
      'config-axis-step-not-positive',
      'config-axis-inverted-range',
      'config-axis-fractional-integer-bound',
      'config-axis-repeated-key',
      'config-axis-above-value-cap',
      'config-axis-generates-invalid-value',
      'config-empty-bases',
      'config-duplicate-base-id',
      'config-invalid-base-id',
      'config-benchmark-costs-mismatch',
      'config-benchmark-costs-percent-above-range',
      'config-benchmark-slippage-percent-negative',
      'config-random-entry-runs-above-cap',
      'config-negative-holding-allowance',
      'config-start-equity-zero',
      'config-candidate-cap-above-hard-cap',
      'config-root-seed-above-u32',
      'config-max-concurrency-string',
      'config-max-concurrency-above-cores',
      'config-gate-min-trades-invalid',
      'config-gate-fraction-invalid',
      'config-gate-percentile-invalid',
      'config-score-cap-invalid',
      'config-score-profit-factor-cap-invalid',
      'config-score-negative-weight',
      'config-score-regime-weight-deferred',
    ]);
    for (const parityCase of fixture.configErrorCases) {
      expect(parityCase.expectedErrorIncludes.length).toBeGreaterThan(0);
    }
    for (const mode of ['blocks', 'code']) {
      const parityCase = findById(fixture.configErrorCases, `config-${mode}-mode-rejected`);
      expect(parityCase.expectedErrorIncludes).toBe('mode must be "params"');
    }

    // 70 held rejections across the five groups; the docs quote this total.
    const heldErrors = [
      fixture.seedErrorCases,
      fixture.axisErrorCases,
      fixture.concurrencyErrorCases,
      fixture.configErrorCases,
      fixture.enumerationErrorCases,
    ].reduce((sum, group) => sum + group.length, 0);
    expect(heldErrors).toBe(70);
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

  it('bounds percent units at the engine fraction limit, not merely at zero', () => {
    // 101 / 100 = 1.01, which the engine's assertNormalizedFraction rejects.
    // Admission must catch it, or a queued run throws after jobs exist.
    for (const id of [
      'config-fee-percent-above-range',
      'config-slippage-percent-above-range',
      'config-stop-loss-percent-above-range',
      'config-take-profit-percent-negative',
      'config-axis-generates-percent-above-range',
    ]) {
      expect(findById(fixture.configErrorCases, id).expectedErrorIncludes)
        .toContain('must be in [0, 100]');
    }
  });

  it('requires a full lowercase hex digest, not just a version prefix', () => {
    for (const id of [
      'seed-empty-dataset-digest',
      'seed-truncated-strategy-digest',
      'seed-uppercase-strategy-digest',
      'seed-non-hex-strategy-digest',
    ]) {
      expect(findById(fixture.seedErrorCases, id).expectedErrorIncludes)
        .toContain('must be a durable');
    }
    for (const id of ['config-dataset-empty-digest', 'config-dataset-uppercase-digest']) {
      expect(findById(fixture.configErrorCases, id).expectedErrorIncludes)
        .toContain('must be a durable dataset-content-v2 identity');
    }
    for (const parityCase of fixture.seedCases) {
      expect(parityCase.input.datasetContentHash).toMatch(/^dataset-content-v2:[0-9a-f]{64}$/);
      expect(parityCase.input.strategyHash).toMatch(/^strategy-v2:[0-9a-f]{64}$/);
    }
  });

  it('names unknown keys in UTF-8 byte order, where UTF-16 would disagree', () => {
    const parityCase = findById(fixture.configErrorCases, 'config-unknown-key-utf8-order');
    const keys = ['\u{1F600}', '＀'];
    // JavaScript's default sort puts the emoji first; UTF-8 bytes put it last.
    expect([...keys].sort()).toEqual(['\u{1F600}', '＀']);
    expect(parityCase.expectedErrorIncludes).toBe('discoveryConfig has unknown key "＀"');
    for (const key of keys) {
      expect(Object.prototype.hasOwnProperty.call(parityCase.input, key)).toBe(true);
    }
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
