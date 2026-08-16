# AlphaFactorForge — Improvement Backlog（可交接任務規格庫）

> 撰寫日期：2026-07-07。依據：[project-audit-masterplan.md](project-audit-masterplan.md)。
>
> **重要：本文件不是第二份任務板。** `tasks.md` 依 AGENTS.md 契約仍是唯一任務板。
> 使用方式：要執行某任務時，(1) 在 `tasks.md` 的 Next/In Progress 加一行並連結到本文件的 Task ID；(2) 把該任務的「Suggested prompt」貼給 coding agent；(3) 完成後在 `tasks.md` 移到 Done。本文件本身只在「任務規格變更」時修改。
>
> 所有任務共同前提：遵守 `AGENTS.md`（scope 控制、branch/PR 流程）與 [agent-execution-protocol.md](agent-execution-protocol.md)。**一個任務 = 一個 branch = 一個 PR。**

> ### 2026-07-12 狀態更新（審計後已變動，優先讀這段）
> 審計（2026-07-07）之後、本文件落地之前，以下已透過 PR 合併進 `main`，**規格保留為記錄但不需再執行**：
> - **FEAT-001 策略庫 = PR #27（Slice 7-3）已完成**（`savedStrategies` + `services/strategyLibrary` + `strategy-library-select` testid）。當時 inline 在 `BacktestPanel`；**後於 REF-003b（PR #41）隨策略表單一起移入 `components/StrategySection.tsx`**，未另立 `LibrarySection`。
> - **Slice 10-1/10-2 圖表 wheel-zoom / drag-pan = PR #28 / #29 已完成**（原列文末「Avoid for now」，現已落地；`CandleChart.tsx` 增至 ~491 行）。
> - **Slice 8b 原生圖表視窗 = PR #30 已完成**（新 `ChartPopoutWindow.tsx`）。
> - **fix：載入 legacy 策略 = PR #31 已完成**。
>
> 因此 `BacktestPanel.tsx` 一度增至 **1362 行 / 43 個 useState**（審計時 1217 / ~30）——**REF-001~003(+003b) 拆解比審計時更迫切**。（策略庫當時 inline；REF-003b 已把它連同策略表單抽入 `StrategySection`，見文末收尾更新。）
>
> **修正後的實際起手順序：DOC-001 → BUG-001 → REF-001 → REF-002 → REF-003 → TEST-002 → FEAT-002 → REF-004 → PERF-001 → …**（FEAT-001 已移除；下表原始編號保留供對照）。
>
> ### 2026-07-12 收尾更新（「Must do now」層完成）
> **DOC-001（#33）、BUG-001（#34）、REF-001（#37）、REF-002（#39）、REF-003（#40）+ REF-003b（#41）全部已合併。** `BacktestPanel.tsx` 由 **1382 → 385 行**，拆成 Sweep / Chart / Dataset / Results / Strategy 五個 section，成為純編排層——審計重構階段（含 ultrareview 追加的 `< 400` 收尾）正式關閉。**目前佇列前緣＝「Should do later」層：TEST-002 → FEAT-002 → REF-004 → PERF-001 → TEST-001 → SEC-001**（Optional：UX-002 / DOC-002；blocked：TEST-003 等 Open Question Q3）。Open Questions Q1–Q6（masterplan §8）仍待 maintainer 裁決。
>
> ### 2026-07-31 PR #76 合併後更新（目前優先讀這段）
> 舊總覽保留為 2026-07-07/12 歷史，不再代表目前執行順序。最新唯讀稽核已把 `BUG-RESULT-CONTEXT-001` 排為唯一 Next；證據與完整順序見 `../handoffs/2026-07-31-pr76-post-merge-audit-v1.md`，狀態只看 `tasks.md`。本檔尾端的 2026-07-31 addendum 已將該 Next task 展開成 execution-ready 規格；其他新 ID 在升為 Next 前仍須由 Planner 展開。

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

> **狀態：✅ 已完成（PR #27，Slice 7-3）。** 策略庫最初 inline 於 `BacktestPanel`，**現已隨 REF-003b（PR #41）移入 `components/StrategySection.tsx`**（未另立 `LibrarySection`）。以下規格保留為歷史記錄，不需再執行；文中 `LibrarySection` / delete_strategy 等為當時未採用的原始構想。

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

---

## 2026-07-31 Post-PR #76 Audit Addendum

This addendum is additive. It does not rewrite the 2026-07-07 audit or reuse
completed task IDs. `tasks.md` remains the only status board; the evidence and
cross-session decisions are preserved in
`../handoffs/2026-07-31-pr76-post-merge-audit-v1.md`.

The audit IDs expanded below as execution-ready coding-agent specifications are
`BUG-RESULT-CONTEXT-001` (2026-07-31), `METRIC-002` (2026-08-01),
`DATA-QUALITY-001` (2026-08-09), `BUG-SWEEP-CONTEXT-001` (2026-08-15), and
`STRATEGY-VALIDATION-001` (2026-08-16), in the adjudicated execution order. The
remaining audit IDs are recorded in `tasks.md` and the handoff, but a Planner
must expand the chosen task to this same format before promoting it to `Next`.
Existing `PERF-001` keeps its original specification above and must not be
duplicated.

## BUG-RESULT-CONTEXT-001 — 回測結果綁定不可變執行快照

- **Category**: Correctness / Persistence provenance
- **Objective**: A completed backtest may be rendered, saved, or exported only
  with the exact strategy, dataset, interval, candles, and run range that
  produced it. Changing any result-affecting input or beginning a dataset load
  must synchronously invalidate incompatible result actions; late asynchronous
  work must never repopulate a stale context.
- **Why this is first**: The current UI can silently persist old metrics/trades
  under a newly edited strategy or newly selected dataset. A wrong durable row
  is more damaging than a visible failure and already affects the shipped
  interactive surface.
- **Files likely affected**:
  - `alpha-factor-forge/src/components/BacktestPanel.tsx`
  - `alpha-factor-forge/src/components/ResultsSection.tsx`
  - new `alpha-factor-forge/src/services/runArtifact.ts`
  - new `alpha-factor-forge/src/services/runArtifact.test.ts`
  - `alpha-factor-forge/e2e/save-message.spec.ts`
  - `alpha-factor-forge/e2e/export.spec.ts`
  - at most one new focused E2E spec for result-context invalidation if keeping
    the existing two specs focused is clearer

### Exact implementation plan

1. Add a pure, DOM/React/IO-free `runArtifact.ts` model. Define an immutable
   completed-run artifact containing the result, a deep strategy snapshot and
   durable strategy identity, dataset id/hash plus the interval, and the exact
   run/holdout range. Provide one canonical context-key/equality helper; do not
   duplicate comparison logic in React event handlers.
2. Replace `BacktestPanel`'s bare result state with that artifact. Route every
   result-affecting strategy change (including mode, signals, execution/risk
   fields, numeric shortcuts, library load, and Sweep Apply Best) through one
   invalidation path before accepting the new value.
3. On dataset selection, synchronously clear the selected dataset's candle
   readiness, completed artifact, and holdout result before starting
   `getCandles`. Store candle readiness with its dataset id/hash rather than
   accepting any non-empty candle array. A rejected load must leave actions
   disabled and must not retain the prior dataset's candles/result.
4. Add one monotonically increasing generation token for dataset loads and
   backtest runs. Capture the generation plus canonical context when work
   starts; before every asynchronous state write, require both still to match.
   Starting a newer load/run or changing inputs invalidates the prior token.
5. Render metrics and build save/export payloads only from the completed
   artifact's snapshots. Never combine a live editor strategy or live dataset
   selection with an older result. Disable Run/Sweep while the selected
   dataset is loading; disable Save/Export whenever no compatible artifact
   exists.
6. Unit-test the pure context/artifact helper for strategy, dataset, interval,
   and range differences plus snapshot immutability. Extend E2E coverage for:
   Run -> edit strategy -> Save/Export unavailable; rerun restores actions;
   Run -> Apply Best -> old actions unavailable. Add a deterministic delayed
   load/run regression only through the existing `?mock=1` development seam;
   if that seam cannot express the race without adding product-only behavior,
   stop and report the spec conflict rather than weakening the test.

### Non-goals

- Do not change metric formulas or contract versions; those belong to
  `METRIC-002`.
- Do not redesign Sweep-result provenance; that belongs to
  `BUG-SWEEP-CONTEXT-001`. This task only invalidates an old completed backtest
  when Apply Best changes the live strategy.
- Do not change SQLite schema, Rust commands/repositories, report schema,
  styling, chart behavior, discovery events, or RUNNER-UI-001.
- Do not add a state-management or hashing dependency.

- **Risk level**: High — silent cross-context persistence is being corrected,
  and careless invalidation can regress current Run/Save/Export flows.
- **Validation plan**:
  - `cd alpha-factor-forge && npm run typecheck`
  - `cd alpha-factor-forge && npm test`
  - `cd alpha-factor-forge && npm run build`
  - `cd alpha-factor-forge && npm run e2e`
  - Manual: run sample -> edit fee/period/mode -> verify metrics and Save/Export
    cannot represent the old result; rerun -> verify all return with matching
    strategy/dataset metadata.
- **Acceptance criteria**:
  - [ ] Save/export read strategy, dataset, interval, range, metrics, and trades
        from one immutable artifact.
  - [ ] Every result-affecting edit invalidates old result actions immediately.
  - [ ] Dataset switch/failure cannot reuse prior candles or result.
  - [ ] Late load/run completion is ignored after any context/generation change.
  - [ ] Unit and E2E regressions cover the shortest reproductions without
        weakening existing assertions or removing any `data-testid`.

### Suggested prompt for coding agent

```text
ROLE: You are the coding agent for exactly one AlphaFactorForge task. Stop after opening its PR.

READ FIRST:
1. AGENTS.md in full.
2. docs/agent-execution-protocol.md sections 2 and 4.
3. docs/improvement-backlog.md task BUG-RESULT-CONTEXT-001 only.
4. handoffs/2026-07-31-pr76-post-merge-audit-v1.md sections for BUG-RESULT-CONTEXT-001 and scope control.

TASK: BUG-RESULT-CONTEXT-001 — bind completed backtests to immutable execution snapshots.

GIT: verify a clean worktree, fetch origin, branch from latest origin/main as
fix/backtest-result-context, and move only this task Next -> In Progress in tasks.md.

SCOPE: touch only the Files likely affected in the task. If a deterministic
dataset-load race test needs another file, stop and report the exact reason
before editing it. No Rust/schema/dependency/style/metric/Sweep provenance work.

IMPLEMENT: follow all six Exact implementation plan steps. Use one pure context
helper and one generation guard; save/export must consume the completed artifact,
never live editor inputs mixed with an old result.

VALIDATE: npm run typecheck, npm test, npm run build, npm run e2e, plus the manual
sample -> run -> edit -> invalidate -> rerun check. Paste real counts/output.

DELIVER: English conventional commit `fix(backtest): bind results to execution context`;
zh-TW draft PR body with 摘要 / 改了什麼 / 驗證清單 / 手動檢查 / 殘餘風險 /
git diff --stat. Update tasks.md to Done and CHANGELOG.md because user-visible
save/export behavior changes. Append the handoff Resolution with commit, PR,
verification, and residual risks. Then STOP; do not merge or start METRIC-002.
```

### Suggested reviewer prompt

```text
Review one PR against BUG-RESULT-CONTEXT-001 and docs/agent-execution-protocol.md section 5.
Prove with file:line evidence that (1) save/export use one immutable artifact,
(2) all strategy/dataset changes invalidate it synchronously, (3) stale async
loads/runs cannot write, (4) tests reproduce run-then-edit and Apply Best, and
(5) there are no metric, Sweep provenance, Rust/schema, dependency, or styling
changes. Return approve / request-changes / escalate in zh-TW; do not edit code.
```

## METRIC-002 — UTC 月報酬以前月月底為基準

Expanded to execution-ready format on 2026-08-01, after `BUG-RESULT-CONTEXT-001`
merged as PR #78 (`38df26a`). Audit evidence: the `METRIC-002` section of
`../handoffs/2026-07-31-pr76-post-merge-audit-v1.md`.

- **Category**: Correctness / persisted calculation contract
- **Objective**: Every UTC calendar month's return must be measured from the
  equity the previous month closed at — and the first month from `startEquity` —
  so a move that happens across a month boundary is counted exactly once instead
  of being discarded. Because the result is persisted evidence that Score and
  the Gate consume, the change ships as a contract version bump, not a silent
  formula edit.
- **Why this is next**: `consistency` in `score-v1` is computed from these
  monthly returns, so the defect does not just mislabel a chart — it inflates
  the rank of strategies whose gains and losses straddle month boundaries. The
  audit's hand-calculated case scores a strategy that doubles, halves, and
  doubles again as perfectly consistent.

### Evidence (re-derived 2026-08-01)

- `alpha-factor-forge/src/core/metrics/index.ts:167-178` keys equity by
  `YYYY-MM` and returns `last / first - 1` **within** each month, so the move
  from one month's close into the next month's first point is never counted.
  `computeMetrics` already resolves `startEquity` at `:94` but does not pass it
  to `monthlyReturns` at `:162`.
- `alpha-factor-forge/src-tauri/src/discovery_core/metrics.rs:296-322` is the
  same algorithm over a `BTreeMap`. The committed cross-language parity only
  proves the two agree; it cannot prove either is right.
- Hand-calculated case (from the audit): equity points Jan-31 = 100,
  Feb-01 = Feb-29 = 200, Mar-01 = Mar-31 = 100, Apr-01 = Apr-30 = 200, with
  `startEquity` = 100. Current output is `[0, 0, 0, 0]`; the correct output is
  `[0, +1, -0.5, +1]`. `consistency` normalized therefore falls from 1 to about
  0.1334, i.e. `score-v1` currently overstates this candidate by about 0.8666 at
  `weight = 1`.
- `alpha-factor-forge/src/services/score.ts:189-205` is the only consumer that
  turns these values into a ranking input.

### Files likely affected

- `alpha-factor-forge/src/core/metrics/index.ts`
- `alpha-factor-forge/src/core/metrics/metrics.test.ts`
- `alpha-factor-forge/src-tauri/src/discovery_core/metrics.rs` (implementation
  plus a new inline `#[cfg(test)] mod tests`, matching `config.rs:1033`)
- `alpha-factor-forge/src/services/discoveryConfig.ts` (`metrics` version pin)
- `alpha-factor-forge/src/parity/backtestFixture.ts` (`METRICS_CONTRACT_VERSION`)
- `alpha-factor-forge/src/parity/benchmarkFixture.ts` (`METRICS_CONTRACT_VERSION`)
- `alpha-factor-forge/src/parity/gateScoreFixture.ts` (`METRICS_CONTRACT_VERSION`)
- `alpha-factor-forge/src/parity/benchmarkFixture.test.ts:49` and
  `alpha-factor-forge/src/parity/gateScoreFixture.test.ts:87` (literal version
  assertions)
- `alpha-factor-forge/src/services/score.test.ts` (consistency regression)
- `alpha-factor-forge/src-tauri/src/discovery_runner/tests.rs` (paused-run
  resume rejection regression — the DB read-back path, see step 6)
- `alpha-factor-forge/src-tauri/src/discovery_core/config.rs` (optional
  additional pure parser test; it may not replace the runner regression)
- `docs/discovery-config-contract.md:37` and `docs/rs-core-parity.md:80`
  (current contract documents that still name the old version)
- Regenerated, never hand-edited: `alpha-factor-forge/fixtures/rs-core/`
  `backtest-v1.json`, `benchmark-v1.json`, `gate-score-v1.json`,
  `runner-config-v1.json`
- `CHANGELOG.md`, `tasks.md`

### `metrics-v1` inventory (measured on `38df26a`)

Every **active** occurrence that must become `metrics-v2`:

| Kind | Count | Locations |
| --- | --- | --- |
| Fixture occurrences | 68 | `fixtures/rs-core/`: backtest 1, benchmark 1, gate-score 1, runner-config 65 — all regenerated, never hand-edited |
| Contract declarations / pins | 5 | `parity/backtestFixture.ts:20`, `parity/benchmarkFixture.ts:31`, `parity/gateScoreFixture.ts:38` (three parallel `METRICS_CONTRACT_VERSION` declarations), `services/discoveryConfig.ts:33` (config pin), `discovery_core/metrics.rs:13` (Rust constant) |
| Literal test assertions | 2 | `parity/benchmarkFixture.test.ts:49`, `parity/gateScoreFixture.test.ts:87` |
| Rust module doc | 1 | `discovery_core/metrics.rs:1` |
| Current contract docs | 2 | `docs/discovery-config-contract.md:37`, `docs/rs-core-parity.md:80` |

Occurrences that **must legitimately keep** the old literal — a repo-wide
zero-hit search is therefore not a valid acceptance check:

- the step 6 rejection regression, whose whole purpose is to feed a stored
  `metrics-v1` config back in;
- `CHANGELOG.md`, `tasks.md`, this specification, and every file under
  `handoffs/`, which record history and must not be rewritten.

### Exact implementation plan

The order is executable as written: the expectations are committed before any
code or fixture can be shaped to fit them.

1. **Write the hand-calculated tests first, against the unchanged formula.** Add
   month-boundary tests on both sides, using the audit case above verbatim plus:
   a single-month series, a series with a gap month, a first month whose
   `startEquity` differs from `equity[0].equity`, and a non-positive base. Add a
   `score.test.ts` regression asserting that the audit case's `consistency`
   entry moves from normalized 1 to about 0.1334 — state the expected value, do
   not merely assert "changed". Run them now and paste the failures in the PR: a
   hand-calculated test that cannot fail before the fix does not prove the fix.
2. Change the TypeScript formula. Give `monthlyReturns` an explicit starting
   base parameter and walk the equity series **in its existing chronological
   order** — do not sort month keys — carrying each month's last equity forward
   as the next present month's base. Pass `computeMetrics`'s already-resolved
   `start` (`index.ts:94`) as the first month's base. Preserve the existing
   total-function behaviour exactly: empty equity still yields `{}`, and a base
   that is not `> 0` still yields `0` for that month rather than a non-finite
   value. A month with no equity points is skipped, and the next present month
   chains from the last present month's close — no synthetic zero-return month
   is inserted.
3. Port the identical change to `metrics.rs::monthly_returns`, keeping the
   `BTreeMap` output type and the existing timestamp-conversion precondition
   comment. The two implementations must be reasoned about as one algorithm;
   any divergence is a parity failure, not a style choice. The step 1 tests must
   now pass on both sides, unchanged from how they were first committed.
4. Bump the contract from `metrics-v1` to `metrics-v2` at every active site in
   the inventory table above, in one commit: the 5 declarations/pins, the 2
   literal test assertions, the 1 Rust module doc, and the 2 current contract
   documents. Do not attempt to de-duplicate the three parallel
   `METRICS_CONTRACT_VERSION` declarations in this task — record that
   duplication as a follow-up bullet in the PR instead.
5. Only now regenerate every affected fixture with its own script
   (`npm run fixtures:backtest`, `fixtures:benchmarks`, `fixtures:gate-score`,
   `fixtures:runner-config`) and commit the regenerated output. Hand-editing a
   fixture, or regenerating one the change does not touch, is a review failure.
   The backtest fixture's sanity checks at `src/parity/backtestFixture.ts:307`
   and `:347` must still hold. Regenerate a second time and record the SHA-256
   of each of the four files from both passes in the PR; "I re-ran it" without
   the two matching digests is not evidence of determinism.
6. Prove the stored-config path fails closed **through the real resume
   read-back**, not through a parser unit test. The path is already confirmed:
   `discovery_runner/mod.rs:472-477` requires the run to be `Paused`, and only
   then does `:479-482` deserialize the persisted `config_json` and call
   `parse_discovery_config`, which compares stored contracts against
   `discovery_contract_versions()` (`config.rs:848-854`). An `idle` run fails
   earlier with an illegal-transition error and therefore cannot exercise the
   metrics mismatch at all — the regression must use `Paused`. In
   `discovery_runner/tests.rs`, create or leave a paused run with no live
   control, set its persisted `config_json.contracts.metrics` to `metrics-v1`,
   call `DiscoveryRunner::resume`, and assert all of: the existing
   contract-mismatch error is returned; the run is still `paused`; and no
   coordinator/control, event, job, or progress write occurred. A pure
   `config.rs` parser test may be added as well, but it does not satisfy this
   step on its own.

### Persisted-record compatibility (maintainer decision, 2026-08-01)

`validation-record-v1`'s `contracts` block records execution, benchmark, gate,
and score, but **not** metrics (`src/services/validationRecord.ts:228-233`;
`src-tauri/src/discovery_runner/execution.rs:619-649`). A record already stored
under the old monthly formula is therefore indistinguishable from a new one, and
`METRIC-002` alone cannot close that gap.

The maintainer adjudicated this on 2026-08-01: keep it out of `METRIC-002`, and
carry it as a **required** successor rather than an optional improvement.

- `METRIC-002` covers the formula, `metrics-v2`, fixture synchronisation, and
  the paused-run fail-closed path only.
- `PERSIST-AUDIT-001` is a **mandatory** follow-up, not optional. It must add a
  metrics contract/version field to the record and bump the record contract to
  `validation-record-v2`; that work spans the TypeScript writer, the Rust
  writer, the Rust validator, and the record fixtures, which is a separate
  persistence-contract change of its own task size.
- Any existing `validation-record-v1` row is treated as **legacy with an unknown
  formula version**. It must never be assumed to be `metrics-v2`. Defaulting a
  legacy record to the new version is a correctness regression, not a
  convenience.
- `RUNNER-UI-001` — and every other flow that needs trustworthy persisted
  records — stays blocked by `PERSIST-AUDIT-001`.

Merging `METRIC-002` therefore does **not** close the audit-integrity finding.
The PR body must say so explicitly.

### Non-goals

- Do not change any other metric formula, `Metrics` field, or the encode/decode
  behaviour in `src/services/metricsCodec.ts`.
- Do not add the metrics version to `validation-record-v1`, and do not bump the
  record contract here — that is `PERSIST-AUDIT-001` (see above).
- Do not de-duplicate the three parallel `METRICS_CONTRACT_VERSION` fixture
  declarations or fold in the separate `discoveryConfig.ts` pin, and do not
  refactor the parity fixture harness.
- Do not rewrite the old literal out of `handoffs/`, `tasks.md` history entries,
  or `CHANGELOG.md`; those are records, not active contract sites.
- Do not touch SQLite schema or migrations, the `backtest_summary` row shape,
  Gate or Score formulas/weights, discovery events, the runner's threading, or
  the interactive backtest UI.
- Do not add dependencies.
- Do not weaken or delete an existing parity assertion to make a regenerated
  fixture agree.

- **Risk level**: High — this rewrites a persisted calculation contract that
  ranking depends on, and it moves 68 committed fixture values. The failure mode
  to guard against is a green build produced by regenerating fixtures around a
  still-wrong formula, which is why step 1 requires the hand-calculated
  expectations to be written and shown failing before step 5 regenerates
  anything.
- **Validation plan**:
  - `cd alpha-factor-forge && npm run typecheck`
  - `cd alpha-factor-forge && npm test`
  - `cd alpha-factor-forge && npm run build`
  - `cd alpha-factor-forge && npm run e2e`
  - `cd alpha-factor-forge/src-tauri && cargo check --locked`
  - `cd alpha-factor-forge/src-tauri && cargo test --locked`
  - Fixture determinism: regenerate twice and paste the SHA-256 of all four
    fixtures from both passes, showing the digests match.
  - Paste the failing output of the new hand-calculated tests against the old
    formula (step 1) alongside their passing output after the fix.
  - Targeted stale-literal search, not a repo-wide zero-hit claim. Show that
    `metrics-v1` is absent from: the 5 declarations/pins, the 2 literal test
    assertions, the Rust module doc, `fixtures/rs-core/`, and the 2 current
    contract documents. The allowlist that may still contain it is exactly: the
    step 6 rejection regression, `CHANGELOG.md`, `tasks.md`, `handoffs/`, and
    this specification.
- **Acceptance criteria**:
  - [ ] The first month is based on `startEquity`; every later month is based on
        the previous present month's closing equity.
  - [ ] TypeScript and Rust produce identical values, proven by regenerated
        parity fixtures plus hand-calculated tests on both sides.
  - [ ] The audit's four-month case yields `[0, +1, -0.5, +1]`, and a
        `score.test.ts` regression pins the resulting `consistency` value.
  - [ ] The hand-calculated tests were written first and shown failing against
        the old formula before any fixture was regenerated.
  - [ ] `metrics-v2` is set at every active site in the inventory table — 5
        declarations/pins, 2 literal assertions, 1 Rust module doc, 2 contract
        documents, and all 68 fixture occurrences — with no stale **active**
        literal left. Occurrences inside the step 6 regression, `CHANGELOG.md`,
        `tasks.md`, `handoffs/`, and this specification are expected and correct.
  - [ ] Resuming a **paused** run whose persisted config carries `metrics-v1`
        returns the existing contract-mismatch error, leaves the run paused, and
        writes no coordinator/control, event, job, or progress state.
  - [ ] `CHANGELOG.md` records the behaviour change, and the PR states that
        `PERSIST-AUDIT-001` remains a required follow-up before persisted
        records can be trusted.

### Suggested prompt for coding agent

```text
ROLE: You are the coding agent for exactly one AlphaFactorForge task. Stop after opening its PR.

READ FIRST:
1. AGENTS.md in full.
2. docs/agent-execution-protocol.md sections 2 and 4.
3. docs/improvement-backlog.md task METRIC-002 only.
4. handoffs/2026-07-31-pr76-post-merge-audit-v1.md section 4 (METRIC-002).

TASK: METRIC-002 — base each UTC month's return on the previous month's closing equity.

GIT: verify a clean worktree, fetch origin, branch from latest origin/main as
fix/monthly-return-baseline, and move only this task Next -> In Progress in tasks.md.

SCOPE: touch only the Files likely affected. This is a persisted calculation
contract change: TS and Rust must move together, and every fixture must be
REGENERATED by its npm script, never hand-edited. Do not add the metrics version
to validation-record-v1 and do not bump the record contract — that is
PERSIST-AUDIT-001. Do not rewrite the old literal out of handoffs/, tasks.md
history, or CHANGELOG.md. No schema, dependency, Gate/Score formula, or UI work.

IMPLEMENT: follow all six Exact implementation plan steps in the order written.
Step 1 (hand-calculated tests, shown FAILING against the unchanged formula) comes
before BOTH the implementation and step 5's fixture regeneration, so neither the
formula nor the fixtures can be shaped to fit an expectation written after the
fact. Step 6 must exercise the real paused-run resume read-back in
discovery_runner/tests.rs, not a config.rs parser test.

VALIDATE: npm run typecheck, npm test, npm run build, npm run e2e, cargo check
--locked, cargo test --locked, the step 1 before/after test output, a double
fixture regeneration with matching SHA-256 digests for all four files, and the
TARGETED stale-literal search from the Validation plan (not a repo-wide
zero-hit claim — several files legitimately keep metrics-v1). Paste real output.

DELIVER: English conventional commit `fix(metrics): base monthly returns on prior month close`;
zh-TW PR body with 摘要 / 改了什麼 / 驗證清單 / 殘餘風險 / git diff --stat. Update
tasks.md to Done and CHANGELOG.md. State explicitly in the PR that PERSIST-AUDIT-001
remains a REQUIRED follow-up and that existing validation-record-v1 rows stay legacy
with an unknown formula version. Then STOP; do not merge and do not start PERSIST-AUDIT-001.
```

### Suggested reviewer prompt

```text
Review one PR against METRIC-002 and docs/agent-execution-protocol.md section 5.
Prove with file:line evidence that (1) the first month uses startEquity and later
months chain from the prior present month's close in BOTH languages, (2) the
hand-calculated cases (including the audit's [0, +1, -0.5, +1] case, a gap month,
and a non-positive base) are asserted with explicit expected values rather than
regenerated-fixture agreement, and that the PR shows them failing against the old
formula, (3) every active site in the inventory table moved to metrics-v2 — 5
declarations/pins, 2 literal assertions, 1 Rust module doc, 2 contract documents,
68 fixture occurrences — while the allowlisted historical occurrences were left
alone, (4) every fixture was regenerated by its script with matching double-run
digests and no parity assertion was weakened, (5) the rejection regression
resumes a PAUSED run through the DB read-back path and asserts the run stays
paused with no coordinator/event/job/progress write — a config.rs parser test
alone is insufficient, and (6) validation-record-v1 was NOT quietly upgraded.
Confirm the PR states PERSIST-AUDIT-001 is still required.
Return approve / request-changes / escalate in zh-TW; do not edit code.
```

---

## DATA-QUALITY-001 — 市場資料匯入語意驗證與已存非法資料 fail closed

Expanded to execution-ready format on 2026-08-09, after `RUNNER-OWNERSHIP-001`
merged as PR #90 (`a451f0d`). Audit evidence: the `DATA-QUALITY-001` section of
`../handoffs/2026-07-31-pr76-post-merge-audit-v1.md`. The three planning
decisions this specification implements — the admission range constants, how the
existing chrono regression is repurposed, and the planning/implementation split —
are adjudicated in
`../handoffs/2026-08-09-data-quality-001-planning-decisions-v1.md` and must not
be re-opened by the coding agent.

- **Category**: Correctness / market-data admission contract
- **Objective**: Dataset identity currently proves only that bytes were not
  altered — it never proves the bytes describe a possible market. Admission must
  gain one semantic validator, implemented identically in TypeScript and Rust,
  that requires plausible epoch-millisecond timestamps, representable dates,
  strictly positive OHLC prices, non-negative volume, and
  `low <= open/close <= high`. Data stored before this contract existed must fail
  closed where it is consumed instead of silently producing evidence.
- **Why this is next**: An epoch-seconds timestamp hashes and imports cleanly
  today, then lands the whole series in 1970, which silently moves every monthly
  return into the wrong month and therefore corrupts `consistency` in `score-v1`
  and the monthly Gate criteria. `{open:100, high:90, low:110, volume:-1}` is
  equally importable, and the backtest engine reads `high`/`low` directly for
  stop-loss and take-profit fills. Both defects produce confident, persisted,
  wrong evidence rather than a visible failure, which is exactly what the
  remaining audit gates exist to prevent.

### Evidence (re-derived 2026-08-09 on `a451f0d`)

- `alpha-factor-forge/src/core/hashing/index.ts:257-277`
  (`normalizeDatasetCandles`) checks only non-empty, `u32` count, finite values,
  `Number.isSafeInteger` timestamps, sort order, and duplicate timestamps.
- `alpha-factor-forge/src-tauri/src/identity.rs:186-219`
  (`normalize_dataset_candles`) is the exact Rust mirror and has the same gap;
  `normalized_number` at `177-184` only rejects non-finite values.
- Neither side checks timestamp magnitude, so `1704067200` (epoch **seconds**
  for 2024-01-01) is accepted as milliseconds and resolves to 1970-01-20.
- `alpha-factor-forge/src/core/metrics/index.ts:177` calls `new Date(p.time)`
  with no guard, so an unrepresentable timestamp yields a `NaN-NaN` month key
  rather than an error. Rust already guards the same conversion at
  `alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs:127`, so the
  two runtimes currently disagree about unrepresentable dates.
- `alpha-factor-forge/src-tauri/src/db/repositories.rs:151-156`
  (`import_dataset_with_candles`) calls only `verify_dataset_identity` before
  opening its transaction, so semantic validity is never asserted at admission.
- `alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs:1093-1116`
  (`load_verified_dataset`) re-verifies the same identity and nothing else, so a
  stored invalid dataset passes straight into a discovery run.
- `alpha-factor-forge/src/tauri-client/dbClient.ts:22-49`
  (`prepareDatasetImport`) is the single TypeScript admission point; the mock
  client reuses it at `mockClient.ts:185-188`, so one change covers the real and
  `?mock=1` paths.
- `alpha-factor-forge/src/components/BacktestPanel.tsx:203-213` accepts whatever
  `db.getCandles` returns for the selected dataset without inspecting it.

### Adjudicated constants (planning decision 1 — do not re-derive)

```text
MIN_MARKET_TIMESTAMP_MS           = 946_684_800_000   // 2000-01-01T00:00:00Z, inclusive
MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE = 4_102_444_800_000 // 2100-01-01T00:00:00Z, exclusive
```

This is a **product plausibility boundary, not a language limit**. The
implementation must still independently assert integrality and successful date
representation in both runtimes, because the contract is that both properties
hold — not that one happens to imply the other today. Use identical constants,
identical inclusive-lower/exclusive-upper semantics, and identical boundary
fixture rows on both sides.

### Rule identifiers (stable; shared fixture keys, evaluated in this order)

| Order | Rule id | Condition that rejects |
| --- | --- | --- |
| 1 | `timestamp_not_integer` | timestamp is not a safe integer (TS) / not integral (Rust) |
| 2 | `timestamp_out_of_range` | `timestamp < MIN` or `timestamp >= MAX_EXCLUSIVE` |
| 3 | `timestamp_not_representable` | `new Date(ts).getTime()` is not finite / `Utc.timestamp_millis_opt(ts).single()` is `None` |
| 4 | `price_not_positive` | any of open/high/low/close is non-finite or `<= 0` |
| 5 | `volume_negative` | volume is non-finite or `< 0` |
| 6 | `high_below_low` | `high < low` |
| 7 | `ohlc_out_of_range` | `open < low`, `open > high`, `close < low`, or `close > high` |

Evaluation stops at the **first** failing candle and reports that candle's
index, its timestamp, and the rule id, so both runtimes produce the same
classification for the same input. A dataset is admitted only if every candle
passes every rule.

**Rule 3 is deliberately unreachable, and that is not a defect.** Every value
that survives rule 2 is representable in both runtimes — the product range is
strictly inside `Date`'s ±8.64e15 ms limit and inside chrono's UTC millisecond
range — so no input can fail rule 3 alone. Planning decision 1 nevertheless
requires representability to be asserted *independently* rather than left as an
implied consequence of the range, so rule 3 stays in the code as defence in
depth against a future range change. It therefore has **no fixture rejection
row and no mutation case**; instead each language unit-tests the
representability predicate directly (call it with `8_500_000_000_000_000`, which
JavaScript can represent and chrono cannot, and with `MAX_EXCLUSIVE`) and
asserts the expected boolean. Wherever this specification says "every rule id",
read it as "every **reachable** rule id" — rules 1, 2, and 4 through 7.

### Files likely affected

- new `alpha-factor-forge/src/core/market-data/quality.ts` (pure; no
  React/DOM/IO, matching the `src/core/*` purity rule)
- new `alpha-factor-forge/src/core/market-data/quality.test.ts`
- new `alpha-factor-forge/src-tauri/src/discovery_core/market_data.rs`
  (pure contract module, plus an inline `#[cfg(test)] mod tests`)
- `alpha-factor-forge/src-tauri/src/discovery_core/mod.rs` (`pub mod
  market_data;` and the parity test module declaration)
- new `alpha-factor-forge/src/parity/marketDataQualityFixture.ts`
- new `alpha-factor-forge/src/parity/marketDataQualityFixture.test.ts`
- new `alpha-factor-forge/scripts/generate-market-data-quality-fixtures.ts`
- new, regenerated and never hand-edited:
  `alpha-factor-forge/fixtures/rs-core/market-data-quality-v1.json`
- new `alpha-factor-forge/src-tauri/src/discovery_core/market_data_parity_tests.rs`
- `alpha-factor-forge/package.json` (one `fixtures:market-data-quality` script;
  **no dependency changes**)
- `alpha-factor-forge/src/tauri-client/dbClient.ts` (TS admission mount point)
- `alpha-factor-forge/src/tauri-client/dbClient.test.ts` (TS admission rejection
  plus mock-store-unchanged assertions)
- `alpha-factor-forge/src/components/BacktestPanel.tsx` (stored-invalid-data
  fail-closed on dataset selection, and the zh-TW re-import guidance copy)
- `alpha-factor-forge/src-tauri/src/db/repositories.rs` (pre-transaction
  admission call; the table-driven atomic mutation tests; and the
  `identity_candles()` test helper at `919-936`, whose timestamps `1`/`2` are no
  longer admissible)
- `alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs`
  (`load_verified_dataset` fail-closed for `start` and `resume`)
- `alpha-factor-forge/src-tauri/src/discovery_runner/tests.rs` (repurposed
  `chrono_invalid_js_safe_timestamp` regression)
- new `docs/market-data-quality-contract.md`
- `CHANGELOG.md`, `tasks.md`

**Explicitly NOT in the list, and their absence is an acceptance check:**
`src/core/hashing/index.ts`, `src-tauri/src/identity.rs`, the identity fixture
`src/core/hashing/identity-v2.fixture.json`, and every file under
`src-tauri/migrations/`.

### Mount points (four call sites, one rule set)

| # | Site | Required behaviour |
| --- | --- | --- |
| 1 | `dbClient.ts` `prepareDatasetImport` | Validate the normalized candles and throw **before** `db.importCandles` is reached, so no boundary call is made. Covers the real and mock clients through one edit. |
| 2 | `BacktestPanel.tsx` candle-load effect | Validate the resolved `db.getCandles` payload. On failure do **not** call `setLoadedCandles`; set a zh-TW error that names the failing candle and tells the user to re-import. `candles` therefore stays `NO_CANDLES`, `liveContext` stays `null`, and Run/Save/Export stay disabled by the existing guards — do not add a second disabling mechanism. |
| 3 | `repositories.rs` `import_dataset_with_candles` | Validate **before** `conn.transaction()`. This ordering is what makes atomicity provable rather than incidental. |
| 4 | `discovery_runner/mod.rs` `load_verified_dataset` | Validate **after** `verify_dataset_identity`, so a tampered payload still reports the identity mismatch first and the two failure classes stay distinguishable. |

### Exact implementation plan

The order is executable as written: the shared expectations are committed before
any implementation can be shaped to fit them.

1. **Write the shared accept/reject matrix first.** Build
   `src/parity/marketDataQualityFixture.ts` plus its generator script and npm
   script, following the existing `gateScoreFixture.ts` /
   `scripts/generate-gate-score-fixtures.ts` pattern (`PARITY_FIXTURE_SCHEMA_VERSION
   = 'rs-core-parity-fixture-v1'`, pure builder, script owns file IO). The matrix
   must contain, at minimum: one fully valid dataset; one rejection row per
   **reachable** rule id (1, 2, 4, 5, 6, 7 — see the note under the rule table
   for why rule 3 is excluded); and explicit boundary rows at
   `MIN - 1`, `MIN`, `MAX_EXCLUSIVE - 1`, and `MAX_EXCLUSIVE`, with `MIN` and
   `MAX_EXCLUSIVE - 1` accepted and the other two rejected. Include
   `1704067200` (the audit's epoch-seconds case, which resolves to
   1970-01-20T17:21:07.200Z when misread as milliseconds) and
   `8_500_000_000_000_000` (the value the current runner regression uses) as
   named rejection rows; both are expected to report `timestamp_out_of_range`,
   because rule 2 precedes rule 3. Each row records the expected rule id and the
   expected failing candle index.
2. **Add the TypeScript validator** in `src/core/market-data/quality.ts`,
   exporting `MARKET_DATA_QUALITY_VERSION = 'market-data-quality-v1'`, both
   constants, the rule-id union, a `MarketDataIssue { index, timestamp, rule }`
   type, a per-candle inspector, a first-issue finder over a slice, and an
   assert helper that throws with a stable technical message. Keep zh-TW UI copy
   out of `core/*`; the component owns user-facing wording.
3. **Add the Rust validator** in
   `src-tauri/src/discovery_core/market_data.rs`, mirroring the module exactly
   and following the crate's error convention (`pub struct MarketDataError(pub
   String);`, as in `config.rs:41` and `backtest.rs:95`). Expose a **field-level**
   entry point taking `(index, timestamp, open, high, low, close, volume)` plus a
   slice helper over `discovery_core::types::Candle`, so `db::Candle` callers
   validate without allocating a converted vector. Declare the module in
   `discovery_core/mod.rs`. Binary-side callers map `MarketDataError` to
   `AppError::Other`, matching how `parse_discovery_config` is mapped at
   `discovery_runner/mod.rs:481`.
4. **Wire the parity tests** — `marketDataQualityFixture.test.ts` on the TS side
   and `market_data_parity_tests.rs` on the Rust side, both reading the single
   generated JSON. Run them before step 5 and show them failing, so the fixture
   cannot later be reshaped around whichever implementation was written first.
5. **Mount all four call sites** exactly as the table above specifies.
6. **Add the table-driven atomic-import mutation tests** in `repositories.rs`.
   Each case starts from a database that already holds one valid imported
   dataset, then attempts to import a payload mutated to violate exactly one
   rule, and asserts all three of: the call returns `Err` naming that rule; the
   `datasets` and `candles` row counts are unchanged; and the pre-existing
   dataset's candle rows are still byte-identical (reuse the `to_bits()`
   comparison style of `candles_equal` at `repositories.rs:259-267`). Add the
   TypeScript counterpart in `dbClient.test.ts`: a rejected
   `prepareDatasetImport` leaves `client.db.getDatasets()` unchanged.
7. **Repurpose the stored-invalid-data regression** (planning decision 2). In
   `discovery_runner/tests.rs`, rewrite
   `chrono_invalid_js_safe_timestamp_persists_failed_run_without_success_event`
   so it inserts the invalid dataset **directly through SQL**, bypassing
   `import_dataset_with_candles`, to simulate data stored before this contract
   existed. **Critical:** compute the row's `dataset_hash` with
   `identity::dataset_content_hash` over the *invalid* candles — hashing performs
   no semantic validation — so the dataset is internally consistent and
   `verify_dataset_identity` still passes. Otherwise the test would pass for the
   wrong reason, reporting an identity mismatch instead of a quality rejection,
   so it must assert the market-data rule id **and** assert the error is not an
   identity mismatch. Because `load_verified_dataset` runs at
   `discovery_runner/mod.rs:348`, **before** any run row is inserted, the
   assertions change shape: `runner.start(...)` now returns `Err`, and the test
   must prove nothing was written — no `discovery_runs` row, no jobs, no
   progress, no emitted event, and no registered coordinator control. Add the
   matching `resume` case for a paused run, modelling it on the existing
   `resume_rejects_a_paused_run_with_the_stale_metrics_contract_without_writes`
   test at `tests.rs:632`, asserting the run stays paused with no writes. The
   metrics-layer chrono guard keeps its own coverage at
   `execution.rs:958`; do not delete it.
8. **Repair the now-inadmissible test fixtures.** `identity_candles()` at
   `repositories.rs:919-936` uses timestamps `1` and `2`; move them into the
   admissible range (the committed TS fixture values `1_721_001_600_000` and
   `1_721_005_200_000` are the natural choice). Its dataset hash is computed, not
   a literal, so nothing else in that test needs updating. **Leave
   `identity.rs:323-357` alone**: those hashing tests keep timestamps `1`/`2`,
   and their continuing to pass unmodified is the mechanical proof that the
   validator did not enter the identity path.
9. **Document the contract.** Add `docs/market-data-quality-contract.md` stating
   the version string, both constants with their UTC meanings and boundary
   semantics, the ordered rule table, the four mount points, the fail-closed
   behaviour for stored data, and an explicit statement that the dataset hash
   preimage is unchanged. Record the behaviour change in `CHANGELOG.md` and move
   the task through `tasks.md`.

### Non-goals

- Do not change the dataset hash definition, its preimage, its version string,
  or the observable output of `normalizeDatasetCandles` /
  `normalize_dataset_candles` for any input they accept today. The validator is
  a separate module invoked at admission, never a step inside identity encoding.
- Do not touch SQLite schema or migrations. Admission is a gate, not persisted
  evidence, so `market-data-quality-v1` is not written to any table.
- Do not automatically repair, rewrite, drop, quarantine, or re-hash stored
  candle bytes. Invalid stored data fails closed and the user re-imports.
- Do not drop individual bad candles and import the remainder. A dataset is
  admitted whole or rejected whole.
- **Do not change the unknown-interval fallback, and do not add interval-cadence
  or gap/continuity validation.** That is `INTERVAL-CONTRACT-001`, which must be
  adjudicated first; the audit explicitly forbids folding it in here.
- Do not add the summary/trade bundle invariants — that is
  `PERSIST-INVARIANT-001`. Do not touch Sweep context, strategy period
  validation, or the runner UI (`BUG-SWEEP-CONTEXT-001`,
  `STRATEGY-VALIDATION-001`, `RUNNER-UI-001`).
- Do not add dependencies, and do not modify `package-lock.json` beyond what an
  untouched `npm install` produces.
- Do not modify `e2e/` specs; this task does not grant e2e edits.
- Do not weaken, delete, or re-point an existing parity or identity assertion to
  make a new rule pass.

- **Risk level**: Medium-high. The rules themselves are simple, but this task
  narrows what the application will accept, so the realistic failure modes are
  (a) the validator leaking into the identity path and silently redefining
  dataset hashes, (b) a stored-data test that passes for the wrong reason
  because identity verification rejected the payload before the new validator
  ran, and (c) TS/Rust drifting on the boundary semantics. Steps 1, 7, and 8 are
  each written specifically to make one of those failures visible.
- **Validation plan**:
  - `cd alpha-factor-forge && npm run typecheck`
  - `cd alpha-factor-forge && npm test`
  - `cd alpha-factor-forge && npm run build`
  - `cd alpha-factor-forge && npm run e2e` (local Windows: default
    `workers=1`; do not override)
  - `cd alpha-factor-forge/src-tauri && cargo check --locked`
  - `cd alpha-factor-forge/src-tauri && cargo test --locked`
  - `cd alpha-factor-forge/src-tauri && cargo clippy --locked --all-targets` —
    four pre-existing warnings in `backtest.rs` / `score.rs` are the accepted
    baseline; paste the output and show no new warning was added.
  - Fixture determinism: regenerate `market-data-quality-v1.json` twice and paste
    both SHA-256 digests, showing they match.
  - Paste the step 4 parity tests failing against the unimplemented validators,
    alongside their passing output afterwards.
  - Paste `git diff --stat` and confirm that `src/core/hashing/index.ts`,
    `src-tauri/src/identity.rs`, `identity-v2.fixture.json`, `src-tauri/migrations/`,
    and `e2e/` are all absent from it.
- **Acceptance criteria**:
  - [ ] One rule set, expressed once per language, is enforced at all four mount
        points; no mount point re-implements or partially applies the rules.
  - [ ] TypeScript and Rust classify every row of the shared fixture identically,
        including the four boundary rows at `MIN - 1`, `MIN`,
        `MAX_EXCLUSIVE - 1`, and `MAX_EXCLUSIVE`.
  - [ ] `1704067200` is rejected as `timestamp_out_of_range` on both sides.
  - [ ] `{open:100, high:90, low:110, close:100, volume:-1}` is rejected on both
        sides, and the reported rule id is the first one in evaluation order.
  - [ ] Every reachable rule id (1, 2, 4, 5, 6, 7) has an atomic-import mutation
        case proving the import is rejected, no row was written, and a previously
        imported valid dataset is still byte-identical. Rule 3 is covered instead
        by a direct predicate unit test in each language.
  - [ ] A dataset stored **before** this contract fails closed when a discovery
        run starts and when a paused run resumes, with no run row, job, progress,
        event, or coordinator control written — and the test proves the rejection
        came from the market-data validator, not from an identity mismatch.
  - [ ] Selecting such a dataset in the UI leaves Run/Save/Export disabled through
        the existing `liveContext == null` path and shows zh-TW guidance to
        re-import; no new disabling mechanism was introduced.
  - [ ] `identity.rs`'s timestamp `1`/`2` hashing tests still pass **unmodified**,
        and the committed dataset hash in `identity-v2.fixture.json` is unchanged
        — the mechanical proof that dataset identity was not redefined.
  - [ ] No migration, schema, dependency, or `e2e/` change is present in the diff.
  - [ ] The unknown-interval fallback is untouched, and the PR states that
        `INTERVAL-CONTRACT-001` remains an open, separate decision.
  - [ ] `docs/market-data-quality-contract.md` exists, `CHANGELOG.md` records the
        behaviour change, and `tasks.md` shows the task in `Done`.

### Suggested prompt for coding agent

```text
ROLE: You are the coding agent for exactly one AlphaFactorForge task. Stop after opening its PR.

READ FIRST:
1. AGENTS.md in full.
2. docs/agent-execution-protocol.md sections 2 and 4.
3. docs/improvement-backlog.md task DATA-QUALITY-001 only.
4. handoffs/2026-07-31-pr76-post-merge-audit-v1.md section 7, and
   handoffs/2026-08-09-data-quality-001-planning-decisions-v1.md in full.

TASK: DATA-QUALITY-001 — matching TS/Rust market-data admission validation, and
fail closed on data stored before the contract existed.

GIT: verify a clean worktree, fetch origin, branch from latest origin/main as
fix/market-data-admission-validation, and move only this task to In Progress in tasks.md.

SCOPE: touch only the Files likely affected. The admission range constants and the
chrono-regression treatment are ALREADY ADJUDICATED in the planning handoff — implement
them, do not re-derive or re-open them. Do NOT touch src/core/hashing/index.ts,
src-tauri/src/identity.rs, identity-v2.fixture.json, src-tauri/migrations/, or e2e/;
their absence from the diff is an acceptance check. No new dependencies. Do not add
interval-cadence or gap validation and do not change the unknown-interval fallback —
that is INTERVAL-CONTRACT-001.

IMPLEMENT: follow all nine Exact implementation plan steps in the written order.
Step 1 (the shared accept/reject fixture) and step 4 (parity tests shown FAILING)
come before step 5's implementation, so neither language's behaviour can be
back-fitted to the other. Step 7 is the highest-risk step: the stored-invalid
dataset MUST be hashed over its own invalid candles and inserted via raw SQL, or the
test will pass for the wrong reason by hitting an identity mismatch instead of the
new validator — assert the rule id AND assert it is not an identity mismatch.
Note that load_verified_dataset runs before any run row is inserted, so start()
returns Err and the correct assertion is that NOTHING was written.

VALIDATE: npm run typecheck, npm test, npm run build, npm run e2e (default workers=1),
cargo check --locked, cargo test --locked, cargo clippy --locked --all-targets (four
pre-existing backtest.rs/score.rs warnings are the accepted baseline — show no NEW
warning), the step 4 before/after parity output, a double fixture regeneration with
matching SHA-256 digests, and git diff --stat. Paste real output; never weaken a test
to reach green.

DELIVER: English conventional commit `fix(data): validate market data at dataset admission`;
zh-TW PR body with 摘要 / 改了什麼 / 驗證清單(勾選 acceptance criteria) / 殘餘風險 /
git diff --stat. Add docs/market-data-quality-contract.md, update CHANGELOG.md, and set
tasks.md to Done. State in the PR that INTERVAL-CONTRACT-001 is still an open separate
decision and that BUG-SWEEP-CONTEXT-001 is the next task. Then STOP; do not merge and
do not start the next task. List out-of-scope ideas as 建議後續 bullets only.
```

### Suggested reviewer prompt

```text
Review one PR against DATA-QUALITY-001 and docs/agent-execution-protocol.md section 5.
Prove with file:line evidence that (1) the dataset hash definition is untouched —
src/core/hashing/index.ts, src-tauri/src/identity.rs, identity-v2.fixture.json and
migrations/ are absent from the diff, and identity.rs's timestamp 1/2 hashing tests
still pass unmodified; (2) one rule set is enforced at all four mount points, with the
Rust import check placed BEFORE conn.transaction() and the runner check placed AFTER
verify_dataset_identity; (3) TS and Rust agree on every fixture row including the four
boundary rows at MIN-1, MIN, MAX_EXCLUSIVE-1 and MAX_EXCLUSIVE, and the fixture was
generated by its script with matching double-run digests rather than hand-edited;
(4) every reachable rule id (1, 2, 4-7; rule 3 is unreachable by construction and is
covered by a direct predicate test) has an atomic-import mutation case asserting
rejection AND zero rows
written AND a previously imported dataset still byte-identical; (5) the stored-invalid
regression inserts its dataset via raw SQL with a hash computed over the invalid
candles, and asserts the market-data rule id rather than an identity mismatch, with no
run row, job, progress, event, or coordinator control written on start and no state
change on resume; (6) the UI fail-closed path reuses the existing liveContext == null
guard instead of adding a second mechanism, and its copy is zh-TW; (7) no interval
cadence, gap, or unknown-interval-fallback change slipped in, and no PERSIST-INVARIANT-001
or BUG-SWEEP-CONTEXT-001 work was bundled; (8) no dependency, schema, or e2e change.
Return approve / request-changes / escalate in zh-TW; do not edit code.
```

---

## BUG-SWEEP-CONTEXT-001 — 參數掃描結果綁定不可變掃描快照

Expanded to execution-ready format on 2026-08-15, after `DATA-QUALITY-001`
merged as PR #94 (`bd445f3`) and the marketing campaign merged as PR #97
(`a3fe2fe`). Audit evidence: §8 of
`../handoffs/2026-07-31-pr76-post-merge-audit-v1.md`. This is order #6 of the
adjudicated post-PR #76 sequence and the last correctness gate before
`STRATEGY-VALIDATION-001`.

- **Category**: Correctness / anti-overfitting discipline
- **Objective**: A completed parameter sweep may be shown, and its cells may be
  applied to the strategy, only while the inputs it optimised over are still the
  live inputs. The sweep result must carry the dataset identity, interval, the
  exact optimised bar range including the Holdout split, the sweep
  configuration, and the base strategy with the swept axes masked; a mismatch
  must hide the heatmap and every apply action, and a late completion must be
  discarded rather than painted into the current inputs.
- **Why this is next**: `BUG-RESULT-CONTEXT-001` closed the same hazard for the
  interactive backtest but explicitly left the sweep open (see that task's
  Resolution, 殘餘風險 bullet 2). Today a sweep run with Holdout **off** stays on
  screen after Holdout is switched **on**, and 套用最佳 still applies a combination
  that was chosen using the out-of-sample tail. That is not a stale-UI annoyance:
  it silently destroys the honesty of the segment the whole Holdout feature
  exists to protect, and the user has no way to see that it happened.

### Evidence (re-derived 2026-08-15 on `a3fe2fe`)

- `alpha-factor-forge/src/components/SweepSection.tsx:219-222` clears the shown
  result only on `resetSignal` (library load), and `232-237` (`clearSweep`) only
  on a sweep-config edit. Nothing else invalidates it.
- Same file `239-264` (`runSweep`) derives the optimised range from the
  `holdout` / `holdoutPct` props at run time, but that range is never recorded
  with the result, so the heatmap cannot know which bars produced it.
- Same file `279-283` (`applySweepBest`) and `346` (per-cell `onPick`) act on
  whatever `sweepResult` currently holds.
- `alpha-factor-forge/src/components/BacktestPanel.tsx:488-507` passes `strat`,
  `interval`, `holdout`, and `holdoutPct` as four independent props, so a sweep
  has no single description of its own inputs to compare against — exactly the
  per-field drift `runArtifact.ts` was created to remove.
- Shortest reproduction: 載入樣本 → 展開參數掃描 → 執行掃描（Holdout OFF）→
  勾選 Holdout → 不重新掃描直接按「套用最佳」。The applied combination was chosen
  on the full period, including the bars now reserved as out-of-sample.
- Second reproduction: 執行掃描 → 改「手續費 %」（非掃描軸）→ heatmap 仍在，
  且每一格的指標值都是用舊手續費算的。

### Files likely affected

- new `alpha-factor-forge/src/services/sweepArtifact.ts` (pure; no React/DOM/IO)
- new `alpha-factor-forge/src/services/sweepArtifact.test.ts`
- `alpha-factor-forge/src/components/SweepSection.tsx`
- `alpha-factor-forge/src/components/BacktestPanel.tsx` (prop wiring only)
- new `alpha-factor-forge/e2e/sweep-context.spec.ts`
- `CHANGELOG.md`, `tasks.md`, this file, and a Resolution section appended to
  `handoffs/2026-07-31-pr76-post-merge-audit-v1.md`

**Explicitly NOT in the list, and their absence is an acceptance check:**
`src/services/paramSweep.ts` (the sweep engine is correct and stays untouched),
`src/services/runArtifact.ts`, `src/tauri-client/**`, `src-tauri/**`,
`src-tauri/migrations/**`, and every existing `e2e/*.spec.ts`.

### Contract shape (adjudicated; do not redesign mid-implementation)

```text
SWEEP_CONTEXT_VERSION = 'sweep-context-v1'

SweepContext = {
  dataset: RunDatasetSnapshot   // reused verbatim from runArtifact.ts
  basis:   { fixed: Record<string, unknown>, swept: SweepParamKey[] }
  config:  SweepConfig          // normalized: y is null (never undefined) on 1-D
  range:   { from, to, holdout: { pct, splitIndex } | null }
}
CompletedSweep = { context: SweepContext, result: SweepResult }
```

Three properties make this correct, and each has a named test:

1. **The masked basis.** `basis.fixed` is the live strategy with the swept axis
   keys **removed**, and `basis.swept` is the sorted axis-key list. Applying a
   cell writes exactly the swept axes, so it cannot invalidate the grid it came
   from — the intentional case the task calls out. Every other strategy field
   stays in `fixed`, so a non-axis edit invalidates. Masking is an **equality**
   decision only; the sweep still executes against the full live strategy.
2. **One range definition.** The optimised range is derived from the panel's
   `RunRange` by one helper: full period, or `[from, splitIndex - 1]` when
   Holdout is on. It therefore shares `holdoutSplitIndex` with `run()` and with
   the sweep's own `from`/`to`, so the recorded range cannot drift from the
   executed one (the BUG-001 boundary stays single-sourced).
3. **One comparison point.** `sameSweepContext` over `canonicalize` is the only
   "is this sweep still valid?" decision. `SweepSection` must not compare
   datasets, strategies, or holdout fields itself.

### Exact implementation plan

1. Add `src/services/sweepArtifact.ts`, importing `RunContext`, `RunRange`,
   `RunDatasetSnapshot`, and `RunHoldoutSplit` from `runArtifact.ts` and
   `SweepConfig` / `SweepParamKey` / `SweepResult` from `paramSweep.ts`. Export
   `SWEEP_CONTEXT_VERSION`, the types above, and:
   - `sweepRangeFromRunRange(range)` — property 2.
   - `normalizeSweepConfig(config)` — forces `y: null` on a 1-D sweep, because
     `canonicalize` is `JSON.stringify`-based and would otherwise key `undefined`
     and `null` differently.
   - `sweptParamKeys(config)` — sorted, de-duplicated axis keys.
   - `describeSweepContext({ run, config })` — deep-cloned and deep-frozen.
   - `sweepContextKey` / `sameSweepContext` (null is never equal to anything).
   - `createSweepArtifact({ context, result })` — `structuredClone` + deep freeze
     so a caller cannot mutate a stored grid.
   - `sweepResultIsWritable({ started, live, generation, owner })` — the pure
     late-completion predicate: the sweep must still own the slot **and** the
     context it started for must still be live.
2. Add `src/services/sweepArtifact.test.ts` covering, by name: swept axes masked
   (1-D and 2-D); a non-axis strategy edit invalidates; every dataset field
   invalidates; Holdout toggle and percentage invalidate; every sweep-config
   field invalidates; `y: undefined` and `y: null` key identically; the Holdout
   range equals `[0, splitIndex - 1]` and shares `holdoutSplitIndex`; null fails
   closed; the artifact is frozen and detached; and the four branches of
   `sweepResultIsWritable`.
3. Rewire `SweepSection` props to a single `liveContext: RunContext | null`,
   replacing `strat`, `interval`, `datasetSelected`, `holdout`, and `holdoutPct`.
   `BacktestPanel` passes the `liveContext` it already computes. Do not add a
   second source of live inputs.
4. Replace the `sweepResult` state with `completedSweep: CompletedSweep | null`
   plus a **render-derived** gate, mirroring `BacktestPanel`'s `artifact` /
   `staleResult`: `sweep` is the completed sweep only while its context still
   matches the live one, and `staleSweep` is "completed but no longer matching".
   The heatmap, 套用最佳, and the applied-cell marker render from `sweep` only, so
   an invalidating edit removes them in the same render that accepts the edit.
5. When `staleSweep`, render one zh-TW notice with `data-testid="sweep-stale"`
   telling the user the previous scan no longer matches and to re-scan. Do not
   add a second disabling mechanism, and do not silently keep the grid visible.
6. Add a generation token + owner ref to `SweepSection` (same shape as
   `BacktestPanel`'s) and drive the `掃描中…` state from `sweepingGen`. On
   completion, write the artifact only when `sweepResultIsWritable` returns true;
   otherwise discard the result **and** leave no error behind.
7. Keep the existing hard clears (`clearSweep` on a config edit, `resetSignal` on
   a library load). They are a strict subset of the new gate, they preserve the
   current behaviour that `e2e/sweep.spec.ts` already asserts, and they match the
   precedent in `loadSavedStrategy`, which clears `completed` even though the
   context gate would also catch it.
8. Add `e2e/sweep-context.spec.ts` with the Holdout-toggle reproduction, the
   non-axis-edit reproduction, and the intentional apply-a-swept-cell case.

### Non-goals

- Do not move the sweep into the Web Worker, add cancellation UI, or change its
  performance characteristics — that is `PERF-001`, which has its own
  specification above and must not be duplicated or partially started here.
- Do not change `paramSweep.ts`: the engine, the axis/combination caps, the
  metric projection, and the `from`/`to` semantics are correct and out of scope.
- Do not persist the sweep artifact. There is no sweep row, no schema change, no
  migration, and no Rust change; the artifact lives only in component state.
- Do not change the interactive backtest's `runArtifact` contract, the strategy
  library, Save/Export, or the report schema.
- Do not add indicator-period validation; a swept axis that produces an invalid
  period still yields a null cell. That is `STRATEGY-VALIDATION-001`.
- Do not weaken or re-point any assertion in the existing `e2e/sweep.spec.ts`,
  `e2e/holdout.spec.ts`, or `e2e/result-context.spec.ts`; every existing
  `data-testid` must survive.
- No new dependencies, and no `package-lock.json` change.

- **Risk level**: Medium. The logic is small and pure, but the realistic failure
  modes are (a) forgetting to mask the swept axes, which makes applying a cell
  invalidate the grid it came from and breaks the shipped apply flow, (b) keying
  the context off a field that legitimately changes between renders, producing a
  heatmap that vanishes for no reason, and (c) leaving one apply path reading the
  ungated state. Steps 1, 2, and 4 are each written to make one of those visible.
- **Validation plan**:
  - `cd alpha-factor-forge && npm run typecheck`
  - `cd alpha-factor-forge && npm test`
  - `cd alpha-factor-forge && npm run build`
  - `cd alpha-factor-forge && npm run e2e` (local Windows: default `workers=1`;
    do not override)
  - No Rust file changes, so `cargo` is not re-run; state this explicitly in the
    PR instead of omitting it.
  - Paste `git diff --stat` and confirm `src/services/paramSweep.ts`,
    `src/services/runArtifact.ts`, `src-tauri/`, and the existing `e2e/*.spec.ts`
    are absent from it.
  - Manual: 載入樣本 → 掃描 → 勾 Holdout → heatmap 與 套用最佳 消失並顯示
    `sweep-stale` → 重新掃描 → 兩者回來且掃描範圍註記為僅樣本內。
- **Acceptance criteria**:
  - [ ] A completed sweep records dataset id + content hash, symbol, interval,
        dataset time range, bar count, the optimised range including the Holdout
        split, the normalized sweep configuration, and the masked base strategy.
  - [ ] Toggling Holdout, or changing its percentage, hides the heatmap, 套用最佳,
        and the applied marker, and shows `sweep-stale`.
  - [ ] A non-axis strategy edit (e.g. 手續費 %) invalidates the sweep the same way.
  - [ ] Applying a swept-axis cell — the intentional case — keeps the grid valid,
        keeps 套用最佳 available, and shows the ✓ applied marker.
  - [ ] A dataset switch cannot leave a previous dataset's heatmap reachable.
  - [ ] A late-completing sweep whose context is no longer live is discarded and
        leaves no result and no error; the predicate is unit-tested on all four
        branches.
  - [ ] The heatmap and every apply path read the gated value; no call site
        compares strategy or dataset fields by hand.
  - [ ] Every existing sweep/holdout/result-context `data-testid` still exists and
        the existing e2e specs pass unmodified.
  - [ ] No dependency, schema, migration, Rust, or `paramSweep.ts` change is in
        the diff; `CHANGELOG.md` records the behaviour change and `tasks.md` shows
        the task in `Done`.

### Suggested reviewer prompt

```text
Review one PR against BUG-SWEEP-CONTEXT-001 and docs/agent-execution-protocol.md section 5.
Prove with file:line evidence that (1) the sweep engine src/services/paramSweep.ts and
src-tauri/ are absent from the diff, and no existing e2e spec was modified; (2) the swept
axis keys are removed from the compared basis, so applying a cell does NOT invalidate the
grid, while a non-axis strategy edit does — both proven by named unit tests; (3) the
optimised range comes from the shared holdoutSplitIndex via one helper and equals
[0, splitIndex-1] when Holdout is on, so the recorded range cannot drift from the executed
one; (4) the heatmap, 套用最佳, and the per-cell onPick all read the render-derived gated
value, not the raw state, and sameSweepContext is the only equality used; (5) a late
completion is discarded by both halves of the guard (generation ownership AND live-context
match) and leaves no error; (6) the zh-TW stale notice appears instead of a silently
hidden grid, and every pre-existing data-testid survives; (7) nothing from PERF-001
(worker/cancellation) or STRATEGY-VALIDATION-001 (period validation) was bundled in.
Return approve / request-changes / escalate in zh-TW; do not edit code.
```

---

## STRATEGY-VALIDATION-001 — 手動策略指標參數的單一執行期驗證

Expanded to execution-ready format on 2026-08-16, after `BUG-SWEEP-CONTEXT-001`
merged as PR #98 (`be4e3c6`). Audit evidence: §9 of
`../handoffs/2026-07-31-pr76-post-merge-audit-v1.md`. This is order #7, the last
correctness gate before `RUNNER-UI-001`.

- **Category**: Correctness / input validation
- **Objective**: One runtime validator, shared by manual strategy execution and
  persistence, that rejects indicator parameters which cannot produce a
  meaningful series — zero, negative, fractional, non-finite, unsafe-integer
  periods, a non-positive Bollinger multiplier, and RSI levels outside 0–100 —
  before a backtest runs or a strategy row is written. Cross-field hypothesis
  constraints are surfaced visibly. UI `min`/`step` are secondary protection
  only.
- **Why this is next**: `fastMA: 2.5` reaches `sma()` as a fractional period, so
  `values[i - 2.5]` is `undefined`, every output becomes `NaN`, every signal is
  `false`, and the backtest reports a confident **zero-trade result** that can be
  saved and exported. `fastMA: 0` and `emaPeriod: 0` take the early-return path
  to the same silent outcome. Nothing between `NumberInput` (finite only) and the
  indicators rejects it.

### Evidence (re-derived 2026-08-16 on `be4e3c6`)

- `alpha-factor-forge/src/components/NumberInput.tsx:46` propagates any finite
  parsed number live and unclamped; `min`/`max` are optional and clamp on blur
  only. `StrategySection.tsx:316-321` passes neither for the indicator grid.
- `alpha-factor-forge/src/services/strategySignals.ts:70-75` hands the raw values
  to `sma`/`ema`/`rsi`/`macd`/`bbands`.
- `alpha-factor-forge/src/core/indicators/index.ts:13-23` — `sma` returns all
  `NaN` for `period <= 0`; for `2.5` the `sum -= values[i - period]` term indexes
  a non-integer position (`undefined`) and poisons the running sum. `ema`
  (`26-39`) additionally *writes* to non-integer indices, so every integer
  position stays `NaN`.
- `alpha-factor-forge/src/services/strategyLibrary.ts:68-72` requires persisted
  numbers to be finite and nothing more.
- `alpha-factor-forge/src/services/strategyRecord.ts:10-14` hashes and persists
  whatever it is given.
- **The rule already exists twice and is enforced nowhere on the manual path**:
  `discoveryConfig.ts:364-380` (`checkNumericParam`, domain table at `65-82`) and
  `embargo.ts:38-43` (`period()`) both require
  `Number.isSafeInteger(value) && value >= 1`.
- Shortest reproduction: 載入樣本 → 快線 MA 改成 `2.5` → 執行回測 → 得到 0 筆交易
  的「成功」結果，且可存檔／匯出。

### Adjudicated scope (do not re-derive)

**1. Hard-validated set = the 11 indicator-grid fields, and only those.**

| Domain (from `discoveryConfig`) | Keys | Rule |
| --- | --- | --- |
| `period` | `fastMA` `slowMA` `emaPeriod` `rsiPeriod` `macdFast` `macdSlow` `macdSignal` `bbPeriod` | safe integer `>= 1` |
| `level` | `rsiBuy` `rsiSell` | in `[0, 100]` |
| `positive` | `bbMult` | `> 0` |

`checkNumericParam` is the **authority** for all eleven; the new module supplies
only the zh-TW wording and the mount points. A test asserts the two agree on a
value battery for every key, so the rule can never fork.

**2. The five execution-model fields are deliberately excluded.**
`feePct`, `slipPct`, `sizePct`, `slPct`, `tpPct` are owned by
`toExecCostFractions`' documented legacy clamping, which is **already tested**:
`sizePct: 0` means 100% (`services.test.ts:90-92`,
`backtest.golden.test.ts:215`), a negative fee clamps to 0 rather than becoming a
rebate (`services.test.ts:85`), and `slPct <= 0` means "off". Applying
discovery's `percent`/`sizePercent` domains here would contradict those committed
contracts and break existing saved strategies. Their upper bound is already
enforced downstream by the engine's `assertNormalizedFraction`. Changing this
legacy conversion is **not** part of this task.

**3. Cross-field rules are warnings, not errors. — ADJUDICATED AND CLOSED
(maintainer, 2026-08-16, PR #99).** The maintainer accepted warnings-not-fatal
with the reasoning that these three rules constrain *hypothesis quality*, not
computability; a manual strategy may deliberately invert its parameters, and the
affected indicator is not necessarily read by the selected signal. Discovery may
keep pruning them inside its search space, but that must not forbid manual
execution or saving. **Do not re-open this by making them fatal.** Reuse
`candidateValidity` / `DISCOVERY_VALIDITY_RULE_IDS` from
`candidateEnumeration.ts` (`fastMA<slowMA`, `macdFast<macdSlow`,
`rsiBuy<rsiSell`) and render them visibly, but do not block.
Reasons, all evidence-based: (a) none of the three produces `NaN` — they are
computable, merely dubious, hypotheses; (b) the repo's own written judgment is
that these are *pruned as the expected outcome of a legal grid, not rejected as
malformed* (`candidateEnumeration.ts:25-35`); (c) whether `fastMA` is even read
depends on the selected signal; (d) blocking them would silently blank the
`(fastMA=20, slowMA=20)` cell of the default 2-D sweep. **Tightening these to
errors later is a one-line change; loosening them after the fact is not.** If the
maintainer wants them fatal, that is a product decision to record here first.

**4. Library load stays loadable.** `strategyFromDef` keeps its current strict
parsing and does **not** gain the new rules. A row saved before this contract must
still load into the form so the user can see and repair the offending field —
there is no strategy delete in the UI (`PARITY-003` is deferred), so rejecting at
load would strand the row permanently. Run and Save are the boundaries that fail
closed.

### Files likely affected

- new `alpha-factor-forge/src/services/strategyValidation.ts` (pure)
- new `alpha-factor-forge/src/services/strategyValidation.test.ts`
- `alpha-factor-forge/src/services/backtestRunner.ts` (assert at the top of
  `runParamsBacktest` — the single funnel for every manual execution, including
  each sweep variant)
- `alpha-factor-forge/src/services/strategyRecord.ts` (assert before hashing, so
  an invalid strategy never acquires a `strategy-v2` identity)
- `alpha-factor-forge/src/services/discoveryConfig.ts` and
  `alpha-factor-forge/src/services/candidateEnumeration.ts` — **move-only, see
  the dependency-inversion note below**
- `alpha-factor-forge/fixtures/rs-core/{benchmark,runner-config}-v1.json`
  (regenerated; the freshness `sourceHashes` of the three edited reference
  modules change, and nothing else may)
- `alpha-factor-forge/src/components/StrategySection.tsx` (issue/warning display,
  Run disabled, `min`/`step` hints, one new `strategy-section` testid)
- `alpha-factor-forge/src/components/NumberInput.tsx` (accept an optional `step`
  and forward `min`/`max`/`step` to the DOM input)
- new `alpha-factor-forge/e2e/strategy-validation.spec.ts`
- `CHANGELOG.md`, `tasks.md`, this file, and a Resolution appended to
  `handoffs/2026-07-31-pr76-post-merge-audit-v1.md`

**Explicitly NOT in the list, and their absence is an acceptance check:**
`src/core/**` (the indicators keep their current behaviour; this is an admission
gate, not an engine change), `src/services/embargo.ts`,
`src/services/strategyLibrary.ts`, `src/parity/*.ts` (the generators themselves),
`src-tauri/**`, and every existing `e2e/*.spec.ts`.

### Dependency inversion — ACCEPTED (maintainer, 2026-08-16, PR #99)

The maintainer accepted the inversion: moving the shared domain rules into a leaf
module that depends only on `strategy` genuinely breaks the
`discoveryConfig -> randomEntry -> backtestRunner` cycle, the old import paths
stay compatible through the re-exports, the existing discovery tests pass
unmodified, and the two parity fixtures changed only three source hashes. The
description below is retained as the record of why the original "do not touch
discovery" constraint could not hold.

The original plan said `discoveryConfig.ts` and `candidateEnumeration.ts` would
not be touched. **That is not achievable**, and the constraint is structural, not
stylistic: `discoveryConfig` imports `randomEntry`, which imports
`backtestRunner`. A validator that `backtestRunner` imports therefore cannot
import `discoveryConfig` — doing so forms an ESM cycle in which
`discoveryConfig`'s top-level constants are still `undefined` when
`candidateEnumeration` reads them (observed: 26 pre-existing tests failing with
`discoveryConfig.contracts is missing key "gate"`).

The resolution is to **invert the dependency, not to fork the rule**:
`NUMERIC_PARAM_DOMAINS`, `checkNumericParam`, `DISCOVERY_VALIDITY_RULE_IDS`, and
`candidateValidity` move **verbatim** into `strategyValidation.ts`, which imports
nothing but `./strategy`; both discovery modules import what they need and
re-export the public names, so every existing import path and consumer is
unchanged. `NUMERIC_PARAM_DOMAINS` becomes exported (it was module-private) so
`discoveryConfig` can keep its axis-integrality check.

The behaviour-neutrality of the move is **mechanically proven, not asserted**:
`discoveryConfig.test.ts`, `candidateEnumeration.test.ts`, and
`runnerConfigFixture.test.ts` must pass completely unmodified, and regenerating
the two affected parity fixtures must change **only** the `generator.sourceHashes`
entries of the edited modules — every expected value byte-identical.

### Exact implementation plan

1. Add `src/services/strategyValidation.ts`:
   - `STRATEGY_PARAM_RULES_VERSION = 'strategy-params-v1'`.
   - `HARD_VALIDATED_PARAM_KEYS` (the 11) and `LEGACY_CLAMPED_PARAM_KEYS` (the 5),
     both typed as `NumericParamKey`.
   - `validateStrategyParams(strat): { ok, issues, warnings }`, where `issues`
     carry `{ key, value, message }` (zh-TW, naming the key exactly as
     `strategyLibrary.ts` already does) and `warnings` carry
     `{ rule, message }` from `candidateValidity`.
   - `assertStrategyParams(strat)` throws `RangeError` with the first issue.
   - Both are pure and never mutate the strategy.
2. Call `assertStrategyParams` at the top of `runParamsBacktest` and inside
   `buildStrategyDef` before `strategyHash`.
3. `NumberInput` gains an optional `step` passed straight to the input; no other
   behaviour change (clamping stays blur-only).
4. `StrategySection` computes the validation once, disables Run when `!ok`
   (mirroring the existing `codeModeAllowsRun` pattern), renders
   `data-testid="strategy-issues"` and `data-testid="strategy-warnings"`, and
   passes `min`/`step` for the eleven fields.
5. Tests, by name: every hard key rejects 0, -1, 2.5, NaN, Infinity and
   `MAX_SAFE_INTEGER + 1` (levels reject 101 / -1 instead of 0); the defaults pass
   with no issue and no warning; the validator agrees with `checkNumericParam` on
   a value battery for all eleven keys; the hard set ∪ legacy set equals the full
   numeric key set derived from `defaultStrategy()` (so a new field must be
   classified); the five legacy keys still accept `sizePct: 0` and `feePct: -1`;
   the three cross-field rules warn without setting `ok: false`;
   `runParamsBacktest` and `buildStrategyDef` reject `fastMA: 2.5`; and a
   fractional sweep axis now yields **null** cells instead of confident zero-trade
   cells.
6. Add `e2e/strategy-validation.spec.ts`: 快線 MA `2.5` disables Run and shows the
   issue (a value `min` cannot silently repair), restoring `9` re-enables it.

### Non-goals

- Do not change `core/indicators` or `core/backtest`. A wrong period is rejected
  at admission; the engines keep their current semantics.
- Do not change `toExecCostFractions`' legacy clamping or the five execution
  fields it owns.
- Do not change the **behaviour** of `discoveryConfig.ts` or
  `candidateEnumeration.ts`. The dependency inversion above is a verbatim move
  plus re-exports; no rule, message, order, or default may change, and their
  tests must pass unmodified. Do not touch `embargo.ts` at all — it keeps its
  own usage-aware `period()` guard, which is a different contract (it validates
  only the periods a signal actually reads) locked by a parity fixture.
- Do not hand-edit the regenerated fixtures; run their scripts and verify that
  only `sourceHashes` moved.
- Do not add the new rules to `strategyFromDef` (see adjudication 4).
- Do not make the cross-field rules fatal (see adjudication 3).
- Do not touch the sweep artifact, the runner UI, or any Rust file.
- No new dependencies; no `package-lock.json` change.

- **Risk level**: Medium. The rule is small, but it narrows what the app accepts,
  so the realistic failures are (a) hard-validating a field whose legacy clamping
  is a tested contract, (b) forking the rule from `checkNumericParam`, and (c)
  blocking a computable hypothesis that a user legitimately wants. Adjudications
  1–3 and the agreement/classification tests exist to make each visible.
- **Validation plan**:
  - `npm run typecheck`; `npm test`; `npm run build`; `npm run e2e` (local
    Windows: default `workers=1`)
  - No Rust change, so `cargo` is not re-run; state that in the PR.
  - Paste `git diff --stat` and confirm `src/core/`, `embargo.ts`,
    `strategyLibrary.ts`, `src/parity/*.ts`, `src-tauri/`, and the existing
    `e2e/*.spec.ts` are absent from it.
  - Paste the discovery-module diff and the fixture diff, showing the move is
    verbatim and that only `sourceHashes` changed.
  - Report the production bundle size before and after.
- **Acceptance criteria**:
  - [ ] One validator is called by both manual execution (`runParamsBacktest`) and
        persistence (`buildStrategyDef`); neither re-implements the rules.
  - [ ] Zero, negative, fractional, non-finite, and unsafe-integer values are
        rejected for all eight period fields; `rsiBuy`/`rsiSell` reject outside
        0–100; `bbMult` rejects `<= 0`.
  - [ ] The validator and `checkNumericParam` agree for every hard key over the
        value battery, proven by test.
  - [ ] Hard ∪ legacy equals the full numeric key set of `ParamsStrategy`, so a
        future field cannot silently escape classification.
  - [ ] `sizePct: 0` and `feePct: -1` still run under the documented legacy
        clamping.
  - [ ] The three cross-field rules are visible in the UI and do not block.
  - [ ] Run is disabled with a zh-TW explanation while any hard rule fails; the
        `min`/`step` hints are additive and clamping stays blur-only.
  - [ ] A previously saved strategy with an invalid period still loads into the
        form and is repairable.
  - [ ] The discovery move is verbatim: `discoveryConfig.test.ts`,
        `candidateEnumeration.test.ts`, and `runnerConfigFixture.test.ts` pass
        unmodified, and the two regenerated fixtures differ only in
        `generator.sourceHashes`.
  - [ ] No `core/`, `embargo.ts`, `strategyLibrary.ts`, parity-generator, Rust,
        schema, dependency, or existing-e2e change; `CHANGELOG.md` records the
        behaviour change and `tasks.md` shows the task in `Done`.

### Suggested reviewer prompt

```text
Review one PR against STRATEGY-VALIDATION-001 and docs/agent-execution-protocol.md
section 5. Prove with file:line evidence that (1) src/core/, embargo.ts,
strategyLibrary.ts, the src/parity generators and src-tauri/ are absent from the diff, no
existing e2e spec changed, and the discoveryConfig/candidateEnumeration edits are a verbatim
move plus re-exports whose own tests pass unmodified with only fixture sourceHashes moving;
(2) checkNumericParam is the single
authority for all eleven hard-validated keys, proven by an agreement test rather than by a
re-implemented rule; (3) the five execution-model fields are excluded and the tested legacy
clamping (sizePct 0 -> 100%, negative fee -> 0) still holds; (4) hard ∪ legacy covers every
numeric ParamsStrategy field, so a new field cannot escape classification; (5) both mount
points — runParamsBacktest and buildStrategyDef — reject before any work or hashing, and a
sweep variant degrades to a null cell instead of a confident zero-trade cell; (6) the
cross-field rules warn without blocking, matching the recorded adjudication, and the UI copy
is zh-TW with Run disabled on hard failures only; (7) a legacy row with an invalid period
still loads into the form. Return approve / request-changes / escalate in zh-TW.
```

---

## RUNNER-UI-001 — Discovery runner frontend boundary and UI

Expanded to execution-ready format on 2026-08-16, after `STRATEGY-VALIDATION-001`
merged as PR #99 (`cbc3f42`) closed the last correctness gate. Audit evidence:
§11 of `../handoffs/2026-07-31-pr76-post-merge-audit-v1.md` (the accepted-contract
note that this task, not an alias task, owns the stale `events.ts` DTOs).

- **Category**: Feature / contract alignment
- **Objective**: Give the frontend a typed, version-checked boundary to the
  merged backend runner, then a progress/results surface built on it.

**This task is delivered as two slices**, because one session cannot honestly
cover both and a UI PR that also redefines the event contract is not reviewable:

| Slice | Scope |
| --- | --- |
| **RUNNER-UI-001a** | The typed boundary only: real `discovery-event-v1` DTOs, a shared authored contract fixture asserted from BOTH languages, version/shape-checked listeners, the missing `get_active_discovery_run` wrapper, typed progress snapshots, and a cancelable stale-timer-safe throttle. **No UI, no mock, no e2e.** |
| **RUNNER-UI-001b** | The runner panel: start/pause/resume/cancel controls, throttled progress, a results list, recovered-run adoption on mount, the `?mock=1` discovery seam, and browser e2e. Must be expanded to this format before it starts. |

### Evidence (re-derived 2026-08-16 on `cbc3f42`)

- `alpha-factor-forge/src/tauri-client/events.ts:7-22` still declares the
  pre-implementation DTOs: `DiscoveryProgress { tested, total, skipped, current:
  { symbol, interval, segment } }` and `DiscoveryResultEvent { segment, score,
  gatePassed }`. The backend emits neither shape.
- The real contract is `alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs:38`
  (`DISCOVERY_EVENT_VERSION = "discovery-event-v1"`) with
  `DiscoveryProgressEvent` (`82-94`), `DiscoveryResultEvent` (`96-110`), and
  `DiscoveryDoneEvent` (`112-123`), all `rename_all = "camelCase"`. **Not one
  field name in the frontend DTOs exists in them**, and `eventVersion`,
  `sequence`, `counts`, `jobIds`, `strategyHash`, `validationRecordId`, and the
  terminal `status`/`errorMessage` are absent from the frontend entirely.
- Nothing imports `events.ts` today (grep: only its own definitions), so the
  drift was invisible and the rewrite is risk-free — but it also means the first
  UI built on it would have consumed `undefined` for every field.
- `alpha-factor-forge/src/tauri-client/commands.ts:166-172` types
  `discovery.progress` as `invoke<unknown>` and has **no wrapper at all** for
  `get_active_discovery_run` (`src-tauri/src/commands/discovery_commands.rs:82`),
  which is the only way to rediscover a run that startup recovery paused.
- `events.ts:43-61` (`throttle`) cannot be cancelled, so a trailing timer
  outlives an unmount and fires into dead state; it also keeps `pending` forever
  and, if a leading call happens while a trailing timer is still queued (which a
  busy sweep or a delayed timer makes reachable), **delivers the same payload
  twice** — harmless for a progress counter, wrong for an append-only results
  list.

### Slice A — files likely affected

- new `alpha-factor-forge/fixtures/rs-core/discovery-event-v1.json` (**authored,
  not generated**: the producer is Rust and the consumer is TypeScript, so
  neither side may generate the other's expectations)
- new `alpha-factor-forge/src-tauri/src/discovery_runner/event_contract_tests.rs`
- `alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs` (one
  `#[cfg(test)] mod event_contract_tests;` declaration — no runtime change)
- `alpha-factor-forge/src/tauri-client/events.ts` (rewritten)
- new `alpha-factor-forge/src/tauri-client/events.test.ts`
- `alpha-factor-forge/src/tauri-client/commands.ts` (typed discovery wrappers)
- `CHANGELOG.md`, `tasks.md`, this file, and a Resolution appended to
  `handoffs/2026-07-31-pr76-post-merge-audit-v1.md`

**Explicitly NOT in slice A, and their absence is an acceptance check:** any
`src/components/**`, `src/tauri-client/mockClient.ts`, `e2e/**`,
`src-tauri/migrations/**`, every other `src-tauri/src/**` file, and any change to
the emitted payloads themselves.

### Slice A — the fixture is the contract

One authored JSON file holds a sample of every event variant, including the
optional-field permutations that `skip_serializing_if` makes observable:

| Sample | Why it exists |
| --- | --- |
| `progressWithCandidate` | all fields present, including `candidate` + `bestStrategyId` |
| `progressMinimal` | `candidate` and `bestStrategyId` **absent keys**, not nulls |
| `resultGatePassed` | `score` present |
| `resultGateFailed` | `score: null` — `Option<f64>` has no `skip_serializing_if`, so the key IS emitted |
| `doneCompleted` | terminal status + `bestStrategyId`, no `errorMessage` |
| `doneFailed` | `errorMessage` present, `bestStrategyId` absent |

Rust asserts `serde_json::to_value(struct) == sample` for each; TypeScript asserts
its parser accepts each sample and reproduces every field. A field added on
either side fails the other side's test, which is the drift guard that was
missing. The fixture is authored by hand and that is recorded in the file itself;
no script may overwrite it.

### Slice A — exact implementation plan

1. Author the fixture with the exact serde spelling: `camelCase`, `RunStatus`
   lowercase, absent optional keys where `skip_serializing_if` applies, and
   `score: null` where it does not.
2. Add the Rust contract test module and declare it in `mod.rs` behind
   `#[cfg(test)]`. Construct each event struct literally (never by running a
   discovery) and compare against the fixture sample. Also assert
   `DISCOVERY_EVENT_VERSION` and the three channel-name constants.
3. Rewrite `events.ts`:
   - `DISCOVERY_EVENT_VERSION` and the three channel names, mirroring Rust.
   - Interfaces matching the Rust structs field for field.
   - `parseDiscoveryProgressEvent` / `Result` / `Done`: return the typed payload
     or `null`. They must reject a wrong/missing `eventVersion`, a missing
     required key, a wrong runtime type, and a non-finite number, and must treat
     an absent optional key and an explicit `null` as the same "no value".
   - `onDiscoveryProgress` / `Result` / `Done` subscribe, parse, and **drop**
     unparseable payloads instead of handing `undefined` fields to the UI; the
     drop is reported through an injectable `onInvalid` hook so the UI slice can
     surface it rather than swallow it.
4. Replace `throttle` with `createThrottle(fn, ms, { now })` returning
   `{ call, cancel }`: leading call immediate, trailing call scheduled once,
   `pending` cleared on every delivery, `cancel()` clearing the timer and the
   pending payload, and no payload ever delivered twice. Use an injectable clock
   so the tests are deterministic under fake timers.
5. `commands.ts`: type `discovery.progress` and the new
   `discovery.getActiveRun()` as `DiscoveryProgressSnapshot | null`, mirroring
   `DiscoveryProgressSnapshot` field for field (including `version`,
   `currentCandidateIndexes`, `lastEventSequence`).
6. Tests: fixture acceptance, every rejection branch, throttle behaviour
   (leading, coalesced trailing, no double delivery, cancel before and after the
   trailing timer, unmount-safety), and one test asserting the TS channel names
   and version string equal the values the Rust test pins.

### Slice A — non-goals

- Do not change any emitted payload, event name, command name, or command
  signature. This slice makes the frontend match the backend, never the reverse.
- Do not build UI, wire a panel, extend `mockClient`, or add e2e — that is
  slice B, and mixing them would make the contract unreviewable.
- Do not add a Rust runtime dependency; the contract test uses the existing
  `serde_json`.
- Do not generate the fixture from either side.
- Do not touch migrations, other Rust modules, or the discovery engine.

- **Risk level**: Low-medium. The code it replaces is unreachable, so the risk is
  concentrated in getting the authored fixture wrong; the Rust assertion is what
  converts that from a silent future bug into a failing test today.
- **Validation plan**:
  - `cd alpha-factor-forge && npm run typecheck && npm test && npm run build`
  - `npm run e2e` (unchanged suite must stay green even though nothing new is
    exercised)
  - `cd alpha-factor-forge/src-tauri && cargo check --locked && cargo test --locked`
  - Paste the fixture-driven Rust test output; if the authored fixture needed a
    correction, say so and show the corrected assertion passing.
  - Paste `git diff --stat` and confirm `src/components/`, `mockClient.ts`,
    `e2e/`, `migrations/`, and all other `src-tauri/src` files are absent.
- **Slice A acceptance criteria**:
  - [ ] Every field of all three events is represented in TypeScript with the
        same name and type the Rust struct serializes.
  - [ ] Rust and TypeScript both assert the same authored fixture, including the
        absent-optional and `score: null` permutations.
  - [ ] An unknown `eventVersion`, a missing required key, a wrong type, and a
        non-finite number are each rejected by a named test, and the listener
        drops rather than forwards them.
  - [ ] `get_active_discovery_run` has a typed wrapper and `progress` is no
        longer `unknown`.
  - [ ] The throttle can be cancelled, never delivers a payload twice, and has a
        test proving a cancelled trailing timer does not fire.
  - [ ] No UI, mock, e2e, migration, payload, or command-signature change.

---

## RUNNER-UI-001b — Discovery runner panel

Expanded on 2026-08-16, after slice a merged as PR #100 (`c236b1b`). Slice a's
entry above records why `RUNNER-UI-001` is delivered in slices.

**Slice b is itself two PRs.** Starting a run means submitting the whole
`discovery-config-v1` envelope — thirteen exact keys, ten pinned contract
versions, dataset identity, base presets with axes, complete Gate and Score
configs, an explicit seed, caps. Assembling that is a contract-adjacent decision
set; rendering progress is a UI problem. One PR containing both is not
reviewable, and the config decisions must be settled before a panel is built on
them:

| Slice | Scope |
| --- | --- |
| **b-1** | `buildDiscoveryConfig`: assemble the envelope from the workspace's dataset + strategy, validate it with the shared parser BEFORE any invoke, and derive everything that can be derived. Pure; no UI. |
| **b-2** | The panel: axes/seed/embargo inputs, start + pause/resume/cancel, mount-time `getActiveRun()` adoption, throttled progress, a rolling results list, the `?mock=1` discovery seam, and e2e. |

The rich Results Explorer stays the separate Phase B task it already is; b-2's
list is a rolling in-run view, not that screen.

### b-1 — what it settles (adjudicated)

1. **One admission authority.** The builder validates with `parseDiscoveryConfig`
   — the same parser the Rust side mirrors — so a malformed run fails in the
   workspace with a path-qualified message and no run row is ever created. The
   builder adds no rule of its own.
2. **Derive, do not ask.** `contracts`, `gateConfig`, and `scoreConfig` are
   copied from their owning constants, and `benchmarkCosts` is derived from the
   base strategy, because the envelope requires the two to agree and a second
   input could only introduce a mismatch.
3. **`maxConcurrency` is always `null` in v1.** The backend resolves it with ITS
   core count; sending a number validated against the WebView's
   `hardwareConcurrency` could be admitted locally and rejected there.
4. **`rootSeed` and `holdingAllowanceBars` are explicit inputs with no hidden
   defaults.** The seed determines the entire Random Entry distribution, so it
   must be a value the user can see, keep, and re-enter; the allowance is the
   VAL-003 "caller-approved, 0 is explicit" contract. `randomRootSeed()`
   generates one, and the panel must display it.
5. **The strategy is deep-cloned into the envelope**, so a later editor keystroke
   cannot change what a submitted run recorded.

### b-1 — findings recorded while implementing (do not re-derive)

- **An empty axis list is legal**: a base with no axes is a single-candidate run
  (validate this exact strategy end to end). The builder must not invent a
  "needs at least one axis" rule; if the panel wants one, that is a UI product
  decision. Pinned by test.
- **The candidate cap is enforced by `enumerateCandidates`, not by envelope
  admission.** An over-budget grid therefore builds successfully and is rejected
  by the backend — before any candidate or job row exists (RUNNER-CONFIG-001).
  The per-axis value limit, by contrast, IS an envelope rule and fails locally.
  b-2 should surface the projected combination count from the exported
  `axisValues`; it must not add a second cap check.
- `randomRootSeed` clamps to `MAX_U32`: `floor(1 * (MAX_U32 + 1))` is one past
  the admissible range, and an injectable generator can return 1.

### b-2 — files likely affected (expand before starting)

- new `alpha-factor-forge/src/components/DiscoveryPanel.tsx` (+ test ids)
- `alpha-factor-forge/src/App.tsx` or the workspace shell that mounts it
- `alpha-factor-forge/src/tauri-client/dataClient.ts` and `mockClient.ts`
  (the discovery seam: the mock must emit `discovery-event-v1` payloads through
  the same channels, so the panel is exercised against the real contract)
- new `alpha-factor-forge/e2e/discovery-runner.spec.ts`
- `CHANGELOG.md`, `tasks.md`, this file, the audit handoff

### b-2 — required behaviour

- On mount, call `getActiveRun()` first: startup recovery can have left a paused
  run, and the DATABASE is the source of truth for progress. Events are a fast
  path layered on top of that snapshot, never the only source.
- Use `createThrottle` for progress and `cancel()` it on unmount.
- Drop-and-report is already implemented in the listeners: the panel must render
  the reported state ("this view may be stale, re-query") rather than swallow it.
- Use `sequence` to ignore an event older than the snapshot the panel adopted.
- Every control disabled while its transition is in flight; a terminal status
  ends the subscription.
- zh-TW copy; every new control gets a `data-testid`.

- **b-1 validation plan**: `npm run typecheck`, `npm test`, `npm run build`,
  `npm run e2e` (unchanged), and `git diff --stat` showing no `src/components/`,
  `mockClient.ts`, `e2e/`, or `src-tauri/` change. No Rust change, so `cargo` is
  not re-run.
- **b-1 acceptance criteria**:
  - [ ] The built envelope is accepted by `parseDiscoveryConfig` and carries
        exactly the thirteen contract keys.
  - [ ] `contracts` / `gateConfig` / `scoreConfig` are copies of the owning
        constants, and mutating the envelope cannot corrupt them.
  - [ ] `benchmarkCosts` follows the base strategy's costs.
  - [ ] The strategy in the envelope is detached from the caller's object.
  - [ ] An invalid axis, an invalid indicator period, a non-params strategy, an
        out-of-range seed, a negative allowance, and a malformed dataset hash all
        throw BEFORE any invoke, each with a named test.
  - [ ] The empty-axis and over-cap boundaries are pinned as behaviour, not
        assumed away.
  - [ ] `randomRootSeed` is admissible at both extremes of its generator.

### b-2 — expanded to execution-ready format, 2026-08-16 (after b-1 merged as PR #101, `bb0d38b`)

- **Objective**: One workspace panel that starts a discovery run from the live
  dataset + strategy, adopts a run that already exists, and shows its progress,
  results, and lifecycle controls — built entirely on slice a's parsed events and
  b-1's pre-validated envelope.

#### Files likely affected

- new `alpha-factor-forge/src/components/DiscoveryPanel.tsx`
- `alpha-factor-forge/src/components/BacktestPanel.tsx` (mount it below the sweep,
  passing the same `liveContext` those sections already receive — the panel needs
  the dataset identity and the strategy, and both live there)
- `alpha-factor-forge/src/tauri-client/dataClient.ts` (extend the seam with
  `discovery` and the three event subscriptions, so the panel never imports
  `commands` / `events` directly)
- `alpha-factor-forge/src/tauri-client/mockClient.ts` (a DEV-only fake runner)
- new `alpha-factor-forge/e2e/discovery-runner.spec.ts`
- `CHANGELOG.md`, `tasks.md`, this file, the audit handoff

**Explicitly NOT in b-2:** the Results Explorer screen (its own Phase B task),
`src-tauri/**`, migrations, `src/services/**` other than reading b-1's builder,
and any change to the event or config contracts.

#### The mock runner (adjudicated)

The mock **must emit real `discovery-event-v1` wire payloads** — camelCase,
omitted optionals, explicit null `score` on gate failure — through the same
subscription functions. Anything else would exercise the panel against a shape
the backend never sends, which is the exact failure slice a existed to end. The
e2e therefore proves the whole chain including the parsers.

Two DEV-only URL knobs, in the same spirit as the existing `candleDelay` /
`candleFailId` controls and behind the same `import.meta.env.DEV` seam:

| Knob | Purpose |
| --- | --- |
| `discoveryStep=<ms>` | pacing between simulated candidates (default small) |
| `discoveryRun=paused` | pre-create a paused run BEFORE mount, so recovered-run adoption is deterministically observable |

Adoption cannot be tested without the second knob: there is no product path that
creates a run and then reloads the window inside one e2e.

#### Required behaviour (each one is an acceptance criterion)

1. **The database is the source of truth.** On mount, call `getActiveRun()` first
   and adopt its snapshot; events are a fast path layered on top. A "重新查詢"
   control re-reads `progress(runId)` at any time.
2. **A forward-only sequence guard PER STATE SLICE, in a pure reducer.** An event
   whose `sequence` is not newer than what that slice already holds is ignored,
   which is what stops a coalesced progress tick from overwriting a terminal
   status delivered by `done`. One counter for status/counts (progress, done,
   snapshots) and a separate one for the result list — **not** a single counter
   for all three channels, which is what this criterion originally said and what
   the PR #102 review rejected: progress is throttled and results are not, so a
   coalesced progress tick landing after a newer result would look stale and its
   counts would be dropped. The ordering must also live in a pure `(state, event)`
   reducer rather than in React refs; the same review proved a ref re-derived from
   state on every render cannot be monotonic, because a result event advances the
   mutable sequence while the state's stays behind, so the next render writes the
   older value back and a replayed event is accepted twice. Delivered as
   `services/discoveryFeed.ts` — do not reintroduce either rejected shape.
3. **Throttled progress, cancelled on unmount**, using slice a's `createThrottle`.
4. **A dropped payload is surfaced, not swallowed.** The listeners already report
   through `onInvalid`; the panel shows a zh-TW notice that the view may be stale
   and offers the re-query control.
5. **Controls follow status**: start only when no non-terminal run exists and the
   workspace has a dataset with candles; pause only while running; resume only
   while paused; cancel while non-terminal. Every control is disabled while its
   own transition is in flight.
6. **Start builds through b-1** (`buildDiscoveryConfig`), so an invalid axis, seed,
   or strategy fails in the panel with the parser's message and no command is
   called. The seed is generated by `randomRootSeed()` and **displayed**, and the
   projected combination count is shown from the exported `axisValues` (never a
   second cap check).
7. **Results are append-only and deduped by sequence**, newest first, capped to a
   rolling window; a gate-failed candidate shows no score.
8. zh-TW copy; every control and readout carries a `data-testid`.

#### Non-goals

- No multi-axis, multi-base, or preset library: one axis over the live strategy.
  Multi-axis is a later product decision, not a contract limit.
- No Results Explorer, no promotion flow, no Test-segment surface.
- Do not add a second candidate-cap check (b-1 records why).
- Do not change the event contract, the envelope, or any command signature.

- **Validation plan**: typecheck, `npm test`, `npm run build`, `npm run e2e`
  (new spec + all existing specs unmodified). No Rust change, so `cargo` is not
  re-run. Report the bundle delta, since the panel pulls `discoveryConfig` and
  its Gate/Score/RandomEntry dependencies into the UI graph for the first time.

---

## CI-TAURI-SMOKE-001 — native Windows build and startup smoke

Expanded on 2026-08-16, after `RUNNER-UI-001b-2` merged as PR #102 (`1c5d4f0`)
finally gave the runner a real invoke/event caller. Audit evidence: §10 of
`../handoffs/2026-07-31-pr76-post-merge-audit-v1.md` ("CI 只有 cargo check/test 與
Vite mock E2E，沒有 native executable/build/invoke/event smoke").

- **Category**: CI / verification coverage
- **Objective**: Prove in CI that the real desktop binary links, starts on a clean
  machine, renders its window, and completes its startup persistence path.

**Delivered as two slices, because the second needs a dependency decision:**

| Slice | Scope |
| --- | --- |
| **a** | A `windows-latest` lane that builds the binary and smokes its startup: process still alive, SQLite database created from nothing, WebView2 host present. No new dependencies. |
| **b** | A scripted invoke/event round trip through the native bridge. Needs a WebDriver stack (`tauri-driver` + `msedgedriver`) — **a maintainer dependency decision**, so it is not started here. |

### Why the existing lanes are not enough (evidence)

- `.github/workflows/ci.yml` `cargo-check` runs on `windows-latest` but only
  `cargo check --locked` and `cargo test --locked`: neither links the app binary,
  so `tauri.conf.json`, the generated icons, the capability files, and the
  WebView2 dependency are never exercised together.
- The `e2e` lane drives the React tree against the in-memory `?mock=1` client, by
  design (`mockClient.ts` header). It cannot touch Tauri.
- Therefore **no lane had ever started the app**. `main.rs:29-41` does the whole
  persistence startup inside `setup` — `db::initialize` resolves the app-data
  directory, creates the file, applies migrations, and then
  `recover_orphans` repairs interrupted runs — and both calls `.expect()`, so a
  failure aborts the process. Until now that path was only ever verified by hand.

### Slice a — what the lane asserts, and why each assertion is meaningful

| Assertion | What it rules out |
| --- | --- |
| a debug binary exists in `target/debug` after `tauri build --debug --no-bundle` | link failures, a missing/invalid `tauri.conf.json`, missing icons |
| the database did **not** exist before the run | a smoke that passes on a leftover file from an earlier attempt |
| the process is still alive after 25s | either `.expect()` in `setup` firing, and any startup panic |
| `%APPDATA%/com.alphafactorforge.desktop/alphafactorforge.sqlite3` exists and is non-empty | app-data resolution and the file being created at all |
| its `-wal` sidecar exists and is non-empty | migrations having actually run. The main file alone does **not** prove this: `db::initialize` sets `journal_mode=WAL` before applying migrations, so the schema lands in the WAL and the main file stays at one 4096-byte header page — exactly what a lane with zero migrations applied would also show. (Migration failure is still caught, by the liveness assertion above: `apply_migrations` sits behind `.expect()`.) |
| an `msedgewebview2` process exists | "the process is running" without the window ever rendering |

A **debug** build on purpose: it links the same binary and runs the same startup
path as release in a fraction of the time on a Windows runner, and `--no-bundle`
skips installer generation, which this lane never needs.

### Slice a — non-goals

- No scripted invoke or event assertion (that is slice b).
- No new dependency, no `package-lock.json` or `Cargo.lock` change.
- Do not add a release/bundle lane: signing and installer generation are a
  release concern, not a per-PR gate.
- Do not weaken any existing lane to make room for this one.

- **Risk level**: Medium — the risk is CI-only. The realistic failure is
  environmental (a Windows runner that cannot render a WebView2 window), which
  would show up as a red lane rather than as a wrong product claim. If it proves
  flaky, the honest response is to keep the build half and drop the launch half,
  not to make the assertions vacuous.
- **Validation plan**: build the binary locally to confirm the lane's build
  command and executable discovery; the launch assertions are exercised by CI on
  the PR itself, and the PR must state which halves were verified where.
- **Acceptance criteria**:
  - [ ] A `windows-latest` lane builds the real binary on every PR.
  - [ ] The lane fails if the app exits early, if the database or its `-wal`
        sidecar is missing or empty, or if no WebView2 host process appears.
  - [ ] The lane smokes the binary by its expected name rather than whichever
        executable it happens to find first.
  - [ ] The lane refuses to pass on a pre-existing database.
  - [ ] No dependency, lockfile, or product-code change.
  - [ ] The PR states plainly that the scripted invoke/event round trip remains
        slice b, pending the WebDriver dependency decision.
