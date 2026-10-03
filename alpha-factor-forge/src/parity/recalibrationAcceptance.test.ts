// P12e-7a — the final acceptance of docs/plans/confirmation-recalibration-plan-v1.md
// §7 is declared for the frozen v2 exactly as the plan states, and has not
// been run. The Rust suite parses each declaration with the engine.

import { describe, expect, it } from 'vitest';
import acceptance from '../../fixtures/research/recalibration-plan-v1-acceptance.json';
import diagnostics from '../../fixtures/research/recalibration-plan-v1-diagnostics.json';
import candidatesFixture from '../../fixtures/rs-core/research-confirmation-candidates-draft-v1.json';
import v2Fixture from '../../fixtures/rs-core/research-confirmation-statistics-v2.json';
import noiseV1Fixture from '../../fixtures/rs-core/research-noise-simulation-v1.json';
import noiseV2Fixture from '../../fixtures/rs-core/research-noise-simulation-v2.json';
import { referenceR3BlockLength } from './confirmationStatisticsV2Fixture';
import { referenceWilsonBoundsPpm } from './noiseSimulationV2Fixture';

describe('recalibration plan v1 final acceptance', () => {
  it('declares the six cells of §7 for the selected method', () => {
    expect(diagnostics.selection).toEqual({ candidate: 'S2', rule: 'R3' });
    const expected = [0, 300000].flatMap((autocorrelationPpm) =>
      [256, 512, 1024].map((bars) => ({
        id: `acceptance-v2-phi${autocorrelationPpm / 1000}-n${bars}`,
        declaration: {
          contractVersion: 'research-noise-simulation-v2',
          statistic: 'research-confirmation-statistics-v2',
          noiseModel: 'ar1-uniform-sum',
          autocorrelationPpm,
          effectMillionths: 0,
          bars,
          candidatesPerConfirmation: 1,
          priorTrials: 0,
          blockLength: referenceR3BlockLength(bars),
          bootstrapSamples: 799,
          simulations: 20000,
          seed: 20261117,
          check: { rule: 'wilson-upper-bound', limitMultiplierPpm: 1200000 },
          checkpoints: [4096 / bars],
          allocation: {
            contractVersion: 'research-alpha-allocation-v1',
            rule: 'declared-schedule',
            scope: 'trial-family',
            totalAlphaPpm: 50000,
            schedule: [25000, 25000],
          },
        },
      })),
    );
    expect(acceptance.runs).toEqual(expected);
    // The diagnostics ran the selected pair with the same block lengths.
    for (const run of acceptance.runs) {
      const diagnostic = diagnostics.size.find(
        (entry) =>
          entry.candidate === 'S2' &&
          entry.rule === 'R3' &&
          entry.declaration.bars === run.declaration.bars &&
          entry.declaration.autocorrelationPpm === run.declaration.autocorrelationPpm,
      );
      expect(diagnostic?.declaration.blockLength).toBe(run.declaration.blockLength);
    }
  });

  it('states the rule’s maxima as the Wilson bound gives them', () => {
    const { rule } = acceptance;
    expect(referenceWilsonBoundsPpm(rule.maximumConfirmationCount, 20000)[1]).toBeLessThanOrEqual(
      rule.confirmationLimitPpm,
    );
    expect(referenceWilsonBoundsPpm(rule.maximumConfirmationCount + 1, 20000)[1]).toBeGreaterThan(
      rule.confirmationLimitPpm,
    );
    expect(referenceWilsonBoundsPpm(rule.maximumFamilyCount, 20000)[1]).toBeLessThanOrEqual(
      rule.familyLimitPpm,
    );
    expect(referenceWilsonBoundsPpm(rule.maximumFamilyCount + 1, 20000)[1]).toBeGreaterThan(
      rule.familyLimitPpm,
    );
  });

  it('has not been run, and no other fixture uses the acceptance seed', () => {
    expect('reports' in acceptance).toBe(false);
    // Every `seed` field anywhere in a document (descriptions may name it).
    const seeds = (value: unknown): unknown[] =>
      Array.isArray(value)
        ? value.flatMap(seeds)
        : value !== null && typeof value === 'object'
          ? Object.entries(value).flatMap(([key, entry]) =>
              key === 'seed' ? [entry, ...seeds(entry)] : seeds(entry),
            )
          : [];
    expect(new Set(seeds(acceptance))).toEqual(new Set([20261117]));
    for (const other of [diagnostics, candidatesFixture, v2Fixture, noiseV1Fixture, noiseV2Fixture]) {
      const found = seeds(other);
      expect(found.length).toBeGreaterThan(0);
      expect(found).not.toContain(20261117);
    }
  });
});
