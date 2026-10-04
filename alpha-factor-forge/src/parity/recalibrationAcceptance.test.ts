// P12e-7a — the final acceptance of docs/plans/confirmation-recalibration-plan-v1.md
// §7 stays declared exactly as frozen. P12e-7b records the single full run;
// the Rust suite replays only each report's declared checkpoint.

import { describe, expect, it } from 'vitest';
import acceptance from '../../fixtures/research/recalibration-plan-v1-acceptance.json';
import output from '../../fixtures/research/recalibration-plan-v1-acceptance-output.json';
import diagnostics from '../../fixtures/research/recalibration-plan-v1-diagnostics.json';
import candidatesFixture from '../../fixtures/rs-core/research-confirmation-candidates-draft-v1.json';
import v2Fixture from '../../fixtures/rs-core/research-confirmation-statistics-v2.json';
import noiseV1Fixture from '../../fixtures/rs-core/research-noise-simulation-v1.json';
import noiseV2Fixture from '../../fixtures/rs-core/research-noise-simulation-v2.json';
import { referenceR3BlockLength } from './confirmationStatisticsV2Fixture';
import { referenceWilsonBoundsPpm } from './noiseSimulationV2Fixture';
import { lowestSupportedLength } from './recalibrationDiagnostics';

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

  it('isolates the final seed from unrelated fixtures after its one declared full run', () => {
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

  it('preserves every raw runner report and independently verifies all three checks per cell', () => {
    expect(output.runs.map((run) => run.id)).toEqual(acceptance.runs.map((run) => run.id));
    expect(acceptance.reports).toEqual(Object.fromEntries(output.runs.map(({ id, report }) => [id, report])));
    const within = (count: number, limit: number) => {
      const n = 20000n, l = BigInt(limit), x = BigInt(count);
      const gap = l * n - x * 1000000n;
      return gap > 0n && gap * gap * 10000n >= 38416n * l * (1000000n - l) * n;
    };
    for (const { id, report } of output.runs) {
      expect(report.statistic, id).toBe('research-confirmation-statistics-v2');
      expect(report.simulations, id).toBe(20000);
      expect(report.checkRule, id).toBe('wilson-upper-bound');
      expect(report.limitMultiplierPpm, id).toBe(1200000);
      expect(report.confirmations, id).toHaveLength(2);
      const checks = [...report.confirmations.map((row) => row.check), report.family];
      const limits = [30000, 30000, 60000];
      for (const [index, check] of checks.entries()) {
        expect(check.nominalPpm, id).toBe(index === 2 ? 50000 : 25000);
        expect(check.limitPpm, id).toBe(limits[index]);
        expect(check.ratePpm, id).toBe(Math.floor(check.count * 1000000 / 20000));
        expect([check.wilsonLowerPpm, check.wilsonUpperPpm], id).toEqual(referenceWilsonBoundsPpm(check.count, 20000));
        expect(check.withinLimit, id).toBe(within(check.count, limits[index]));
      }
      expect(report.status, id).toBe(checks.every((check, index) => within(check.count, limits[index])) ? 'WITHIN_LIMITS' : 'EXCEEDS_LIMITS');
      const declaration = acceptance.runs.find((run) => run.id === id)!.declaration;
      expect(report.checkpoints.map((row) => row.simulations), id).toEqual(declaration.checkpoints);
      for (const [index, row] of report.confirmations.entries()) {
        expect(row.confirmationNumber, id).toBe(index + 1);
        expect(row.alphaPpm, id).toBe(25000);
        expect(row.familyTests, id).toBe((index + 1) * 2);
        expect(row.check.count, id).toBe(row.counts.rejectingSimulations);
      }
    }
    const cells = acceptance.runs.map((run) => ({
      bars: run.declaration.bars, autocorrelationPpm: run.declaration.autocorrelationPpm,
      passes: output.runs.find((entry) => entry.id === run.id)!.report.status === 'WITHIN_LIMITS',
    }));
    const lowest = lowestSupportedLength(cells);
    expect(acceptance.result).toEqual({
      lowestSupportedTestedLength: lowest,
      supportedTestedLengths: lowest == null ? [] : [256, 512, 1024].filter((bars) => bars >= lowest),
    });
  });
});
