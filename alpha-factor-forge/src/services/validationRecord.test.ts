// PERSIST-001 acceptance tests (TypeScript side of the PR #64 Resolution
// checklist): codec guard, benchmark snapshot, gate encoding, discriminated
// union, record round-trip, and D3 bundle semantics.

import { describe, it, expect } from 'vitest';
import type { BacktestResult } from '../core/backtest';
import type { ClosedTrade, EquityPoint, Metrics } from '../core/metrics';
import { planValidationSplit } from '../core/validation/split';
import { deriveEmbargoBars } from './embargo';
import { defaultStrategy } from './strategy';
import { DEFAULT_GATE_CONFIG, type GateVerdict } from './gate';
import { scoreCandidate } from './score';
import type { BenchmarkRun } from './benchmarks';
import type { RandomEntryBenchmark } from './randomEntry';
import { assertJsonSafe, encodeMetrics } from './metricsCodec';
import {
  BENCHMARK_RECORD_VERSION,
  VALIDATION_RECORD_VERSION,
  buildBenchmarkRecord,
  buildValidationBundle,
  buildValidationRecord,
  encodeGateVerdict,
  type AssessmentOutcome,
  type BenchmarkRecord,
} from './validationRecord';

const zeroMetrics = (): Metrics => ({
  netReturn: 0,
  cagr: 0,
  maxDrawdown: 0,
  sharpe: 0,
  sortino: 0,
  calmar: 0,
  winRate: 0,
  tradeCount: 0,
  profitFactor: 0,
  avgTradeReturn: 0,
  medianTradeReturn: 0,
  avgHoldingBars: 0,
  exposure: 0,
  turnover: 0,
  largestWin: 0,
  largestLoss: 0,
  consecutiveLosses: 0,
  monthlyReturns: {},
});

const trade = (bars: number): ClosedTrade => ({
  entryTime: 0,
  exitTime: bars,
  side: 'LONG',
  entryPrice: 100,
  exitPrice: 101,
  pnl: 10,
  pnlPct: 0.1,
  bars,
});

const equityFrom = (startTime: number, n: number): EquityPoint[] =>
  Array.from({ length: n }, (_, i) => ({ time: startTime + i, equity: 1000 + i }));

const fakeResult = (
  startTime: number,
  metricsOver: Partial<Metrics> = {},
  trades: ClosedTrade[] = [trade(2)],
): BacktestResult => ({
  trades,
  equity: equityFrom(startTime, 10),
  metrics: { ...zeroMetrics(), cagr: 0.4, sortino: 2, profitFactor: 2, ...metricsOver },
});

const fakeBenchmarks = (): BenchmarkRun[] => {
  const ids = ['buyHold', 'smaCross', 'rsiReversion', 'bollingerReversion'] as const;
  return ids.map((id) => ({
    id,
    strat: id === 'buyHold' ? null : defaultStrategy(),
    result: fakeResult(0, { netReturn: 0.1 }),
  }));
};

const fakeRandomEntry = (): RandomEntryBenchmark => ({
  runs: 50,
  seed: 7,
  netReturns: Array.from({ length: 50 }, (_, i) => i / 100),
  candidateNetReturn: 0.6,
  candidatePercentile: 96,
});

const benchmarkRecord = (): BenchmarkRecord =>
  buildBenchmarkRecord({
    interval: '1h',
    validationRange: { from: 50, to: 69 },
    costs: { feePct: 0.05, slipPct: 0.02 },
    benchmarks: fakeBenchmarks(),
    randomEntry: fakeRandomEntry(),
  });

const passVerdict = (): GateVerdict => ({
  pass: true,
  criteria: [
    { id: 'minTrades', pass: true, value: 36, threshold: 30 },
    { id: 'randomEntryPercentile', pass: true, value: 96, threshold: 95 },
  ],
  config: DEFAULT_GATE_CONFIG,
});

const failVerdict = (): GateVerdict => ({
  pass: false,
  criteria: [{ id: 'minTrades', pass: false, value: 3, threshold: 30 }],
  config: DEFAULT_GATE_CONFIG,
});

const validationRun = () => ({ train: fakeResult(0), validation: fakeResult(100) });

const passingOutcome = (run = validationRun()): AssessmentOutcome => ({
  passed: true,
  gate: passVerdict(),
  score: scoreCandidate({
    validationRun: { validation: run.validation },
    strat: defaultStrategy(),
    testedCombinations: 100,
  }),
});

const recordArgs = (outcome: AssessmentOutcome, run = validationRun()) => ({
  strategyId: 1,
  strategyHash: 'strat-hash',
  datasetId: 2,
  datasetHash: 'ds-hash',
  embargo: deriveEmbargoBars(defaultStrategy(), 0),
  splitPlan: planValidationSplit(100, 22),
  validationRun: run,
  benchmark: benchmarkRecord(),
  outcome,
  testedCombinations: 100,
});

describe('metricsCodec.assertJsonSafe', () => {
  it('throws on any nested unencoded non-finite number, with its path', () => {
    expect(() => assertJsonSafe({ a: [{ b: Infinity }] }, 'x')).toThrow(/x\.a\[0\]\.b/);
    expect(() => assertJsonSafe({ a: NaN })).toThrow(/non-finite/);
    expect(() => assertJsonSafe([-Infinity])).toThrow(/non-finite/);
    expect(() => assertJsonSafe({ ok: 1, nested: { arr: [0.5, null, 'txt'] } })).not.toThrow();
  });
});

describe('buildBenchmarkRecord', () => {
  it('snapshots metrics only and requires all four deterministic benchmarks', () => {
    const rec = benchmarkRecord();
    expect(rec.version).toBe(BENCHMARK_RECORD_VERSION);
    expect(rec.benchmarks).toHaveLength(4);
    expect(rec.benchmarks[0]).toMatchObject({ id: 'buyHold', strat: null });
    for (const b of rec.benchmarks) {
      expect(b.metrics.values.netReturn).toBe(0.1);
      expect(b).not.toHaveProperty('result'); // no equity/trades ever
    }
    expect(rec.randomEntry.netReturns).toHaveLength(50);
    expect(() =>
      buildBenchmarkRecord({
        interval: '1h',
        validationRange: { from: 0, to: 9 },
        costs: { feePct: 0, slipPct: 0 },
        benchmarks: fakeBenchmarks().slice(1),
        randomEntry: fakeRandomEntry(),
      }),
    ).toThrow(/missing deterministic benchmark/);
  });
});

describe('encodeGateVerdict', () => {
  it('encodes non-finite criterion values with an explicit status', () => {
    const verdict: GateVerdict = {
      pass: true,
      criteria: [
        { id: 'avgTradeReturn', pass: true, value: Infinity, threshold: 0 },
        { id: 'maxDrawdown', pass: true, value: 0.1, threshold: 0.35 },
        { id: 'rollingConsistency', pass: false, value: null, threshold: 0.55, detail: 'short' },
      ],
      config: DEFAULT_GATE_CONFIG,
    };
    const enc = encodeGateVerdict(verdict);
    expect(enc.criteria[0]).toMatchObject({ value: null, valueStatus: 'positive_infinity' });
    expect(enc.criteria[1]).toMatchObject({ value: 0.1, valueStatus: null });
    expect(enc.criteria[2]).toMatchObject({ value: null, valueStatus: null, detail: 'short' });
    expect(() => assertJsonSafe(enc)).not.toThrow();
  });
});

describe('buildValidationRecord', () => {
  it('builds a self-contained pass record that survives a JSON round-trip', () => {
    const record = buildValidationRecord(recordArgs(passingOutcome()));
    expect(record.version).toBe(VALIDATION_RECORD_VERSION);
    expect(record.contracts).toEqual({
      execution: 'backtest-execution-v1',
      benchmark: 'benchmark-suite-v1',
      gate: 'gate-v1',
      score: 'score-v1',
    });
    expect(record.gatePassed).toBe(true);
    expect(record.score?.formulaVersion).toBe('score-v1');
    expect(record.trainMetrics).toEqual(encodeMetrics(validationRun().train.metrics));
    expect(JSON.parse(JSON.stringify(record))).toEqual(record);
  });

  it('a failing outcome cannot carry a score and records score contract null', () => {
    const record = buildValidationRecord(recordArgs({ passed: false, gate: failVerdict() }));
    expect(record.gatePassed).toBe(false);
    expect(record.score).toBeNull();
    expect(record.contracts.score).toBeNull();
  });

  it('fails closed on inconsistent outcome, evidence, or non-finite leaks', () => {
    // outcome flag must match the embedded verdict
    expect(() =>
      buildValidationRecord(recordArgs({ passed: false, gate: passVerdict() })),
    ).toThrow(/outcome\.passed/);
    // testedCombinations must match the score evidence
    const args = recordArgs(passingOutcome());
    expect(() => buildValidationRecord({ ...args, testedCombinations: 999 })).toThrow(/evidence/);
    // a non-finite value hidden in an unencoded corner throws before stringify
    const leaked = recordArgs(passingOutcome());
    leaked.embargo = { ...leaked.embargo, embargoBars: Infinity };
    expect(() => buildValidationRecord(leaked)).toThrow(/non-finite/);
  });
});

describe('buildValidationBundle (D3 semantics)', () => {
  it('assembles a passing bundle: train Phase B null, validation fully set', () => {
    const run = validationRun();
    const record = buildValidationRecord(recordArgs(passingOutcome(run), run));
    const bundle = buildValidationBundle({ record, validationRun: run });

    expect(bundle.trainSummary.segment).toBe('train');
    expect(bundle.trainSummary.gate_passed).toBeUndefined(); // serializes as null
    expect(bundle.trainSummary.score).toBeUndefined();
    expect(bundle.trainSummary.score_breakdown_json).toBeUndefined();
    expect(bundle.trainSummary.benchmark_result_json).toBeUndefined();
    expect(bundle.trainSummary.start_time).toBe(0);
    expect(bundle.trainSummary.end_time).toBe(9);

    expect(bundle.validationSummary.segment).toBe('validation');
    expect(bundle.validationSummary.gate_passed).toBe(true);
    expect(bundle.validationSummary.score).toBe(record.score!.score);
    expect(JSON.parse(bundle.validationSummary.score_breakdown_json!)).toEqual(record.score);
    expect(JSON.parse(bundle.validationSummary.benchmark_result_json!)).toEqual(record.benchmark);
    expect(bundle.validationSummary.start_time).toBe(100);

    expect(bundle.record.record_version).toBe(VALIDATION_RECORD_VERSION);
    expect(bundle.record.gate_passed).toBe(true);
    expect(bundle.record.score).toBe(record.score!.score);
    expect(JSON.parse(bundle.record.record_json)).toEqual(record);
    expect(bundle.trainTrades).toHaveLength(1);
    expect(bundle.validationTrades).toHaveLength(1);
  });

  it('a failing bundle nulls every score field but keeps the benchmark view', () => {
    const run = validationRun();
    const record = buildValidationRecord(recordArgs({ passed: false, gate: failVerdict() }, run));
    const bundle = buildValidationBundle({ record, validationRun: run });
    expect(bundle.validationSummary.gate_passed).toBe(false);
    expect(bundle.validationSummary.score).toBeNull();
    expect(bundle.validationSummary.score_breakdown_json).toBeNull();
    expect(bundle.validationSummary.benchmark_result_json).not.toBeNull();
    expect(bundle.record.score).toBeNull();
  });

  it('rejects a run that does not match the record snapshots', () => {
    const run = validationRun();
    const record = buildValidationRecord(recordArgs(passingOutcome(run), run));
    const other = {
      train: fakeResult(0, { cagr: 0.99 }),
      validation: run.validation,
    };
    expect(() => buildValidationBundle({ record, validationRun: other })).toThrow(/snapshot/);
  });
});
