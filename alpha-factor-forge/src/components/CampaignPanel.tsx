import React, { useCallback, useEffect, useRef, useState } from 'react';
import { campaigns, db, discovery, isTauri } from '../tauri-client/dataClient';
import type {
  CampaignAdmission, CampaignPreview, CampaignSummary, MarketInstrument, MarketSnapshotOption,
} from '../tauri-client/commands';
import {
  buildCampaignDeclaration, buildCampaignRunConfig,
  type CampaignSamplePolicy, type CampaignSampling, type SelectedSnapshot,
} from '../services/campaignAuthoring';
import { randomRootSeed } from '../services/discoveryRunConfig';
import type { ParamsStrategy } from '../services/strategy';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';

type Draft = { option: MarketSnapshotOption; instrument: MarketInstrument; policy: CampaignSamplePolicy };
type SavedBinding = {
  instrumentId: string;
  snapshotId: string;
  datasetHash: string;
  interval: string;
  fromMs: number;
  toMs: number;
  samplePolicy: CampaignSamplePolicy;
};

const EMPTY_POLICY: CampaignSamplePolicy = {
  minimumTotalBars: 0, minimumTrainBars: 0, foldValidationBars: 0, foldCount: 0, rationale: '',
};
const INITIAL_SAMPLING: CampaignSampling = {
  alphaPpm: 50_000, maxRelativeStandardErrorPpm: 200_000,
  bootstrapSamples: 100_000, maxBootstrapSamples: 200_000,
};
const POLICY_LABEL: Record<Exclude<keyof CampaignSamplePolicy, 'rationale'>, string> = {
  minimumTotalBars: '總 bars 下限', minimumTrainBars: 'Train bars 下限',
  foldValidationBars: '每折 Validation bars', foldCount: 'Train 折數',
};
const SAMPLING_LABEL: Record<keyof CampaignSampling, string> = {
  alphaPpm: '顯著水準（ppm）', maxRelativeStandardErrorPpm: '最大相對標準誤（ppm）',
  bootstrapSamples: 'Bootstrap 樣本數', maxBootstrapSamples: 'Bootstrap 上限',
};
const REASON_LABEL: Record<string, string> = {
  snapshot_changed: '快照已變更', ledger_prefix_unproven: '帳本前綴無法證明',
  ledger_count_inconsistent: '帳本計數不一致', family_unknown: '未知試驗家族',
  family_quarantined: '試驗家族已隔離', registry_chain_broken: '帳本鏈結損壞',
  no_effective_trials: '沒有有效試驗', legacy_trials_unknown: '舊試驗數未知',
  insufficient_total_bars: '歷史 bars 不足', walk_forward_not_eligible: 'Walk-forward 不符合條件',
  precision_not_eligible: '精度不足',
};

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function savedBindings(row: CampaignSummary): SavedBinding[] {
  const value = row.document.instruments;
  if (!Array.isArray(value)) throw new Error('已保存的 campaign 缺少 instruments');
  return value as SavedBinding[];
}

export function CampaignPanel({ strategy }: { strategy: ParamsStrategy }): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const [open, setOpen] = useState(false);
  const [instruments, setInstruments] = useState<MarketInstrument[]>([]);
  const [snapshots, setSnapshots] = useState<MarketSnapshotOption[]>([]);
  const [saved, setSaved] = useState<CampaignSummary[]>([]);
  const [selected, setSelected] = useState<Draft[]>([]);
  const [sampling, setSampling] = useState<CampaignSampling>(INITIAL_SAMPLING);
  const [preview, setPreview] = useState<{ document: string; result: CampaignPreview } | null>(null);
  const [frozenId, setFrozenId] = useState<string | null>(null);
  const [choice, setChoice] = useState('');
  const [rootSeed, setRootSeed] = useState(randomRootSeed);
  const [holdingAllowanceBars, setHoldingAllowanceBars] = useState(0);
  const [decision, setDecision] = useState<{ runId: number; value: CampaignAdmission } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const draftRevision = useRef(0);

  const reload = useCallback(async () => {
    const [nextInstruments, nextSnapshots, nextSaved] = await Promise.all([
      campaigns.listInstruments(), campaigns.listSnapshots(), campaigns.list(),
    ]);
    setInstruments(nextInstruments);
    setSnapshots(nextSnapshots);
    setSaved(nextSaved);
  }, []);

  useEffect(() => {
    if (!open || !isTauri()) return;
    let cancelled = false;
    Promise.all([campaigns.listInstruments(), campaigns.listSnapshots(), campaigns.list()])
      .then(([rows, options, campaignsSaved]) => {
        if (cancelled) return;
        setInstruments(rows);
        setSnapshots(options);
        setSaved(campaignsSaved);
      })
      .catch((cause) => { if (!cancelled) setError(errorText(cause)); });
    return () => { cancelled = true; };
  }, [open]);

  const invalidateDraft = () => { draftRevision.current += 1; setPreview(null); setFrozenId(null); };
  const act = async (name: string, action: () => Promise<void>): Promise<void> => {
    if (busy != null) return;
    setBusy(name); setError(null); setMessage(null);
    try { await action(); } catch (cause) { setError(errorText(cause)); }
    finally { setBusy(null); }
  };

  const addSnapshot = () => {
    const option = snapshots.find((item) => item.snapshot.snapshotId === choice);
    if (!option) return;
    const instrument = instruments.find((item) => item.id === option.snapshot.instrumentRowId);
    if (!instrument) { setError('此 snapshot 缺少對應的 instrument 修訂'); return; }
    if (instrument.listedFrom == null) { setError('此 instrument 缺少上市起點，無法宣告'); return; }
    setSelected((previous) => [
      ...previous.filter((item) => item.option.snapshot.instrumentId !== option.snapshot.instrumentId),
      { option, instrument, policy: { ...EMPTY_POLICY } },
    ]);
    setChoice(''); invalidateDraft(); setError(null);
  };

  const setPolicy = (id: string, key: keyof CampaignSamplePolicy, value: string) => {
    setSelected((previous) => previous.map((item) => item.option.snapshot.instrumentId === id
      ? { ...item, policy: { ...item.policy, [key]: key === 'rationale' ? value : Number(value) } }
      : item));
    invalidateDraft();
  };

  const draftDocument = () => buildCampaignDeclaration(selected as SelectedSnapshot[], sampling);
  const previewDraft = () => act('preview', async () => {
    const document = draftDocument();
    const revision = draftRevision.current;
    const result = await campaigns.preview(document);
    if (revision !== draftRevision.current) return;
    setPreview({ document: JSON.stringify(document), result });
    setMessage(result.instruments.every((item) => item.resolved)
      ? '快照已由後端逐項驗證；可凍結保存'
      : '有快照尚未通過後端驗證，請先修正');
  });
  const freeze = () => act('freeze', async () => {
    const document = draftDocument();
    if (preview?.document !== JSON.stringify(document) || preview.result.instruments.some((item) => !item.resolved)) {
      throw new Error('請先取得全部通過的最新預覽');
    }
    const id = await campaigns.freeze(document);
    if (id !== preview.result.campaignId) throw new Error('凍結 ID 與預覽 ID 不一致，請重新整理');
    setFrozenId(id);
    await reload();
    setMessage(`已凍結並保存 campaign ${id}`);
  });

  const start = (row: CampaignSummary, binding: SavedBinding) => act('start', async () => {
    const active = await discovery.getActiveRun();
    if (active != null) throw new Error('已有探索任務執行中，請等它完成後再啟動下一個 instrument');
    if (strategy.mode !== 'params') throw new Error('目前 campaign 啟動需使用參數模式策略');
    const option = snapshots.find((item) => item.snapshot.snapshotId === binding.snapshotId);
    if (option && (option.snapshot.instrumentId !== binding.instrumentId
      || option.snapshot.datasetHash !== binding.datasetHash || option.snapshot.interval !== binding.interval
      || option.dataset.startTime !== binding.fromMs || option.dataset.endTime !== binding.toMs)) {
      throw new Error('已保存的 snapshot 與目前清單不一致');
    }
    const datasets = option ? [] : await db.getDatasets();
    const datasetId = option?.dataset.id ?? datasets.find((item) => item.dataset_hash === binding.datasetHash
      && item.interval === binding.interval && item.start_time === binding.fromMs
      && item.end_time === binding.toMs)?.id;
    if (datasetId == null) throw new Error('找不到此 campaign 的資料集；請確認工作區資料仍在');
    const { envelope } = buildCampaignRunConfig({ id: datasetId, hash: binding.datasetHash }, binding.samplePolicy, strategy, rootSeed, holdingAllowanceBars);
    const runId = await campaigns.start(envelope, row.campaignId, binding.instrumentId);
    setMessage(`已啟動 ${binding.instrumentId} 的探索任務 #${runId}`);
    try {
      const value = await campaigns.admission(runId);
      if (value != null) setDecision({ runId, value });
      setSaved(await campaigns.list());
    } catch (cause) {
      setError(`run #${runId} 已啟動，但判定或清單讀取失敗：${errorText(cause)}。可按「重新整理清單」重試讀取。`);
    }
  });
  const showDecision = (runId: number) => act('decision', async () => {
    const value = await campaigns.admission(runId);
    if (value == null) throw new Error(`找不到 run #${runId} 的 campaign 判定`);
    setDecision({ runId, value });
  });

  let currentDocument: string | null = null;
  try { currentDocument = JSON.stringify(draftDocument()); } catch { /* incomplete form */ }
  const canFreeze = preview != null && preview.document === currentDocument
    && preview.result.instruments.length === selected.length
    && preview.result.instruments.every((item) => item.resolved) && busy == null;
  const decisionReport = decision?.value.report;
  const reasons = Array.isArray(decisionReport?.reasons) ? decisionReport.reasons : [];
  const precision = decisionReport?.precision;
  const snapshot = decisionReport?.snapshot;
  const compact = (value: unknown) => value == null ? '—' : typeof value === 'object' ? JSON.stringify(value) : String(value);
  const precisionValues = precision != null && typeof precision === 'object' ? precision as Record<string, unknown> : null;
  const snapshotValues = snapshot != null && typeof snapshot === 'object' ? snapshot as Record<string, unknown> : null;

  return <section data-testid="campaign-panel" style={{ ...S.card, marginTop: 12 }}>
    <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
      <h2 style={{ ...S.h2, margin: 0 }}>研究 Campaign</h2>
      <button data-testid="campaign-toggle" style={S.btnGhost} onClick={() => setOpen((value) => !value)}>{open ? '收合' : '展開'}</button>
    </div>
    {open && <div style={{ display: 'grid', gap: 14, marginTop: 12 }}>
      <p style={{ color: t.color.muted, margin: 0 }}>從既有 snapshot 撰寫宣告。資料與 snapshot 需先由資料來源工具建立；預覽會由後端檢查每個綁定。</p>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <select data-testid="campaign-snapshot-choice" aria-label="新增 snapshot" style={{ ...S.input, width: 'auto', minWidth: 290 }} value={choice} onChange={(event) => setChoice(event.target.value)}>
          <option value="">選擇已建立的 snapshot</option>
          {snapshots.filter((item) => item.qualificationEligible).map((item) =>
            <option key={item.snapshot.snapshotId} value={item.snapshot.snapshotId}>{item.snapshot.instrumentId} · {item.dataset.interval} · {item.snapshot.snapshotId.slice(0, 10)}</option>)}
        </select>
        <button data-testid="campaign-add" style={S.btnGhost} disabled={!choice || busy != null} onClick={addSnapshot}>加入 instrument</button>
        <button data-testid="campaign-refresh" style={S.btnGhost} disabled={busy != null} onClick={() => act('refresh', reload)}>重新整理清單</button>
      </div>
      {snapshots.length === 0 && <div data-testid="campaign-no-snapshots" style={{ color: t.color.warn }}>尚無已建立的 snapshot；請先用資料來源工具建立。</div>}
      {selected.map(({ option, policy }) => <div key={option.snapshot.instrumentId} data-testid={`campaign-draft-${option.snapshot.instrumentId}`} style={{ ...S.card, display: 'grid', gap: 8 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8 }}>
          <strong>{option.snapshot.instrumentId}</strong>
          <button style={S.btnGhost} onClick={() => { setSelected((prev) => prev.filter((item) => item.option.snapshot.instrumentId !== option.snapshot.instrumentId)); invalidateDraft(); }}>移除</button>
        </div>
        <div style={{ color: t.color.muted, fontFamily: t.font.mono, overflowWrap: 'anywhere' }}>snapshot {option.snapshot.snapshotId} · dataset {option.dataset.id} · {option.dataset.candleCount} bars · {option.dataset.interval}</div>
        <div style={{ ...S.grid3, gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))' }}>
          {(['minimumTotalBars', 'minimumTrainBars', 'foldValidationBars', 'foldCount'] as const).map((key) => <label key={key} style={S.label}>{POLICY_LABEL[key]}<input data-testid={`campaign-${key}-${option.snapshot.instrumentId}`} style={S.input} type="number" min="1" value={policy[key] || ''} onChange={(event) => setPolicy(option.snapshot.instrumentId, key, event.target.value)} /></label>)}
        </div>
        <label style={S.label}>樣本政策理由<textarea data-testid={`campaign-rationale-${option.snapshot.instrumentId}`} style={S.input} rows={2} value={policy.rationale} onChange={(event) => setPolicy(option.snapshot.instrumentId, 'rationale', event.target.value)} /></label>
      </div>)}
      <div style={{ ...S.grid3, gridTemplateColumns: 'repeat(auto-fit, minmax(145px, 1fr))' }}>
        {(['alphaPpm', 'maxRelativeStandardErrorPpm', 'bootstrapSamples', 'maxBootstrapSamples'] as const).map((key) => <label key={key} style={S.label}>{SAMPLING_LABEL[key]}<input data-testid={`campaign-${key}`} style={S.input} type="number" min="1" value={sampling[key]} onChange={(event) => { setSampling((prev) => ({ ...prev, [key]: Number(event.target.value) })); invalidateDraft(); }} /></label>)}
      </div>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <button data-testid="campaign-preview-button" style={S.btnGhost} disabled={busy != null || selected.length === 0} onClick={previewDraft}>後端預覽</button>
        <button data-testid="campaign-freeze" style={S.btn} disabled={!canFreeze} onClick={freeze}>凍結並保存</button>
      </div>
      {preview && <div data-testid="campaign-preview" style={S.card}>
        <div style={{ overflowWrap: 'anywhere' }}>Campaign ID：{preview.result.campaignId}</div>
        {preview.result.instruments.map((item) => <div key={item.instrumentId}>{item.instrumentId}：{item.resolved ? `已解析，${item.barCount} bars` : `未通過：${item.error ?? '未知原因'}`}</div>)}
      </div>}
      {frozenId && <div data-testid="campaign-frozen" style={{ overflowWrap: 'anywhere' }}>已保存：{frozenId}</div>}
      <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
        <label style={S.label}>run seed<input data-testid="campaign-seed" style={S.input} type="number" min="0" value={rootSeed} onChange={(event) => setRootSeed(Number(event.target.value))} /></label>
        <label style={S.label}>持倉延伸 bars<input data-testid="campaign-holding" style={S.input} type="number" min="0" value={holdingAllowanceBars} onChange={(event) => setHoldingAllowanceBars(Number(event.target.value))} /></label>
      </div>
      <div data-testid="campaign-saved-list" style={{ display: 'grid', gap: 8 }}>
        <h3 style={{ ...S.h2, fontSize: t.font.size }}>已保存 Campaign</h3>
        {saved.length === 0 && <span style={{ color: t.color.muted }}>尚無已保存的宣告</span>}
        {saved.map((row) => <div key={row.campaignId} data-testid={`campaign-saved-${row.campaignId}`} style={{ ...S.card, display: 'grid', gap: 6 }}>
          <div style={{ overflowWrap: 'anywhere', fontFamily: t.font.mono }}>{row.campaignId}</div>
          {savedBindings(row).map((binding) => <div key={binding.instrumentId} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
            <span>{binding.instrumentId}</span>
            <button data-testid={`campaign-start-${binding.instrumentId}`} style={S.btnGhost} disabled={busy != null} onClick={() => start(row, binding)}>啟動此 instrument</button>
          </div>)}
          {row.runs.map((run) => <div key={run.runId}>
            <button data-testid={`campaign-decision-${run.runId}`} style={S.btnGhost} disabled={busy != null} onClick={() => showDecision(run.runId)}>run #{run.runId} · {run.instrumentId} · {run.status}</button>
          </div>)}
        </div>)}
      </div>
      {decision && <div data-testid="campaign-decision-view" style={S.card}>
        <h3 style={{ ...S.h2, fontSize: t.font.size }}>run #{decision.runId} · Admission：{decision.value.status}</h3>
        <div>原因：{reasons.length ? reasons.map((reason) => `${REASON_LABEL[String(reason)] ?? String(reason)}（${String(reason)}）`).join('、') : '無'}</div>
        <div>精度：{precisionValues == null ? '尚無可用的帳本計數' : <>
          家族試驗 {compact(precisionValues.familyTrials)} · 檢定數 {compact(precisionValues.familyTests)} ·
          最佳調整 p {compact(precisionValues.bestAdjustedPValue)} ·
          精度所需樣本 {compact(precisionValues.requiredSamplesForPrecision)}
        </>}</div>
        <div>snapshot：{compact(snapshotValues?.snapshotId)} · {compact(snapshotValues?.barCount)} bars</div>
        <div>凍結成本：fee {decision.value.feePct}% · slip {decision.value.slipPct}%</div>
      </div>}
      {error && <div data-testid="campaign-error" style={S.banner('error')}>{error}</div>}
      {message && <div data-testid="campaign-message" style={S.banner('ok')}>{message}</div>}
    </div>}
  </section>;
}
