# Handoff: 全專案錯誤 / Bug / 測試缺口審查（僅記錄，未修改程式）

Date: 2026-09-25
Repo: yoyoCadence/AlphaFactorForge
Branch: claude/project-review-bugs-tests-3p4w8h（基底 `main` @ `f747131`，PR #123 合併後）
PR: 無
Status: open — 待使用者逐項確認後再排入 `tasks.md` 實作；本次**沒有改動任何程式碼或測試**

## Summary

依使用者要求，對 `alpha-factor-forge/`（TS core / services / components / tauri-client、Rust
discovery_core / runner / runtime / commands）做一次審查，只記錄不修改。先跑驗證建立基準，再讀
高風險路徑，疑似 bug 盡量用一次性腳本實測（腳本放在 git 忽略的目錄，跑完即刪，repo 內沒有殘留檔案）。

結論：現有測試全綠，核心回測的防洩漏設計（nextOpen 延遲成交、holdout / 驗證分段、Test 不參與排名）
整體紮實。但有 **4 項 P1**：3 項會實際扭曲研究結論或違反架構契約，1 項是 UI 卡頓風險；
另有數項 P2 的正確性 / 可用性問題，以及幾個值得補上的測試缺口。

## Baseline verification（本次實測）

| 檢查 | 結果 |
| --- | --- |
| `npm ci` | ok |
| `npm run typecheck` | ok |
| `npm test`（Vitest） | **975 passed**（54 files） |
| `cargo test --locked`（Linux，需先裝 webkit2gtk/gtk 系統庫） | **445 passed**（94 + 349 + 2），`tasks.md` 記的是 444（348） |
| `cargo clippy --locked --all-targets` | 見文末附錄 |
| Playwright e2e（`?mock=1`） | 見文末附錄 |

## Findings

嚴重度：**P1** = 會產生錯誤的研究結論或違反架構契約；**P2** = 邊界情況的錯誤結果、UX 缺陷或
潛在風險；**P3** = 文件 / 衛生問題。「已實測」= 用腳本重現過；「推論」= 讀程式得出，需要再確認。

### P1-1 Runner 年化一律用加密貨幣曆法，ETF / 股票日線被高估（推論，需確認）

- 位置：`src-tauri/src/discovery_runner/execution.rs:310` → `benchmarks::bars_per_year(interval)`
  （`src-tauri/src/discovery_core/benchmarks.rs:107`，`"1d" => 365`）；TS 同樣在
  `src/services/backtestRunner.ts` 的 `BARS_PER_YEAR`。
- 問題：P08 已經定義 `sessionsPerYear`（例如 Tiingo 用 252，見 `docs/market-source-tiingo-v1.md:55`），
  也有 `etf_metrics.rs`，但 runner 從沒用到。P09 / P10 的 ingest 會產生 `datasets` 資料列，並透過
  `market_snapshots.dataset_id` 綁定，runner 則接受任何已驗證的 dataset。
- 影響：同一條權益曲線，Sharpe / Sortino 會被放大 √(365/252) ≈ **1.20 倍**；`years = bars/365` 低估
  持有年數，CAGR / Calmar 會偏離。這些數字會流進 gate、score 和排名，也就是會系統性偏好 ETF 候選。
- 需確認：runner 目前是否真的會吃到 ETF dataset（UI、指令路徑是否已開放）。如果還沒開放，這就是
  P12d / P16 之前必須補上的前置條件。
- 建議：年化因子改從 dataset 綁定的 snapshot → instrument / calendar 取得；沒有 snapshot 的舊
  dataset 維持 365（legacy），並把年化因子寫進 engine fingerprint。
- 測試：一個綁定 `sessionsPerYear=252` 的 dataset 跑 runner，斷言 Sharpe 用的是 √252。

### P1-2 空單虧損超過 100% 時帳戶變負值，CAGR / Calmar 失真（已實測）

- 位置：`src/core/backtest/index.ts:131-136`（Rust 鏡像：`discovery_core/backtest.rs`），以及
  `src/core/metrics/index.ts:96-99`。
- 重現：價格 100 → 250 → 300，`direction:'short'`、`sizingPct:1`，結果期末權益 **-10000**、
  `netReturn = -2`（-200%）、`maxDrawdown = 2`，但 **`cagr = 0`、`calmar = 0`**。
  `Math.pow(-1, 1/years)` 在這組 bar 數恰好是偶數次方而得到 1；其他 bar 數會是 **NaN**，Rust
  `powf` 也一樣。
- 問題：「1x 無槓桿空單」沒有強制平倉 / 破產規則，所以帳戶可以變成負數。權益 ≤ 0 之後，
  `rets` 會跳過這段報酬，Sharpe 也跟著失真。NaN 還可能經 serde 序列化成 `null`，碰到 DB CHECK。
- 建議（屬於契約決策）：擇一。(a) 權益 ≤ 0 時強制平倉並標記 ruin，之後不再開倉；
  (b) `end <= 0` 時把 CAGR 定義成 -1 並加上明確狀態。TS / Rust 要一起改，fixture 升版。
- 測試：TS 與 Rust 各加一個空單爆倉案例，斷言指標都是有限值、與契約一致，並加入 parity fixture。

### P1-3 connect 模式下，桌面端仍直接寫入 SQLite（第二個 writer）（推論，需設計決策）

- 位置：`src-tauri/src/runtime/connect.rs:157`（以讀寫模式開 `db::open_migrated`）、
  `src-tauri/src/runtime/host.rs:71`（`HostMode::Connected` 也回傳 db），以及
  `src-tauri/src/commands/db_commands.rs` 的 `import_candles` / `save_strategy` /
  `save_backtest_result` / `save_validation_record`。
- 問題：P04b 契約是 "never two writers"，runner / ledger 的寫入都在 ownership epoch 檢查的交易裡。
  但背景 service 持有 lease 時，桌面端這 4 個寫入指令完全沒有 mode 檢查，會直接寫進同一個 DB：
  不經 epoch 驗證，不 bump `stateVersion`，也沒有 admission guard。目前只有 `run_migrations`
  會在 connect 模式拒絕。
- 影響：寫入在 SQLite 層有 busy timeout 保護，但違反所有權契約；service 的 snapshot / event 也不知道
  這些寫入。trial ledger 的 backfill 讀的是 `datasets`，一致性假設可能被破壞。
- 建議：擇一。(a) connect 模式把這些寫入改走 control API proxy；(b) 明確在契約記載「手動結果寫入
  不受 lease 約束」，並補上理由與測試。
- 測試：`discovery_runner/tests/host.rs` 加一個案例：connect 模式呼叫寫入指令，斷言它被拒絕，
  或被正確代理。

### P1-4 同步 Tauri 指令在主執行緒執行，而且和 runner 共用 DB mutex（推論，需量測）

- 位置：`src-tauri/src/commands/db_commands.rs` 全部是 `pub fn`（非 `async`）。Tauri 2 的非 async
  指令預設在主執行緒執行。
- 影響：大型 `get_candles`（例如 1m 一年約 52 萬根）、`import_candles`（JSON 反序列化 + 單一交易）
  會阻塞主執行緒，WebView 無回應。runner 的 claim / commit 也會取同一把 `db` mutex
  （`discovery_runner/mod.rs:1091, 1299`），探索進行中 UI 指令必須排隊。這正對到 AGENTS.md 高風險
  清單的「long-running jobs staying off the UI thread」。
- 建議：DB 指令改成 `#[tauri::command(async)]` 或 `async fn` + `spawn_blocking`；`get_candles`
  考慮分頁。
- 測試：很難用單元測試量，建議在 native-smoke lane 加入匯入 50 萬根的耗時與回應性檢查，或先手動量測。

### P2-1 Web Worker 是死碼，互動回測 / 掃描 / holdout 都跑在 UI 執行緒

- 位置：`src/workers/backtest.worker.ts` 從未被 `new Worker(...)` 使用（全 repo 搜尋無結果）。
  `BacktestPanel.run()` 同步呼叫 3 次 `runParamsBacktest`（full + in / out sample）；`SweepSection`
  只 `setTimeout(20)` 讓出一次，接著同步跑最多 256 組。
- 影響：資料量大時 UI 凍結，和 AGENTS.md 的架構描述不一致。
- 建議：接上 worker（協定已經有了），或者從文件中移除 worker 的角色。二選一，不要讓兩者繼續不一致。

### P2-2 DSL `SUSPICIOUS` 子字串過濾誤判正常策略名稱（已實測）

- 位置：`src/core/strategy-dsl/validator.ts:39-44, 84-89`；Rust 鏡像 `discovery_core/dsl.rs:55`。
- 重現：`name` 設成 `"Important breakout"`（含 `import`）、`"Window range"`、
  `"Trend following process"`、`"Documented MA"`，全部被拒。
- 問題：過濾對整個 JSON 做子字串比對，但 DSL 本身是白名單結構、不會執行字串，所以這個過濾只帶來
  誤判，幾乎沒有安全價值。之後 AI 產生的名稱很容易踩到。
- 建議：`name` 改成限制字元集或長度，token 掃描排除 `name`，或改成只比對完整單字。TS / Rust 要一起改，
  fixture 升版。

### P2-3 DSL `ParamSpec` 的 min/max 目前沒有語意，lookback / embargo 只看 default（潛在風險）

- 位置：`validator.ts:134`（`params.set(name, spec.default)`）、`evaluator.ts:30-35`、
  Rust `dsl.rs:588-601`，以及 `execution.rs:344`（embargo 用 `max_lookback_bars`）。
- 問題：現在只執行 fixed DSL，所以正確。但只要之後加入「DSL 參數掃描」，而某個取值的 `len`
  大於 default，embargo 就會被低估，造成跨段污染；`float` 型別的參數也可能產生非整數週期。
- 建議：在契約和程式註解寫明「min/max 保留、未啟用」。啟用掃描時，每個取值點都要重新 validate，
  並且用 max 計算 lookback。
- 測試：一個守衛測試，斷言 `maxLookbackBars` 以 default 計算。之後語意改變時，這個測試會提醒更新 embargo。

### P2-4 `runBacktest` 輸入驗證缺口，錯誤不會拋出，而是回傳「看起來正常」的結果（已實測）

- 位置：`src/core/backtest/index.ts:107-111`（Rust 鏡像同）。
- 重現：`startEquity: -5` 回傳 0 筆交易、`netReturn 0`；`from:3, to:1` 回傳空權益、所有指標為 0；
  `signals.entry` 長度比 candles 短也沒有被檢查（缺的部分當作 false）。
- 建議：`startEquity` 必須是有限正數，`from ≤ to`，而且落在範圍內，`signals` 長度必須等於
  candles 長度；不符合就拋 `RangeError`，與既有的 `validateNormalizedFractions` 同一風格。

### P2-5 ATR 註解寫 Wilder，實作卻是 EMA(2/(n+1))

- 位置：`src/core/indicators/index.ts:119-123`（Rust 同）。
- 影響：數值和 TradingView / Wilder ATR（RMA，α=1/n）不同。DSL 白名單包含 ATR，使用者或 AI 會預期
  標準定義。
- 建議：屬於契約決策。改成 RMA 就要讓 indicator fixture 升版；不改的話，至少修正註解並寫進 DSL 文件。

### P2-6 `sma` / `ema` 的累加和遇到 NaN 會永久污染（已實測）

- 位置：`src/core/indicators/index.ts:13-39`。
- 重現：`sma([1,2,NaN,4,5,6,7],2)` 從第 3 根之後**全部是 NaN**。
- 影響：輸入都經過 market-data quality 檢查，目前 DSL 也只吃價格來源，所以低風險。但未來如果允許
  「指標套指標」，或讀到舊資料，結果會靜默變成 0 筆交易。
- 建議：要嘛在文件寫明「輸入必須全為有限值」並在入口 assert，要嘛改成視窗重算。

### P2-7 `ChartPopoutWindow` 的監聽器在部分失敗時洩漏

- 位置：`src/components/ChartPopoutWindow.tsx:27-45`。
- 問題：`Promise.all` 裡如果一個 `listen` 成功、另一個失敗，成功的那個 unlisten 不會被保存，
  也不會被呼叫。`DiscoveryPanel` 已經用逐一註冊、保存的正確寫法，這裡可以比照。

### P2-8 JSON 匯入的 interval 是自由字串，未知值會靜默用 365 年化

- 位置：`src/components/BacktestPanel.tsx:304`、`backtestRunner.ts:30-34`。
- 備註：已有 `tasks.md` 的 **INTERVAL-CONTRACT-001**，這裡不另開任務；只是提醒例如 `"1H"`、
  `"60m"` 會被當成日線年化，Sharpe 誤差可達數十倍。建議提高這個任務的優先級。

### P2-9 小問題

- `save_report` 的副檔名檢查區分大小寫（`report.JSON` 會被拒），見 `commands/file_commands.rs:59`。
- Tauri 2 的 app 自訂指令預設所有視窗都能 invoke，所以 pop-out 視窗也能呼叫
  `import_candles` / `start_discovery`。內容是同源、可信的，風險低；如果要落實最小權限，可以用
  `AppManifest` 限制。
- `secret_commands` / `ai_commands` 仍然是 `NotImplemented`（Phase C 預期如此）。需確認 UI 沒有入口。

### P3 文件 / 衛生

- `tasks.md` Current Snapshot 的 Rust 數量是 444（94 + 348 + 2），實測 445（94 + **349** + 2）。
- `AGENTS.md` §0 寫 "Rust 1.77+"，`Cargo.toml` 是 `rust-version = "1.89"`。
- `Cargo.toml` 仍有 `# SKELETON - compilable scaffold` 註解，已經過時。
- CI 的 `cargo` 只在 windows-latest 跑。Linux 本地 / 雲端 session 需要 `libwebkit2gtk-4.1-dev`、
  `libgtk-3-dev` 等系統庫（本次靠 apt 安裝才能編譯），README 沒有寫。可以考慮加一個 SessionStart hook。

## Test gaps（建議補的測試）

| # | 缺口 | 為什麼重要 | 建議做法 |
| --- | --- | --- | --- |
| T1 | **invoke 參數名稱契約測試**不存在 | AGENTS.md 把「Tauri invoke argument naming」列為高風險；目前 mockClient 接受任何 key，e2e 抓不到命名錯誤 | Rust 或 vitest 解析 `#[tauri::command]` 簽名（snake → camel），與 `commands.ts` 的 `invoke(name, {keys})` 比對 |
| T2 | **通用因果性（no-lookahead）測試**：只有 `backtest.fills.test.ts:46` 一個 nextOpen 案例 | 防未來資料洩漏是第一風險 | 對 indicators、`buildSignals`、`evaluateDSL`、`runBacktest` 做「擾動第 k 根之後的資料，≤k 的輸出完全不變」的性質測試，並用多組隨機種子 |
| T3 | 空單爆倉 / 權益 ≤ 0 | P1-2 | TS + Rust + parity fixture |
| T4 | 回測帳務守恆性質測試 | 保障 cash + 部位市值 = equity，以及手續費非負等不變量 | 隨機訊號 × 隨機 K 線的性質測試 |
| T5 | DSL validator 的 TS↔Rust 差分模糊測試 | 目前只有固定 fixture | 隨機產生合法 / 非法樹，比較兩邊的 `ok`、`errors`、`maxLookbackBars` |
| T6 | DSL 名稱誤判 | P2-2 | 修正後加入正反例 |
| T7 | connect 模式寫入 | P1-3 | `tests/host.rs` |
| T8 | ETF 年化 | P1-1 | runner 整合測試 |
| T9 | `runBacktest` 非法輸入 | P2-4 | `backtest.contract.test.ts` |
| T10 | React 元件單元測試只有 `MetricsTable` | 世代 token / 過期結果守衛完全依賴 e2e | 可以用 vitest + jsdom 測 `BacktestPanel` 的 run / load 競態；若維持只靠 e2e，要在文件寫明理由 |

## Recommended order

1. **先決策**，再動手：P1-2（爆倉語意）、P1-3（connect 模式寫入）、P2-5（ATR 定義）。
   這三項都會改契約或 fixture，應各自成為獨立 PR。
2. **低成本、高價值的測試**：T1、T2、T9。純新增，不改行為，可以先做，也能幫忙找出其他問題。
3. P1-1（年化）排在 P12d / P16 開放 ETF 研究之前。
4. P1-4 / P2-1（執行緒）：先量測，再決定。
5. P2-2、P2-6、P2-7、P2-9、P3：小修，可以合併成一個 hygiene PR。

## Appendix: clippy / e2e

- `cargo clippy --locked --all-targets`：exit 0，共 5 個既存 warning（lib 4 個 + bin test 1 個），和
  `tasks.md` 記載的「five pre-existing warnings」一致：clamp-like pattern、手寫
  `!RangeInclusive::contains`、兩處 `map_or` 可簡化、`&PathBuf` 應改 `&Path`。
- Playwright e2e（`npx playwright test`，chromium，`?mock=1`）：exit 0，全部通過。
  輸出被截斷，沒有保留總數那一行，但 Playwright 只要有失敗就會回傳非 0。

## Addendum (2026-09-25, same session) — superseded by v2

The user asked for a full-depth review, so a per-file deep-reading version has been written: `handoffs/2026-09-25-project-review-bugs-tests-v2.md`.
The body of this v1 is kept as-is (append-only); the corrections are:

- **P1-2 (short-position ruin)** → downgraded to P2 in v2 (B1). The contract already documents the short as a "1× no-liquidation" model, and in discovery MDD > 0.35 is rejected by the Gate; what remains is a metric inconsistency.
- **P1-4 (main-thread blocking)** is already tracked as **DB-ASYNC-001**; v2 (A5) adds new evidence (sync commands + HTTP proxied to the service, holding the DB lock across the whole dataset load at run start, artifact IO under the DB lock).
- **P2-1 (Web Worker unused)** is already tracked as **PERF-001**.
- **P2-5 (ATR)**: the Rust comment already states this is a deliberate contract choice; only the TS comment is misleading → P3.
- **P2-8** is already tracked as INTERVAL-CONTRACT-001, and **P3 Rust version drift** as TOOLCHAIN-001.
- v1 missed several items, which v2 adds: A2 zero-trade candidates making the whole run fail + the default Gate being impossible to pass on the bundled sample (reproduced); A3 discovery defaults to same-candle close fills; B5 silent crash on startup failure; B8 CSP blocking fonts; B10 cross-run score ranking; B11 eight IDs from the 2026-08-07 review never made it into the task list.
- e2e measured at **78/78 passed**.
