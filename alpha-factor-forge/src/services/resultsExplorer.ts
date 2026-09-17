// P01 Results Explorer — the pure half of the saved-results viewer.
//
// The component reads three existing tables through the typed client
// (`backtest_summary`, `trades`, `validation_records`) and asks this module how
// to present them. Nothing here recomputes a metric, a gate, or a score: every
// number shown is the persisted number, and a persisted null stays a dash.
//
// Two rules from the plan (docs/plans/active-plan.md §4.7, P01):
//   - Validation ranking only. `test` segment rows never exist today (the
//     split contract never executes Test) but migration 0001 admits the value,
//     so the reader hides it explicitly rather than trusting the writer. The
//     split plan's Test range is likewise never surfaced.
//   - Missing history is stated, not papered over. `backtest_summary` is a
//     latest-result projection whose trades are replaced on every re-save, so
//     a record whose summaries were overwritten reports that, and a
//     `record_json` that cannot be read is shown as unreadable with the reason.

import type {
  BacktestSummary,
  Dataset,
  StrategyDef,
  ValidationRecordRow,
} from '../tauri-client/commands';
import { VALIDATION_RECORD_VERSION } from './validationRecord';

// ---------- visibility ----------

/** Segments a reader may show. `test` is excluded by contract. */
export const VISIBLE_SEGMENTS: readonly BacktestSummary['segment'][] = ['train', 'validation', 'full'];

export function hideTestSegments(summaries: readonly BacktestSummary[]): BacktestSummary[] {
  return summaries.filter((summary) => VISIBLE_SEGMENTS.includes(summary.segment));
}

// ---------- validation ranking ----------

/**
 * Validation ranking, gate-passed candidates first by persisted score
 * (descending), then gate-failed rows. Ties and the failed block fall back to
 * newest first, then higher id, so the order is total and stable. Scores are
 * read from the row, never from `record_json`: the DB CHECK (PERSIST-001)
 * already guarantees gate fail ⇒ null score.
 */
export function rankValidationRecords(records: readonly ValidationRecordRow[]): ValidationRecordRow[] {
  const newestFirst = (a: ValidationRecordRow, b: ValidationRecordRow): number => {
    const byTime = (b.created_at ?? '').localeCompare(a.created_at ?? '');
    if (byTime !== 0) return byTime;
    return (b.id ?? 0) - (a.id ?? 0);
  };
  return records.slice().sort((a, b) => {
    if (a.gate_passed !== b.gate_passed) return a.gate_passed ? -1 : 1;
    if (a.gate_passed && b.gate_passed) {
      const sa = typeof a.score === 'number' ? a.score : Number.NEGATIVE_INFINITY;
      const sb = typeof b.score === 'number' ? b.score : Number.NEGATIVE_INFINITY;
      if (sa !== sb) return sb - sa;
    }
    return newestFirst(a, b);
  });
}

export interface ExplorerFilters {
  strategyId: number | null;
  datasetId: number | null;
  gatePassedOnly: boolean;
}

export const NO_FILTERS: ExplorerFilters = { strategyId: null, datasetId: null, gatePassedOnly: false };

export function filterRecords(
  records: readonly ValidationRecordRow[],
  filters: ExplorerFilters,
): ValidationRecordRow[] {
  return records.filter((row) =>
    (filters.strategyId == null || row.strategy_id === filters.strategyId)
    && (filters.datasetId == null || row.dataset_id === filters.datasetId)
    && (!filters.gatePassedOnly || row.gate_passed));
}

export function filterSummaries(
  summaries: readonly BacktestSummary[],
  filters: Pick<ExplorerFilters, 'strategyId' | 'datasetId'>,
): BacktestSummary[] {
  return summaries.filter((row) =>
    (filters.strategyId == null || row.strategy_id === filters.strategyId)
    && (filters.datasetId == null || row.dataset_id === filters.datasetId));
}

// ---------- joins to the parent rows ----------

/** The latest summary per segment for one strategy × dataset. A segment that
 *  was never saved (or whose row was deleted) is simply absent. */
export function latestSummariesFor(
  summaries: readonly BacktestSummary[],
  strategyId: number,
  datasetId: number,
): Partial<Record<BacktestSummary['segment'], BacktestSummary>> {
  const out: Partial<Record<BacktestSummary['segment'], BacktestSummary>> = {};
  for (const summary of hideTestSegments(summaries)) {
    if (summary.strategy_id !== strategyId || summary.dataset_id !== datasetId) continue;
    const current = out[summary.segment];
    // The backend key upserts, so at most one row per segment exists; keep the
    // newest if a mock or a future reader ever hands us two.
    if (!current || (summary.created_at ?? '') > (current.created_at ?? '')) out[summary.segment] = summary;
  }
  return out;
}

/** Every persisted column of a summary row, in one place, so the identity
 *  comparison below cannot silently skip a column that a future save changes. */
export const SUMMARY_COLUMNS: readonly (keyof BacktestSummary)[] = [
  'id', 'strategy_id', 'dataset_id', 'segment', 'start_time', 'end_time',
  'net_return', 'cagr', 'max_drawdown', 'sharpe', 'sortino', 'calmar', 'win_rate',
  'trade_count', 'profit_factor', 'avg_trade_return', 'median_trade_return',
  'exposure', 'turnover', 'largest_win', 'largest_loss', 'consecutive_losses',
  'gate_passed', 'score', 'score_breakdown_json', 'benchmark_result_json', 'created_at',
];

/**
 * Whether two summary rows are the same persisted row with the same content.
 *
 * The persistence key (strategy, dataset, segment) reuses the row id and
 * `created_at` on a re-save, so neither identifies a generation; the whole
 * row does. Absent and null are the same thing here: the mock stores whatever
 * the writer omitted, SQLite reads it back as null. Numbers are compared with
 * `Object.is` so NaN never sneaks through as "equal to itself".
 */
export function sameSummaryRow(a: BacktestSummary, b: BacktestSummary): boolean {
  return SUMMARY_COLUMNS.every((column) => {
    const x = a[column] ?? null;
    const y = b[column] ?? null;
    return Object.is(x, y);
  });
}

export function describeStrategy(strategies: readonly StrategyDef[], id: number): string {
  const def = strategies.find((row) => row.id === id);
  return def ? `${def.name} · ${def.type} #${id}` : `策略 #${id}（已不存在）`;
}

export function describeDataset(datasets: readonly Dataset[], id: number): string {
  const ds = datasets.find((row) => row.id === id);
  return ds ? `${ds.symbol} ${ds.interval} #${id}` : `資料集 #${id}（已不存在）`;
}

// ---------- formatting (persisted values only) ----------

export const DASH = '—';

export function fmtPct(x: number | null | undefined): string {
  return typeof x === 'number' && Number.isFinite(x) ? `${(x * 100).toFixed(2)}%` : DASH;
}

export function fmtNum(x: number | null | undefined, digits = 2): string {
  return typeof x === 'number' && Number.isFinite(x) ? x.toFixed(digits) : DASH;
}

export function fmtInt(x: number | null | undefined): string {
  return typeof x === 'number' && Number.isFinite(x) ? String(x) : DASH;
}

export function fmtTime(ms: number | null | undefined): string {
  if (typeof ms !== 'number' || !Number.isFinite(ms)) return DASH;
  return new Date(ms).toISOString().replace('T', ' ').slice(0, 16);
}

/** `strategy-v2:abcd…` → `abcd…ef01` (prefix dropped, 12 hex chars). */
export function shortHash(hash: string | null | undefined): string {
  if (!hash) return DASH;
  const body = hash.includes(':') ? hash.slice(hash.indexOf(':') + 1) : hash;
  return body.length > 12 ? `${body.slice(0, 8)}…${body.slice(-4)}` : body;
}

export interface SummaryCell {
  label: string;
  value: string;
}

/** The persisted metric columns of one summary row, formatted. Nulls are
 *  dashes: a null column means the mapper narrowed a non-finite value (see
 *  `metricsMapper.ts`), and the reader has no business guessing it. */
export function summaryCells(summary: BacktestSummary): SummaryCell[] {
  return [
    { label: '淨報酬', value: fmtPct(summary.net_return) },
    { label: 'CAGR', value: fmtPct(summary.cagr) },
    { label: '最大回撤', value: fmtPct(summary.max_drawdown) },
    { label: 'Sharpe', value: fmtNum(summary.sharpe) },
    { label: 'Sortino', value: fmtNum(summary.sortino) },
    { label: 'Calmar', value: fmtNum(summary.calmar) },
    { label: '勝率', value: fmtPct(summary.win_rate) },
    { label: '交易數', value: fmtInt(summary.trade_count) },
    { label: 'Profit Factor', value: fmtNum(summary.profit_factor) },
    { label: '平均每筆', value: fmtPct(summary.avg_trade_return) },
    { label: '曝險', value: fmtPct(summary.exposure) },
    { label: '換手', value: fmtNum(summary.turnover) },
  ];
}

// ---------- record_json (defensive read of the immutable snapshot) ----------

export interface ParsedGateCriterion {
  id: string;
  pass: boolean;
  value: number | null;
  valueStatus: string | null;
  threshold: number | null;
}

export interface ParsedScoreEntry {
  id: string;
  raw: number | null;
  normalized: number | null;
  weight: number | null;
  contribution: number | null;
}

export interface ParsedBenchmark {
  id: string;
  netReturn: number | null;
}

export type ParsedRecord =
  | {
    kind: 'v2';
    contracts: Record<string, string | null>;
    strategyHash: string | null;
    datasetHash: string | null;
    embargo: { embargoBars: number; maxSignalLookbackBars: number; holdingAllowanceBars: number } | null;
    /** Train and Validation bar ranges only; the Test range is never surfaced. */
    split: { totalBars: number | null; train: Range | null; validation: Range | null } | null;
    validationNetReturn: number | null;
    gate: { pass: boolean; criteria: ParsedGateCriterion[] } | null;
    score: { score: number; components: ParsedScoreEntry[]; penalties: ParsedScoreEntry[] } | null;
    benchmarks: ParsedBenchmark[];
    randomEntry: { runs: number; candidatePercentile: number } | null;
    testedCombinations: number | null;
    /** Sections the snapshot should have carried but did not. */
    missing: string[];
  }
  | { kind: 'legacy'; version: string }
  | { kind: 'unreadable'; reason: string };

export interface Range { from: number; to: number }

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);
const finiteOrNull = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null;
const stringOrNull = (value: unknown): string | null => (typeof value === 'string' ? value : null);
const range = (value: unknown): Range | null => {
  if (!isObject(value)) return null;
  const from = finiteOrNull(value.from);
  const to = finiteOrNull(value.to);
  return from != null && to != null ? { from, to } : null;
};

const scoreEntries = (value: unknown): ParsedScoreEntry[] =>
  Array.isArray(value)
    ? value.filter(isObject).map((entry) => ({
      id: stringOrNull(entry.id) ?? '?',
      raw: finiteOrNull(entry.raw),
      normalized: finiteOrNull(entry.normalized),
      weight: finiteOrNull(entry.weight),
      contribution: finiteOrNull(entry.contribution),
    }))
    : [];

/**
 * Read the parts of a `validation-record-v2` snapshot the explorer shows.
 * Anything absent or malformed is reported in `missing` / as null rather than
 * defaulted — the snapshot is the audit evidence, and a reader that invents
 * values would be worse than one that says "not recorded".
 */
export function parseValidationRecordJson(row: Pick<ValidationRecordRow, 'record_version' | 'record_json'>): ParsedRecord {
  if (row.record_version !== VALIDATION_RECORD_VERSION) {
    return { kind: 'legacy', version: row.record_version };
  }
  let json: unknown;
  try {
    json = JSON.parse(row.record_json);
  } catch (error) {
    return { kind: 'unreadable', reason: error instanceof Error ? error.message : String(error) };
  }
  if (!isObject(json)) return { kind: 'unreadable', reason: 'record_json 不是物件' };
  if (json.version !== VALIDATION_RECORD_VERSION) {
    return { kind: 'unreadable', reason: `record_json.version 為 ${String(json.version)}，與列的 record_version 不符` };
  }

  const missing: string[] = [];
  const need = <T>(key: string, value: T | null): T | null => {
    if (value == null) missing.push(key);
    return value;
  };

  const contracts: Record<string, string | null> = {};
  if (isObject(json.contracts)) {
    for (const [key, value] of Object.entries(json.contracts)) contracts[key] = stringOrNull(value);
  } else {
    missing.push('contracts');
  }

  const embargoRaw = isObject(json.embargo) ? json.embargo : null;
  const embargo = embargoRaw
    && finiteOrNull(embargoRaw.embargoBars) != null
    && finiteOrNull(embargoRaw.maxSignalLookbackBars) != null
    && finiteOrNull(embargoRaw.holdingAllowanceBars) != null
    ? {
      embargoBars: embargoRaw.embargoBars as number,
      maxSignalLookbackBars: embargoRaw.maxSignalLookbackBars as number,
      holdingAllowanceBars: embargoRaw.holdingAllowanceBars as number,
    }
    : null;

  const splitRaw = isObject(json.splitPlan) ? json.splitPlan : null;
  const split = splitRaw
    ? { totalBars: finiteOrNull(splitRaw.totalBars), train: range(splitRaw.train), validation: range(splitRaw.validation) }
    : null;

  const validationMetrics = isObject(json.validationMetrics) && isObject(json.validationMetrics.values)
    ? json.validationMetrics.values
    : null;

  const gateRaw = isObject(json.gate) ? json.gate : null;
  const gate = gateRaw && typeof gateRaw.pass === 'boolean'
    ? {
      pass: gateRaw.pass,
      criteria: Array.isArray(gateRaw.criteria)
        ? gateRaw.criteria.filter(isObject).map((criterion) => ({
          id: stringOrNull(criterion.id) ?? '?',
          pass: criterion.pass === true,
          value: finiteOrNull(criterion.value),
          valueStatus: stringOrNull(criterion.valueStatus),
          threshold: finiteOrNull(criterion.threshold),
        }))
        : [],
    }
    : null;

  const scoreRaw = isObject(json.score) ? json.score : null;
  const scoreValue = scoreRaw ? finiteOrNull(scoreRaw.score) : null;
  const score = scoreRaw && scoreValue != null
    ? { score: scoreValue, components: scoreEntries(scoreRaw.components), penalties: scoreEntries(scoreRaw.penalties) }
    : null;
  // A gate-failed record legitimately has `score: null`; only a PASSED record
  // is missing something when its score cannot be read.
  if (gate?.pass === true && score == null) missing.push('score');

  const benchRaw = isObject(json.benchmark) ? json.benchmark : null;
  const benchmarks: ParsedBenchmark[] = benchRaw && Array.isArray(benchRaw.benchmarks)
    ? benchRaw.benchmarks.filter(isObject).map((entry) => ({
      id: stringOrNull(entry.id) ?? '?',
      netReturn: isObject(entry.metrics) && isObject(entry.metrics.values)
        ? finiteOrNull(entry.metrics.values.netReturn)
        : null,
    }))
    : [];
  const randomEntryRaw = benchRaw && isObject(benchRaw.randomEntry) ? benchRaw.randomEntry : null;
  const randomEntry = randomEntryRaw
    && finiteOrNull(randomEntryRaw.runs) != null
    && finiteOrNull(randomEntryRaw.candidatePercentile) != null
    ? { runs: randomEntryRaw.runs as number, candidatePercentile: randomEntryRaw.candidatePercentile as number }
    : null;

  if (benchmarks.length === 0) missing.push('benchmark');

  const tested = isObject(json.testedCombinations) ? finiteOrNull(json.testedCombinations.n) : null;

  return {
    kind: 'v2',
    contracts,
    strategyHash: need('strategyHash', stringOrNull(json.strategyHash)),
    datasetHash: need('datasetHash', stringOrNull(json.datasetHash)),
    embargo: need('embargo', embargo),
    split: need('splitPlan', split),
    validationNetReturn: validationMetrics ? finiteOrNull(validationMetrics.netReturn) : null,
    gate: need('gate', gate),
    score,
    benchmarks,
    randomEntry: need('randomEntry', randomEntry),
    testedCombinations: need('testedCombinations', tested),
    missing,
  };
}
