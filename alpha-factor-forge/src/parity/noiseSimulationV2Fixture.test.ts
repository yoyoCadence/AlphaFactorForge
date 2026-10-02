// P12e-5 — the committed fixture is what the independent reference computes,
// and the reference's Wilson bounds (found by bisection) agree with the plan.

import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-noise-simulation-v2.json';
import {
  referenceNoiseSimulationV2,
  referenceWilsonBoundsPpm,
  type NoiseSimulationV2Declaration,
  type NoiseSimulationV2Report,
} from './noiseSimulationV2Fixture';

interface FixtureCase {
  id: string;
  declaration: NoiseSimulationV2Declaration;
  expected: NoiseSimulationV2Report;
}

const cases = fixture.cases as FixtureCase[];

describe('research-noise-simulation-v2 fixture', () => {
  it.each(cases.map((testCase) => [testCase.id, testCase] as const))(
    '%s matches the reference',
    async (_, testCase) => {
      expect(await referenceNoiseSimulationV2(testCase.declaration)).toEqual(testCase.expected);
    },
  );

  it('never contains a cell of the recalibration plan', () => {
    for (const { declaration } of cases) {
      expect([20261005, 20261117]).not.toContain(declaration.seed);
      expect(declaration.simulations).toBeLessThan(100);
    }
  });

  it('finds the Wilson bounds the plan and the PR #135 review state', () => {
    // Plan §7: at most 552 and 1134 of 20,000 simulations.
    expect(referenceWilsonBoundsPpm(552, 20000)[1]).toBeLessThanOrEqual(30000);
    expect(referenceWilsonBoundsPpm(553, 20000)[1]).toBeGreaterThan(30000);
    expect(referenceWilsonBoundsPpm(1134, 20000)[1]).toBeLessThanOrEqual(60000);
    expect(referenceWilsonBoundsPpm(1135, 20000)[1]).toBeGreaterThan(60000);
    // The v1 acceptance results: 5.45%–7.61% and 3.99%–5.88%.
    expect(referenceWilsonBoundsPpm(129, 2000)).toEqual([54547, 76123]);
    expect(referenceWilsonBoundsPpm(97, 2000)).toEqual([39919, 58812]);
    // The ends of the scale.
    expect(referenceWilsonBoundsPpm(0, 5)).toEqual([0, 434492]);
    expect(referenceWilsonBoundsPpm(5, 5)).toEqual([565508, 1000000]);
  });

  it('agrees with the floating-point Wilson formula to one ppm', () => {
    const z = 1.96;
    for (const simulations of [1, 7, 100, 4000]) {
      for (let count = 0; count <= simulations; count += Math.max(1, simulations / 50)) {
        const x = Math.floor(count);
        const p = x / simulations;
        const centre = p + (z * z) / (2 * simulations);
        const spread = z * Math.sqrt((p * (1 - p)) / simulations + (z * z) / (4 * simulations ** 2));
        const scale = 1 + (z * z) / simulations;
        const [lower, upper] = referenceWilsonBoundsPpm(x, simulations);
        expect(Math.abs(lower - ((centre - spread) / scale) * 1e6)).toBeLessThanOrEqual(1);
        expect(Math.abs(upper - ((centre + spread) / scale) * 1e6)).toBeLessThanOrEqual(1);
      }
    }
  });
});
