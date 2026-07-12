# AlphaFactorForge — Improvement Backlog（可交接任務規格庫）

> 撰寫日期：2026-07-07。依據：[project-audit-masterplan.md](project-audit-masterplan.md)。
>
> **重要：本文件不是第二份任務板。** `tasks.md` 依 AGENTS.md 契約仍是唯一任務板。
> 使用方式：要執行某任務時，(1) 在 `tasks.md` 的 Next/In Progress 加一行並連結到本文件的 Task ID；(2) 把該任務的「Suggested prompt」貼給 coding agent；(3) 完成後在 `tasks.md` 移到 Done。本文件本身只在「任務規格變更」時修改。
>
> 所有任務共同前提：遵守 `AGENTS.md`（scope 控制、branch/PR 流程）與 [agent-execution-protocol.md](agent-execution-protocol.md)。**一個任務 = 一個 branch = 一個 PR。**

> ### 2026-07-12 狀態更新（審計後已變動，優先讀這段）
> 審計（2026-07-07）之後、本文件落地之前，以下已透過 PR 合併進 `main`，**規格保留為記錄但不需再執行**：
> - **FEAT-001 策略庫 = PR #27（Slice 7-3）已完成**（inline 在 `BacktestPanel`：`savedStrategies` + `services/strategyLibrary` + `strategy-library-select` testid）。
> - **Slice 10-1/10-2 圖表 wheel-zoom / drag-pan = PR #28 / #29 已完成**（原列文末「Avoid for now」，現已落地；`CandleChart.tsx` 增至 ~491 行）。
> - **Slice 8b 原生圖表視窗 = PR #30 已完成**（新 `ChartPopoutWindow.tsx`）。
> - **fix：載入 legacy 策略 = PR #31 已完成**。
>
> 因此 `BacktestPanel.tsx` 已增至 **1362 行 / 43 個 useState**（審計時 1217 / ~30）——**REF-001~003 拆解比審計時更迫切**；策略庫是 inline 落地，REF-003 應多抽一個 `LibrarySection`。
>
> **修正後的實際起手順序：DOC-001 → BUG-001 → REF-001 → REF-002 → REF-003 → TEST-002 → FEAT-002 → REF-004 → PERF-001 → …**（FEAT-001 已移除；下表原始編號保留供對照）。
>
> ### 2026-07-12 收尾更新（「Must do now」層完成）
> **DOC-001（#33）、BUG-001（#34）、REF-001（#37）、REF-002（#39）、REF-003（#40）+ REF-003b（#41）全部已合併。** `BacktestPanel.tsx` 由 **1382 → 385 行**，拆成 Sweep / Chart / Dataset / Results / Strategy 五個 section，成為純編排層——審計重構階段（含 ultrareview 追加的 `< 400` 收尾）正式關閉。**目前佇列前緣＝「Should do later」層：TEST-002 → FEAT-002 → REF-004 → PERF-001 → TEST-001 → SEC-001**（Optional：UX-002 / DOC-002；blocked：TEST-003 等 Open Question Q3）。Open Questions Q1–Q6（masterplan §8）仍待 maintainer 裁決。

## 執行順序總覽

| 順位 | Task | 分級 | Effort | 依賴 |
| --- | --- | --- | --- | --- |
| 1 | DOC-001 文件狀態單一事實來源 | **Must do now** | S | 無 |
| 2 | BUG-001 參數掃描尊重 Holdout | **Must do now** | S | 無 |
| ~~3~~ | ~~REF-001 抽出 SweepSection~~ ✅ **已完成 (PR #37)** | — | — | — |
| ~~4~~ | ~~REF-002 抽出 ChartSection~~ ✅ **已完成 (PR #39)** | — | — | — |
| ~~5~~ | ~~REF-003 抽出 DatasetSection + ResultsSection~~ ✅ **已完成 (PR #40 + #41 REF-003b)** | — | — | — |
| ~~6~~ | ~~FEAT-001 策略庫（=tasks.md Slice 7-3）~~ ✅ **已完成 (PR #27)** | — | — | — |
| 7 | TEST-002 回測引擎 golden tests + 對照報告 | Should do later | S-M | 無（先於任何引擎修改） |
| 8 | FEAT-002 交易明細（trades）持久化 | Should do later | M | TEST-002 建議先行 |
| 9 | REF-004 insert_strategy UPSERT 語義修正 | Should do later | S | FEAT-001 定案語義 |
| 10 | PERF-001 掃描移入 Web Worker | Should do later | M | REF-001 |
| 11 | TEST-001 補 e2e：模式切換／非法 code 錯誤顯示 | Should do later | S | 無 |
| 12 | SEC-001 npm audit 盤點報告 | Should do later | S | 無 |
| 13 | UX-002 頂層 Error Boundary | Optional | S | 無 |
| 14 | DOC-002 工作區衛生（gitignore／mock 偏差清單／legacy 標記） | Optional | S | 無 |
| 15 | TEST-003 ESLint + Prettier 工具鏈 | Optional（**blocked：Open Question Q3**） | M | 使用者核准新依賴 |
| — | 交易所資料 fetch、Slice 10 pan/zoom、Slice 8b 真視窗、i18n、狀態管理套件、Service Worker | **Avoid for now** | — | 見文末說明 |

---

## DOC-001 — 文件狀態單一事實來源

- **Category**: Documentation
- **Objective**: 消除 README / AGENTS.md 中已證偽的狀態敘述，讓「目前狀態」只活在 `tasks.md` Current Snapshot，其他文件以連結指向。
- **Context**: README 仍寫「25 tests」「rustc/cargo 不在 PATH、native Tauri 未驗證」；AGENTS.md §0.1 寫「not currently a valid Git repository」。實況（tasks.md）：125 tests、cargo tauri dev 通過、repo 已有 26 個 PR。agent 讀到假前提會做錯計畫（audit P4）。
- **Files likely affected**: `README.md`、`AGENTS.md`、`alpha-factor-forge/TODO.md`（僅頂部加一行指向）、`tasks.md`（僅確認 Current Snapshot 正確，不重寫）。

### Exact implementation plan

1. 讀 `tasks.md` 的 Current Snapshot 與 Done 區，確認最新事實（測試數、Rust 環境、CI 狀態）。
2. `AGENTS.md` §0.1：把「Test coverage」「Deployment / cache notes」兩段中的過時句子改為現況一句話 + 「Latest status lives in `tasks.md` → Current Snapshot」。**刪除**「The folder is not currently a valid Git repository…」整句。
3. `README.md`：三個語言版的「目前狀態 / Current Status / 現在の状態」段落，各縮減為 3-4 個 bullet：(a) 指向 `tasks.md` Current Snapshot 為唯一狀態來源；(b) baseline 驗證指令不變；(c) 保留「勿 `npm audit fix --force`」警語。刪除具體數字型敘述（25 tests、PATH 狀態）。其餘章節（架構、邊界、Roadmap）**不動**。
4. `README.md`「已知問題與待確認」中 Tauri scaffold 小節：「仍待本機 cargo check」「Rust/Cargo 不在 PATH」兩句改為現況（CI 有 cargo-check + cargo test）。
5. `alpha-factor-forge/TODO.md` 頂部加一行：「狀態快照請看根目錄 `tasks.md`；本檔為 Phase A 檔案級對照表。」內容不改。
6. 全文搜尋 `25 tests`、`not currently a valid Git repository`、`rustc.*PATH` 確認清零。

### Non-goals

- 不重寫 README 結構、不合併三語版本、不動 Roadmap/邊界章節。
- 不改 `tasks.md` 的任務內容。
- 不刪任何歷史文件（HISTORY/CONVERSATION_HISTORY 原樣保留）。

- **Risk level**: Low
- **Validation plan**: 純文件變更。`cd alpha-factor-forge && npm run typecheck`（確認沒誤碰程式）；人工重讀三份文件的變更段落；`git diff --stat` 應只含 md 檔。
- **Acceptance criteria**:
  - [ ] `grep -rn "25 tests" README.md AGENTS.md` 無結果
  - [ ] AGENTS.md 不再宣稱 git repo 無效
  - [ ] README 三語狀態段各 ≤ 5 bullets 且指向 tasks.md
  - [ ] `git diff` 只觸及 `README.md` / `AGENTS.md` / `alpha-factor-forge/TODO.md`（±tasks.md 一行）

### Suggested prompt for coding agent

```text
Read AGENTS.md fully, then docs/improvement-backlog.md task DOC-001 only. Execute exactly that task.

Repo: AlphaFactorForge. Branch off latest main as docs/status-single-source.
Scope: README.md (the three per-language "current status" sections + the Tauri scaffold known-issues bullets only), AGENTS.md §0.1 (two stale sentences), alpha-factor-forge/TODO.md (add one pointer line at top). Do NOT restructure README, do NOT touch code, do NOT edit tasks.md except verifying its Current Snapshot is the source of truth.
Facts to encode: current status lives in tasks.md Current Snapshot; the repo IS a valid git repo with CI (typecheck/test/build/cargo-check+test/e2e); do not state absolute test counts anywhere outside tasks.md; keep the "never npm audit fix --force" warning.
Validate: run `cd alpha-factor-forge && npm run typecheck` (must pass, proves no accidental code edits); `git diff --stat` must list only the three/four md files.
Deliver: commit `docs: point status to tasks.md single source`, PR body in zh-TW per repo convention, include the git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task DOC-001.
Check: (1) only README.md / AGENTS.md / alpha-factor-forge/TODO.md (and at most one line in tasks.md) changed; (2) no stale claims remain (search "25 tests", "not currently a valid Git repository", rustc/PATH claims); (3) README structure and the three-language layout preserved; (4) the npm-audit warning survived; (5) no code files touched. Output: scope compliance verdict, list of any remaining stale claims, merge recommendation.
```

---

## BUG-001 — 參數掃描尊重 Holdout（樣本內掃描）

- **Category**: Product / Correctness
- **Objective**: Holdout 開啟時，參數掃描只在樣本內區間最佳化，杜絕「在樣本外資料上調參後再用同段資料驗證」的資料窺探。
- **Context**: `runParamSweep` 已支援 `from`/`to`（`src/services/paramSweep.ts`，注釋明寫 sweep in-sample only 的用途），但 `BacktestPanel.tsx` 的 `runSweep()` 沒有傳，導致掃描永遠用全期資料。這與產品反過擬合主軸直接矛盾（audit P1）。**此為行為變更，maintainer 已在 masterplan R1 核可方向。**
- **Files likely affected**: `alpha-factor-forge/src/components/BacktestPanel.tsx`（`runSweep()` 與掃描區塊 UI 文案）、`alpha-factor-forge/e2e/sweep.spec.ts`（新增斷言）。

### Exact implementation plan

1. 在 `BacktestPanel.tsx` 的 `runSweep()` 內，於取得 `cs`（candles）後計算與 `run()` 完全相同的切分：`const split = Math.max(1, Math.min(n - 1, Math.floor(n * (1 - holdoutPct / 100))))`（n = cs.length）。建議抽一個 module-level 純函數 `holdoutSplitIndex(n: number, holdoutPct: number): number` 讓 `run()` 與 `runSweep()` 共用，消除重複。
2. `holdout === true` 時呼叫 `runParamSweep({ candles: cs, strat, interval, sweep: sweepConfig, from: 0, to: split - 1 })`；`holdout === false` 時維持現狀（不傳 from/to）。
3. UI 標示：掃描結果區（`SweepHeatmap` 上方說明文字或 sweep 區塊 header 旁）在 holdout 開啟時顯示「掃描範圍：樣本內（前 {100-holdoutPct}%）」，關閉時不顯示。用現有文字樣式，不新增元件。
4. `HELP.sweep` 說明文字補一句：holdout 開啟時掃描僅使用樣本內資料。
5. e2e：`e2e/sweep.spec.ts` 加一個 flow——載入樣本 → 開 holdout → 展開掃描 → 執行 → 斷言樣本內標示文字出現。另確認既有 sweep spec（holdout 關閉）不需改動即綠。
6. 驗證後在 `CHANGELOG.md` Unreleased 加一行（zh-TW 或英文均可，跟隨現有格式）。

### Non-goals

- 不動 `src/services/paramSweep.ts`（引擎不改）。
- 不做 walk-forward、不做「掃描後自動跑 OOS 驗證」。
- 不改 holdout 本身的切分邏輯與 `run()` 行為。

- **Risk level**: Low（行為變更但範圍小、單向；holdout 關閉路徑零改動）
- **Validation plan**:
  - `cd alpha-factor-forge && npm run typecheck && npm test && npm run build`
  - `npm run e2e`（14+1 specs 全綠）
  - 手動：載樣本 → 開 holdout(30%) → 掃 fastMA 5-20 → 熱力圖出現且標示樣本內；關 holdout → 再掃 → 無標示、結果與改動前一致。
- **Acceptance criteria**:
  - [ ] holdout 開啟時 `runParamSweep` 收到 `from:0, to:split-1`（split 與 `run()` 一致）
  - [ ] UI 有樣本內範圍標示；holdout 關閉時無任何變化
  - [ ] 新 e2e 斷言通過；既有測試全綠
  - [ ] `git diff` 僅觸及 BacktestPanel.tsx、sweep.spec.ts、CHANGELOG.md、（可選）HELP 文案

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task BUG-001 only. Execute exactly that task, following its 6-step implementation plan literally.

Repo: AlphaFactorForge, work in alpha-factor-forge/. Branch off latest main as fix/sweep-respects-holdout.
Key facts: runParamSweep in src/services/paramSweep.ts already accepts from/to — do NOT modify that file. The holdout split in BacktestPanel.run() is the reference formula; extract it into a shared pure helper `holdoutSplitIndex(n, holdoutPct)` used by both run() and runSweep(). Preserve every existing data-testid. UI copy is zh-TW.
Validate: npm run typecheck && npm test && npm run build && npm run e2e (all green; add the new e2e flow to e2e/sweep.spec.ts).
Deliver: commit `fix(ui): sweep optimises in-sample only when holdout is on`, PR body in zh-TW with a before/after behaviour note and validation checklist, plus git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task BUG-001.
Verify: (1) paramSweep.ts untouched; (2) split formula identical between run() and runSweep() (shared helper); (3) holdout-off path byte-identical behaviour (no from/to passed); (4) new e2e asserts the in-sample label; (5) no data-testid removed (grep data-testid diff); (6) CI evidence for typecheck/test/build/e2e. Flag any scope creep (e.g. engine edits, walk-forward, refactors beyond the shared helper). Output: verdict + merge recommendation.
```

---

## REF-001 — 從 BacktestPanel 抽出 SweepSection

- **Category**: Refactor
- **Objective**: 把「參數掃描」整個區塊（狀態、handlers、`AxisEditor`、`SweepHeatmap`、相關常數）搬到 `src/components/SweepSection.tsx`，行為與 DOM 零變更，BacktestPanel 減少 ~300 行。
- **Context**: BacktestPanel 1217 行是全 repo 最大 AI 誤改熱點（audit P2）。掃描區塊耦合最低（只需要 candles、strat、interval、setStrat、setMsg），最適合當第一刀。**選在 BUG-001 之後執行以免同檔衝突。**
- **Files likely affected**: 新增 `src/components/SweepSection.tsx`；修改 `src/components/BacktestPanel.tsx`；（不動 e2e——testid 全保留即應全綠）。

### Exact implementation plan

1. 建 `src/components/SweepSection.tsx`。搬過去的內容（**剪下貼上，不重寫**）：
   - 常數/helper：`SWEEP_PARAM_LABEL`、`SWEEP_METRIC_LABEL`、`fmtSweepMetric`、`sweepBestLabel`、`heatColor`
   - 子元件：`AxisEditor`、`SweepHeatmap`
   - 狀態：`sweepOpen/sweepX/sweepY/sweepUse2d/sweepMetric/sweeping/sweepResult/sweepErr/appliedCell`
   - handlers：`clearSweep`、`runSweep`、`applySweepCombo`、`applySweepBest`
   - JSX：`{candles.length > 0 && <section …參數掃描…>}` 整段
2. Props 介面（保持最小）：`candles: CoreCandle[]`、`strat: ParamsStrategy`、`interval: string`、`holdout: boolean`、`holdoutPct: number`、`onApplyCombo(patch: Partial<ParamsStrategy>, appliedKeys: NumKey[], label: string): void`。`appliedKeys` 的 state 與樣式仍留在 BacktestPanel（策略表單需要它），SweepSection 透過 `onApplyCombo` 回報。`NumKey` 型別若兩邊共用，搬到 `src/services/strategy.ts` 旁或新 `src/components/types.ts`——擇一，優先放 `strategy.ts`（它已定義相近型別）。
3. 樣式常數 `S` 兩檔都要用：把 `S` 抽到 `src/components/panelStyles.ts` 並讓兩檔 import（純搬移，不改值）。`HelpTip` 的 `HELP.sweep/runSweep/applyBest` 文案隨區塊搬入 SweepSection 或集中留在原地經 props 傳入——**選擇：文案常數整個 `HELP` map 留在 BacktestPanel，把用到的三條字串經 props 傳入**（避免拆散文案審閱點）。
4. BacktestPanel 對應區塊改為 `<SweepSection …/>`；刪除已搬走的 state/handler/import。
5. 逐一核對搬移後 JSX 的 `data-testid` 清單與搬移前相同：`sweep-toggle`、`sweep-metric`、`sweep-2d`、`sweep-combos`、`run-sweep`、`apply-best`、`sweep-cell-*`、`sweep-best-marker`、`sweep-applied-marker`。
6. 跑完整驗證（含 e2e sweep spec 不改而綠）。

### Non-goals

- 不改任何行為、文案、樣式值、DOM 結構。
- 不引入 context/reducer/狀態管理套件。
- 不順手改 BacktestPanel 其他區塊（那是 REF-002/003）。
- 不改 e2e 檔案（它們是本次的驗收工具）。

- **Risk level**: Medium（機械式，但量大；靠 e2e 與 testid 清單壓風險）
- **Validation plan**: `npm run typecheck && npm test && npm run build && npm run e2e`；肉眼比對 `git diff` 確認 SweepSection 內容與原檔逐行對應（允許 import/props 差異）；手動走一次掃描→點格→套用最佳→表單藍框。
- **Acceptance criteria**:
  - [ ] BacktestPanel.tsx 減少 ≥ 250 行；SweepSection.tsx 為新檔
  - [ ] e2e `sweep.spec.ts` 未修改且全綠
  - [ ] 上列 9 組 data-testid 全部存在
  - [ ] 無新依賴、無行為差異（掃描結果數值不變）

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task REF-001 only. This is a MOVE-ONLY refactor.

Repo: AlphaFactorForge, alpha-factor-forge/. Branch off latest main as refactor/extract-sweep-section.
Hard rules: cut-and-paste the sweep block from src/components/BacktestPanel.tsx into new src/components/SweepSection.tsx per the task's step list; do not rewrite logic, rename state, or change any style value / zh-TW copy / DOM structure; keep every data-testid (the task lists all 9); e2e files must NOT be edited — they are the acceptance gate. Extract the shared style object S into src/components/panelStyles.ts (verbatim). The HELP copy map stays in BacktestPanel; pass the three sweep strings via props.
Validate: npm run typecheck && npm test && npm run build && npm run e2e — all green with zero e2e edits.
Deliver: commit `refactor(ui): extract SweepSection from BacktestPanel`, PR body in zh-TW stating "move-only, no behaviour change" + line-count before/after + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task REF-001 (move-only refactor).
Verify: (1) diff is a relocation — compare moved blocks against the old file for logic edits; (2) zero e2e file changes and e2e CI green; (3) all 9 sweep data-testids present; (4) no new dependencies; (5) BacktestPanel shrank ≥250 lines; (6) style values and zh-TW copy byte-identical; (7) no other section touched. Any behaviour tweak (however sensible) = request changes. Output: verdict + merge recommendation.
```

---

## REF-002 — 抽出 ChartSection（圖表 + 回放 + 此根資訊 + 快捷參數列）

- **Category**: Refactor
- **Objective**: 把「圖表」card 整塊（overlay toggles、pop-out 按鈕、replay 控制列、bar-info 列、QUICK_FIELDS 列）搬到 `src/components/ChartSection.tsx`，再減 BacktestPanel ~250 行。
- **Context**: REF-001 的第二刀。這塊 state 較多（replay 系列、hoverBar、show、poppedChart），但邊界清楚：輸入 candles/strat/result/signalSeries，輸出 setStrat（快捷欄）、hoverBar 給 bar-info。
  > **2026-07-12 註**：Slice 10（pan/zoom，PR #28/#29）與 Slice 8b（原生視窗，PR #30 → `ChartPopoutWindow.tsx` + `popoutWindows.publishChartCursor`）**已落地**。因此 REF-002 的搬移範圍必須**一併納入 pan/zoom 的可視窗格狀態與原生視窗游標同步的接線**，且維持 move-only（不改 pan/zoom 行為）。執行前先重新盤點 `BacktestPanel` 目前與圖表相關的所有 state/effect（含 `publishChartCursor`），再定 props 介面。`CandleChart.tsx`/`scale.ts` 本身仍不改。
- **Files likely affected**: 新增 `src/components/ChartSection.tsx`；修改 `BacktestPanel.tsx`。

### Exact implementation plan

1. 搬移範圍（剪貼）：`show/replayOn/replayCursor/replayPlaying/replaySpeed/hoverBar/poppedChart` 七個 state、replay 三個 effect、`toggleReplayPlay`、`activeBar/activeCandle/liveEntry/liveExit/livePosition/posText/posColor` 推導、`renderChart`、`OVERLAY_LABEL`、`POS_LABEL`、圖表 `<section>` 全部 JSX、`FloatingPanel`（chart 那個）與 `PoppedOutNote`（chart 用法）。
2. `signalSeries` 的 `useMemo` 一併搬入（它只餵 bar-info）；`positionAtTime` 需要 `result.trades`——經 props 傳 `trades: ClosedTrade[] | undefined`。
3. Props：`candles`、`strat`、`trades`、`quickFields`（沿用 QUICK_FIELDS 常數，可一起搬）、`isAppliedKey/appliedInputStyle/appliedLabelStyle`（以 props 函數傳入，或把 `appliedKeys` 陣列傳入並在 ChartSection 內重建三個 helper——**選擇後者**，傳 `appliedKeys: NumKey[]` + `onChangeParam(key, value)`）、`helpReplayText`。
4. `PoppedOutNote` 與 metrics 的 pop-out 仍被 BacktestPanel 使用 → `PoppedOutNote` 抽到 `src/components/PoppedOutNote.tsx` 供兩處 import（verbatim 搬移）。
5. data-testid 清單核對：`replay-toggle/replay-reset/replay-back/replay-play/replay-cursor/replay-fwd/replay-speed/replay-readout/bar-info/bar-position/popout-chart/chart-popout(-close)/quick-applied-*` + `help-replay`。
6. 全套驗證；`replay.spec.ts`、`hover.spec.ts`、`popout.spec.ts` 不改而綠。

### Non-goals

- 不改 CandleChart.tsx / scale.ts。
- 不實作 pan/zoom、不改 replay 計時邏輯。
- 不動 sweep/dataset/results 區塊。

- **Risk level**: Medium
- **Validation plan**: 同 REF-001（typecheck/test/build/e2e 全綠、e2e 零修改）；手動：回放播放/暫停/速度、hover 讀數、pop-out 內即時反映參數修改。
- **Acceptance criteria**:
  - [ ] ChartSection.tsx 新檔；BacktestPanel 再減 ≥ 200 行
  - [ ] replay/hover/popout 三個 spec 未修改且全綠
  - [ ] 列出的 data-testid 全數存在
  - [ ] pop-out 圖表仍隨左欄編輯即時重繪（同一 React state 樹）

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task REF-002 only. MOVE-ONLY refactor, second cut after REF-001.
Branch: refactor/extract-chart-section (off latest main, which already contains REF-001).
Follow the task's 6 steps: move the chart card (overlays, replay controls+effects, bar-info derivation, quick param row, chart FloatingPanel) into src/components/ChartSection.tsx; extract PoppedOutNote into its own file used by both; pass appliedKeys + onChangeParam via props. Do not touch CandleChart.tsx, scale.ts, sweep/dataset/results blocks, or any e2e file. Keep every data-testid listed in the task.
Validate: npm run typecheck && npm test && npm run build && npm run e2e — all green, zero e2e edits.
Deliver: commit `refactor(ui): extract ChartSection from BacktestPanel`, zh-TW PR body, line counts + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task REF-002 (move-only).
Verify: (1) replay/hover/popout specs untouched + green; (2) moved effects identical (interval timing 400/speed, StrictMode-safe stop effect preserved); (3) signalSeries memo dependencies unchanged; (4) data-testid list intact; (5) CandleChart/scale untouched; (6) no behaviour/style/copy drift. Output: verdict + merge recommendation.
```

---

## REF-003 — 抽出 DatasetSection 與 ResultsSection，BacktestPanel 成為編排層

> **狀態：✅ 已完成。** DatasetSection + ResultsSection = PR #40（811→648 行）；`< 400` 驗收由 **REF-003b（PR #41）抽出 StrategySection** 達成（648→**385 行**）。REF-003 原文把策略表單留在 panel，ultrareview 指出這樣達不到 `< 400`，故追加 003b 把策略卡（含內嵌策略庫）也抽出。整個 REF 系列（001/002/003/003b）到此關閉。以下規格保留為記錄。

- **Category**: Refactor
- **Objective**: 第三刀：資料集 card → `DatasetSection.tssx`；回測績效 card（metrics 表、匯出、儲存、metrics pop-out）→ `ResultsSection.tsx`。完成後 BacktestPanel 剩策略表單 + 執行模型 + holdout + run + 各 section 編排，目標 < 400 行。
- **Context**: 收尾 audit R2。策略表單暫留 panel（它與 strat state 是同一件事，下一步如需再拆另立任務）。
- **Files likely affected**: 新增 `src/components/DatasetSection.tsx`、`src/components/ResultsSection.tsx`；修改 `BacktestPanel.tsx`。

### Exact implementation plan

1. DatasetSection 搬移：`datasets/selId/busyData/importText` 相關 JSX 與 `loadSample/importJson/refresh` handlers、`normalizeCandle/pickNum` helpers。Props：`onDatasetsChanged`、`onError/onMsg`（或回傳事件）——**選擇**：把 `datasets/selId` state 留在 panel（run/save 需要），DatasetSection 收 `datasets/selId/busy` + `onSelect/onLoadSample/onImportJson` props；handlers 留 panel。這使本步驟只搬 JSX + 兩個 pure helpers（風險最低）。
2. ResultsSection 搬移：回測績效 `<section>` JSX、`renderMetricsTable`、`METRIC_ROWS`、`pct/num` helpers、`exporting/exportNotice/poppedMetrics` state、`exportResult` handler、metrics 的 FloatingPanel。Props：`result/holdoutResult/holdout/selected/strat/stratName/onStratName/onSave/saving/helpTexts`。
3. `metricCols` 推導隨 ResultsSection 搬入。
4. data-testid 核對：`export-json/export-csv/export-status/popout-metrics/metrics-popout(-close)/col-全期(樣本內/樣本外)/run-backtest`（run 鈕留 panel）。`load-sample`、`holdout-toggle` 留在各自搬移後位置。
5. 全套驗證；`export.spec.ts`、`holdout.spec.ts`、`popout.spec.ts` 不改而綠。
6. PR 描述附三步重構總結表（REF-001~003 各檔行數 before/after）。

### Non-goals

- 不拆策略表單（params/blocks/code 編輯器留 panel）。
- 不改 save/export 邏輯本身。
- 不引入 store/context。

- **Risk level**: Medium
- **Validation plan**: 同前兩步；手動全流程走一遍（載樣本→回測→holdout 三欄→匯出兩鍵→儲存→pop-out metrics）。
- **Acceptance criteria**:
  - [x] BacktestPanel.tsx < 400 行（385 行，由 REF-003b 達成）
  - [x] export/holdout/popout specs 未修改且全綠
  - [x] 全部既有 testid 存在；儲存與匯出行為不變
  - [x] REF 系列總結表附在 PR（#41）

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task REF-003 only. MOVE-ONLY refactor, final cut of the BacktestPanel decomposition.
Branch: refactor/extract-dataset-results-sections (off latest main containing REF-001/002).
Follow the plan exactly: DatasetSection gets JSX + normalizeCandle/pickNum only (state & handlers stay in the panel, passed via props); ResultsSection gets the metrics card incl. export/save UI, exporting/exportNotice/poppedMetrics state, exportResult handler, METRIC_ROWS + formatters. Strategy form / exec model / holdout / run button stay in BacktestPanel. No e2e edits. Keep all data-testids listed in the task.
Validate: npm run typecheck && npm test && npm run build && npm run e2e — all green, zero e2e edits. Manually run the full flow in ?mock=1 (sample → run → holdout 3 columns → export JSON+CSV → save → popout metrics) and report results.
Deliver: commit `refactor(ui): extract Dataset/Results sections; BacktestPanel becomes orchestrator`, zh-TW PR body with the REF-001..003 before/after line-count table + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task REF-003 (move-only, final decomposition step).
Verify: (1) BacktestPanel < 400 lines; (2) export/holdout/popout specs untouched + green; (3) save path still goes through buildStrategyDef + metricsToBacktestSummary (no inline mapping snuck in); (4) exportResult logic byte-equivalent; (5) props are plain data/functions, no context/store introduced; (6) strategy form untouched. Output: verdict + merge recommendation.
```

---

## FEAT-001 — 策略庫（tasks.md Slice 7-3）

> **狀態：✅ 已完成（PR #27，Slice 7-3）。策略庫已 inline 落地於 `BacktestPanel`。以下規格保留為歷史記錄，不需再執行。** 若日後要把它抽成獨立 `LibrarySection`，併入 REF-003 處理。

- **Category**: Product
- **Objective**: 列出已儲存策略、載回表單、可刪除——取代 legacy 的 localStorage `cd_stratlib`，關閉「存了拿不回」的回訪斷點。
- **Context**: `get_strategies` command 與 Rust `list_strategies` 已存在；缺 `delete_strategy` command、前端 UI 與載回反序列化。tasks.md 已把 7-3 列在 Next。**執行前確認 REF-001 已合併**（新增區塊放進獨立 section 檔，不回填巨石）。
- **Files likely affected**: 新增 `src/components/LibrarySection.tsx`；`src/tauri-client/commands.ts`（+deleteStrategy）；`src/tauri-client/mockClient.ts`（+對應 mock）；`src-tauri/src/commands/db_commands.rs`、`src-tauri/src/db/repositories.rs`（+delete_strategy + 單元測試）；`src-tauri/src/main.rs`（註冊 command）；`BacktestPanel.tsx`（掛載 section + onLoadStrategy）；新增 `e2e/library.spec.ts`。

### Exact implementation plan

1. Rust：`repositories::delete_strategy(conn, id) -> AppResult<()>`（`DELETE FROM strategy_def WHERE id=?1`；backtest_summary 有 ON DELETE CASCADE，注意在 PR 描述標明會連帶刪 summary）。加 `db_commands::delete_strategy` 並在 `main.rs` 註冊。加一個 repositories 單元測試（插入→刪除→list 為空；驗證 cascade）。
2. TS bridge：`commands.ts` `db.deleteStrategy(id)`；`mockClient.ts` 同步實作（含從陣列移除）。
3. 反序列化 helper：`src/services/strategyRecord.ts` 加 `parseStrategyDef(def: StrategyDef): ParamsStrategy | null`——`JSON.parse(original_definition_json)` 後以 `defaultStrategy()` 為底做 shallow merge + 欄位型別檢查（number/string/enum 白名單），任何異常回傳 null。附單元測試（正常/缺欄/壞 JSON/未知 mode）。
4. UI：`LibrarySection.tsx`（放在掃描區塊下方）：初始化與每次儲存成功後 `db.getStrategies()`；表格列 name/type/lifecycle/created；每列「載入」「刪除」鈕。載入 → `parseStrategyDef` 成功則 `setStrat` + `setStratName(def.name)` + 訊息；失敗顯示錯誤。刪除 → confirm 樣式沿用現有 msg/err 列（不用 window.confirm，改兩段式按鈕：點「刪除」變「確認刪除？」再點才刪，5 秒後復原——零新依賴）。
5. `BacktestPanel` 傳 `onLoaded(strat, name)` 與 `savedTick`（每次 save 成功 +1 觸發 library refresh）。
6. e2e `library.spec.ts`（mock 模式）：載樣本→回測→儲存→library 出現一列→改參數→點載入→表單值恢復→刪除→列表清空。
7. `tasks.md`：把 Slice 7-3 從 Next 移到 Done 並附驗證證據（由執行 agent 完成）。

### Non-goals

- 不做搜尋/標籤/排序/改名/複製（改名依賴 REF-004 的 upsert 語義，另行處理）。
- 不做策略版本歷史。
- 不動 save 流程本身。

- **Risk level**: Medium（跨 TS/Rust/e2e 三層，但每層都小）
- **Validation plan**:
  - `npm run typecheck && npm test && npm run build && npm run e2e`
  - `cd src-tauri && cargo check --locked && cargo test --locked`
  - 手動（Tauri）：`cargo tauri dev` 走 save→list→load→delete，確認 SQLite 實際變化（用 CLI 查，勿信 GUI viewer 的 WAL 快照）。
- **Acceptance criteria**:
  - [ ] 儲存後策略出現在庫列表；載入可還原 params/blocks/code 三模式的表單
  - [ ] 刪除有兩段式確認且連帶刪除該策略的 summary（PR 描述明示此語義）
  - [ ] 壞 JSON 載入顯示錯誤而非崩潰（單元測試覆蓋）
  - [ ] Rust 測試 + e2e 新 spec 全綠
  - [ ] mockClient 與真實 command 行為同步更新

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task FEAT-001 (= tasks.md Slice 7-3) only. Execute its 7-step plan exactly.
Branch: feat/ui-port-slice7-3-strategy-library (off latest main; REF-001 must already be merged).
Cross-layer task: Rust delete_strategy (+unit test, note the ON DELETE CASCADE consequence in the PR body), typed TS wrapper + mockClient parity, parseStrategyDef with defaultStrategy() merge + unit tests, LibrarySection.tsx UI (zh-TW copy, two-step inline delete confirm — no window.confirm, no new deps), e2e library.spec.ts in mock mode, and move Slice 7-3 to Done in tasks.md with evidence.
Do NOT add search/tags/rename; do NOT touch the save flow or metricsToBacktestSummary.
Validate: npm run typecheck && npm test && npm run build && npm run e2e; cd src-tauri && cargo check --locked && cargo test --locked. All green.
Deliver: commit `feat(ui-port): strategy library (Slice 7-3)`, zh-TW PR body with validation checklist + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task FEAT-001.
Verify: (1) delete cascade consequence documented and tested; (2) parseStrategyDef rejects malformed/unknown-mode JSON (tests prove it) — loading must never crash the panel; (3) mockClient mirrors the new command; (4) main.rs registers delete_strategy; (5) e2e covers save→list→load→delete; (6) no rename/search scope creep; (7) tasks.md updated. Output: verdict + merge recommendation.
```

---

## TEST-002 — 回測引擎 golden tests + legacy 對照報告

- **Category**: Testing / Reliability
- **Objective**: 把 `core/backtest` 現行行為鎖進快照級測試，並產出「與 legacy 差異」的對照報告，作為後續任何引擎修改的裁決輸入（audit P10、Open Question Q2）。
- **Context**: 引擎有幾個未裁決點：nextOpen 模式 exitTime 記當根時間但價格取次根開盤；SL/TP 觸發價不套 slippage；short 保證金記帳模型；`close()` 內 void 掉的死變數。在裁決前，先讓「現狀」不可默默改變。
- **Files likely affected**: 新增 `src/core/backtest/backtest.golden.test.ts`；新增 `docs/engine-parity-report.md`；**不改任何產品程式**。

### Exact implementation plan

1. Golden tests：用 `makeSampleCandles({ seed: 42, count: 300 })` 產生固定資料，跑 4 個組態：(a) long/close-fill/無SLTP；(b) long/nextOpen；(c) both + SL2%/TP4%；(d) short。對每個組態斷言：trades 數、首末 trade 的 entry/exit time+price（精確到 1e-9）、netReturn/maxDrawdown/sharpe（toBeCloseTo 6 位）。快照值由第一次執行輸出後寫死進測試（測試檔注釋標明「行為鎖，非正確性背書」）。
2. 邊界案例測試：entry 與 exit 同根、資料只有 1 根、`from===to`、sizePct 0（應回退 100%）、fee/slip 負值（應 clamp 0）——只斷言不拋例外 + 關鍵不變量（cash 不為 NaN、trades 序列時間遞增）。
3. 對照報告 `docs/engine-parity-report.md`：逐項列出（至少）：nextOpen exitTime/價格不一致、SL/TP 無滑價、SL 與 TP 同根同觸發時 SL 優先、short 記帳模型、eod 強制平倉用收盤價；每項寫「現行為 / legacy 行為（引用 `AlphaFactorForge.dc.html` 對應段落或標注找不到）/ 建議：保留或修正 / 影響面」。**報告只建議，不修改。**
4. 報告尾附「若裁決為修正」的後續任務草稿格式（供未來開 BUG-00x）。

### Non-goals

- 不修改 `core/backtest`（一行都不改，包括死變數）。
- 不裁決；裁決權在 maintainer（Q2）。

- **Risk level**: Low（純新增測試與文件）
- **Validation plan**: `npm test`（新測試綠、既有 125 綠）；`npm run typecheck`。
- **Acceptance criteria**:
  - [ ] golden tests 覆蓋 4 組態 + 5 邊界案例，全綠
  - [ ] `core/backtest/index.ts` 零改動
  - [ ] `docs/engine-parity-report.md` 逐項含現況/legacy 對照/建議
  - [ ] tests 注釋明示「行為鎖」性質

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task TEST-002 only.
Branch: test/backtest-golden-lock (off latest main).
Rules: you may NOT modify any file under src/core/ — this task adds src/core/backtest/backtest.golden.test.ts and docs/engine-parity-report.md only. Generate fixtures with makeSampleCandles({seed:42,count:300}); run the 4 configs + 5 edge cases from the plan; hard-code the observed values as the golden expectations with a comment that these lock CURRENT behaviour (not correctness). For the parity report, read the legacy runBacktestCore in AlphaFactorForge.dc.html (root of repo) to fill the legacy column; where you cannot find the legacy behaviour, write "not located" — do not guess.
Validate: npm run typecheck && npm test (all green).
Deliver: commit `test(core): golden-lock backtest behaviour + legacy parity report`, zh-TW PR body + git diff summary. The report must make NO code changes and NO verdicts — recommendations only.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task TEST-002.
Verify: (1) zero diffs under src/core except the new test file; (2) golden values are asserted (not recomputed from the engine at runtime in a way that would always pass); (3) edge cases assert invariants, not just "no throw"; (4) parity report cites legacy code locations or honestly says "not located"; (5) no verdicts/behaviour changes. Output: verdict + merge recommendation.
```

---

## FEAT-002 — 交易明細（trades）持久化

- **Category**: Reliability / Product
- **Objective**: 儲存結果時把 `result.trades` 寫入 `trades` 表（隨 summary 同交易、replace 語義），補齊三層持久化的第三層。
- **Context**: schema 已建、前端有完整 trades 資料（匯出 CSV 都在用），只缺落庫（audit P6）。
- **Files likely affected**: `src-tauri/src/db/repositories.rs`（insert_trades + 測試）、`src-tauri/src/commands/db_commands.rs`（save_backtest_result 簽名擴充）、`src/tauri-client/commands.ts`、`src/tauri-client/mockClient.ts`、`src/services/metricsMapper.ts` 旁新增 trade 映射 helper（或放 `strategyRecord.ts`——放新檔 `src/services/tradesMapper.ts`）、`BacktestPanel.tsx`（save 呼叫帶 trades）。

### Exact implementation plan

1. Rust：`save_backtest_result(state, summary, trades: Vec<TradeRow>)`——新 DTO `TradeRow { entry_time, exit_time, side, entry_price, exit_price, pnl, pnl_pct, reason: Option<String> }`（`bars` 對應欄位 schema 沒有——**不加欄位**，bars 不存；fee/slippage 欄留 NULL 並在注釋標明 Phase A 未逐筆記錄）。在同一個 transaction 內：upsert summary → `DELETE FROM trades WHERE backtest_summary_id=?` → 批次 INSERT。回傳 summary id。
2. Rust 單元測試：save 兩次同 key → trades 不重複（replace 語義）；cascade 刪 strategy → trades 清空。
3. TS：`commands.ts` 的 `saveBacktestResult(summary, trades)`；新 `src/services/tradesMapper.ts` 把 `ClosedTrade`（camelCase）映射為 snake_case row（單一映射點原則，加 3 個單元測試）。
4. `BacktestPanel.save()` 傳入 `tradesToRows(result.trades)`。
5. mockClient 同步簽名（存進記憶體 map 即可）。
6. 注意 invoke 參數命名：Tauri v2 預設 camelCase（現有 `strategyId` 先例），Rust 端參數 `trades` 單字無歧義。

### Non-goals

- 不做 trades 讀取 UI（Results Explorer 是 Phase B）。
- 不改 schema（bars 欄不補；那需要 0002 migration，另案）。
- 不動 summary upsert 語義。

- **Risk level**: Medium（跨層 + transaction 語義）
- **Validation plan**: `cargo check --locked && cargo test --locked`；`npm run typecheck && npm test && npm run build && npm run e2e`；手動 Tauri：save 後用 sqlite CLI `SELECT COUNT(*) FROM trades …` 驗證（勿用 GUI viewer 讀 WAL 快照）。
- **Acceptance criteria**:
  - [ ] save 一次 → trades 行數 = result.trades.length；重存同 key 不累積
  - [ ] transaction：summary 失敗時 trades 不落庫（測試證明）
  - [ ] 映射走單一 helper 且有測試
  - [ ] e2e 既有 save flow 不變綠

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task FEAT-002 only. Execute the 6-step plan exactly.
Branch: feat/persist-trades (off latest main).
Key semantics: summary upsert + DELETE-then-INSERT trades inside ONE rusqlite transaction; bars is NOT stored (no schema change allowed — migrations are append-only and 0002 is out of scope); fee/slippage columns stay NULL with a comment. Trade field mapping lives in new src/services/tradesMapper.ts only (single-mapping-point rule, like metricsToBacktestSummary). Update mockClient signature in lockstep.
Validate: cd src-tauri && cargo check --locked && cargo test --locked; cd .. && npm run typecheck && npm test && npm run build && npm run e2e. All green.
Deliver: commit `feat(persistence): save trade detail rows with backtest summary`, zh-TW PR body + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task FEAT-002.
Verify: (1) single transaction covers upsert+delete+insert (check for partial-write windows); (2) replace semantics tested; (3) no migration files touched; (4) mapping only in tradesMapper.ts, with tests; (5) mockClient signature matches; (6) BacktestPanel change is minimal (one call-site arg). Output: verdict + merge recommendation.
```

---

## REF-004 — `insert_strategy` UPSERT 語義修正

- **Category**: Refactor / Reliability
- **Objective**: 同 hash 再儲存時更新可變欄位（至少 `name`、`updated_at`），讓「改名再存」不再默默失效。
- **Context**: tasks.md Backlog 既有項；現有 Rust 測試文件化了舊行為，需一併更新（audit P8）。策略庫（FEAT-001）上線後使用者才會實際感受到此 bug，故排在其後。
- **Files likely affected**: `src-tauri/src/db/repositories.rs`（UPSERT 語句 + 測試）。

### Exact implementation plan

1. `insert_strategy` 的 `ON CONFLICT(strategy_hash) DO UPDATE SET` 擴充為 `name=excluded.name, source=excluded.source, updated_at=datetime('now')`。**lifecycle 不自動覆寫**（它屬審核流程，維持 DB 現值）——在注釋寫明理由。
2. 更新既有測試 `insert_strategy_upserts_on_hash_without_duplicating`：斷言改名後 name 更新、lifecycle 保留、無新列。
3. 補一個測試：同 hash 不同 name 存兩次 → 一列、name 為新值。

### Non-goals

- 不改 hash 定義、不動 TS 端。

- **Risk level**: Low
- **Validation plan**: `cargo check --locked && cargo test --locked`；前端全套跑一次確認無影響。
- **Acceptance criteria**:
  - [ ] 改名重存生效；lifecycle 不被覆寫（測試證明）
  - [ ] 只有 repositories.rs 變更

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task REF-004 only.
Branch: fix/strategy-upsert-mutable-fields (off latest main).
Change exactly one UPSERT statement in src-tauri/src/db/repositories.rs per the plan (update name/source/updated_at; deliberately NOT lifecycle — write the comment explaining why), update the existing upsert test's expectations, add the rename-persists test.
Validate: cd src-tauri && cargo check --locked && cargo test --locked; then the frontend suite (npm run typecheck && npm test) to prove no cross-layer impact.
Deliver: commit `fix(db): strategy upsert refreshes mutable fields`, zh-TW PR body + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task REF-004.
Verify: (1) only repositories.rs changed; (2) lifecycle intentionally excluded with comment; (3) tests updated to assert new semantics (not just still passing); (4) no TS changes needed (signature unchanged). Output: verdict + merge recommendation.
```

---

## PERF-001 — 參數掃描移入 Web Worker（含取消）

- **Category**: Performance
- **Objective**: 掃描（最多 256 次回測）移出 UI thread，掃描中 UI 可互動、可取消；為未來大資料集鋪路。
- **Context**: `backtest.worker.ts` 已有 jobId+event 協定骨架但零使用（audit P3）。**先做 sweep 就好**（單次回測在千根級資料下仍瞬時）。**執行前提：REF-001 已合併**（改動集中在 SweepSection）。
- **Files likely affected**: `src/workers/backtest.worker.ts`（+`runSweep` 訊息型別）、新增 `src/services/sweepWorkerClient.ts`（Promise 包裝 + cancel）、`src/components/SweepSection.tsx`。

### Exact implementation plan

1. Worker 端：`InMsg` 增加 `{ type:'runSweep', jobId, payload: RunParamSweepArgs }`，回 `{ type:'sweepResult'|'error', jobId, payload }`。沿用「無 callback 跨界、只有 jobId 協定」的既有硬規則（檔頭注釋）。
2. Client：`sweepWorkerClient.ts` 以 `new Worker(new URL('../workers/backtest.worker.ts', import.meta.url), { type:'module' })` 建立（Vite 標準寫法，零新依賴）；`runSweepInWorker(args, jobId): { promise, cancel }`——cancel = terminate + 重建 worker（最簡正確語義；注釋標明）。
3. SweepSection：`runSweep` 改 await worker 結果；`sweeping` 期間顯示既有「掃描中…」+ 新增「取消」鈕（`data-testid="cancel-sweep"`）；取消後回到未掃描狀態。移除 `setTimeout(20)` hack。
4. 確保 `RunParamSweepArgs`/`SweepResult` 皆為 structured-clone 安全（現況是純資料，應可直接傳）。
5. 單元測試：paramSweep 不動；`sweepWorkerClient` 在 vitest 環境 mock Worker 介面測 resolve/cancel 兩路徑。e2e：sweep spec 應不改而綠（如時序造成 flake，允許把等待改為 `expect(...).toPass()` 式輪詢——這是本任務唯一允許的 e2e 修改）。

### Non-goals

- 單次回測與 holdout 不搬（維持同步）。
- 不做進度百分比（worker 端逐格回報屬進階，另案）。
- 不動 paramSweep 引擎。

- **Risk level**: Medium-High（併發、React 生命週期、e2e 時序）
- **Validation plan**: 全套 + 手動：掃 256 組合時拖動 replay slider 應流暢；點取消即停；連續快速點掃描兩次無殘留結果錯亂。
- **Acceptance criteria**:
  - [ ] 掃描期間 UI 可互動；取消鈕生效
  - [ ] 掃描結果與同步版本 bit-for-bit 相同（用固定 seed 樣本比對一次並寫入 PR）
  - [ ] worker 檔頭硬規則注釋保留
  - [ ] e2e 全綠（僅允許輪詢化修改）

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task PERF-001 only. REF-001 must already be merged.
Branch: perf/sweep-in-worker (off latest main).
Follow the 5-step plan: extend the existing jobId protocol in backtest.worker.ts (keep its hard-rules header comment), add sweepWorkerClient.ts (Vite `new URL` worker, cancel = terminate+recreate, documented), wire SweepSection with a cancel button (data-testid="cancel-sweep"), remove the setTimeout(20) paint hack. Determinism check: run one seeded sweep sync vs worker and paste the identical best-cell values into the PR body.
The ONLY permitted e2e edits are converting fixed waits to polling assertions if timing flakes; otherwise zero e2e changes.
Validate: npm run typecheck && npm test && npm run build && npm run e2e, plus the manual interactivity/cancel checks from the task.
Deliver: commit `perf(ui): run parameter sweep in the web worker with cancel`, zh-TW PR body + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task PERF-001.
Verify: (1) no callbacks cross the worker boundary (jobId protocol only); (2) cancel cannot leave a stale result landing after reset (check jobId guard on message handling); (3) double-click run is safe; (4) paramSweep.ts untouched; (5) determinism evidence in PR; (6) e2e edits limited to polling conversions. Output: verdict + merge recommendation.
```

---

## TEST-001 — 補 e2e：模式切換與非法 code 錯誤顯示

- **Category**: Testing
- **Objective**: 覆蓋 tasks.md Backlog 既列的缺口：params/blocks/code 分頁切換、非法 code 表達式紅框錯誤、合法 code 跑通、save 訊息。
- **Context**: 這些是 mock seam 設計時就點名的「remaining flows」；一次一個 spec 檔、可獨立交給最便宜的 agent。
- **Files likely affected**: 新增 `e2e/strategy-modes.spec.ts`（單檔涵蓋 4 個 test）。

### Exact implementation plan

1. Test 1：切三個模式分頁，斷言各模式專屬控件出現（params 的進場訊號 select、blocks 的「＋ 規則」、code 的 entry textarea）。
2. Test 2：code 模式輸入 `crossUp(` → 紅字錯誤出現；Run 後錯誤列出現（現行為：可按、報錯）。
3. Test 3：code 模式預設表達式 → Run → metrics 表出現。
4. Test 4：save 流程（mock）→ 「已存檔：strategy #…」訊息出現。
5. 需要的話為分頁鈕/錯誤span加 `data-testid`（允許的最小產品程式修改；逐一列在 PR）。

### Non-goals

- 不改任何行為；不加 Run 防呆（那是另一個 backlog 項）。

- **Risk level**: Low
- **Validation plan**: `npm run e2e`（18 specs 全綠）；全套其餘照跑。
- **Acceptance criteria**:
  - [ ] 4 個 test 全綠且不依賴固定 sleep
  - [ ] 產品程式修改僅限新增 data-testid

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task TEST-001 only.
Branch: test/e2e-strategy-modes (off latest main).
Add e2e/strategy-modes.spec.ts with the 4 tests in the plan, mock mode (page.goto('/?mock=1')), style-matched to the existing specs (getByTestId, no fixed sleeps). You may add data-testid attributes to the mode tab buttons / code error spans — list every product-code line you touch in the PR body; any other product change is out of scope.
Validate: npm run typecheck && npm test && npm run build && npm run e2e.
Deliver: commit `test(e2e): strategy mode switching + code-mode error flows`, zh-TW PR body + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task TEST-001.
Verify: (1) product diffs are data-testid additions only; (2) no fixed sleeps; (3) tests assert real user-visible outcomes (error text, metrics table) not implementation details; (4) suite green in CI. Output: verdict + merge recommendation.
```

---

## SEC-001 — npm audit 盤點報告（不升級、不 force）

- **Category**: Security
- **Objective**: 產出 5 個既知 vulnerabilities 的分類報告（套件、路徑、severity、是否 dev-only、修復選項與相容性風險），供 maintainer 決定升級窗口。
- **Context**: README 記錄了 5 個 vulnerabilities 並明文禁止 `npm audit fix --force`（audit P13、Q6）。
- **Files likely affected**: 新增 `docs/security-audit-npm.md`；**不改 package.json / lockfile**。

### Exact implementation plan

1. `cd alpha-factor-forge && npm audit --json > audit.json`（audit.json 不入庫）。
2. 整理成表：advisory、套件、依賴鏈（誰引入）、severity、是否僅 devDependencies、官方修復版本、升級會不會跨 major（對 Vite 5 / Tauri CLI 2 / Vitest 2 的相容性註記）。
3. 每項給建議：`safe-now`（patch 級可直接升）/ `needs-window`（跨 major，等升級窗口）/ `accept-risk`（dev-only 且無 runtime 面）。
4. 報告存 `docs/security-audit-npm.md`；**不執行任何升級**。

### Non-goals

- 不改依賴、不跑 `npm audit fix`（含非 force 版本）。

- **Risk level**: Low
- **Validation plan**: `git diff` 僅含新 md；`npm run typecheck` 過（證明沒動到程式）。
- **Acceptance criteria**:
  - [ ] 5 項全數分類，含依賴鏈與相容性註記
  - [ ] lockfile / package.json 零改動

### Suggested prompt for coding agent

```text
Read AGENTS.md, then docs/improvement-backlog.md task SEC-001 only.
Branch: docs/npm-audit-triage (off latest main).
Run `npm audit --json` in alpha-factor-forge/ (do NOT run any form of npm audit fix; do NOT touch package.json or package-lock.json), and write docs/security-audit-npm.md classifying each advisory per the plan (dependency chain, severity, dev-only?, fix version, major-bump?, recommendation safe-now/needs-window/accept-risk).
Validate: git diff shows only the new md; npm run typecheck still green.
Deliver: commit `docs(security): npm audit triage report`, zh-TW PR body + git diff summary.
```

### Suggested reviewer prompt

```text
Review PR <link> against docs/improvement-backlog.md task SEC-001.
Verify: (1) zero dependency-file changes; (2) every advisory has a dependency chain and an actionable recommendation; (3) no upgrade was performed. Output: verdict + merge recommendation.
```

---

## UX-002 — 頂層 Error Boundary（Optional）

- **Category**: UX / Reliability
- **Objective**: render 例外時顯示可複製錯誤訊息的 fallback 卡片而非白屏。
- **Files likely affected**: 新增 `src/components/AppErrorBoundary.tsx`；`src/main.tsx`（包一層）。
- **Exact implementation plan**: (1) class component `AppErrorBoundary`（`getDerivedStateFromError` + `componentDidCatch` console.error）；fallback 用現有卡片樣式顯示錯誤字串與「重新載入」鈕（`location.reload()`）；(2) `main.tsx` 以 `<AppErrorBoundary><App/></AppErrorBoundary>` 包裹；(3) 單元測試：故意丟例外的子元件 → fallback 出現。
- **Non-goals**: 不做錯誤上報、不做分區 boundary。
- **Risk level**: Low
- **Validation plan**: 全套 + 手動在 dev 臨時丟錯驗證（驗證後移除臨時碼）。
- **Acceptance criteria**:
  - [ ] 人工注入 render 錯誤顯示 fallback 而非白屏
  - [ ] 單元測試覆蓋
- **Suggested prompt for coding agent**:

```text
Read AGENTS.md, then docs/improvement-backlog.md task UX-002 only.
Branch: fix/app-error-boundary. Add AppErrorBoundary per the plan (class component, existing card styles, zh-TW copy, reload button), wrap <App/> in main.tsx, add a unit test with a throwing child. No other UI changes.
Validate: npm run typecheck && npm test && npm run build && npm run e2e.
Deliver: commit `fix(ui): top-level error boundary`, zh-TW PR body + git diff summary.
```

- **Suggested reviewer prompt**:

```text
Review PR <link> vs task UX-002: boundary wraps App once, fallback uses existing styles, test proves catch path, zero behaviour change elsewhere. Output verdict + merge recommendation.
```

---

## DOC-002 — 工作區衛生（Optional）

- **Category**: Documentation / Hygiene
- **Objective**: (1) `.gitignore` 補未追蹤雜物型樣（`*.zip`、`.thumbnail`、`.vite/`、`uploads/pasted-*` 視 maintainer 意願）；(2) `mockClient.ts` 檔頭加「與真後端已知偏差」清單（import 無 upsert 去重、無 CHECK 約束）；(3) 在 README 工作區內容段為 legacy 三件套加「唯讀參考，勿修改」標記。
- **Files likely affected**: `.gitignore`、`src/tauri-client/mockClient.ts`（僅注釋）、`README.md`（三處一句話）。
- **Exact implementation plan**: 逐項照 Objective；zip 是否 ignore 先在 PR 描述問 maintainer（預設加入）。
- **Non-goals**: 不刪任何檔案、不搬移 legacy（那是 Open Question Q4 的決策）。
- **Risk level**: Low
- **Validation plan**: `git status` 顯示雜物不再列為 untracked；全套 typecheck。
- **Acceptance criteria**:
  - [ ] `git status` 乾淨（雜物被 ignore）
  - [ ] mockClient 偏差清單就位；README 標記就位
- **Suggested prompt for coding agent**:

```text
Read AGENTS.md, then docs/improvement-backlog.md task DOC-002 only.
Branch: docs/workspace-hygiene. Apply the three items exactly (gitignore patterns, mockClient header comment listing known deviations from the Rust backend, README read-only markers for the legacy trio). Comment-only change in mockClient — no code. Do not delete or move any file.
Validate: git status clean of the listed clutter; npm run typecheck green.
Deliver: commit `docs: workspace hygiene markers`, zh-TW PR body + git diff summary.
```

- **Suggested reviewer prompt**:

```text
Review PR <link> vs task DOC-002: gitignore additions match the listed clutter only, mockClient diff is comments-only, README markers minimal. Output verdict + merge recommendation.
```

---

## TEST-003 — ESLint + Prettier 工具鏈（**blocked on Q3**）

- **Category**: Testing / Engineering Quality
- **Objective**: 引入 eslint（typescript-eslint、react-hooks）+ prettier + CI job，warn 起步。
- **Context**: 需新 devDependencies，**等 maintainer 在 Open Question Q3 核准後才可執行**。
- **Files likely affected**: `package.json`、新 `eslint.config.js`、`.prettierrc`、`ci.yml`、（首次格式化另開獨立 commit）。
- **Exact implementation plan**（核准後）：(1) 安裝並鎖版本；(2) config 以「不與現有風格打架」為原則（2 空格、單引號、行寬跟隨現檔約 100-120、prettier 只管排版）；(3) `npm run lint` script；(4) CI 加 lint job（僅 lint，不 auto-fix）；(5) 現有違規以 `--max-warnings` 寬限逐步收斂，**不做全庫 reformat**（避免污染 blame 與所有 open PR）。
- **Non-goals**: 全庫一次性 reformat；Rust 端 clippy gate（另案）。
- **Risk level**: Medium（依賴新增 + CI 行為變更）
- **Validation plan**: lint 本機與 CI 皆跑；全套既有驗證不受影響。
- **Acceptance criteria**:
  - [ ] `npm run lint` 可跑且 CI 有 job
  - [ ] 未做全庫 reformat；新規則對既有碼僅 warn
- **Suggested prompt / reviewer prompt**: 核准後由 Planner 依上述計畫展開（此任務涉及依賴選版，不建議直接交給最低成本 agent）。

---

## Avoid for now（明確不做清單）

| 項目 | 為什麼現在不做 | 何時重新評估 |
| --- | --- | --- |
| 交易所資料 fetch（reqwest/tokio） | 新依賴 + 網路錯誤面大；等 Q1 決策與依賴核准 | Q1 拍板後立為正式任務 |
| ~~Slice 10 chart pan/zoom~~ ✅ 已完成（PR #28/#29） | ~~tasks.md 已標 low priority~~ | ~~REF-002 合併後~~ → REF-002 須改為「保留 pan/zoom 狀態」的 move-only |
| Slice 8b 真 Tauri 第二視窗 | 8a 已覆蓋需求；8b 無法 browser-e2e、驗證成本高 | 使用者提出多螢幕需求時 |
| i18n 抽字串 | 單人 zh-TW 使用；抽象成本 > 收益 | 有第二語言需求時 |
| 引入狀態管理套件（zustand/jotai…） | REF-001~003 用 props 已足夠；新依賴需核准 | 策略庫+多分頁上線後如 props 鏈過深 |
| Service Worker / PWA 離線 | 目標形態是 Tauri desktop，PWA 線已凍結 | 僅當 PWA 線復活 |
| 全庫 reformat / 大規模 rename | 污染 blame、撞所有進行中分支 | TEST-003 落地且無 open PR 時 |
