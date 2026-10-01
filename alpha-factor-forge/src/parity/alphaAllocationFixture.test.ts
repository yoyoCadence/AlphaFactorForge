// P12e-2 — the committed fixture is exactly what the independent reference
// computes. The Rust tests reproduce the fixture; this test keeps the fixture
// from being edited by hand or drifting from the reference.

import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/research-alpha-allocation-v1.json';
import {
  referenceAllocation,
  referenceAllocationId,
  referenceEqualSchedule,
  type AlphaAllocationFixtureCase,
} from './alphaAllocationFixture';

type FixtureCase = AlphaAllocationFixtureCase & { expected?: unknown; error?: string };
type EqualSplit = {
  id: string;
  totalAlphaPpm: number;
  confirmations: number;
  schedule?: number[];
  error?: string;
};

const cases = fixture.cases as FixtureCase[];
const splits = fixture.equalSchedules as EqualSplit[];

describe('research-alpha-allocation-v1 fixture', () => {
  it.each(cases.map((testCase) => [testCase.id, testCase] as const))(
    '%s matches the reference',
    async (_, testCase) => {
      if (testCase.error === undefined) {
        expect(await referenceAllocation(testCase)).toEqual(testCase.expected);
      } else {
        expect(testCase.expected).toBeUndefined();
        await expect(referenceAllocation(testCase)).rejects.toThrow();
      }
    },
  );

  it.each(splits.map((split) => [split.id, split] as const))(
    'equal split %s matches the reference',
    (_, split) => {
      const run = () => referenceEqualSchedule(split.totalAlphaPpm, split.confirmations);
      if (split.error === undefined) {
        expect(run()).toEqual(split.schedule);
      } else {
        expect(split.schedule).toBeUndefined();
        expect(run).toThrow();
      }
    },
  );

  it('never hands out more than the declared total', async () => {
    for (const { declaration, error } of cases) {
      if (error !== undefined) continue;
      const reserved: number[] = [];
      for (;;) {
        const report = await referenceAllocation({ declaration, reserved });
        if (report.alphaPpm === null) break;
        reserved.push(report.alphaPpm);
      }
      expect(reserved).toEqual(declaration.schedule);
      expect(reserved.reduce((total, alpha) => total + alpha, 0)).toBeLessThanOrEqual(
        declaration.totalAlphaPpm,
      );
    }
  });

  it('binds the identity to the schedule order', async () => {
    const declaration = cases[0].declaration;
    const id = await referenceAllocationId(declaration);
    expect(id).toBe('104705944f7da4b3bceb04e805f4fefa35db1930c332d820ec58fad94d1b8e3a');
    const reversed = { ...declaration, schedule: [...declaration.schedule].reverse() };
    expect(await referenceAllocationId(reversed)).not.toBe(id);
  });
});
