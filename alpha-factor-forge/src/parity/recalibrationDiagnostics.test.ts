// P12e-6b — the recorded eligibility, power runs and selection are what the
// plan §6 rule gives on the committed reports; the rule itself is checked at
// its boundaries. The Rust suite re-runs each report's declared prefix.

import { describe, expect, it } from 'vitest';
import diagnostics from '../../fixtures/research/recalibration-plan-v1-diagnostics.json';
import {
  lowestSupportedLength,
  pairResults,
  passesScreen,
  requiredPowerRuns,
  select,
  type DiagnosticReport,
  type DiagnosticRun,
  type PairResult,
} from './recalibrationDiagnostics';

const size = diagnostics.size as DiagnosticRun[];
const power = diagnostics.power as DiagnosticRun[];
const reports = diagnostics.reports as {
  size: Record<string, DiagnosticReport>;
  power: Record<string, DiagnosticReport>;
};
const screen = diagnostics.screen;

function report(family: number, ...confirmations: number[]): DiagnosticReport {
  return {
    status: 'unused',
    family: { count: family },
    confirmations: confirmations.map((count) => ({
      check: { count },
      counts: { netReturnRejectingSimulations: count },
    })),
  };
}

function cells(passing: Record<number, [boolean, boolean]>) {
  return Object.entries(passing).flatMap(([bars, [atZero, atPointThree]]) => [
    { autocorrelationPpm: 0, bars: Number(bars), passes: atZero },
    { autocorrelationPpm: 300000, bars: Number(bars), passes: atPointThree },
  ]);
}

describe('recalibration plan v1 diagnostics', () => {
  it('declares the whole grid on the diagnostic seed only', () => {
    expect(size).toHaveLength(36);
    expect(power).toHaveLength(36);
    const ids = [...size, ...power].map((run) => run.id);
    expect(new Set(ids).size).toBe(72);
    for (const run of [...size, ...power]) {
      const declaration = run.declaration as DiagnosticRun['declaration'] & {
        seed: number;
        simulations: number;
        checkpoints: number[];
      };
      expect(declaration.seed).toBe(20261005);
      expect(declaration.simulations).toBe(4000);
      expect(declaration.checkpoints).toEqual([4096 / declaration.bars]);
      expect(run.id).toMatch(
        new RegExp(
          `^(size|power)-${run.candidate}-${run.rule}-phi${declaration.autocorrelationPpm / 1000}-n${declaration.bars}$`,
        ),
      );
    }
    expect(JSON.stringify(diagnostics)).not.toContain('20261117');
  });

  it('records a report for every size run, and its status is the screen on its counts', () => {
    expect(Object.keys(reports.size).sort()).toEqual(size.map((run) => run.id).sort());
    for (const run of size) {
      const passes = passesScreen(reports.size[run.id], screen);
      expect(reports.size[run.id].status, run.id).toBe(passes ? 'WITHIN_LIMITS' : 'EXCEEDS_LIMITS');
    }
  });

  it('runs exactly the power runs the size screen requires', () => {
    const pairs = pairResults(size, reports.size, screen);
    const required = requiredPowerRuns(power, pairs);
    expect(diagnostics.requiredPowerRuns).toEqual(required);
    expect(Object.keys(reports.power).sort()).toEqual([...required].sort());
    for (const id of required) expect(reports.power[id].status, id).toBe('MEASURED');
  });

  it('records every pair, failed cells included, and the selection the rule gives', () => {
    const pairs = pairResults(size, reports.size, screen);
    const { pairs: withPower, selected } = select(pairs, power, reports.power);
    expect(diagnostics.pairs).toEqual(withPower);
    expect(diagnostics.selection).toEqual(selected);
    expect(diagnostics.pairs).toHaveLength(6);
    for (const pair of diagnostics.pairs as PairResult[]) expect(pair.cells).toHaveLength(6);
  });
});

describe('the plan §6 rule', () => {
  it('screens each confirmation at 120 and the family at 240, inclusive', () => {
    expect(passesScreen(report(240, 120, 120), screen)).toBe(true);
    expect(passesScreen(report(241, 120, 120), screen)).toBe(false);
    expect(passesScreen(report(240, 121, 0), screen)).toBe(false);
    expect(passesScreen(report(240, 0, 121), screen)).toBe(false);
  });

  it('supports a length only when both noise models pass there and at every longer length', () => {
    expect(lowestSupportedLength(cells({ 256: [true, true], 512: [true, true], 1024: [true, true] }))).toBe(256);
    expect(lowestSupportedLength(cells({ 256: [true, false], 512: [true, true], 1024: [true, true] }))).toBe(512);
    // A pass below a failure does not count.
    expect(lowestSupportedLength(cells({ 256: [true, true], 512: [false, true], 1024: [true, true] }))).toBe(1024);
    expect(lowestSupportedLength(cells({ 256: [true, true], 512: [true, true], 1024: [true, false] }))).toBeNull();
    expect(lowestSupportedLength([])).toBeNull();
  });

  it('runs power only at and above an eligible pair’s lowest supported length', () => {
    const pairs: PairResult[] = [
      { candidate: 'V1', rule: 'R3', cells: [], lowestSupportedLength: 512, power: null },
      { candidate: 'V1', rule: 'R4', cells: [], lowestSupportedLength: null, power: null },
    ];
    const ids = requiredPowerRuns(power, pairs);
    expect(ids).toEqual([
      'power-V1-R3-phi0-n512',
      'power-V1-R3-phi0-n1024',
      'power-V1-R3-phi300-n512',
      'power-V1-R3-phi300-n1024',
    ]);
  });

  const at = (bars: number, ...entries: [string, number][]) => ({
    runs: entries.map(([id]) => {
      const [, candidate, rule, phi] = id.split('-');
      return {
        id,
        candidate,
        rule,
        declaration: { autocorrelationPpm: Number(phi.slice(3)) * 1000, bars, effectMillionths: 1 },
      } as DiagnosticRun;
    }),
    reports: Object.fromEntries(entries.map(([id, count]) => [id, report(0, count, 0)])),
  });
  const pair = (candidate: 'V1' | 'S1' | 'S2', rule: 'R3' | 'R4', length: number | null): PairResult => ({
    candidate,
    rule,
    cells: [],
    lowestSupportedLength: length,
    power: null,
  });

  it('prefers the smallest length, then the larger smaller-of-two power, then V1 < S1 < S2 and R3 < R4', () => {
    const grid = at(
      512,
      ['power-S1-R3-phi0', 300],
      ['power-S1-R3-phi300', 200],
      ['power-S2-R4-phi0', 210],
      ['power-S2-R4-phi300', 250],
    );
    // The smaller of the two noise models decides: S2-R4 (210) beats S1-R3 (200).
    expect(select([pair('S1', 'R3', 512), pair('S2', 'R4', 512)], grid.runs, grid.reports).selected).toEqual({
      candidate: 'S2',
      rule: 'R4',
    });
    // A smaller supported length beats any power.
    const longer = at(1024, ['power-V1-R3-phi0', 999], ['power-V1-R3-phi300', 999]);
    expect(
      select(
        [pair('V1', 'R3', 1024), pair('S1', 'R3', 512)],
        [...longer.runs, ...grid.runs],
        { ...longer.reports, ...grid.reports },
      ).selected,
    ).toEqual({ candidate: 'S1', rule: 'R3' });
    // Equal power: plan order.
    const tie = at(
      512,
      ['power-S1-R4-phi0', 5],
      ['power-S1-R4-phi300', 5],
      ['power-S1-R3-phi0', 5],
      ['power-S1-R3-phi300', 6],
    );
    expect(select([pair('S1', 'R4', 512), pair('S1', 'R3', 512)], tie.runs, tie.reports).selected).toEqual({
      candidate: 'S1',
      rule: 'R3',
    });
    const candidates = at(512, ['power-S2-R3-phi0', 5], ['power-S2-R3-phi300', 5], ['power-V1-R4-phi0', 5], ['power-V1-R4-phi300', 5]);
    expect(
      select([pair('S2', 'R3', 512), pair('V1', 'R4', 512)], candidates.runs, candidates.reports).selected,
    ).toEqual({ candidate: 'V1', rule: 'R4' });
    expect(select([pair('V1', 'R3', null)], [], {}).selected).toBeNull();
  });
});
