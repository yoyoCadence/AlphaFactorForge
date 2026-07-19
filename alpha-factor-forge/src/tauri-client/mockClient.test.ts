// PERSIST-001 mock-parity tests (PR #65 review P2): the dev mock must reject
// exactly the bundles native Tauri rejects, keep its append-only records
// immutable, and return detached rows from every read.

import { describe, it, expect } from 'vitest';
import type { BacktestResult } from '../core/backtest';
import type { Metrics } from '../core/metrics';
import { planValidationSplit } from '../core/validation/split';
import { deriveEmbargoBars } from '../services/embargo';
import { defaultStrategy } from '../services/strategy';
import { DEFAULT_GATE_CONFIG, type GateVerdict } from '../services/gate';
import { scoreCandidate } from '../services/score';
import type { BenchmarkRun } from '../services/benchmarks';
import {
  buildBenchmarkRecord,
  buildValidationBundle,
  buildValidationRecord,
  type ValidationBundle,
} from '../services/validationRecord';
import { makeMockClient } from './mockClient';

const metrics = (): Metrics => ({
  netReturn: 0.2, cagr: 0.4, maxDrawdown: 0.1, sharpe: 1, sortino: 2, calmar: 2,
  winRate: 0.6, tradeCount: 3, profitFactor: 2, avgTradeReturn: 0.05,
  medianTradeReturn: 0.05, avgHoldingBars: 2, exposure: 0.5, turnover: 0.05,
  largestWin: 0.1, largestLoss: -0.05, consecutiveLosses: 1, monthlyReturns: {},
});

const result = (startTime: number): BacktestResult => ({
  trades: [{ entryTime: startTime, exitTime: startTime + 2, side: 'LONG', entryPrice: 100, exitPrice: 105, pnl: 5, pnlPct: 0.05, bars: 2 }],
  equity: Array.from({ length: 10 }, (_, i) => ({ time: startTime + i, equity: 1000 + i })),
  metrics: metrics(),
});

const verdict = (): GateVerdict => ({
  pass: true,
  criteria: [{ id: 'minTrades', pass: true, value: 36, threshold: 30 }],
  config: DEFAULT_GATE_CONFIG,
});

const makeBundle = (): ValidationBundle => {
  const run = { train: result(0), validation: result(100) };
  const benchmarks: BenchmarkRun[] = (
    ['buyHold', 'smaCross', 'rsiReversion', 'bollingerReversion'] as const
  ).map((id) => ({ id, strat: id === 'buyHold' ? null : defaultStrategy(), result: result(100) }));
  const record = buildValidationRecord({
    strategyId: 1,
    strategyHash: 'hash-s',
    datasetId: 2,
    datasetHash: 'hash-d',
    embargo: deriveEmbargoBars(defaultStrategy(), 0),
    splitPlan: planValidationSplit(100, 22),
    validationRun: run,
    benchmark: buildBenchmarkRecord({
      interval: '1h',
      validationRange: { from: 50, to: 69 },
      costs: { feePct: 0.05, slipPct: 0.02 },
      benchmarks,
      randomEntry: { runs: 20, seed: 7, netReturns: Array(20).fill(0.01), candidateNetReturn: 0.2, candidatePercentile: 96 },
    }),
    outcome: {
      passed: true,
      gate: verdict(),
      score: scoreCandidate({
        validationRun: { validation: run.validation },
        strat: defaultStrategy(),
        testedCombinations: 1,
      }),
    },
    testedCombinations: 1,
  });
  return buildValidationBundle({ record, validationRun: run });
};

const save = (db: ReturnType<typeof makeMockClient>['db'], b: ValidationBundle) =>
  db.saveValidationRecord(b.trainSummary, b.trainTrades, b.validationSummary, b.validationTrades, b.record);

describe('mockClient validation records', () => {
  it('saves composer bundles, appends on re-save, and reads back detached rows', async () => {
    const { db } = makeMockClient();
    const b = makeBundle();
    const id1 = await save(db, b);
    const id2 = await save(db, b);
    expect(id2).not.toBe(id1);

    const list = await db.listValidationRecords();
    expect(list).toHaveLength(2);
    expect(list[0].id).toBe(id2); // newest first

    // detached reads: tampering with a returned row must not touch the store
    list[0].record_json = 'tampered';
    expect((await db.getValidationRecord(id2)).record_json).toBe(b.record.record_json);
    const got = await db.getValidationRecord(id1);
    got.gate_passed = false;
    expect((await db.getValidationRecord(id1)).gate_passed).toBe(true);

    expect(await db.listValidationRecords(b.record.strategy_id)).toHaveLength(2);
    expect(await db.listValidationRecords(999)).toHaveLength(0);
    // summaries still flow through the normal upsert path (latest view)
    expect(await db.getBacktestResults()).toHaveLength(2);
  });

  it('rejects the same illegal bundles native Tauri rejects, persisting nothing', async () => {
    const { db } = makeMockClient();
    const b = makeBundle();
    const attempt = (over: Partial<ValidationBundle>) =>
      save(db, { ...b, ...over });

    await expect(attempt({ trainSummary: { ...b.trainSummary, segment: 'full' } }))
      .rejects.toThrow(/train segment/);
    await expect(attempt({ record: { ...b.record, strategy_id: 999 } }))
      .rejects.toThrow(/identity/);
    await expect(attempt({ trainSummary: { ...b.trainSummary, gate_passed: false } }))
      .rejects.toThrow(/Phase B/);
    await expect(attempt({ validationSummary: { ...b.validationSummary, benchmark_result_json: null } }))
      .rejects.toThrow(/benchmark/);
    // finite but DIFFERENT score between latest view and record row
    await expect(attempt({ validationSummary: { ...b.validationSummary, score: 999 } }))
      .rejects.toThrow(/equal the record score/);
    // passing gate without a score
    await expect(attempt({ record: { ...b.record, score: null } }))
      .rejects.toThrow(/finite score/);
    // envelope contradicting the row
    await expect(attempt({
      record: {
        ...b.record,
        record_json: b.record.record_json.replace('"gatePassed":true', '"gatePassed":false'),
      },
    })).rejects.toThrow(/gatePassed/);
    // summary breakdown snapshot differing from the record's
    await expect(attempt({
      validationSummary: {
        ...b.validationSummary,
        score_breakdown_json: '{"formulaVersion":"score-v1","score":123}',
      },
    })).rejects.toThrow(/breakdown/);

    // every rejection happened BEFORE any write
    expect(await db.listValidationRecords()).toHaveLength(0);
    expect(await db.getBacktestResults()).toHaveLength(0);
  });

  it('accepts key-order-only JSON differences exactly like the Rust validator', async () => {
    const { db } = makeMockClient();
    const b = makeBundle();
    const reorder = (json: string): string => {
      const o = JSON.parse(json) as Record<string, unknown>;
      return JSON.stringify(
        Object.fromEntries(Object.keys(o).reverse().map((k) => [k, o[k]])),
      );
    };
    const id = await save(db, {
      ...b,
      validationSummary: {
        ...b.validationSummary,
        score_breakdown_json: reorder(b.validationSummary.score_breakdown_json!),
        benchmark_result_json: reorder(b.validationSummary.benchmark_result_json!),
      },
    });
    expect(id).toBeGreaterThan(0);
  });

  it('rejects a benchmark impersonated by null, a non-object, or a wrong version', async () => {
    const { db } = makeMockClient();
    const b = makeBundle();
    // keep summary and envelope CONSISTENT — the shape lock itself must reject
    const withBenchmark = (benchJson: string) => {
      const env = JSON.parse(b.record.record_json) as Record<string, unknown>;
      env.benchmark = JSON.parse(benchJson);
      return {
        ...b,
        validationSummary: { ...b.validationSummary, benchmark_result_json: benchJson },
        record: { ...b.record, record_json: JSON.stringify(env) },
      };
    };
    for (const bogus of [
      'null',
      '[]',
      '{}',
      JSON.stringify({ version: 'bench-record-v999', benchmarks: [], randomEntry: {} }),
      JSON.stringify({ version: 'bench-record-v1', benchmarks: [] }),
    ]) {
      await expect(save(db, withBenchmark(bogus))).rejects.toThrow(/bench-record-v1 object/);
    }
    expect(await db.listValidationRecords()).toHaveLength(0);
  });
});
