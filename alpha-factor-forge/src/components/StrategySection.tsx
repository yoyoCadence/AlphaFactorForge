// Strategy editor section, extracted from BacktestPanel (REF-003b, move-only).
//
// The whole 策略 card: mode tabs (params / blocks / code), the strategy-library
// picker (list / load / refresh) + name field, the params signal selects, the
// blocks rule-builder, the code-mode editor, the indicator-period grid, the
// execution model, the Holdout toggle, and the Run button. Presentational: the
// strategy, holdout split, library rows, and every action live in BacktestPanel
// and arrive as props. Behaviour is identical to the pre-extraction inline block.

import React from 'react';
import {
  OPERAND_IDS,
  type ParamsStrategy,
  type SignalId,
  type Rule,
  type RuleOp,
  type OperandId,
} from '../services/strategy';
import { SUPPORTED_SIGNALS } from '../services/strategySignals';
import { compileExpression } from '../services/exprInterpreter';
import type { StrategyDef } from '../tauri-client/commands';
import { HelpTip } from './HelpTip';
import { NumberInput } from './NumberInput';
import { S } from './panelStyles';
import type { NumKey } from './panelTypes';

const IND_FIELDS: { key: NumKey; label: string }[] = [
  { key: 'fastMA', label: '快線 MA' },
  { key: 'slowMA', label: '慢線 MA' },
  { key: 'emaPeriod', label: 'EMA' },
  { key: 'rsiPeriod', label: 'RSI 週期' },
  { key: 'rsiBuy', label: 'RSI 買' },
  { key: 'rsiSell', label: 'RSI 賣' },
  { key: 'macdFast', label: 'MACD 快' },
  { key: 'macdSlow', label: 'MACD 慢' },
  { key: 'macdSignal', label: 'MACD 訊號' },
  { key: 'bbPeriod', label: 'BB 週期' },
  { key: 'bbMult', label: 'BB 倍數' },
];

const EXEC_FIELDS: { key: NumKey; label: string }[] = [
  { key: 'feePct', label: '手續費 %' },
  { key: 'slipPct', label: '滑價 %' },
  { key: 'sizePct', label: '部位 %' },
  { key: 'slPct', label: '停損 %' },
  { key: 'tpPct', label: '停利 %' },
];

const SIG_LABEL: Record<SignalId, string> = {
  maCrossUp: 'MA 金叉', maCrossDown: 'MA 死叉',
  emaCrossUp: '價格上穿 EMA', emaCrossDown: '價格下穿 EMA',
  priceAboveSlow: '價格 > 慢線', priceBelowSlow: '價格 < 慢線',
  rsiOversold: 'RSI 超賣上穿', rsiOverbought: 'RSI 超買下穿',
  macdCrossUp: 'MACD 金叉', macdCrossDown: 'MACD 死叉',
  bbLowerTouch: '觸布林下軌', bbUpperTouch: '觸布林上軌',
  stochOversold: 'Stoch 超賣(未支援)', stochOverbought: 'Stoch 超買(未支援)',
};

const OPERAND_LABEL: Record<OperandId, string> = {
  price: '價格', open: '開', high: '高', low: '低', volume: '量',
  maFast: '快線', maSlow: '慢線', ema: 'EMA', rsi: 'RSI',
  macd: 'MACD', macdSignal: 'MACD訊號', macdHist: 'MACD柱',
  bbUpper: '布林上', bbMid: '布林中', bbLower: '布林下',
};
const RULE_OPS: RuleOp[] = ['>', '<', '>=', '<=', 'crossUp', 'crossDown'];
const OP_LABEL: Record<RuleOp, string> = { '>': '>', '<': '<', '>=': '≥', '<=': '≤', crossUp: '上穿', crossDown: '下穿' };

const MODE_LABEL: Record<ParamsStrategy['mode'], string> = { params: '參數', blocks: '積木', code: '程式碼' };

interface CodeExpressionValidation {
  entryError: string | null;
  exitError: string | null;
  valid: boolean;
}

function expressionError(value: string): string | null {
  try {
    compileExpression(value, OPERAND_IDS);
    return null;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

/** Validate both manual code-mode expressions through the same interpreter used at run time. */
function validateCodeExpressions(entryCode: string, exitCode: string): CodeExpressionValidation {
  const entryError = expressionError(entryCode);
  const exitError = expressionError(exitCode);
  return { entryError, exitError, valid: entryError == null && exitError == null };
}

/** Editable AND-list of blocks-mode rules (left operand · op · right). */
function RuleRows({ title, rules, onChange }: { title: string; rules: Rule[]; onChange: (rules: Rule[]) => void }): React.ReactElement {
  const update = (i: number, patch: Partial<Rule>) => onChange(rules.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));
  const add = () => onChange([...rules, { l: 'price', op: '>', r: 'maSlow' }]);
  const remove = (i: number) => onChange(rules.filter((_, idx) => idx !== i));
  return (
    <div style={{ marginBottom: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
        <span style={S.label}>{title}（全部成立才觸發）</span>
        <button style={{ ...S.btnGhost, padding: '2px 8px' }} onClick={add}>＋ 規則</button>
      </div>
      {rules.length === 0 && <div style={{ fontSize: 11, color: '#aaa599' }}>（無規則 → 不觸發）</div>}
      {rules.map((r, i) => (
        <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 58px 1fr 22px', gap: 4, marginBottom: 4 }}>
          <select value={r.l} onChange={(e) => update(i, { l: e.target.value as OperandId })} style={{ ...S.input, fontSize: 11 }}>
            {OPERAND_IDS.map((id) => <option key={id} value={id}>{OPERAND_LABEL[id]}</option>)}
          </select>
          <select value={r.op} onChange={(e) => update(i, { op: e.target.value as RuleOp })} style={{ ...S.input, fontSize: 11, padding: '5px 2px' }}>
            {RULE_OPS.map((op) => <option key={op} value={op}>{OP_LABEL[op]}</option>)}
          </select>
          <input list="operand-list" value={r.r} onChange={(e) => update(i, { r: e.target.value })} placeholder="series 或數字" style={{ ...S.input, fontSize: 11 }} />
          <button onClick={() => remove(i)} title="刪除" style={{ ...S.btnGhost, padding: '2px 0' }}>×</button>
        </div>
      ))}
    </div>
  );
}

/** A code-mode expression field with live (per-keystroke) interpreter validation. */
function CodeField({ id, label, value, error, onChange }: { id: string; label: string; value: string; error: string | null; onChange: (v: string) => void }): React.ReactElement {
  const errorId = `${id}-error`;
  return (
    <label style={{ display: 'flex', flexDirection: 'column', gap: 3, marginBottom: 8 }}>
      <span style={S.label}>{label}</span>
      <textarea
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        rows={2}
        spellCheck={false}
        aria-invalid={error != null}
        aria-describedby={error ? errorId : undefined}
        style={{ ...S.input, fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, resize: 'vertical', borderColor: error ? '#d23b2f' : '#d6d2c8' }}
      />
      {error && <span id={errorId} style={{ fontSize: 10, color: '#b23b2e' }}>{error}</span>}
    </label>
  );
}

export interface StrategySectionProps {
  strat: ParamsStrategy;
  onStratChange: React.Dispatch<React.SetStateAction<ParamsStrategy>>;
  stratName: string;
  onStratNameChange: (name: string) => void;
  savedStrategies: StrategyDef[];
  savedStrategyId: number | null;
  loadingStrategies: boolean;
  onSelectSaved: (id: number | null) => void;
  onLoadStrategy: () => void;
  onRefreshStrategies: () => void;
  appliedKeys: NumKey[];
  onChangeParam: (key: NumKey, value: number) => void;
  holdout: boolean;
  onHoldoutToggle: (checked: boolean) => void;
  holdoutPct: number;
  onHoldoutPctChange: (n: number) => void;
  running: boolean;
  canRun: boolean;
  onRun: () => void;
  help: { strategy: string; exec: string; holdout: string; run: string };
}

export function StrategySection({
  strat,
  onStratChange,
  stratName,
  onStratNameChange,
  savedStrategies,
  savedStrategyId,
  loadingStrategies,
  onSelectSaved,
  onLoadStrategy,
  onRefreshStrategies,
  appliedKeys,
  onChangeParam,
  holdout,
  onHoldoutToggle,
  holdoutPct,
  onHoldoutPctChange,
  running,
  canRun,
  onRun,
  help,
}: StrategySectionProps): React.ReactElement {
  // Highlight styling for a param that the last sweep-apply set (blue accent).
  const isAppliedKey = (key: NumKey) => appliedKeys.includes(key);
  const appliedInputStyle = (key: NumKey, base: React.CSSProperties): React.CSSProperties =>
    isAppliedKey(key) ? { ...base, borderColor: '#2f6df0', background: '#eef4ff' } : base;
  const appliedLabelStyle = (key: NumKey): React.CSSProperties =>
    isAppliedKey(key) ? { ...S.label, color: '#2f6df0', fontWeight: 700 } : S.label;
  const codeValidation = validateCodeExpressions(strat.entryCode, strat.exitCode);
  const codeModeAllowsRun = strat.mode !== 'code' || codeValidation.valid;

  return (
    <section style={S.card}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
        <h2 style={{ ...S.h2, margin: 0 }}>策略</h2>
        <HelpTip id="strategy" label="策略" text={help.strategy} />
        <div style={{ display: 'flex', gap: 2 }}>
          {(['params', 'blocks', 'code'] as const).map((mode) => (
            <button
              key={mode}
              data-testid={`strategy-mode-${mode}`}
              aria-pressed={strat.mode === mode}
              onClick={() => onStratChange((s) => ({ ...s, mode }))}
              style={{
                ...S.btnGhost,
                padding: '3px 10px',
                background: strat.mode === mode ? '#16150f' : '#efece5',
                color: strat.mode === mode ? '#fff' : '#16150f',
                borderColor: strat.mode === mode ? '#16150f' : '#d6d2c8',
              }}
            >
              {MODE_LABEL[mode]}
            </button>
          ))}
        </div>
      </div>

      <div style={{ display: 'flex', alignItems: 'flex-end', gap: 6, marginBottom: 10, paddingBottom: 10, borderBottom: '1px solid #efece5', flexWrap: 'wrap' }}>
        <label style={{ display: 'flex', flex: 1, minWidth: 160, flexDirection: 'column', gap: 3 }}>
          <span style={S.label}>目前策略名稱</span>
          <input
            data-testid="strategy-name"
            value={stratName}
            onChange={(e) => onStratNameChange(e.target.value)}
            placeholder="策略名稱（可留空）"
            style={S.input}
          />
        </label>
        <label style={{ display: 'flex', flex: 1, minWidth: 180, flexDirection: 'column', gap: 3 }}>
          <span style={S.label}>策略庫（SQLite）</span>
          <select
            data-testid="strategy-library-select"
            value={savedStrategyId ?? ''}
            onChange={(e) => onSelectSaved(e.target.value ? Number(e.target.value) : null)}
            style={S.input}
            disabled={loadingStrategies}
          >
            <option value="">{loadingStrategies ? '讀取中…' : savedStrategies.length === 0 ? '尚無已存策略' : '選擇已存策略'}</option>
            {savedStrategies.filter((row) => row.id != null).map((row) => (
              <option key={row.id} value={row.id} disabled={!['params', 'blocks', 'code'].includes(row.type)}>
                {row.name} · {row.type}
              </option>
            ))}
          </select>
        </label>
        <button data-testid="load-strategy" style={S.btnGhost} onClick={onLoadStrategy} disabled={savedStrategyId == null || loadingStrategies}>
          載入
        </button>
        <button
          data-testid="refresh-strategies"
          title="重新讀取策略庫"
          style={S.btnGhost}
          onClick={onRefreshStrategies}
          disabled={loadingStrategies}
          aria-busy={loadingStrategies}
        >
          重新整理
        </button>
      </div>

      {strat.mode === 'params' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 7, marginBottom: 8 }}>
          <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
            <span style={S.label}>進場訊號</span>
            <select value={strat.entrySig} onChange={(e) => onStratChange((s) => ({ ...s, entrySig: e.target.value as SignalId }))} style={S.input}>
              {SUPPORTED_SIGNALS.map((id) => (
                <option key={id} value={id}>{SIG_LABEL[id]}</option>
              ))}
            </select>
          </label>
          <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
            <span style={S.label}>出場訊號</span>
            <select value={strat.exitSig} onChange={(e) => onStratChange((s) => ({ ...s, exitSig: e.target.value as SignalId }))} style={S.input}>
              {SUPPORTED_SIGNALS.map((id) => (
                <option key={id} value={id}>{SIG_LABEL[id]}</option>
              ))}
            </select>
          </label>
        </div>
      )}
      {strat.mode === 'blocks' && (
        <>
          <datalist id="operand-list">
            {OPERAND_IDS.map((id) => <option key={id} value={id} />)}
          </datalist>
          <RuleRows title="進場規則" rules={strat.entryRules} onChange={(rules) => onStratChange((s) => ({ ...s, entryRules: rules }))} />
          <RuleRows title="出場規則" rules={strat.exitRules} onChange={(rules) => onStratChange((s) => ({ ...s, exitRules: rules }))} />
        </>
      )}
      {strat.mode === 'code' && (
        <div style={{ marginBottom: 8 }}>
          <CodeField id="strategy-entry-code" label="進場條件 (entry)" value={strat.entryCode} error={codeValidation.entryError} onChange={(v) => onStratChange((s) => ({ ...s, entryCode: v }))} />
          <CodeField id="strategy-exit-code" label="出場條件 (exit)" value={strat.exitCode} error={codeValidation.exitError} onChange={(v) => onStratChange((s) => ({ ...s, exitCode: v }))} />
          <div style={{ fontSize: 10, color: '#8a8678', lineHeight: 1.5 }}>
            變數：{OPERAND_IDS.join(' ')}
            <br />
            函式：prev(x) · crossUp(a,b) · crossDown(a,b)　運算子：+ - * / &gt; &lt; &gt;= &lt;= == != &amp;&amp; || !
          </div>
          <div style={{ fontSize: 10, color: '#aaa599', marginTop: 4 }}>
            code 模式為手動專用，AI 不會使用；以安全直譯器求值（無 eval）。
          </div>
        </div>
      )}

      <div style={S.grid3}>
        {IND_FIELDS.map((f) => (
          <label key={f.key} data-testid={isAppliedKey(f.key) ? `applied-${f.key}` : undefined} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
            <span style={appliedLabelStyle(f.key)}>{isAppliedKey(f.key) ? `✓ ${f.label}` : f.label}</span>
            <NumberInput value={strat[f.key]} onChange={(n) => onChangeParam(f.key, n)} style={appliedInputStyle(f.key, S.input)} />
          </label>
        ))}
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 6, margin: '12px 0 8px' }}>
        <h2 style={{ ...S.h2, margin: 0 }}>執行模型</h2>
        <HelpTip id="exec" label="執行模型" text={help.exec} />
      </div>
      <div style={S.grid3}>
        {EXEC_FIELDS.map((f) => (
          <label key={f.key} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
            <span style={S.label}>{f.label}</span>
            <NumberInput value={strat[f.key]} onChange={(n) => onChangeParam(f.key, n)} style={S.input} />
          </label>
        ))}
        <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
          <span style={S.label}>方向</span>
          <select value={strat.direction} onChange={(e) => onStratChange((s) => ({ ...s, direction: e.target.value as ParamsStrategy['direction'] }))} style={S.input}>
            <option value="long">做多</option>
            <option value="short">做空</option>
            <option value="both">雙向</option>
          </select>
        </label>
        <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
          <span style={S.label}>成交價</span>
          <select value={strat.fillMode} onChange={(e) => onStratChange((s) => ({ ...s, fillMode: e.target.value as ParamsStrategy['fillMode'] }))} style={S.input}>
            <option value="close">當根收盤</option>
            <option value="nextOpen">次根開盤</option>
          </select>
        </label>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 12, flexWrap: 'wrap' }}>
        <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#8a8678', flexWrap: 'wrap' }}>
          <input
            type="checkbox"
            data-testid="holdout-toggle"
            checked={holdout}
            onChange={(e) => onHoldoutToggle(e.target.checked)}
          />
          Holdout 樣本外驗證
          {holdout && (
            <>
              <span style={{ color: '#cfccc4' }}>·</span>末
              <NumberInput
                value={holdoutPct}
                min={5}
                max={90}
                onChange={onHoldoutPctChange}
                style={{ ...S.input, width: 52, fontSize: 11, padding: '3px 5px' }}
              />
              % 為樣本外
            </>
          )}
        </label>
        <HelpTip id="holdout" label="Holdout" text={help.holdout} />
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 8 }}>
        <button
          data-testid="run-backtest"
          style={{ ...S.btn, flex: 1 }}
          onClick={onRun}
          disabled={running || !canRun || !codeModeAllowsRun}
          aria-busy={running}
        >
          {running ? '回測中…' : '▶ 執行回測'}
        </button>
        <HelpTip id="run" label="執行回測" text={help.run} align="right" />
      </div>
    </section>
  );
}
