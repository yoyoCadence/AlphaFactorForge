// PERSIST-001: the immutable validation-record composer (PR #64 handoff
// Resolution, revised Option C).
//
// Builds the self-contained `validation-record-v1` decision audit snapshot and
// the full save bundle (Train + Validation summary rows + trade rows + record
// row) that the atomic `save_validation_record` command persists in ONE
// transaction. Pure — no IO/UI; unwired until the runner/Results Explorer
// slices call it.
//
// Discipline (Resolution D3/D4): the Gate outcome is a discriminated union so
// "gate failed but has a score" is unrepresentable; every payload passes the
// shared metricsCodec's recursive non-finite guard before it may be
// serialized; the record embeds enough snapshots and contract versions to
// reconstruct the judgment after the mutable `backtest_summary` view is
// overwritten by a re-run. Test segments are never read, executed, or saved.

import type { Metrics } from '../core/metrics';
import type { ParamsStrategy } from './strategy';
import type { EmbargoDerivation } from './embargo';
import type { ValidationSplitPlan } from '../core/validation/split';
import type { ValidationRunResult } from './validationRun';
import {
  DETERMINISTIC_BENCHMARK_IDS,
  type BenchmarkCosts,
  type BenchmarkRun,
  type DeterministicBenchmarkId,
} from './benchmarks';
import type { RandomEntryBenchmark } from './randomEntry';
import type { GateVerdict, GateCriterionId, GateConfig } from './gate';
import type { ScoreBreakdown } from './score';
import { SCORE_FORMULA_VERSION } from './score';
import { assertJsonSafe, encodeMetrics, type EncodedMetrics } from './metricsCodec';
import { nonFiniteStatus, type NonFiniteStatus } from './nonFinite';
import { metricsToBacktestSummary } from './metricsMapper';
import { tradesToRows } from './tradesMapper';
import type { BacktestSummary, TradeRow, ValidationRecordRow } from '../tauri-client/commands';

export const VALIDATION_RECORD_VERSION = 'validation-record-v1';
export const BENCHMARK_RECORD_VERSION = 'bench-record-v1';
// Contract versions recorded for reproducibility. They name the adopted docs:
export const BENCHMARK_CONTRACT_VERSION = 'benchmark-suite-v1'; // docs/benchmark-suite-contract.md
export const GATE_CONTRACT_VERSION = 'gate-v1'; // docs/gate-contract.md
export const EXECUTION_CONTRACT_VERSION = 'backtest-execution-v1'; // docs/backtest-execution-contract.md

// ---------- benchmark record (Resolution D2) ----------

export interface BenchmarkRecordEntry {
  id: DeterministicBenchmarkId;
  /** The exact strategy backtested; null only for buyHold, whose behaviour is
   *  fixed by the benchmark contract (enter first tested close, hold to EOD). */
  strat: ParamsStrategy | null;
  /** JSON-safe metrics snapshot — never equity/trades. */
  metrics: EncodedMetrics;
}

export interface BenchmarkRecord {
  version: typeof BENCHMARK_RECORD_VERSION;
  benchmarkContract: typeof BENCHMARK_CONTRACT_VERSION;
  interval: string;
  /** Inclusive bar range of the validation segment (split-plan contract). */
  validationRange: { from: number; to: number };
  startEquity: number | null;
  /** Candidate-inherited costs (legacy percent units). */
  costs: BenchmarkCosts;
  benchmarks: BenchmarkRecordEntry[];
  /** Full Random Entry evidence, including the netReturns distribution. */
  randomEntry: RandomEntryBenchmark;
}

export interface BuildBenchmarkRecordArgs {
  interval: string;
  validationRange: { from: number; to: number };
  startEquity?: number;
  costs: BenchmarkCosts;
  benchmarks: BenchmarkRun[];
  randomEntry: RandomEntryBenchmark;
}

/** Snapshot the §6 benchmark outputs: metrics only, all four ids required. */
export function buildBenchmarkRecord(args: BuildBenchmarkRecordArgs): BenchmarkRecord {
  const byId = new Map(args.benchmarks.map((b) => [b.id, b]));
  const missing = DETERMINISTIC_BENCHMARK_IDS.filter((id) => !byId.has(id));
  if (missing.length > 0) {
    throw new RangeError(`missing deterministic benchmark(s): ${missing.join(', ')}`);
  }
  return {
    version: BENCHMARK_RECORD_VERSION,
    benchmarkContract: BENCHMARK_CONTRACT_VERSION,
    interval: args.interval,
    validationRange: { ...args.validationRange },
    startEquity: args.startEquity ?? null,
    costs: { ...args.costs },
    benchmarks: DETERMINISTIC_BENCHMARK_IDS.map((id) => {
      const run = byId.get(id)!;
      return { id, strat: run.strat ? { ...run.strat } : null, metrics: encodeMetrics(run.result.metrics) };
    }),
    randomEntry: { ...args.randomEntry, netReturns: [...args.randomEntry.netReturns] },
  };
}

// ---------- gate encoding (Resolution D4) ----------

export interface EncodedGateCriterion {
  id: GateCriterionId;
  pass: boolean;
  /** Finite observed value, or null with an explicit status. */
  value: number | null;
  valueStatus: NonFiniteStatus | null;
  threshold: number;
  detail?: string;
}

export interface EncodedGateVerdict {
  version: typeof GATE_CONTRACT_VERSION;
  pass: boolean;
  criteria: EncodedGateCriterion[];
  config: GateConfig;
}

/** JSON-safe GateVerdict: criterion values may be non-finite (e.g. an
 *  infinite metric fed a criterion), so they get the METRIC-001 statuses. */
export function encodeGateVerdict(verdict: GateVerdict): EncodedGateVerdict {
  return {
    version: GATE_CONTRACT_VERSION,
    pass: verdict.pass,
    criteria: verdict.criteria.map((c) => {
      const status = c.value == null ? null : nonFiniteStatus(c.value);
      return {
        id: c.id,
        pass: c.pass,
        value: status ? null : c.value,
        valueStatus: status,
        threshold: c.threshold,
        ...(c.detail !== undefined ? { detail: c.detail } : {}),
      };
    }),
    config: { ...verdict.config },
  };
}

// ---------- assessment outcome (Resolution D3 discriminated union) ----------

/** Gate pass carries a full ScoreBreakdown; gate fail can never carry one. */
export type AssessmentOutcome =
  | { passed: true; gate: GateVerdict; score: ScoreBreakdown }
  | { passed: false; gate: GateVerdict };

// ---------- the immutable record (Resolution D4) ----------

export interface ValidationRecord {
  version: typeof VALIDATION_RECORD_VERSION;
  contracts: {
    execution: typeof EXECUTION_CONTRACT_VERSION;
    benchmark: typeof BENCHMARK_CONTRACT_VERSION;
    gate: typeof GATE_CONTRACT_VERSION;
    score: typeof SCORE_FORMULA_VERSION | null;
  };
  strategyId: number;
  strategyHash: string;
  datasetId: number;
  datasetHash: string;
  embargo: EmbargoDerivation;
  splitPlan: ValidationSplitPlan;
  trainMetrics: EncodedMetrics;
  validationMetrics: EncodedMetrics;
  benchmark: BenchmarkRecord;
  gate: EncodedGateVerdict;
  gatePassed: boolean;
  score: ScoreBreakdown | null;
  testedCombinations: { n: number; basis: 'lineage-final-unique' };
}

export interface BuildValidationRecordArgs {
  strategyId: number;
  strategyHash: string;
  datasetId: number;
  datasetHash: string;
  embargo: EmbargoDerivation;
  splitPlan: ValidationSplitPlan;
  /** Train + Validation segment results (VAL-002). Test does not exist. */
  validationRun: Pick<ValidationRunResult, 'train' | 'validation'>;
  benchmark: BenchmarkRecord;
  outcome: AssessmentOutcome;
  /** Lineage-final unique hypotheses; must equal the score's evidence when
   *  the gate passed. */
  testedCombinations: number;
}

/** Build the self-contained immutable snapshot. Throws on inconsistent
 *  outcome/verdict/evidence and on any unencoded non-finite number. */
export function buildValidationRecord(args: BuildValidationRecordArgs): ValidationRecord {
  const { outcome } = args;
  if (outcome.passed !== outcome.gate.pass) {
    throw new RangeError('outcome.passed must equal the GateVerdict.pass it embeds');
  }
  if (!Number.isSafeInteger(args.testedCombinations) || args.testedCombinations < 1) {
    throw new RangeError('testedCombinations must be a positive safe integer');
  }
  if (outcome.passed) {
    if (!Number.isFinite(outcome.score.score)) {
      throw new RangeError('a passing outcome requires a finite score');
    }
    if (outcome.score.testedCombinations.n !== args.testedCombinations) {
      throw new RangeError('testedCombinations must match the score evidence');
    }
  }

  const record: ValidationRecord = {
    version: VALIDATION_RECORD_VERSION,
    contracts: {
      execution: EXECUTION_CONTRACT_VERSION,
      benchmark: BENCHMARK_CONTRACT_VERSION,
      gate: GATE_CONTRACT_VERSION,
      score: outcome.passed ? SCORE_FORMULA_VERSION : null,
    },
    strategyId: args.strategyId,
    strategyHash: args.strategyHash,
    datasetId: args.datasetId,
    datasetHash: args.datasetHash,
    embargo: { ...args.embargo },
    splitPlan: args.splitPlan,
    trainMetrics: encodeMetrics(args.validationRun.train.metrics),
    validationMetrics: encodeMetrics(args.validationRun.validation.metrics),
    benchmark: args.benchmark,
    gate: encodeGateVerdict(outcome.gate),
    gatePassed: outcome.passed,
    score: outcome.passed ? outcome.score : null,
    testedCombinations: { n: args.testedCombinations, basis: 'lineage-final-unique' },
  };

  assertJsonSafe(record, 'validation record');
  return record;
}

// ---------- the atomic save bundle (Resolution D5) ----------

export interface ValidationBundle {
  trainSummary: BacktestSummary;
  trainTrades: TradeRow[];
  validationSummary: BacktestSummary;
  validationTrades: TradeRow[];
  record: ValidationRecordRow;
}

export interface BuildValidationBundleArgs {
  record: ValidationRecord;
  /** The same run the record snapshotted — trades/equity feed the summaries. */
  validationRun: Pick<ValidationRunResult, 'train' | 'validation'>;
}

const segmentTimes = (equity: { time: number }[]): { startTime: number; endTime: number } => {
  if (equity.length === 0) throw new RangeError('segment equity must not be empty');
  return { startTime: equity[0].time, endTime: equity[equity.length - 1].time };
};

const metricsMatch = (snapshot: EncodedMetrics, metrics: Metrics): boolean =>
  JSON.stringify(snapshot) === JSON.stringify(encodeMetrics(metrics));

/**
 * Assemble everything `save_validation_record` persists in one transaction.
 * Per D3: the Train row's Phase B fields stay unset (null in SQLite); the
 * Validation row carries gate_passed + benchmark record, and score fields
 * exactly when the gate passed.
 */
export function buildValidationBundle(args: BuildValidationBundleArgs): ValidationBundle {
  const { record, validationRun } = args;
  if (!metricsMatch(record.trainMetrics, validationRun.train.metrics)
    || !metricsMatch(record.validationMetrics, validationRun.validation.metrics)) {
    throw new RangeError('validationRun does not match the record metric snapshots');
  }

  const trainSummary = metricsToBacktestSummary(validationRun.train.metrics, {
    strategyId: record.strategyId,
    datasetId: record.datasetId,
    segment: 'train',
    ...segmentTimes(validationRun.train.equity),
  });

  const validationSummary: BacktestSummary = {
    ...metricsToBacktestSummary(validationRun.validation.metrics, {
      strategyId: record.strategyId,
      datasetId: record.datasetId,
      segment: 'validation',
      ...segmentTimes(validationRun.validation.equity),
    }),
    gate_passed: record.gatePassed,
    score: record.score ? record.score.score : null,
    score_breakdown_json: record.score ? JSON.stringify(record.score) : null,
    benchmark_result_json: JSON.stringify(record.benchmark),
  };

  const row: ValidationRecordRow = {
    strategy_id: record.strategyId,
    dataset_id: record.datasetId,
    record_version: record.version,
    gate_passed: record.gatePassed,
    score: record.score ? record.score.score : null,
    record_json: JSON.stringify(record),
  };

  return {
    trainSummary,
    trainTrades: tradesToRows(validationRun.train.trades),
    validationSummary,
    validationTrades: tradesToRows(validationRun.validation.trades),
    record: row,
  };
}
