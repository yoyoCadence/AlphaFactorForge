# Handoff: 全專案審查（架構／測試／UI／功能建議）

Date: 2026-10-13（Asia/Taipei）
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/project-review-2026-10-13`
Reviewed baseline: `77c1bda6ffb57365b2b9003dd89f3a155123a93e`（main，PR #167 已合併）
PR: 由本分支開啟（純文件）。
Status: Review complete — 14 項發現（6 項 P2、8 項 P3；其中 A6 於 2026-08-07 已提出但未被追蹤）。六項快修已排入 `tasks.md` Next，其餘登記 Backlog；八項需維護者決策後才能動工。

## Summary

維護者要求審查整個專案、架構與測試，確認 bug 與可改善處，並檢視 UI／視覺設計與可添加的功能。
既有測試在 main 全數通過；本次發現的問題都是測試未涵蓋的行為，其中互動類問題已用 `?mock=1` 開發伺服器加 Playwright 實測重現。
本 PR 只新增這份紀錄與 `tasks.md` 條目，未修改產品、測試、依賴、CI 或 schema。

維護者已決定（2026-10-13）：本輪執行「記錄＋六項快修」；UI／功能第一波的優先方向是**結果視覺化**，先登記 Backlog，不在本輪實作。

## Scope and method

- 通讀前端：`src/core/*`、`src/services/*`、`src/components/*`、`src/charts/*`、`src/theme/*`、`src/tauri-client/*`、worker。
- Rust 重點抽查：`main.rs`、`commands/*`、`db/mod.rs`、`0001_init.sql`、`discovery_core/dsl.rs`、`discovery_runner/execution.rs`、`runtime/control_api.rs`、`market/http.rs`、`tauri.conf.json`、capabilities、`Cargo.toml`。
- **未逐行重審**：trial ledger、market ingest／snapshot、campaign admission、confirmation／noise simulation。這些區塊已有 #1–#165 的分批驗收紀錄，本次不重複，也不對它們下新結論。
- UI：在 1440×900 以 mock client 實際操作並截圖（紙感工坊、霧面玻璃、早報紙本、粗野派四套皮膚，以及封鎖 Google Fonts 的對照）。探測用的開發伺服器已關閉。
- Playwright E2E（85 個、23 檔）本次未跑完整套；以針對性探測取代。

## Verification

在乾淨 worktree 的 main（`77c1bda`）量得：

| 項目 | 結果 |
| --- | --- |
| `npm run typecheck` | 通過 |
| `npx vitest run` | **1131 / 1131**（70 檔） |
| `cargo test --locked`（`CARGO_TARGET_DIR` 置於 OneDrive 外） | **600 通過、1 ignored**（170 + 428 + 2） |
| `npm audit --omit=dev` | 0 |
| `npm audit`（含 devDependencies） | **1 high**：`source-map-js@1.2.1`（GHSA-68fv-2mgg-jv7q），經 `vite → postcss` 帶入 |

## Findings

級別沿用本 repo 慣例：P2＝穩健性／使用上會出錯，P3＝次要。

| # | 級別 | ID | 問題 | 證據 | 建議修法 |
| --- | --- | --- | --- | --- | --- |
| A1 | P2 | BUG-SWEEP-BLUR-001 | 掃描軸欄位沒改值、只是失焦，整張熱力圖被清空，進行中的掃描也被取消。焦點在軸欄位時點熱力圖格或「套用最佳」，點擊會落空（失焦先卸載了目標） | 實測：格數 16 → 0，欄位值 `5` → `5`。[NumberInput.tsx:56-61](../alpha-factor-forge/src/components/NumberInput.tsx#L56-L61) 失焦一律呼叫 `onChange`；[SweepSection.tsx:409-410](../alpha-factor-forge/src/components/SweepSection.tsx#L409-L410) 每次 `onChange` 都 `clearSweep()` | 失焦時只在提交值與目前 `value` 不同才 `onChange`；越界值仍夾限後送出 |
| A2 | P3 | （併入 A1） | 同根因：策略欄位失焦即移除「✓ 掃描套用」標記，並觸發無謂的 `changeStrategy()` | 實測：標記 1 → 0。[BacktestPanel.tsx:298-301](../alpha-factor-forge/src/components/BacktestPanel.tsx#L298-L301) | 同 A1 |
| A3 | P2 | DSL-WARMUP-NOT-001 | 指標暖機期（NaN）的比較結果為 false，再經 `NOT`／`!` 取反變 true，指標尚未成形就發出訊號。手動 code 模式與 `strategy-dsl-v1` 都受影響；資料短而 lookback 長時會進到 Validation 段 | 實測 `!(rsi > 70)` 在 RSI 為 NaN 的 bar 全為 true。[exprInterpreter.ts:296-299](../alpha-factor-forge/src/services/exprInterpreter.ts#L296-L299)、[evaluator.ts:114-116](../alpha-factor-forge/src/core/strategy-dsl/evaluator.ts#L114-L116)、[dsl.rs:735-741](../alpha-factor-forge/src-tauri/src/discovery_core/dsl.rs#L735-L741)。`docs/strategy-dsl-contract.md` 只規定「含 NaN 的比較為 false」，沒有規定取反 | **需決策**（見下） |
| A4 | P2 | UX-FEEDBACK-001 | 成功／錯誤訊息固定在頁首且不會消失；在頁面下方操作時回饋在視窗外 | 實測：在探索面板啟動後，訊息位於 y = −959 px（視窗高 900）。[BacktestPanel.tsx:463-464](../alpha-factor-forge/src/components/BacktestPanel.tsx#L463-L464) | 訊息區在捲動容器內 sticky 置頂、可關閉、成功訊息自動消失、加 `role` |
| A5 | P2 | UX-LAYOUT-001 | 1440×900 下「執行回測」位於 y = 1555 px；游標在圖表上時滾輪被圖表攔截，頁面不捲動；跑完後圖表在視窗外 | 實測：`scrollTop` 200 → 200，可視視窗起點 100 → 152。[CandleChart.tsx:216-229](../alpha-factor-forge/src/charts/CandleChart.tsx#L216-L229) | 版面重整（見 UI 節）；滾輪行為**需決策** |
| A6 | P2 | FONT-OFFLINE-001 | 打包版 CSP 不允許外部樣式與字型，十套皮膚全部退回系統字，中文落到細明體類；離線時亦同 | CSP 在 [tauri.conf.json](../alpha-factor-forge/src-tauri/tauri.conf.json)；字型網址在 [theme.ts:760-782](../alpha-factor-forge/src/theme/theme.ts#L760-L782)。實測封鎖字型後載入 0 個字面（正常為 153） | **需決策**（字型檔納入 repo） |
| A7 | P2 | APP-STARTUP-ERROR-001 | 啟動失敗時直接 `panic!`。release 是 GUI 子系統，使用者只看到程式打不開，沒有任何訊息 | [main.rs:119-122](../alpha-factor-forge/src-tauri/src/main.rs#L119-L122)；觸發情境包含 schema 較新、擁有者未發布端點、資料庫損毀 | 顯示原生訊息框後以非零碼結束 |
| A8 | P3 | CHART-PRECISION-001 | 價格軸、最新價標籤、「此根資訊」固定 2 位小數，低於 0.01 的價格全部顯示 `0.00` | [CandleChart.tsx:368](../alpha-factor-forge/src/charts/CandleChart.tsx#L368)、[CandleChart.tsx:474](../alpha-factor-forge/src/charts/CandleChart.tsx#L474)、[ChartSection.tsx:307](../alpha-factor-forge/src/components/ChartSection.tsx#L307) | 依價格區間量級決定小數位的單一 helper |
| A9 | P3 | TRADE-REASON-001 | 引擎算出出場原因後丟棄。資料庫有 `reason` 欄、Results Explorer 有「原因」欄，永遠顯示「—」 | [backtest/index.ts:123](../alpha-factor-forge/src/core/backtest/index.ts#L123) 的 `_reason` 未使用；[tradesMapper.ts:19](../alpha-factor-forge/src/services/tradesMapper.ts#L19) 固定寫 `null` | **需決策**（trade 形狀與 fixture） |
| A10 | P3 | REACT-STYLE-WARN-001 | 開發模式 console 出現 React 警告：`border` 簡寫與 `borderColor` 混用 | 探測時出現 2 次。[StrategySection.tsx:339-341](../alpha-factor-forge/src/components/StrategySection.tsx#L339-L341)、[ChartSection.tsx:201-202](../alpha-factor-forge/src/components/ChartSection.tsx#L201-L202) | 改寫完整 `border` 簡寫 |
| A11 | P3 | HELPTIP-SHADOW-001 | 全域按鈕陰影套到 15 px 的「?」圓鈕，粗野派皮膚出現黑色偏移色塊 | 截圖可見。[styles.css:51](../alpha-factor-forge/src/styles.css#L51)、[HelpTip.tsx:67-81](../alpha-factor-forge/src/components/HelpTip.tsx#L67-L81) | 「?」與浮動面板關閉鈕明確設 `boxShadow: 'none'` |
| A12 | P3 | DSL-NAME-FALSEPOS-001 | DSL 可疑字串掃描涵蓋 `name` 欄位，名稱含 window、process、import、document 就被拒 | [validator.ts:39-44](../alpha-factor-forge/src/core/strategy-dsl/validator.ts#L39-L44) 與 [validator.ts:83-89](../alpha-factor-forge/src/core/strategy-dsl/validator.ts#L83-L89)；Rust 同一份清單在 [dsl.rs:55-82](../alpha-factor-forge/src-tauri/src/discovery_core/dsl.rs#L55-L82) | **需決策**（admission 契約） |
| A13 | P3 | METRIC-DISPLAY-001 | 顯示層多處不一致，見下方清單 | 見下 | 僅改顯示層 |
| A14 | P3 | IMPORT-UX-001 | 匯入只接受貼上「物件陣列」JSON，不吃 Binance kline 陣列、字串數字、CSV；`symbol` 未 trim 會回英文內部錯誤；秒級時間戳只回 `timestamp_out_of_range` 原文 | [BacktestPanel.tsx:320-342](../alpha-factor-forge/src/components/BacktestPanel.tsx#L320-L342) | 併入「資料入口」功能 |

A13 的細項：

- 短樣本的年化數字沒有任何提示。600 根 1h 樣本顯示 CAGR 620.61%、Calmar 74.36；seed 的驗證紀錄顯示 CAGR 3479.84%。
- `pct()` 沒有非有限值防護（[MetricsTable.tsx:17-19](../alpha-factor-forge/src/components/MetricsTable.tsx#L17-L19)）。空頭虧損超過 100% 時 `Math.pow(負數, 1/years)` 為 NaN（[metrics/index.ts:99](../alpha-factor-forge/src/core/metrics/index.ts#L99)），畫面會出現 `NaN%`。
- Sortino／Calmar 為無限大時顯示「—」，Profit Factor 卻顯示「∞」。經 pop-out 的 JSON 橋接後無限大會變 null。
- 時間一律是 UTC，但沒有標示。
- Results Explorer 清單直接顯示原始 ISO 時間（[ResultsExplorer.tsx:392](../alpha-factor-forge/src/components/ResultsExplorer.tsx#L392)）；Gate 表所有數值固定 4 位小數，交易數顯示成 `9.0000`。

低風險備註：`Math.max(...wins.map())`（[metrics/index.ts:159-160](../alpha-factor-forge/src/core/metrics/index.ts#L159-L160)）在交易數極大時可能超過參數數量上限；`atr` 的註解寫 Wilder，但實作用 EMA（[indicators/index.ts:119-123](../alpha-factor-forge/src/core/indicators/index.ts#L119-L123)）。

### 重現方式（互動類）

以 `?mock=1&seedHistory=1` 開啟，視窗 1440×900。

1. A1：載入樣本資料 → 展開參數掃描 → 執行掃描，出現 16 格 → 點一下 X 軸的「起」欄位後按 Tab。熱力圖消失，欄位值不變。
2. A2：勾選第二維 → 執行掃描 → 套用最佳 → 策略表單的「快線 MA」出現 ✓ → 點進該欄位後按 Tab。✓ 消失。
3. A4：捲到探索面板 → 啟動探索。「已啟動探索任務」訊息出現在頁首，位於視窗上方之外。
4. A5：把游標放在圖表上滾動滾輪。圖表縮放，頁面不動。
5. A3：直譯器層已實測——RSI 為 NaN 的 bar 上 `rsi > 70` 為 false、`!(rsi > 70)` 為 true；`rsi != 50` 為 false 但 `!(rsi == 50)` 為 true。圖表的「此根資訊」與回測讀的是同一個訊號序列（code 模式輸入 `!(rsi > 70)`，預期第 2–14 根的進場條件顯示成立；這一步的畫面未另外截圖）。

探測腳本要點：`.app-workspace` 才是捲動容器；Playwright 的 `click()` 會捲動它，量測幾何或送滾輪事件前要先明確 `scrollTo`。

## 先前審查提出、但從未進入 tasks.md 的項目

來源是 [2026-08-07 的審查 handoff](2026-08-07-project-review-and-skin-002-v1.md)。`tasks.md` 與 `docs/improvement-backlog.md` 都查不到下列 ID，本次一併補登。

| ID | 本次查核結果 |
| --- | --- |
| FONT-OFFLINE-001 | 未修，即 A6 |
| EXECUTION-TIMING-DECISION-001 | 未決。預設仍是訊號當根收盤成交（[strategy.ts:112](../alpha-factor-forge/src/services/strategy.ts#L112)） |
| METRIC-ANNUALIZATION-001 | 未修。與已在 Backlog 的 INTERVAL-CONTRACT-001 相關：未知 interval 靜默以 365 年化（[backtestRunner.ts:29-33](../alpha-factor-forge/src/services/backtestRunner.ts#L29-L33)），30m、2h、12h、1w 的資料會得到錯誤的 CAGR 與 Sharpe |
| A11Y-SEMANTICS-001 | 未修。`src/components` 與 `src/charts` 內沒有任何元素層的 `onKeyDown` 或 `tabIndex`（只有 HelpTip 與 FloatingPanel 以 document 監聽 Escape）；可點擊的表格列與熱力圖格無法用鍵盤操作；沒有 `role="alert"` |
| TEST-HIDDEN-TEST-001 | Rust 執行器已解決：[execution.rs:886-901](../alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs#L886-L901) 只取到 Validation 結尾。TS 的 `validationRun.ts` 仍以完整陣列計算訊號，但只有 mock 與 fixture 使用 |
| BENCH-RANDOM-CONTRACT-001、PERSIST-RESULT-HISTORY-001、RUNNER-FAILSAFE-001 | **本次未重新驗證**，排程前需先查核現況 |
| favicon | 未修，只影響瀏覽器模式 |

## Review Notes

### 架構

維持現況即可的部分：純函式 core、typed client seam、TS／Rust parity fixtures、版本化契約與 fail-closed、workspace ownership 與事件帳本、loopback 控制 API（只綁 127.0.0.1、常數時間比對 token、拒絕帶 Origin 的請求、請求大小有界）、對外抓取的主機白名單與禁止轉址、CI 的原生 smoke。

| # | 觀察 | 建議 |
| --- | --- | --- |
| C1 | 單頁垂直堆疊七個功能區，全部展開高 3587 px，沒有導覽 | 見 UI 節 E1 |
| C2 | `BacktestPanel` 仍握有 20 個以上的 state／ref，以 generation token 手工防競態 | 以 `useReducer` 收斂工作階段狀態，不新增依賴 |
| C3 | 約 100 處寫死字級（11 px 58 處、12 px 28 處、10 px 8 處、9 px 2 處）繞過 theme token；參數標籤表有 4 份、applied 樣式 helper 有 2 份 | 字級 token 化，抽出共用標籤表 |
| C4 | 大資料路徑：K 線以 JSON 物件陣列過 IPC；`createRunArtifact` 深拷貝並凍結整條 equity 與 trades；單次回測仍在主執行緒（worker 已有 `run` 協定但 UI 未用）；圖表最多顯示 500 根 | 單次回測移入 worker；其餘等資料量需求出現再做 |
| C5 | `candles` 同時有 `UNIQUE(dataset_id, timestamp)` 與同欄位索引（[0001_init.sql:33-35](../alpha-factor-forge/src-tauri/migrations/0001_init.sql#L33-L35)），是重複索引 | 以新 migration 移除重複索引 |
| C6 | UI 狀態不持久化：資料集、策略草稿、Holdout、疊圖、面板展開在重啟後全部重置。`app_settings` 表存在但沒有 typed wrapper | 見功能建議 F-5 |
| C7 | TS／Rust 雙實作讓每次核心變更都要改兩邊並重產 fixture | 維持現況。互動回測改走 Rust 或 WASM 會牴觸 AGENTS.md 現行約束，需另行決策 |
| C8 | `tasks.md` 約 367 KB，Current Snapshot 有 88 條；`improvement-backlog.md` 約 176 KB；handoffs 超過 100 份。上一節的漏接就是後果 | 見決策事項 |
| C9 | `main.rs`、`db/mod.rs`、`Cargo.toml` 的檔頭仍寫 SKELETON 或 `todo!()`；`export_report` stub 仍註冊為 command | 低優先清理 |
| C10 | 沒有頂層 Error Boundary，任何一個 render 例外都會讓整個視窗變空白。例：[CampaignPanel.tsx:54-59](../alpha-factor-forge/src/components/CampaignPanel.tsx#L54-L59) 的 `savedBindings` 在 render 期間 throw | 提升 `docs/improvement-backlog.md` 既有的 UX-002 |

### 測試

| # | 缺口 | 建議 |
| --- | --- | --- |
| D1 | React 元件沒有元件層測試，只靠 85 個 mock E2E，A1 與 A2 因此漏網 | 先以 Playwright 補迴歸；元件測試層需新 devDependency |
| D2 | 沒有 ESLint。原始碼有 7 處 `eslint-disable react-hooks/exhaustive-deps`，但這條規則從未被執行 | 解鎖 TEST-003 |
| D3 | `tsconfig.json` 的 `include` 只有 `src`，`e2e/` 與 `scripts/` 從未被 CI 型別檢查 | 加一份 E2E 用的 tsconfig 與 CI 步驟 |
| D4 | 核心沒有性質測試 | 加「無未來函數」性質測試：用前 k 根算出的訊號，必須等於用全部資料算出的前 k 根 |
| D5 | 打包版的字型、CSP 下的 worker、pop-out 視窗沒有自動化驗證 | 修 A6 後在 native smoke 加字型載入與一次 worker 掃描 |
| D6 | CI 沒有 rustfmt、clippy、cargo audit、toolchain pin | 已在 Backlog：CI-RUSTFMT-001、SEC-RUST-001、TOOLCHAIN-001 |
| D7 | 十套皮膚沒有截圖回歸 | 字型本地化後再加 |
| D8 | 完整 `npm audit` 目前有 1 high（`source-map-js`，僅 devDependency） | 依 `docs/security-audit-npm.md` 手動處理，不可用 `npm audit fix`。這是 SEC-NPM-AUDIT-SCHEDULE-001 所說情況的實例 |

### UI／視覺設計

皮膚系統（token、對比測試、十種風格）是強項。主要問題在資訊架構與結果呈現，不在配色。

- **E1 版面**：左欄表單很長，右欄績效卡下方大片空白，主要操作在兩屏之下。建議改成分頁式工作區（回測｜掃描｜探索｜Campaign｜歷史），回測頁左欄放資料與策略並固定執行列，右側讓圖表與績效同屏。標頭的 `/backtest` 改成真實分頁；`database already initialized at startup` 這類開發用字串改成工作區、宿主、執行中的指示。
- **E2 績效呈現**：12 列「標籤……數值」橫跨約 1000 px，難以掃讀，也沒有任何圖。`result.equity`、`result.trades`、`metrics.monthlyReturns` 都已算好卻沒有畫出來。
- **E3 圖表**：沒有時間軸；十字線只有垂直線，沒有價格與時間標籤；最多 500 根且沒有總覽；進出場標記不分進場、出場與原因；沒有持倉區間底色；Holdout 分界沒有畫在圖上。
- **E4 熱力圖**：單色階看不出跨零指標的正負分界。有號指標建議改用以 0 為中點的發散色階，沿用 `chart.up` 與 `chart.down`。
- **E5 回饋**：長任務沒有進度條，探索只有「完成 x/y」文字；空狀態只有一行字；錯誤常是帶 `Error:` 前綴的英文內部字串。
- **E6 動效**：目前只有按鈕的 hover 與 active。可加面板展開過渡、數值更新淡入、掃描逐格填入、探索進度條、skeleton 載入。全部要尊重 `prefers-reduced-motion`，並符合工作站的定位。
- **E7 一致性**：中英混用（run seed、持倉延伸 bars、snapshot、instrument）；Campaign 直接用 ppm 而沒有說明。

### 功能建議

`docs/creative-feature-roadmap.md` 的 F1–F12 中，F1、F2、F5、F6、F7、F8、F11、F12 尚未實作；F3、F4、F9、F10 只有後端或部分。Legacy 原型已有而 Tauri 版缺少：權益與回撤圖、Buy&Hold 對照、交易表、日期區間回測、範例策略、UI 內下載、ATR 與 Stoch 運算元、策略刪除。

排序依維護者 2026-10-13 的選擇，結果視覺化在最前：

1. **結果視覺化**：F-1 權益曲線與水下回撤圖（含 Holdout 分界、Buy&Hold 疊線）→ KPI 卡片取代長表格 → F-2 回測結果內的交易明細表 → F-3 月報酬熱圖與完整指標。
2. **圖表強化**：時間軸、十字線數值標籤、熱力圖發散色階、Holdout 分界線、持倉區間底色。
3. **版面與導覽**：E1 的分頁式工作區、固定執行列。
4. **資料入口**：F-4 檔案與 CSV 匯入、資料集管理與健康徽章；F-11 把既有的 Binance 後端 adapter 接到 UI（PARITY-002）。
5. **反過擬合工具**：F-6 過擬合風險燈號、F-7 掃描穩健度（鄰域平均、最少交易數門檻）、F-8 session 實驗日誌、F-9 策略對決、F-10 Monte Carlo 交易順序重抽。
6. **其他**：F-5 UI 狀態持久化、F-12 探索 UI 補強（後端的 `axes` 已是陣列，UI 只開放一軸）、報告圖卡匯出、快捷鍵、指標擴充、診斷頁。

不建議的項目與專案既有的清單一致：實盤下單、警示與 webhook、雲端同步、插件市集、手機版、i18n、排行榜、多資產組合。

## Required Action / Decision

### 已排入 Next 的六項快修（各一個 PR，依序，前一個合併後才開下一個）

1. BUG-SWEEP-BLUR-001（A1、A2）
2. REACT-STYLE-WARN-001 與 HELPTIP-SHADOW-001（A10、A11）
3. CHART-PRECISION-001（A8）
4. METRIC-DISPLAY-001（A13）
5. UX-FEEDBACK-001（A4）。訊息文字不可更動，多個 E2E 以 `getByText` 鎖定
6. APP-STARTUP-ERROR-001（A7）

### 需維護者決定後才能動工

| 事項 | 為何需要決定 |
| --- | --- |
| DSL-WARMUP-NOT-001 的修法 | 改成三值邏輯會改變訊號語意，需要契約版本與 TS／Rust fixture 重產。應在 P15 讓 AI 產生 DSL 之前完成 |
| TRADE-REASON-001 | 在 trade 形狀加 `reason` 會動到 parity fixture 與不可變紀錄的內容 |
| DSL-NAME-FALSEPOS-001 | 改變 `strategy-dsl-v1` 的 admission 行為 |
| FONT-OFFLINE-001 | 需把字型檔納入 repo。九種中文字體體積大，建議縮到二至三種並做子集 |
| 圖表滾輪行為 | 改成 Ctrl＋滾輪可解決捲動被攔截，但 `zoom.spec.ts` 與 `pan.spec.ts` 鎖定現行行為 |
| 預設成交價、未知 interval 的處理 | 影響既有結果的可比性，屬版本化的產品決策 |
| 新 devDependency（ESLint、元件測試） | Open Question Q3 尚未核准 |
| `tasks.md` 歸檔瘦身 | AGENTS.md §5 規定不得擅自重組大段內容。建議把 Done 與歷史快照移到 `docs/history/`，並規定 review handoff 的 follow-up 必須在同一個 PR 進 Backlog |

### Backlog 條目對照

`tasks.md` 的「Project review follow-ups (2026-10-13)」為下列主題各立了一個 ID：

| Backlog ID | 對應本文 |
| --- | --- |
| FEAT-RESULT-VIZ-001 | 功能建議 1、UI 節 E2 |
| FEAT-CHART-AXES-001 | 功能建議 2、UI 節 E3 與 E4 |
| FEAT-DATA-ENTRY-001 | 功能建議 4、A14 |
| FEAT-ROBUSTNESS-001 | 功能建議 5 |
| FEAT-UI-STATE-001 | 架構 C6 |
| FEAT-DISCOVERY-UI-001 | 功能建議 6、UI 節 E5 |
| UX-002 | 架構 C10 |
| FAVICON-001 | 先前審查的 favicon 項 |
| TEST-E2E-TYPECHECK-001 | 測試 D3 |
| TEST-NO-LOOKAHEAD-001 | 測試 D4 |
| DB-CANDLE-INDEX-001 | 架構 C5 |
| SEC-NPM-DEV-001 | 測試 D8 |
| DOC-TASKS-ARCHIVE-001 | 架構 C8 |

## Resolution

Pending。六項快修各自完成時，在此追加 PR、最終 head 與驗證結果；原始內容保留。

本紀錄的 PR 是 [#168](https://github.com/yoyoCadence/AlphaFactorForge/pull/168)，2026-10-13 合併為 `0741726`，CI 六項通過。

### 2026-10-14 — BUG-SWEEP-BLUR-001（A1、A2）

- 分支 `fix/number-input-blur-commit`，由合併後的 main（`0741726`）建立；PR 由該分支開啟。
- 修改：[NumberInput.tsx](../alpha-factor-forge/src/components/NumberInput.tsx) 失焦時先算出夾限後的值（抽成純函式 `blurCommitValue`），只有與目前 `value` 不同才呼叫 `onChange`。輸入中的即時回報與越界值的夾限都不變；8 個使用處沒有修改。
- 測試：`NumberInput.test.ts` 8 個案例（含 MIN／MAX ± 1、清空、無法解析的草稿）；`e2e/sweep.spec.ts` 新增兩個迴歸——未改值失焦後熱力圖與 ✓ 標記保留、焦點在欄位內時點格子仍會套用、真的改值仍會清空。
- 反向驗證：把元件還原成修改前的版本，兩個新 E2E 都失敗（格數 16 → 0；✓ 標記消失）；套回修正後通過。
- 驗證：Vitest 1139／1139、Playwright 87／87、`npm run typecheck`、`npm run build`。未動 Rust。

### 2026-10-14 — REACT-STYLE-WARN-001 ＋ HELPTIP-SHADOW-001（A10、A11）

- 上一項 BUG-SWEEP-BLUR-001 的 PR 是 [#169](https://github.com/yoyoCadence/AlphaFactorForge/pull/169)，2026-10-14 合併為 `43eea86`，CI 六項通過。
- 維護者 2026-10-14 授權：本審查的快修 PR 在 CI 全數通過後可由 agent 自行合併。
- 分支 `fix/style-shorthand-and-icon-shadow`，由 `43eea86` 建立；PR 由該分支開啟。
- 修改：套用標記、無效參數、策略模式按鈕、程式碼編輯器錯誤狀態改寫完整的 `border` 簡寫，不再只覆寫 `borderColor`；Results Explorer 分頁改用四個方向的 longhand。「?」說明鈕、浮動面板關閉鈕、圖表「重置」鈕明確設為無陰影。畫面外觀不變。
- 測試：新增 `e2e/style-warnings.spec.ts`。一個走過上述每個狀態切換並斷言 console 沒有樣式衝突警告；另一個在粗野派皮膚檢查三個小按鈕的 computed `box-shadow`。兩個測試在修改前的程式上都會失敗。
- 驗證：Vitest 1139／1139、Playwright 89／89、`npm run typecheck`、`npm run build`。未動 Rust。

### 2026-10-14 — CHART-PRECISION-001（A8）

- 上一項的 PR 是 [#170](https://github.com/yoyoCadence/AlphaFactorForge/pull/170)，CI 六項通過後依授權合併為 `db2cecc`。
- 分支 `fix/chart-price-precision`，由 `db2cecc` 建立；PR 由該分支開啟。
- 修改：`charts/scale.ts` 新增 `priceDecimals`（取「最大值的四位有效數字」與「比四分之一區間再細一位」兩者較大者，限制在 2–8 位）與 `priceGutterWidth`（八個字元以內維持原本 54px）。價格軸、最新價標籤與「此根資訊」共用。平盤區間改以數值的 1% 擴展，不再固定 ±1。
- 測試：`scale.test.ts` 新增 11 個案例，含 10、1、0.1、0.01 等十的次方邊界；期望值是實際執行函式後，再對照規則逐一核對。既有的平盤擴展測試隨行為更新。
- 實測：在 mock UI 匯入約 0.0000123 的資料集，價格軸與「此根資訊」顯示 8 位小數（例：`0.00001264`），標籤完整落在加寬的刻度欄內；原本全部是 `0.00`。
- 驗證：Vitest 1150／1150、Playwright 89／89、`npm run typecheck`、`npm run build`。未動 Rust。

### 2026-10-14 — METRIC-DISPLAY-001（A13）

- 上一項的 PR 是 [#171](https://github.com/yoyoCadence/AlphaFactorForge/pull/171)，CI 六項通過後依授權合併為 `384ba8d`。
- 分支 `fix/metric-display`，由 `384ba8d` 建立；PR 由該分支開啟。只改顯示層，沒有改任何指標、紀錄、契約或 fixture。
- 修改：
  - 績效表：無限大的比率在每一列都顯示「∞」，任何列都不會出現 `NaN%`。
  - 原生績效 pop-out：每一欄先經共用的 `encodeMetrics` 編碼、到達後還原，無限大不再在 JSON 中變成 null。解碼放在 `tauri-client/windowBridge.ts`，因為 `metricsCodec.ts` 是 Gate／Score parity fixture 以雜湊釘住的參考原始碼。
  - 結果涵蓋不到一年時，績效表下方加註年化數字是由短樣本外推（`services/sampleSpan.ts`）。
  - Results Explorer 與 Research History：時間標示 UTC；`created_at` 等時間戳統一格式；`gate-v1` 的 Gate 值依單位顯示（次數、百分比、百分位），其他版本維持四位小數；低價幣的交易價格給足小數位。
- 測試：一年邊界的樣本期間、績效列格式、時間戳與 Gate 格式、橋接經 JSON 往返的單元測試；新增 `e2e/metric-display.spec.ts`。
- 驗證：Vitest 1170／1170、Playwright 91／91、`npm run typecheck`、`npm run build`。
- 未做：原生 pop-out 視窗沒有實際開啟檢查，僅有橋接編解碼的單元測試。

### 2026-10-14 — UX-FEEDBACK-001（A4）

- 上一項的 PR 是 [#172](https://github.com/yoyoCadence/AlphaFactorForge/pull/172)，CI 六項通過後依授權合併為 `a3554f8`。
- 分支 `fix/status-message-visibility`，由 `a3554f8` 建立；PR 由該分支開啟。
- 修改：`BacktestPanel.tsx` 的兩則訊息改為固定在捲動工作區頂端（`position: sticky`，底下墊不透明色帶，避免半透明皮膚透字），各加關閉鈕與 `role`（錯誤為 `alert`、一般訊息為 `status`）。一般訊息 8 秒後自動消失，錯誤保留到被關閉或取代。訊息文字一字未改。
- 測試：新增 `e2e/status-messages.spec.ts`——在頁面下方啟動探索後訊息仍在視窗內並可關閉；一般訊息會自動消失而同時存在的錯誤保留。把固定定位拿掉時，視窗內斷言失敗（viewport ratio 0）。
- 驗證：Vitest 1170／1170、Playwright 93／93、`npm run typecheck`、`npm run build`。

### 2026-10-14 — APP-STARTUP-ERROR-001（A7）

- 上一項的 PR 是 [#173](https://github.com/yoyoCadence/AlphaFactorForge/pull/173)，CI 六項通過後依授權合併為 `816bf94`。
- 分支 `fix/startup-error-dialog`，由 `816bf94` 建立；PR 由該分支開啟。六項快修中的最後一項。
- 修改：新增 `src-tauri/src/desktop/startup_error.rs`。`main.rs` 中工作區開啟失敗的 `panic!`、取得資料資料夾的 `expect`、`Builder::run` 的 `expect` 改為呼叫 `fail`：先寫到 stderr；release 版在 Windows 以 `MessageBoxW` 顯示依原因分類的建議、技術原因與工作區路徑；行程以代碼 1 結束。debug 版（native smoke 使用）保有主控台並立即結束，自動化流程不會被對話框卡住。`Cargo.toml` 只為已鎖定的 `windows-sys` 多加一個 feature，`Cargo.lock` 不變。
- 測試：6 個單元測試涵蓋各種失敗原因的訊息。
- 驗證：`cargo test --locked` 606 通過、1 ignored；`cargo check --locked --all-targets`；`cargo check --locked --release`。
- 未做：對話框只在 release 建置出現，尚未實際開啟確認。

### 快修完成狀態（2026-10-14）

Wave 1 的六項快修全部完成，依序為 #169、#170、#171、#172、#173 與本項。其餘項目仍在 `tasks.md` 的「Project review follow-ups (2026-10-13)」，其中標記 *decision* 的需維護者先決定。
