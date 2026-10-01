// P12e-1 — the committed fixture is exactly what the independent reference
// computes. The Rust tests reproduce the fixture; this test keeps the fixture
// from being edited by hand or drifting from the reference.

import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-confirmation-statistics-v1.json';
import {
  referenceConfirmationTests,
  splitmix64Outputs,
  type ConfirmationFixtureCase,
} from './confirmationStatisticsFixture';

describe('research-confirmation-statistics-v1 fixture', () => {
  it('uses SplitMix64 as published', () => {
    // Reference outputs of Vigna's splitmix64.c for these starting states.
    expect(splitmix64Outputs('0', 3)).toEqual([
      '16294208416658607535',
      '7960286522194355700',
      '487617019471545679',
    ]);
    expect(splitmix64Outputs('1234567', 5)).toEqual([
      '6457827717110365317',
      '3203168211198807973',
      '9817491932198370423',
      '4593380528125082431',
      '16408922859458223821',
    ]);
    for (const vector of fixture.prngVectors) {
      expect(splitmix64Outputs(vector.state, vector.outputs.length)).toEqual(vector.outputs);
    }
  });

  it.each(fixture.cases.map((testCase) => [testCase.id, testCase] as const))(
    '%s matches the reference',
    (_, testCase) => {
      expect(referenceConfirmationTests(testCase as ConfirmationFixtureCase)).toEqual(
        testCase.expected.tests,
      );
    },
  );

  it('keeps every declared block length within the square-root rule', () => {
    for (const testCase of fixture.cases) {
      for (const candidate of testCase.candidates) {
        expect(testCase.declaration.blockLength ** 2).toBeLessThanOrEqual(candidate.returns.length);
        expect(candidate.benchmarkReturns).toHaveLength(candidate.returns.length);
      }
      expect(testCase.familyTests % 2).toBe(0);
      expect(testCase.familyTests).toBeGreaterThanOrEqual(testCase.candidates.length * 2);
    }
  });
});
