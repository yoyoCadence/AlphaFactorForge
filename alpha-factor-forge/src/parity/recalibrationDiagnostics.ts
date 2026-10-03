// P12e-6b — the selection rule of docs/plans/confirmation-recalibration-plan-v1.md
// §6, applied to committed diagnostic reports. Used by the record script and
// by the Vitest that re-derives the recorded selection from the reports.
// Test support only: nothing in the product reads it.

export type Candidate = 'V1' | 'S1' | 'S2';
export type BlockRule = 'R3' | 'R4';

export interface DiagnosticRun {
  id: string;
  candidate: Candidate;
  rule: BlockRule;
  declaration: { autocorrelationPpm: number; bars: number; effectMillionths: number };
}

export interface DiagnosticReport {
  status: string;
  family: { count: number };
  confirmations: {
    check: { count: number };
    counts: { netReturnRejectingSimulations: number };
  }[];
}

export interface Screen {
  confirmationLimitCount: number;
  familyLimitCount: number;
}

export interface PairResult {
  candidate: Candidate;
  rule: BlockRule;
  cells: { autocorrelationPpm: number; bars: number; passes: boolean }[];
  lowestSupportedLength: number | null;
  /** Smaller of the two noise models' power at the lowest supported length. */
  power: number | null;
}

const CANDIDATE_ORDER: Candidate[] = ['V1', 'S1', 'S2'];
const RULE_ORDER: BlockRule[] = ['R3', 'R4'];

/** Plan §6's screen, recomputed from the counts (not read from `status`). */
export function passesScreen(report: DiagnosticReport, screen: Screen): boolean {
  return (
    report.family.count <= screen.familyLimitCount &&
    report.confirmations.every((row) => row.check.count <= screen.confirmationLimitCount)
  );
}

/** The cell's two noise models must pass at `n` and at every larger length. */
export function lowestSupportedLength(
  cells: { autocorrelationPpm: number; bars: number; passes: boolean }[],
): number | null {
  const lengths = [...new Set(cells.map((cell) => cell.bars))].sort((a, b) => a - b);
  let lowest: number | null = null;
  for (let index = lengths.length - 1; index >= 0; index -= 1) {
    const atLength = cells.filter((cell) => cell.bars === lengths[index]);
    if (atLength.length === 0 || !atLength.every((cell) => cell.passes)) break;
    lowest = lengths[index];
  }
  return lowest;
}

/** Size results per candidate/rule pair, in the plan's tie-break order. */
export function pairResults(
  size: DiagnosticRun[],
  reports: Record<string, DiagnosticReport>,
  screen: Screen,
): PairResult[] {
  return CANDIDATE_ORDER.flatMap((candidate) =>
    RULE_ORDER.map((rule) => {
      const cells = size
        .filter((run) => run.candidate === candidate && run.rule === rule)
        .map((run) => {
          const report = reports[run.id];
          if (!report) throw new Error(`no size report for ${run.id}`);
          return {
            autocorrelationPpm: run.declaration.autocorrelationPpm,
            bars: run.declaration.bars,
            passes: passesScreen(report, screen),
          };
        });
      return { candidate, rule, cells, lowestSupportedLength: lowestSupportedLength(cells), power: null };
    }),
  );
}

/** Power runs the plan requires: eligible pairs, at and above their lowest length. */
export function requiredPowerRuns(power: DiagnosticRun[], pairs: PairResult[]): string[] {
  return power
    .filter((run) => {
      const pair = pairs.find((entry) => entry.candidate === run.candidate && entry.rule === run.rule);
      return (
        pair !== undefined &&
        pair.lowestSupportedLength !== null &&
        run.declaration.bars >= pair.lowestSupportedLength
      );
    })
    .map((run) => run.id);
}

/**
 * Plan §6 selection: the smallest lowest supported length, then the larger
 * power at that length (the smaller of the two noise models' counts), then
 * V1 < S1 < S2 and R3 < R4. `null` when no pair is eligible.
 */
export function select(
  pairs: PairResult[],
  power: DiagnosticRun[],
  powerReports: Record<string, DiagnosticReport>,
): { pairs: PairResult[]; selected: { candidate: Candidate; rule: BlockRule } | null } {
  const withPower = pairs.map((pair) => {
    if (pair.lowestSupportedLength === null) return pair;
    const counts = power
      .filter(
        (run) =>
          run.candidate === pair.candidate &&
          run.rule === pair.rule &&
          run.declaration.bars === pair.lowestSupportedLength,
      )
      .map((run) => {
        const report = powerReports[run.id];
        if (!report) throw new Error(`no power report for ${run.id}`);
        return report.confirmations[0].counts.netReturnRejectingSimulations;
      });
    return { ...pair, power: Math.min(...counts) };
  });
  const eligible = withPower
    .map((pair) => ({
      pair,
      order: CANDIDATE_ORDER.indexOf(pair.candidate) * RULE_ORDER.length + RULE_ORDER.indexOf(pair.rule),
    }))
    .filter(({ pair }) => pair.lowestSupportedLength !== null);
  if (eligible.length === 0) return { pairs: withPower, selected: null };
  eligible.sort(
    (a, b) =>
      (a.pair.lowestSupportedLength as number) - (b.pair.lowestSupportedLength as number) ||
      (b.pair.power as number) - (a.pair.power as number) ||
      a.order - b.order,
  );
  const { candidate, rule } = eligible[0].pair;
  return { pairs: withPower, selected: { candidate, rule } };
}
