// DEV/TEST-ONLY saved-history seed for the Results Explorer (P01).
//
// `?mock=1&seedHistory=1` fills the in-memory mock with what a workspace holds
// after real work: one dataset, two strategies, a Train + Validation bundle per
// strategy, one manual `full` result, and one `test`-segment row the explorer
// must hide. The validation bundles are produced by the REAL composer chain
// (split → segmented backtests → §6 benchmarks → Random Entry → Gate → Score →
// `validation-record-v2`), so the seeded `record_json` is exactly what the
// backend would have stored, not a convenient shape. The same builder feeds
// the explorer's vitest, which is what keeps its parser honest.
//
// Reached only through `mockClient.ts`, which is itself `import.meta.env.DEV`
// gated; nothing here is a product path.

import type { BacktestSummary, StrategyDef, TradeRow } from './commands';
import { prepareDatasetImport, type PreparedDatasetImport } from './dbClient';
import { makeSampleCandles } from '../services/sampleData';
import { toCoreCandles } from '../services/candleAdapter';
import { defaultStrategy, type ParamsStrategy } from '../services/strategy';
import { buildStrategyDef } from '../services/strategyRecord';
import { deriveEmbargoBars } from '../services/embargo';
import { runValidationBacktests } from '../services/validationRun';
import { runDeterministicBenchmarks } from '../services/benchmarks';
import { runRandomEntryBenchmark } from '../services/randomEntry';
import { evaluateGate, type GateConfig } from '../services/gate';
import { scoreCandidate } from '../services/score';
import {
  buildBenchmarkRecord,
  buildValidationBundle,
  buildValidationRecord,
  type AssessmentOutcome,
  type ValidationBundle,
} from '../services/validationRecord';
import { metricsToBacktestSummary } from '../services/metricsMapper';
import { runParamsBacktest } from '../services/backtestRunner';
import { tradesToRows } from '../services/tradesMapper';

export const SEED_INTERVAL = '1h';
/** The sample-candle seed. Chosen (by probing) so that the default MA 9/21
 *  fails the §5.1 gate and MA 3/8 passes the lenient gate below, giving the
 *  explorer one scored row and one gate-failed row from real evidence. */
export const SEED_CANDLE_SEED = 34;
/** Small on purpose: the seed runs in the browser before the first read. */
const SEED_RANDOM_ENTRY_RUNS = 50;
const SEED_RANDOM_ENTRY_SEED = 7;

/** Thresholds the sample series can meet. The record embeds the exact config
 *  it was judged with, so a lenient gate is honest evidence, not a shortcut —
 *  no seeded strategy beats the default gate on drifting sample data. */
export const SEED_LENIENT_GATE: Partial<GateConfig> = {
  minTrades: 1,
  minRollingPositiveRatio: 0.01,
  maxDrawdown: 1,
  maxMonthlyContribution: 1,
  maxSingleTradeContribution: 1,
  minRandomEntryPercentile: 0,
};

export interface SeedStrategy {
  name: string;
  strat: ParamsStrategy;
  gateConfig?: Partial<GateConfig>;
}

export const SEED_STRATEGIES: readonly SeedStrategy[] = [
  { name: 'seed MA 9/21', strat: defaultStrategy() },
  { name: 'seed MA 3/8', strat: { ...defaultStrategy(), fastMA: 3, slowMA: 8 }, gateConfig: SEED_LENIENT_GATE },
];

export interface ComposeArgs {
  strat: ParamsStrategy;
  candles: ReturnType<typeof toCoreCandles>;
  interval: string;
  strategyId: number;
  strategyHash: string;
  datasetId: number;
  datasetHash: string;
  gateConfig?: Partial<GateConfig>;
}

/** One complete assessment through the production services. */
export function composeValidationBundle(args: ComposeArgs): ValidationBundle {
  const { strat, candles, interval } = args;
  const embargo = deriveEmbargoBars(strat, 0);
  const validationRun = runValidationBacktests({ candles, strat, interval, embargoBars: embargo.embargoBars });
  const costs = { feePct: strat.feePct, slipPct: strat.slipPct };
  const { from, to } = validationRun.plan.validation;
  const benchmarks = runDeterministicBenchmarks({ candles, interval, costs, from, to });
  const randomEntry = runRandomEntryBenchmark({
    candles,
    interval,
    costs,
    from,
    to,
    candidateResult: validationRun.validation,
    seed: SEED_RANDOM_ENTRY_SEED,
    runs: SEED_RANDOM_ENTRY_RUNS,
  });
  const gate = evaluateGate({ candidateResult: validationRun.validation, benchmarks, randomEntry, config: args.gateConfig });
  const outcome: AssessmentOutcome = gate.pass
    ? { passed: true, gate, score: scoreCandidate({ validationRun, strat, testedCombinations: 1 }) }
    : { passed: false, gate };
  const record = buildValidationRecord({
    strategyId: args.strategyId,
    strategyHash: args.strategyHash,
    datasetId: args.datasetId,
    datasetHash: args.datasetHash,
    embargo,
    splitPlan: validationRun.plan,
    validationRun,
    benchmark: buildBenchmarkRecord({ interval, validationRange: { from, to }, costs, benchmarks, randomEntry }),
    outcome,
    testedCombinations: 1,
  });
  return buildValidationBundle({ record, validationRun });
}

export interface HistorySeedWriter {
  importCandles(dataset: PreparedDatasetImport['dataset'], candles: PreparedDatasetImport['candles']): Promise<number>;
  saveStrategy(def: StrategyDef): Promise<number>;
  saveBacktestResult(summary: BacktestSummary, trades: TradeRow[]): Promise<number>;
  saveValidationRecord(
    trainSummary: BacktestSummary,
    trainTrades: TradeRow[],
    validationSummary: BacktestSummary,
    validationTrades: TradeRow[],
    record: ValidationBundle['record'],
  ): Promise<number>;
}

export interface HistorySeedReport {
  datasetId: number;
  strategyIds: number[];
  recordIds: number[];
  fullSummaryId: number;
  hiddenTestSummaryId: number;
}

/** Write the seed through the mock's own save paths, in the order a user would. */
export async function seedHistory(writer: HistorySeedWriter): Promise<HistorySeedReport> {
  const dbCandles = makeSampleCandles({ seed: SEED_CANDLE_SEED });
  const prepared = await prepareDatasetImport({
    exchange: 'mock',
    symbol: 'SEED',
    interval: SEED_INTERVAL,
    source: 'import',
    candles: dbCandles,
  });
  const datasetId = await writer.importCandles(prepared.dataset, prepared.candles);
  const candles = toCoreCandles(prepared.candles);

  const strategyIds: number[] = [];
  const recordIds: number[] = [];
  for (const { name, strat, gateConfig } of SEED_STRATEGIES) {
    const def = await buildStrategyDef(strat, name);
    const strategyId = await writer.saveStrategy(def);
    strategyIds.push(strategyId);
    const bundle = composeValidationBundle({
      strat,
      candles,
      interval: SEED_INTERVAL,
      strategyId,
      strategyHash: def.strategy_hash,
      datasetId,
      datasetHash: prepared.dataset.dataset_hash,
      gateConfig,
    });
    recordIds.push(await writer.saveValidationRecord(
      bundle.trainSummary,
      bundle.trainTrades,
      bundle.validationSummary,
      bundle.validationTrades,
      bundle.record,
    ));
  }

  // A manual "full" save of the first strategy, as the Backtest panel does.
  const full = runParamsBacktest({ candles, strat: SEED_STRATEGIES[0].strat, interval: SEED_INTERVAL });
  const fullSummaryId = await writer.saveBacktestResult(
    metricsToBacktestSummary(full.metrics, {
      strategyId: strategyIds[0],
      datasetId,
      segment: 'full',
      startTime: candles[0].t,
      endTime: candles[candles.length - 1].t,
    }),
    tradesToRows(full.trades),
  );

  // A Test-segment row. No product writer creates one today, but migration
  // 0001 admits the value; the explorer must hide it whatever wrote it.
  const hiddenTestSummaryId = await writer.saveBacktestResult(
    {
      strategy_id: strategyIds[0],
      dataset_id: datasetId,
      segment: 'test',
      start_time: candles[0].t,
      end_time: candles[candles.length - 1].t,
      net_return: 9.99,
      trade_count: 0,
    },
    [],
  );

  return { datasetId, strategyIds, recordIds, fullSummaryId, hiddenTestSummaryId };
}
