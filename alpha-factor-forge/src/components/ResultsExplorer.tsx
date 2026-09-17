// P01 — Results Explorer: re-open what the workspace has already saved.
//
// Reads the existing `validation_records`, `backtest_summary`, and `trades`
// tables through the typed client and shows them without recomputing anything
// (`services/resultsExplorer.ts` owns every presentation rule). Four rules
// shape the component — three from the plan, one from the acceptance review:
//   1. Validation ranking only, Test hidden. Rows are ordered by the persisted
//      gate/score; a `test` segment is filtered out by the service even though
//      no writer produces one today.
//   2. Loads on demand, never behind the user's back. The explorer reads when
//      it is opened or refreshed and stamps the load time; a background run that
//      finishes later does not replace what is on screen (the runner panel
//      reports that itself). A selection survives a refresh and, if the row is
//      no longer in the filtered list, is reported as such rather than dropped.
//      A read that fails stays failed — with its message — until the user asks
//      again; the first read happens once per open, never in a retry loop
//      (acceptance review R2).
//   3. Missing history is said out loud. Summaries are a latest-result
//      projection whose trades are replaced on re-save, so a record whose
//      summaries are gone says so; a snapshot that cannot be read says why.
//   4. Trades belong to the summary on screen, not to an id. The persistence
//      key reuses a summary id on re-save and replaces its trades, so a trade
//      read is a one-transaction (summary, trades) pair whose summary must equal
//      the displayed row column for column (`sameSummaryRow`); a mismatch is
//      disclosed and the trades are not shown, and a response that lands after
//      a refresh is dropped (acceptance review R1).

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { db, isTauri } from '../tauri-client/dataClient';
import type {
  BacktestSummary,
  Dataset,
  StrategyDef,
  TradeRow,
  ValidationRecordRow,
} from '../tauri-client/commands';
import {
  DASH,
  NO_FILTERS,
  sameSummaryRow,
  describeDataset,
  describeStrategy,
  filterRecords,
  filterSummaries,
  fmtInt,
  fmtNum,
  fmtPct,
  fmtTime,
  hideTestSegments,
  latestSummariesFor,
  parseValidationRecordJson,
  rankValidationRecords,
  shortHash,
  summaryCells,
  type ExplorerFilters,
  type ParsedRecord,
} from '../services/resultsExplorer';
import { HelpTip } from './HelpTip';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';

interface Loaded {
  strategies: StrategyDef[];
  datasets: Dataset[];
  summaries: BacktestSummary[];
  records: ValidationRecordRow[];
  loadedAt: number;
}

type View = 'records' | 'summaries';

/** idle: never read for this open. failed keeps `err` (and any earlier
 *  `data`) until an explicit refresh; nothing re-reads on its own. */
type LoadStatus = 'idle' | 'loading' | 'ready' | 'failed';

/** What one trade read established about the summary it was asked for. */
type DetailState =
  | { kind: 'ok'; trades: TradeRow[] }
  /** The persisted row no longer equals the displayed row: something re-saved
   *  the same key in between. The trades are NOT kept — they belong to
   *  `latest`, not to what is on screen. */
  | { kind: 'stale'; latest: BacktestSummary }
  /** The summary row itself is gone. */
  | { kind: 'gone' };

const SEGMENT_LABEL: Record<BacktestSummary['segment'], string> = {
  train: 'Train',
  validation: 'Validation',
  test: 'Test',
  full: '全期',
};

const GATE_LABEL: Record<string, string> = {
  minTrades: '最少交易數',
  avgTradeReturn: '平均每筆（成本後）',
  rollingConsistency: '滾動一致性',
  maxDrawdown: '最大回撤',
  monthlyConcentration: '單月集中度',
  tradeConcentration: '單筆集中度',
  benchmarkWins: '勝過基準',
  randomEntryPercentile: 'Random Entry 百分位',
};

export function ResultsExplorer(): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<LoadStatus>('idle');
  const [err, setErr] = useState<string | null>(null);
  const [data, setData] = useState<Loaded | null>(null);
  const [view, setView] = useState<View>('records');
  const [filters, setFilters] = useState<ExplorerFilters>(NO_FILTERS);
  const [selectedRecordId, setSelectedRecordId] = useState<number | null>(null);
  const [selectedSummaryId, setSelectedSummaryId] = useState<number | null>(null);
  // Trade detail is fetched per displayed summary on demand. Entries are keyed
  // by summary id but only ever hold what was verified against the row on
  // screen (rule 4); a refresh replaces the snapshot and empties this map.
  const [trades, setTrades] = useState<Map<number, DetailState>>(new Map());
  const [tradesLoading, setTradesLoading] = useState<number | null>(null);
  const [tradesErr, setTradesErr] = useState<string | null>(null);
  // Bumped by every list read. A trade read captures the generation it was
  // started under and is dropped if a refresh has moved it on since (rule 4):
  // a late response must not populate the cache of a newer snapshot.
  const readGenRef = useRef(0);

  const load = useCallback(async () => {
    readGenRef.current += 1;
    setStatus('loading');
    setErr(null);
    try {
      const [strategies, datasets, summaries, records] = await Promise.all([
        db.getStrategies(),
        db.getDatasets(),
        db.getBacktestResults(),
        db.listValidationRecords(),
      ]);
      setData({ strategies, datasets, summaries: hideTestSegments(summaries), records, loadedAt: Date.now() });
      setTrades(new Map());
      setTradesErr(null);
      setStatus('ready');
    } catch (error) {
      // Keep whatever was on screen and say why the read failed; do not retry.
      setErr(error instanceof Error ? error.message : String(error));
      setStatus('failed');
    }
  }, []);

  // The first open reads exactly once. `status` is the guard: a failure moves
  // it to 'failed', which this effect does not touch, so the only way to read
  // again is the refresh button (rule 2, review R2).
  useEffect(() => {
    if (open && status === 'idle' && isTauri()) void load();
  }, [open, status, load]);
  const loading = status === 'loading';

  const rankedRecords = useMemo(
    () => (data ? rankValidationRecords(filterRecords(data.records, filters)) : []),
    [data, filters],
  );
  const visibleSummaries = useMemo(
    () => (data ? filterSummaries(data.summaries, filters) : []),
    [data, filters],
  );

  const selectedRecord = data?.records.find((row) => row.id === selectedRecordId) ?? null;
  const selectedRecordListed = rankedRecords.some((row) => row.id === selectedRecordId);
  const selectedSummary = data?.summaries.find((row) => row.id === selectedSummaryId) ?? null;
  const selectedSummaryListed = visibleSummaries.some((row) => row.id === selectedSummaryId);

  /** Read the (summary, trades) pair for the DISPLAYED summary and keep the
   *  trades only if the persisted summary is still that exact row. */
  const loadTrades = async (displayed: BacktestSummary): Promise<void> => {
    const summaryId = displayed.id!;
    if (trades.has(summaryId)) return;
    const gen = readGenRef.current;
    setTradesLoading(summaryId);
    setTradesErr(null);
    try {
      const detail = await db.getBacktestResultDetail(summaryId);
      // A refresh happened while this was in flight: the snapshot this read was
      // meant for is gone, so the answer is discarded rather than cached.
      if (gen !== readGenRef.current) return;
      const state: DetailState = detail == null
        ? { kind: 'gone' }
        : sameSummaryRow(displayed, detail.summary)
          ? { kind: 'ok', trades: detail.trades }
          : { kind: 'stale', latest: detail.summary };
      setTrades((current) => new Map(current).set(summaryId, state));
    } catch (error) {
      if (gen !== readGenRef.current) return;
      setTradesErr(error instanceof Error ? error.message : String(error));
    } finally {
      setTradesLoading((current) => (current === summaryId ? null : current));
    }
  };

  const setFilter = <K extends keyof ExplorerFilters>(key: K, value: ExplorerFilters[K]): void =>
    setFilters((current) => ({ ...current, [key]: value }));

  const selectStyle: React.CSSProperties = { ...S.input, fontSize: 11, minWidth: 160 };
  const cell: React.CSSProperties = { padding: '3px 8px', whiteSpace: 'nowrap' };
  const rowStyle = (selected: boolean): React.CSSProperties => ({
    ...S.tableRow,
    cursor: 'pointer',
    background: selected ? t.color.accentWash : 'transparent',
  });

  return (
    <section style={{ ...S.card, marginTop: 12 }} data-testid="results-explorer">
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: open ? 10 : 0, flexWrap: 'wrap' }}>
        <h2 style={{ ...S.h2, margin: 0 }}>歷史結果（Results Explorer）</h2>
        <HelpTip
          id="results-explorer"
          label="歷史結果"
          text="重新開啟資料庫裡已保存的結果：驗證紀錄（依 Validation 的 Gate／Score 排名，Test 永不顯示）與回測摘要（最新結果投影）及其交易明細。此處只讀取，不重算；空值以「—」呈現。"
        />
        <button data-testid="results-explorer-toggle" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => setOpen((o) => !o)}>
          {open ? '收合' : '展開'}
        </button>
        {data != null && (
          <span data-testid="results-explorer-loaded-at" data-loaded-at={data.loadedAt} style={{ fontSize: 11, color: t.color.muted }}>
            載入於 {fmtTime(data.loadedAt)} · 紀錄 {data.records.length} · 摘要 {data.summaries.length}
          </span>
        )}
      </div>

      {open && (
        <>
          <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
            <div style={{ ...S.tabTrack, width: 220 }}>
              <button data-testid="results-explorer-view-records" style={S.tab(view === 'records')} onClick={() => setView('records')}>驗證紀錄</button>
              <button data-testid="results-explorer-view-summaries" style={S.tab(view === 'summaries')} onClick={() => setView('summaries')}>回測摘要</button>
            </div>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
              <span style={S.label}>策略</span>
              <select
                data-testid="results-explorer-filter-strategy"
                value={filters.strategyId ?? ''}
                onChange={(e) => setFilter('strategyId', e.target.value === '' ? null : Number(e.target.value))}
                style={selectStyle}
              >
                <option value="">全部策略</option>
                {(data?.strategies ?? []).map((row) => (
                  <option key={row.id} value={row.id}>{row.name} · {row.type} #{row.id}</option>
                ))}
              </select>
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
              <span style={S.label}>資料集</span>
              <select
                data-testid="results-explorer-filter-dataset"
                value={filters.datasetId ?? ''}
                onChange={(e) => setFilter('datasetId', e.target.value === '' ? null : Number(e.target.value))}
                style={selectStyle}
              >
                <option value="">全部資料集</option>
                {(data?.datasets ?? []).map((row) => (
                  <option key={row.id} value={row.id}>{row.symbol} {row.interval} #{row.id}</option>
                ))}
              </select>
            </label>
            {view === 'records' && (
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, paddingBottom: 4 }}>
                <input
                  data-testid="results-explorer-filter-gate"
                  type="checkbox"
                  checked={filters.gatePassedOnly}
                  onChange={(e) => setFilter('gatePassedOnly', e.target.checked)}
                />
                只看通過 Gate
              </label>
            )}
            <button data-testid="results-explorer-refresh" style={S.btnGhost} onClick={() => void load()} disabled={loading} aria-busy={loading}>
              {loading ? '載入中…' : '重新整理'}
            </button>
          </div>

          {!isTauri() && (
            <div style={{ fontSize: 11, color: t.color.muted, marginTop: 8 }}>瀏覽器模式沒有資料庫可讀。</div>
          )}
          {err && (
            <div data-testid="results-explorer-error" style={{ fontSize: 12, color: t.color.danger, marginTop: 8 }}>
              讀取失敗：{err}
              {data != null ? '　下方仍是上次載入的資料。' : ''}
              　按「重新整理」再試。
            </div>
          )}

          {data != null && view === 'records' && (
            <RecordsView
              data={data}
              rows={rankedRecords}
              selected={selectedRecord}
              selectedListed={selectedRecordListed}
              onSelect={setSelectedRecordId}
              trades={trades}
              tradesLoading={tradesLoading}
              tradesErr={tradesErr}
              onLoadTrades={loadTrades}
              cell={cell}
              rowStyle={rowStyle}
            />
          )}

          {data != null && view === 'summaries' && (
            <SummariesView
              data={data}
              rows={visibleSummaries}
              selected={selectedSummary}
              selectedListed={selectedSummaryListed}
              onSelect={setSelectedSummaryId}
              trades={trades}
              tradesLoading={tradesLoading}
              tradesErr={tradesErr}
              onLoadTrades={loadTrades}
              cell={cell}
              rowStyle={rowStyle}
            />
          )}
        </>
      )}
    </section>
  );
}

// ---------- validation records ----------

interface ListProps<Row> {
  data: Loaded;
  rows: Row[];
  selected: Row | null;
  selectedListed: boolean;
  onSelect: (id: number) => void;
  trades: Map<number, DetailState>;
  tradesLoading: number | null;
  tradesErr: string | null;
  onLoadTrades: (displayed: BacktestSummary) => Promise<void>;
  cell: React.CSSProperties;
  rowStyle: (selected: boolean) => React.CSSProperties;
}

function RecordsView(props: ListProps<ValidationRecordRow>): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const { data, rows, selected, selectedListed, onSelect, cell, rowStyle } = props;

  return (
    <>
      {rows.length === 0 ? (
        <div data-testid="results-explorer-empty" style={{ fontSize: 12, color: t.color.muted, marginTop: 10 }}>
          {data.records.length === 0 ? '資料庫裡還沒有驗證紀錄。探索任務完成的候選會出現在這裡。' : '目前篩選條件下沒有紀錄。'}
        </div>
      ) : (
        <table data-testid="results-explorer-records" style={{ marginTop: 10, borderCollapse: 'collapse', fontSize: 11, fontFamily: t.font.mono }}>
          <thead>
            <tr>
              {['#', '紀錄', '策略', '資料集', 'Gate', 'Score', '工作', '建立時間', '版本'].map((head) => (
                <th key={head} style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'left' }}>{head}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, index) => (
              <tr
                key={row.id}
                data-testid={`results-explorer-record-${row.id}`}
                data-gate={row.gate_passed ? 'pass' : 'fail'}
                style={rowStyle(row.id === selected?.id)}
                onClick={() => onSelect(row.id!)}
              >
                <td style={cell}>{index + 1}</td>
                <td style={cell}>#{row.id}</td>
                <td style={cell}>{describeStrategy(data.strategies, row.strategy_id)}</td>
                <td style={cell}>{describeDataset(data.datasets, row.dataset_id)}</td>
                <td style={{ ...cell, color: row.gate_passed ? t.color.accent : t.color.muted }}>{row.gate_passed ? '通過' : '未通過'}</td>
                {/* Gate-failed rows have no score by contract, not a zero. */}
                <td style={cell}>{row.gate_passed ? fmtNum(row.score, 4) : DASH}</td>
                <td style={cell}>{row.discovery_run_id != null ? `探索 #${row.discovery_run_id}` : '手動'}</td>
                <td style={cell}>{row.created_at ?? DASH}</td>
                <td style={cell}>{row.record_version}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {selected != null && !selectedListed && (
        <div data-testid="results-explorer-stale-selection" style={{ fontSize: 12, color: t.color.warn, marginTop: 8 }}>
          所選紀錄 #{selected.id} 不在目前的篩選清單中；下方仍顯示它，換一筆即可替換。
        </div>
      )}

      {selected != null && <RecordDetail record={selected} {...props} />}
    </>
  );
}

function RecordDetail(props: ListProps<ValidationRecordRow> & { record: ValidationRecordRow }): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const { data, record, cell } = props;
  const parsed = useMemo(() => parseValidationRecordJson(record), [record]);
  const latest = latestSummariesFor(data.summaries, record.strategy_id, record.dataset_id);
  const sectionTitle: React.CSSProperties = { fontSize: 12, fontWeight: 700, marginTop: 12, marginBottom: 4 };
  const mono: React.CSSProperties = { fontFamily: t.font.mono, fontSize: 11 };

  return (
    <div data-testid="results-explorer-record-detail" style={{ marginTop: 12, borderTop: `1px solid ${t.color.line}`, paddingTop: 10, fontSize: 12 }}>
      <div style={mono}>
        <b>驗證紀錄 #{record.id}</b> · {describeStrategy(data.strategies, record.strategy_id)} · {describeDataset(data.datasets, record.dataset_id)}
        <br />
        工作 {record.discovery_run_id != null ? `探索 #${record.discovery_run_id}` : '手動保存'} · 建立 {record.created_at ?? DASH} · {record.record_version}
        {parsed.kind === 'v2' && (
          <>
            <br />
            策略指紋 {shortHash(parsed.strategyHash)} · 資料指紋 {shortHash(parsed.datasetHash)}
          </>
        )}
      </div>

      <div style={sectionTitle}>最新摘要（可被重跑覆寫的投影）</div>
      <SegmentSummaries latest={latest} note="此紀錄的 Train／Validation 摘要已不存在或已被後續重跑覆寫；下方不可變快照仍是當時的證據。" {...props} />

      <div style={sectionTitle}>不可變快照</div>
      {parsed.kind === 'legacy' && (
        <div data-testid="results-explorer-legacy" style={{ color: t.color.warn }}>
          {parsed.version}：舊版紀錄，指標公式未記錄；僅保留列上的 Gate／Score，不解讀快照內容。
        </div>
      )}
      {parsed.kind === 'unreadable' && (
        <div data-testid="results-explorer-unreadable" style={{ color: t.color.danger }}>
          快照無法讀取：{parsed.reason}
        </div>
      )}
      {parsed.kind === 'v2' && <SnapshotView parsed={parsed} cell={cell} />}
      {parsed.kind === 'v2' && parsed.missing.length > 0 && (
        <div data-testid="results-explorer-missing" style={{ color: t.color.warn, marginTop: 6 }}>
          快照缺少：{parsed.missing.join('、')}
        </div>
      )}
      <div style={{ ...S.label, marginTop: 8 }}>Test 段未執行、未揭露；此處不會顯示 Test 區間或結果。</div>
    </div>
  );
}

function SnapshotView({ parsed, cell }: { parsed: Extract<ParsedRecord, { kind: 'v2' }>; cell: React.CSSProperties }): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const mono: React.CSSProperties = { fontFamily: t.font.mono, fontSize: 11 };
  const head = (labels: string[]): React.ReactElement => (
    <thead>
      <tr>{labels.map((label) => <th key={label} style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'left' }}>{label}</th>)}</tr>
    </thead>
  );

  return (
    <div style={mono}>
      <div>
        契約 {Object.entries(parsed.contracts).map(([key, value]) => `${key}=${value ?? 'null'}`).join(' · ') || DASH}
      </div>
      <div>
        Embargo {parsed.embargo ? `${parsed.embargo.embargoBars} 根（lookback ${parsed.embargo.maxSignalLookbackBars} + 持倉寬限 ${parsed.embargo.holdingAllowanceBars}）` : DASH}
        {' · '}
        Train {parsed.split?.train ? `[${parsed.split.train.from}, ${parsed.split.train.to}]` : DASH}
        {' · '}
        Validation {parsed.split?.validation ? `[${parsed.split.validation.from}, ${parsed.split.validation.to}]` : DASH}
        {' · '}
        總根數 {fmtInt(parsed.split?.totalBars)} · 已測組合 {fmtInt(parsed.testedCombinations)}
      </div>

      {parsed.gate && (
        <table data-testid="results-explorer-gate" style={{ marginTop: 6, borderCollapse: 'collapse', fontSize: 11 }}>
          {head(['Gate 條件', '結果', '觀測值', '門檻'])}
          <tbody>
            {parsed.gate.criteria.map((criterion) => (
              <tr key={criterion.id} data-testid={`results-explorer-gate-${criterion.id}`} data-pass={criterion.pass ? 'true' : 'false'} style={S.tableRow}>
                <td style={cell}>{GATE_LABEL[criterion.id] ?? criterion.id}</td>
                <td style={{ ...cell, color: criterion.pass ? t.color.accent : t.color.danger }}>{criterion.pass ? '通過' : '未通過'}</td>
                <td style={cell}>{criterion.value != null ? fmtNum(criterion.value, 4) : (criterion.valueStatus ?? DASH)}</td>
                <td style={cell}>{fmtNum(criterion.threshold, 4)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {parsed.score && (
        <table data-testid="results-explorer-score" style={{ marginTop: 6, borderCollapse: 'collapse', fontSize: 11 }}>
          {head([`Score ${fmtNum(parsed.score.score, 4)}`, '原始值', '正規化', '權重', '貢獻'])}
          <tbody>
            {parsed.score.components.map((entry) => (
              <tr key={`c-${entry.id}`} style={S.tableRow}>
                <td style={cell}>{entry.id}</td>
                <td style={cell}>{fmtNum(entry.raw, 4)}</td>
                <td style={cell}>{fmtNum(entry.normalized, 4)}</td>
                <td style={cell}>{fmtNum(entry.weight, 2)}</td>
                <td style={cell}>{fmtNum(entry.contribution, 4)}</td>
              </tr>
            ))}
            {parsed.score.penalties.map((entry) => (
              <tr key={`p-${entry.id}`} style={S.tableRow}>
                <td style={cell}>−{entry.id}</td>
                <td style={cell}>{fmtNum(entry.raw, 4)}</td>
                <td style={cell}>{fmtNum(entry.normalized, 4)}</td>
                <td style={cell}>{fmtNum(entry.weight, 2)}</td>
                <td style={cell}>{fmtNum(entry.contribution, 4)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {parsed.gate?.pass === false && <div style={{ color: t.color.muted, marginTop: 4 }}>未通過 Gate，依契約沒有 Score。</div>}

      {parsed.benchmarks.length > 0 && (
        <div data-testid="results-explorer-benchmarks" style={{ marginTop: 6 }}>
          候選 Validation 淨報酬 {fmtPct(parsed.validationNetReturn)} · 基準：
          {parsed.benchmarks.map((b) => ` ${b.id} ${fmtPct(b.netReturn)}`).join(' ·')}
          {parsed.randomEntry && ` · Random Entry ${parsed.randomEntry.runs} 次，候選百分位 ${fmtNum(parsed.randomEntry.candidatePercentile, 1)}`}
        </div>
      )}
    </div>
  );
}

function SegmentSummaries(
  props: ListProps<ValidationRecordRow> & {
    latest: Partial<Record<BacktestSummary['segment'], BacktestSummary>>;
    note: string;
  },
): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const { latest, note, cell } = props;
  const segments = (['train', 'validation'] as const).filter((segment) => latest[segment] != null);
  if (segments.length === 0) {
    return <div data-testid="results-explorer-no-summary" style={{ color: t.color.warn, fontSize: 12 }}>{note}</div>;
  }
  const cellsBySegment = segments.map((segment) => summaryCells(latest[segment]!));
  return (
    <>
      <table data-testid="results-explorer-segment-summaries" style={{ borderCollapse: 'collapse', fontSize: 11, fontFamily: t.font.mono }}>
        <thead>
          <tr>
            <th style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'left' }} />
            {segments.map((segment) => (
              <th key={segment} style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'right' }}>{SEGMENT_LABEL[segment]} #{latest[segment]!.id}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {cellsBySegment[0].map((first, rowIndex) => (
            <tr key={first.label} style={S.tableRow}>
              <td style={{ ...cell, color: t.color.muted }}>{first.label}</td>
              {cellsBySegment.map((cells, colIndex) => (
                <td key={segments[colIndex]} style={{ ...cell, textAlign: 'right', fontWeight: 600 }}>{cells[rowIndex].value}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {segments.length < 2 && <div style={{ color: t.color.warn, fontSize: 12, marginTop: 4 }}>{note}</div>}
      <div style={{ display: 'flex', gap: 8, marginTop: 6, flexWrap: 'wrap' }}>
        {segments.map((segment) => (
          <TradesBlock key={segment} summary={latest[segment]!} label={SEGMENT_LABEL[segment]} {...props} />
        ))}
      </div>
    </>
  );
}

// ---------- summaries ----------

function SummariesView(props: ListProps<BacktestSummary>): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const { data, rows, selected, selectedListed, onSelect, cell, rowStyle } = props;

  return (
    <>
      {rows.length === 0 ? (
        <div data-testid="results-explorer-empty" style={{ fontSize: 12, color: t.color.muted, marginTop: 10 }}>
          {data.summaries.length === 0 ? '資料庫裡還沒有回測摘要。按「存檔」或完成探索任務後會出現在這裡。' : '目前篩選條件下沒有摘要。'}
        </div>
      ) : (
        <table data-testid="results-explorer-summaries" style={{ marginTop: 10, borderCollapse: 'collapse', fontSize: 11, fontFamily: t.font.mono }}>
          <thead>
            <tr>
              {['摘要', '策略', '資料集', '區段', '區間', '淨報酬', 'CAGR', '最大回撤', '交易數', 'Gate', '建立時間'].map((head) => (
                <th key={head} style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'left' }}>{head}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr
                key={row.id}
                data-testid={`results-explorer-summary-${row.id}`}
                data-segment={row.segment}
                style={rowStyle(row.id === selected?.id)}
                onClick={() => onSelect(row.id!)}
              >
                <td style={cell}>#{row.id}</td>
                <td style={cell}>{describeStrategy(data.strategies, row.strategy_id)}</td>
                <td style={cell}>{describeDataset(data.datasets, row.dataset_id)}</td>
                <td style={cell}>{SEGMENT_LABEL[row.segment]}</td>
                <td style={cell}>{fmtTime(row.start_time)} → {fmtTime(row.end_time)}</td>
                <td style={cell}>{fmtPct(row.net_return)}</td>
                <td style={cell}>{fmtPct(row.cagr)}</td>
                <td style={cell}>{fmtPct(row.max_drawdown)}</td>
                <td style={cell}>{fmtInt(row.trade_count)}</td>
                <td style={cell}>{row.gate_passed == null ? DASH : row.gate_passed ? '通過' : '未通過'}</td>
                <td style={cell}>{row.created_at ?? DASH}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {selected != null && !selectedListed && (
        <div data-testid="results-explorer-stale-selection" style={{ fontSize: 12, color: t.color.warn, marginTop: 8 }}>
          所選摘要 #{selected.id} 不在目前的篩選清單中；下方仍顯示它，換一筆即可替換。
        </div>
      )}

      {selected != null && (
        <div data-testid="results-explorer-summary-detail" style={{ marginTop: 12, borderTop: `1px solid ${t.color.line}`, paddingTop: 10, fontSize: 12 }}>
          <div style={{ fontFamily: t.font.mono, fontSize: 11 }}>
            <b>摘要 #{selected.id}</b> · {SEGMENT_LABEL[selected.segment]} · {describeStrategy(data.strategies, selected.strategy_id)} · {describeDataset(data.datasets, selected.dataset_id)}
            <br />
            {fmtTime(selected.start_time)} → {fmtTime(selected.end_time)} · 建立 {selected.created_at ?? DASH}
            {selected.gate_passed != null && ` · Gate ${selected.gate_passed ? '通過' : '未通過'}`}
            {selected.gate_passed && ` · Score ${fmtNum(selected.score, 4)}`}
          </div>
          <table style={{ marginTop: 6, borderCollapse: 'collapse', fontSize: 11, fontFamily: t.font.mono }}>
            <tbody>
              {summaryCells(selected).map((c) => (
                <tr key={c.label} style={S.tableRow}>
                  <td style={{ ...cell, color: t.color.muted }}>{c.label}</td>
                  <td style={{ ...cell, textAlign: 'right', fontWeight: 600 }}>{c.value}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div style={{ marginTop: 6 }}>
            <TradesBlock summary={selected} label={SEGMENT_LABEL[selected.segment]} {...props} />
          </div>
        </div>
      )}
    </>
  );
}

// ---------- trades ----------

function TradesBlock<Row>(props: ListProps<Row> & { summary: BacktestSummary; label: string }): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const { summary, label, trades, tradesLoading, tradesErr, onLoadTrades, cell } = props;
  const id = summary.id!;
  const detail = trades.get(id);
  const rows = detail?.kind === 'ok' ? detail.trades : null;
  const expected = summary.trade_count ?? null;

  return (
    <div data-testid={`results-explorer-trades-${id}`} style={{ minWidth: 0 }}>
      {detail == null ? (
        <button
          data-testid={`results-explorer-load-trades-${id}`}
          style={{ ...S.btnGhost, padding: '3px 10px' }}
          onClick={() => void onLoadTrades(summary)}
          disabled={tradesLoading != null}
          aria-busy={tradesLoading === id}
        >
          {tradesLoading === id ? '載入中…' : `載入 ${label} 交易明細（${fmtInt(expected)} 筆）`}
        </button>
      ) : detail.kind === 'stale' ? (
        <div data-testid={`results-explorer-stale-detail-${id}`} style={{ fontSize: 12, color: t.color.warn }}>
          摘要 #{id} 在載入後已被重新保存（畫面淨報酬 {fmtPct(summary.net_return)}、交易數 {fmtInt(expected)}；
          資料庫最新 淨報酬 {fmtPct(detail.latest.net_return)}、交易數 {fmtInt(detail.latest.trade_count)}）。
          明細屬於最新結果，未載入到這份畫面；請按「重新整理」查看最新結果。
        </div>
      ) : detail.kind === 'gone' ? (
        <div data-testid={`results-explorer-gone-detail-${id}`} style={{ fontSize: 12, color: t.color.warn }}>
          資料庫已無摘要 #{id}；請按「重新整理」。
        </div>
      ) : rows == null || rows.length === 0 ? (
        <div data-testid={`results-explorer-no-trades-${id}`} style={{ fontSize: 12, color: t.color.muted }}>
          {expected != null && expected > 0
            ? `摘要記錄 ${expected} 筆交易，但資料庫裡沒有對應明細（可能已被覆寫或早於明細保存）。`
            : `${label}：此結果沒有交易。`}
        </div>
      ) : (
        <table data-testid={`results-explorer-trade-rows-${id}`} style={{ borderCollapse: 'collapse', fontSize: 11, fontFamily: t.font.mono }}>
          <thead>
            <tr>
              {['#', '方向', '進場', '出場', '進場價', '出場價', '損益', '損益%', '原因'].map((head) => (
                <th key={head} style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'left' }}>{head}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((trade, index) => (
              <tr key={`${trade.entry_time}-${index}`} style={S.tableRow}>
                <td style={cell}>{index + 1}</td>
                <td style={cell}>{trade.side}</td>
                <td style={cell}>{fmtTime(trade.entry_time)}</td>
                <td style={cell}>{fmtTime(trade.exit_time)}</td>
                <td style={cell}>{fmtNum(trade.entry_price, 4)}</td>
                <td style={cell}>{fmtNum(trade.exit_price, 4)}</td>
                <td style={cell}>{fmtNum(trade.pnl, 4)}</td>
                <td style={cell}>{fmtPct(trade.pnl_pct)}</td>
                <td style={cell}>{trade.reason ?? DASH}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {tradesErr && tradesLoading == null && detail == null && (
        <div data-testid="results-explorer-trades-error" style={{ fontSize: 12, color: t.color.danger }}>{tradesErr}</div>
      )}
    </div>
  );
}
