// P05 — the research history: every attempt the runner made, the hypothesis
// it tested (frozen before execution), what it ran against and on, and the
// immutable result behind a completed attempt. Read-only: the runner writes
// this inside its own transactions and nothing edits or deletes it, so a
// re-run adds attempts and the earlier ones stay exactly as they were. The
// Results Explorer above shows the LATEST projection; this shows the history
// that produced it.

import React, { useCallback, useEffect, useState } from 'react';
import { research } from '../tauri-client/dataClient';
import type { ResearchAttempt, ResearchAttemptDetail, ResearchAttemptStatus } from '../tauri-client/commands';
import { DASH, fmtNum, fmtPct, shortHash } from '../services/resultsExplorer';
import { HelpTip } from './HelpTip';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';

const STATUS_LABEL: Record<ResearchAttemptStatus, string> = {
  submitted: '已提交',
  running: '執行中',
  completed: '已完成',
  failed: '失敗',
  skipped: '略過',
};

type LoadState = 'idle' | 'loading' | 'ready' | 'error';

/** The pieces of a `candidate-result-v1` document the list needs. */
function resultDigest(result: Record<string, unknown> | null): { train: string; validation: string } {
  const segment = (key: 'train' | 'validation'): string => {
    const summary = (result?.[key] as { summary?: Record<string, unknown> } | undefined)?.summary;
    if (summary == null) return DASH;
    const net = typeof summary.net_return === 'number' ? fmtPct(summary.net_return) : DASH;
    const trades = typeof summary.trade_count === 'number' ? summary.trade_count : DASH;
    return `${net} · ${trades} 筆`;
  };
  return { train: segment('train'), validation: segment('validation') };
}

function outcomeText(attempt: ResearchAttempt): string {
  const outcome = attempt.outcome;
  if (outcome == null) return DASH;
  if (attempt.status === 'completed') {
    const gate = outcome.gatePassed === true ? '通過' : outcome.gatePassed === false ? '未通過' : DASH;
    const score = typeof outcome.score === 'number' ? fmtNum(outcome.score) : DASH;
    return `Gate ${gate} · Score ${score}`;
  }
  if (typeof outcome.error === 'string') return outcome.error;
  if (typeof outcome.reason === 'string') return outcome.reason;
  return JSON.stringify(outcome);
}

function fmtStamp(iso: string | null): string {
  if (iso == null) return DASH;
  return iso.replace('T', ' ').slice(0, 16);
}

export function ResearchHistory(): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const [open, setOpen] = useState(false);
  const [state, setState] = useState<LoadState>('idle');
  const [err, setErr] = useState<string | null>(null);
  const [attempts, setAttempts] = useState<ResearchAttempt[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setDetail] = useState<ResearchAttemptDetail | null>(null);
  const [detailErr, setDetailErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    setState('loading');
    setErr(null);
    try {
      const rows = await research.listAttempts({ limit: 200 });
      setAttempts(rows);
      setState('ready');
    } catch (error) {
      setErr(error instanceof Error ? error.message : String(error));
      setState('error');
    }
  }, []);

  // Loaded when opened, and re-read on demand; the list never refreshes
  // itself under the reader's eyes.
  useEffect(() => {
    if (open && state === 'idle') void load();
  }, [open, state, load]);

  async function select(id: number): Promise<void> {
    setSelectedId(id);
    setDetail(null);
    setDetailErr(null);
    try {
      const loaded = await research.getAttempt(id);
      if (loaded == null) {
        setDetailErr(`嘗試 #${id} 已不存在`);
        return;
      }
      setDetail(loaded);
    } catch (error) {
      setDetailErr(error instanceof Error ? error.message : String(error));
    }
  }

  const cell: React.CSSProperties = { padding: '3px 8px', borderBottom: `1px solid ${t.color.line}`, whiteSpace: 'nowrap' };
  const digest = detail != null ? resultDigest(detail.result) : null;

  return (
    <section style={{ ...S.card, marginTop: 12 }} data-testid="research-history">
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: open ? 10 : 0, flexWrap: 'wrap' }}>
        <h2 style={{ ...S.h2, margin: 0 }}>研究歷史（Research History）</h2>
        <HelpTip
          id="research-history"
          label="研究歷史"
          text="探索任務的每一次嘗試：執行前凍結的假說、輸入與引擎指紋、結果或失敗原因，以及完成時保存的不可變結果檔。重跑會新增嘗試，不會改寫舊的；上方「歷史結果」顯示的是最新投影。"
        />
        <button data-testid="research-history-toggle" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => setOpen((o) => !o)}>
          {open ? '收合' : '展開'}
        </button>
        {open && (
          <button data-testid="research-history-refresh" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => void load()} disabled={state === 'loading'}>
            重新讀取
          </button>
        )}
        {state === 'ready' && (
          <span data-testid="research-history-count" style={{ fontSize: 11, color: t.color.muted }}>嘗試 {attempts.length}</span>
        )}
      </div>

      {open && (
        <>
          {err != null && <div data-testid="research-history-error" style={{ fontSize: 12, color: t.color.danger }}>{err}</div>}
          {state === 'ready' && attempts.length === 0 && (
            <div data-testid="research-history-empty" style={{ fontSize: 12, color: t.color.muted }}>
              還沒有研究嘗試。啟動探索任務後，每個候選都會在這裡留下一筆。
            </div>
          )}
          {attempts.length > 0 && (
            <table data-testid="research-history-attempts" style={{ borderCollapse: 'collapse', fontSize: 11, fontFamily: t.font.mono }}>
              <thead>
                <tr>
                  {['#', '工作', '候選', '狀態', '結果', '策略', '提交', '完成'].map((head) => (
                    <th key={head} style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'left' }}>{head}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {attempts.map((row) => (
                  <tr
                    key={row.id}
                    data-testid={`research-history-attempt-${row.id}`}
                    data-status={row.status}
                    style={{ cursor: 'pointer', background: row.id === selectedId ? t.color.surface2 : undefined }}
                    onClick={() => void select(row.id)}
                  >
                    <td style={cell}>#{row.id}</td>
                    <td style={cell}>{row.discoveryRunId ?? DASH}</td>
                    <td style={cell}>{row.candidateIndex ?? DASH}</td>
                    <td style={cell}>{STATUS_LABEL[row.status] ?? row.status}</td>
                    <td style={{ ...cell, whiteSpace: 'normal', maxWidth: 360 }}>{outcomeText(row)}</td>
                    <td style={cell} title={String(row.inputFingerprint.strategyHash ?? '')}>
                      #{row.strategyId} {shortHash(typeof row.inputFingerprint.strategyHash === 'string' ? row.inputFingerprint.strategyHash : null)}
                    </td>
                    <td style={cell}>{fmtStamp(row.submittedAt)}</td>
                    <td style={cell}>{fmtStamp(row.finishedAt)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {detailErr != null && <div data-testid="research-history-detail-error" style={{ fontSize: 12, color: t.color.danger, marginTop: 8 }}>{detailErr}</div>}
          {detail != null && (
            <div data-testid="research-history-detail" style={{ marginTop: 10, fontSize: 12, display: 'grid', gap: 4 }}>
              <div>
                <strong>嘗試 #{detail.attempt.id}</strong> · {detail.attempt.attemptKey} · {STATUS_LABEL[detail.attempt.status]}
                {detail.attempt.epoch != null && <span style={{ color: t.color.muted }}> · epoch {detail.attempt.epoch}</span>}
              </div>
              {detail.hypothesis != null ? (
                <div data-testid="research-history-hypothesis">
                  <div><strong>假說 #{detail.hypothesis.id}</strong>（{detail.hypothesis.source} · {detail.hypothesis.variationKind ?? DASH}）</div>
                  <div>機制：{detail.hypothesis.mechanism}</div>
                  <div>失效情境：{detail.hypothesis.failureModes}</div>
                </div>
              ) : (
                <div style={{ color: t.color.muted }}>此嘗試沒有假說紀錄。</div>
              )}
              <div data-testid="research-history-fingerprints" style={{ color: t.color.muted, fontFamily: t.font.mono, fontSize: 11 }}>
                資料集 {shortHash(typeof detail.attempt.inputFingerprint.datasetHash === 'string' ? detail.attempt.inputFingerprint.datasetHash : null)}
                {' · '}策略 {shortHash(typeof detail.attempt.inputFingerprint.strategyHash === 'string' ? detail.attempt.inputFingerprint.strategyHash : null)}
                {' · '}引擎 {String(detail.attempt.engineFingerprint.package ?? DASH)}
                {detail.attempt.resultArtifact != null && ` · 結果檔 ${detail.attempt.resultArtifact.sha256.slice(0, 12)} (${detail.attempt.resultArtifact.byteLen} bytes)`}
              </div>
              {detail.result != null && digest != null && (
                <div data-testid="research-history-result">
                  Train {digest.train} · Validation {digest.validation}
                </div>
              )}
              {detail.resultError != null && (
                <div data-testid="research-history-result-error" style={{ color: t.color.danger }}>結果檔無法讀取：{detail.resultError}</div>
              )}
              {detail.result == null && detail.resultError == null && detail.attempt.status !== 'completed' && (
                <div style={{ color: t.color.muted }}>{outcomeText(detail.attempt)}</div>
              )}
            </div>
          )}
        </>
      )}
    </section>
  );
}
