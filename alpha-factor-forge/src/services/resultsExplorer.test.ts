// P01 Results Explorer — pure presentation rules over persisted rows.
//
// The record parser is exercised against records produced by the REAL
// composer chain (through the dev mock's seed), not against a hand-typed
// shape, so a change to `validation-record-v2` that the explorer cannot read
// fails here first.

import { describe, expect, it } from 'vitest';
import type { BacktestSummary, ValidationRecordRow } from '../tauri-client/commands';
import { makeMockClient } from '../tauri-client/mockClient';
import { seedHistory, SEED_LENIENT_GATE, SEED_STRATEGIES } from '../tauri-client/mockHistorySeed';
import { GATE_CONTRACT_VERSION, DEFAULT_GATE_CONFIG } from './gate';
import { SCORE_FORMULA_VERSION } from './score';
import {
  describeDataset,
  describeStrategy,
  filterRecords,
  fmtPct,
  fmtTime,
  hideTestSegments,
  latestSummariesFor,
  parseValidationRecordJson,
  rankValidationRecords,
  shortHash,
  summaryCells,
} from './resultsExplorer';

const row = (over: Partial<ValidationRecordRow>): ValidationRecordRow => ({
  id: 1,
  strategy_id: 1,
  dataset_id: 1,
  record_version: 'validation-record-v2',
  gate_passed: false,
  score: null,
  record_json: '{}',
  created_at: '2026-01-01T00:00:00Z',
  ...over,
});

describe('rankValidationRecords', () => {
  it('puts gate-passed rows first by score, then failed rows newest first', () => {
    const rows = [
      row({ id: 1, gate_passed: false, created_at: '2026-01-01T00:00:00Z' }),
      row({ id: 2, gate_passed: true, score: 0.4 }),
      row({ id: 3, gate_passed: false, created_at: '2026-01-03T00:00:00Z' }),
      row({ id: 4, gate_passed: true, score: 0.9 }),
      row({ id: 5, gate_passed: true, score: 0.4, created_at: '2026-01-02T00:00:00Z' }),
    ];
    expect(rankValidationRecords(rows).map((r) => r.id)).toEqual([4, 5, 2, 3, 1]);
  });

  it('is stable for identical rows and does not mutate its input', () => {
    const rows = [row({ id: 1 }), row({ id: 2 })];
    const ranked = rankValidationRecords(rows);
    expect(ranked.map((r) => r.id)).toEqual([2, 1]);
    expect(rows.map((r) => r.id)).toEqual([1, 2]);
  });
});

describe('filters and visibility', () => {
  it('filters by strategy, dataset, and gate', () => {
    const rows = [
      row({ id: 1, strategy_id: 1, dataset_id: 1, gate_passed: true, score: 0.1 }),
      row({ id: 2, strategy_id: 2, dataset_id: 1 }),
      row({ id: 3, strategy_id: 1, dataset_id: 2 }),
    ];
    expect(filterRecords(rows, { strategyId: 1, datasetId: null, gatePassedOnly: false }).map((r) => r.id)).toEqual([1, 3]);
    expect(filterRecords(rows, { strategyId: null, datasetId: 1, gatePassedOnly: true }).map((r) => r.id)).toEqual([1]);
  });

  it('hides test-segment summaries whatever wrote them', () => {
    const base: BacktestSummary = { strategy_id: 1, dataset_id: 1, segment: 'full', start_time: 1, end_time: 2 };
    const visible = hideTestSegments([
      base,
      { ...base, segment: 'test' },
      { ...base, segment: 'validation' },
      { ...base, segment: 'train' },
    ]);
    expect(visible.map((s) => s.segment)).toEqual(['full', 'validation', 'train']);
    expect(latestSummariesFor([base, { ...base, segment: 'test' }], 1, 1)).toEqual({ full: base });
  });
});

describe('formatting', () => {
  it('shows persisted nulls as dashes instead of inventing numbers', () => {
    const cells = summaryCells({
      strategy_id: 1,
      dataset_id: 1,
      segment: 'full',
      start_time: 0,
      end_time: 1,
      net_return: 0.1234,
      sortino: null,
      trade_count: 3,
    });
    const byLabel = Object.fromEntries(cells.map((c) => [c.label, c.value]));
    expect(byLabel['淨報酬']).toBe('12.34%');
    expect(byLabel['Sortino']).toBe('—');
    expect(byLabel['CAGR']).toBe('—');
    expect(byLabel['交易數']).toBe('3');
    expect(fmtPct(Number.NaN)).toBe('—');
    expect(fmtTime(Date.UTC(2024, 0, 2, 3, 4))).toBe('2024-01-02 03:04');
    expect(shortHash('strategy-v2:0123456789abcdef0123456789abcdef')).toBe('01234567…cdef');
    expect(shortHash(null)).toBe('—');
  });

  it('labels missing parents honestly', () => {
    expect(describeStrategy([], 7)).toBe('策略 #7（已不存在）');
    expect(describeDataset([], 7)).toBe('資料集 #7（已不存在）');
  });
});

describe('parseValidationRecordJson', () => {
  it('reports legacy versions and unreadable JSON without guessing', () => {
    expect(parseValidationRecordJson({ record_version: 'validation-record-v1', record_json: '{}' }))
      .toEqual({ kind: 'legacy', version: 'validation-record-v1' });
    expect(parseValidationRecordJson({ record_version: 'validation-record-v2', record_json: '{not json' }).kind)
      .toBe('unreadable');
    expect(parseValidationRecordJson({ record_version: 'validation-record-v2', record_json: '[]' }).kind)
      .toBe('unreadable');
    const mismatch = parseValidationRecordJson({
      record_version: 'validation-record-v2',
      record_json: JSON.stringify({ version: 'validation-record-v1' }),
    });
    expect(mismatch.kind).toBe('unreadable');
  });

  it('lists every absent section of an otherwise-versioned snapshot', () => {
    const parsed = parseValidationRecordJson({
      record_version: 'validation-record-v2',
      record_json: JSON.stringify({ version: 'validation-record-v2', gate: { pass: true, criteria: [] } }),
    });
    expect(parsed.kind).toBe('v2');
    if (parsed.kind !== 'v2') throw new Error('unreachable');
    expect(parsed.missing.slice().sort()).toEqual([
      'benchmark', 'contracts', 'datasetHash', 'embargo', 'randomEntry', 'score', 'splitPlan', 'strategyHash', 'testedCombinations',
    ]);
  });

  it('reads a real composer-built passing and failing record', async () => {
    const { db } = makeMockClient();
    const report = await seedHistory(db);
    const records = await db.listValidationRecords();
    expect(records).toHaveLength(SEED_STRATEGIES.length);

    const ranked = rankValidationRecords(records);
    // Row order is evidence from the rows themselves: the passing MA 3/8
    // assessment leads, the failed default assessment follows.
    expect(ranked[0].gate_passed).toBe(true);
    expect(ranked[0].strategy_id).toBe(report.strategyIds[1]);
    expect(ranked[1].gate_passed).toBe(false);
    expect(ranked[1].strategy_id).toBe(report.strategyIds[0]);

    const passed = parseValidationRecordJson(ranked[0]);
    if (passed.kind !== 'v2') throw new Error(`expected v2, got ${passed.kind}`);
    expect(passed.missing).toEqual([]);
    expect(passed.contracts.gate).toBe(GATE_CONTRACT_VERSION);
    expect(passed.contracts.score).toBe(SCORE_FORMULA_VERSION);
    expect(passed.gate?.pass).toBe(true);
    expect(passed.gate?.criteria).toHaveLength(8);
    expect(passed.gate?.criteria.every((c) => c.pass)).toBe(true);
    // The lenient thresholds are in the snapshot, not hidden.
    const minTrades = passed.gate?.criteria.find((c) => c.id === 'minTrades');
    expect(minTrades?.threshold).toBe(SEED_LENIENT_GATE.minTrades);
    expect(passed.score?.score).toBe(ranked[0].score);
    expect(passed.score?.components.length).toBeGreaterThan(0);
    expect(passed.benchmarks.map((b) => b.id)).toEqual(['buyHold', 'smaCross', 'rsiReversion', 'bollingerReversion']);
    expect(passed.randomEntry?.runs).toBe(50);
    expect(passed.testedCombinations).toBe(1);
    expect(passed.split?.train).toBeTruthy();
    expect(passed.split?.validation).toBeTruthy();
    expect(passed.strategyHash?.startsWith('strategy-v2:')).toBe(true);
    expect(passed.datasetHash?.startsWith('dataset-content-v2:')).toBe(true);
    expect(passed.validationNetReturn).not.toBeNull();

    const failed = parseValidationRecordJson(ranked[1]);
    if (failed.kind !== 'v2') throw new Error(`expected v2, got ${failed.kind}`);
    expect(failed.missing).toEqual([]);
    expect(failed.contracts.score).toBeNull();
    expect(failed.gate?.pass).toBe(false);
    expect(failed.gate?.criteria.some((c) => !c.pass)).toBe(true);
    expect(failed.gate?.criteria.find((c) => c.id === 'minTrades')?.threshold).toBe(DEFAULT_GATE_CONFIG.minTrades);
    expect(failed.score).toBeNull();

    // The latest-view join: Train + Validation rows per strategy, the manual
    // `full` row only for the first, and the test row nowhere.
    const summaries = await db.getBacktestResults();
    const first = latestSummariesFor(summaries, report.strategyIds[0], report.datasetId);
    expect(Object.keys(first).sort()).toEqual(['full', 'train', 'validation']);
    expect(first.full?.id).toBe(report.fullSummaryId);
    const second = latestSummariesFor(summaries, report.strategyIds[1], report.datasetId);
    expect(Object.keys(second).sort()).toEqual(['train', 'validation']);
    expect(summaries.some((s) => s.id === report.hiddenTestSummaryId)).toBe(true);
    expect(hideTestSegments(summaries).some((s) => s.id === report.hiddenTestSummaryId)).toBe(false);

    // Trades read back for the rows that have them, empty for the test row.
    const validationTrades = await db.getTrades(second.validation!.id!);
    expect(validationTrades).toHaveLength(second.validation!.trade_count!);
    expect(validationTrades.every((t, i) => i === 0 || t.entry_time >= validationTrades[i - 1].entry_time)).toBe(true);
    expect(await db.getTrades(report.hiddenTestSummaryId)).toEqual([]);
    expect(await db.getTrades(99_999)).toEqual([]);
  });
});
