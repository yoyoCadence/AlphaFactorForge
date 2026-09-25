# Handoff: 全專案深度審查 v2 — 錯誤 / Bug / 測試缺口（僅記錄，未修改程式）

Date: 2026-09-25
Repo: yoyoCadence/AlphaFactorForge
Branch: claude/project-review-bugs-tests-3p4w8h（基底 `main` @ `f747131`）
PR: 無
Supersedes: `2026-09-25-project-review-bugs-tests-v1.md`（v1 是風險抽樣版；本版為逐檔深讀版，v1 的勘誤附在 v1 文末）
Status: open — 待使用者逐項確認後再排入 `tasks.md`；本次**沒有改動任何程式碼或測試**

## Summary

使用者要求做到「完整檢查」的程度，所以本版把 v1 沒深讀的部分補齊，包括 Rust 的 runner、
execution、db、runtime、control API、market 與 research，以及 TS 的 services、components、charts、
hashing 和 market-data。疑似 bug 盡量用一次性腳本實測；腳本放在 git 忽略的 `node_modules/.probe/`，
跑完即刪，repo 內沒有殘留。

整體結論有三點：

1. 程式碼品質高於一般水準。契約、parity fixture 和 fail-closed 的紀律都很一致。
2. 最大的風險不在「程式寫錯」，而在研究結論的有效性：年化基準、同根收盤成交、零交易候選會讓整個
   run 失敗、ETF 資料與加密貨幣曆法混用。這些都是「程式照規格跑，但規格本身會產生偏誤」的類型。
3. 流程有一個盲點：2026-08-07 審查提出的 8 個追蹤 ID 從未進入 `tasks.md`（見 B11）。

每項標記來源：
- **〔新〕**：本次首次發現。
- **〔已追蹤：ID〕**：`tasks.md` 已有對應任務，本報告只補充證據。
- **〔前次提出未入帳：ID〕**：2026-08-07 handoff 提過，但不在 `tasks.md`，也不在 `docs/improvement-backlog.md`。

## Baseline verification（實測）

| 檢查 | 結果 |
| --- | --- |
| `npm ci` / `npm run typecheck` | ok / ok |
| `npm test`（Vitest） | **975 passed**（54 files） |
| `cargo test --locked`（Linux，需先 apt 安裝 webkit2gtk/gtk） | **445 passed**（94 + 349 + 2）；`tasks.md` 記 444（348） |
| `cargo clippy --locked --all-targets` | exit 0，5 個既存 warning（與 `tasks.md` 一致） |
| Playwright e2e（chromium，`?mock=1`，本地 retries=0） | **78/78 passed**（22 files），連續兩次皆 exit 0 |

## 審查範圍與已確認健全的部分（避免重工）

以下讀過，**沒有發現問題**，列出以免下一位再查一次：

- **Test 隔離**：Rust runner 只把 `candles[..=validation.to]` 交給訊號與回測，Test 段完全不進入計算
  （`discovery_runner/execution.rs:886-901`）。walk-forward 的每個 fold 都用自己的前綴切片建訊號。
- **Embargo 公式**：MA 交叉 `max(fast,slow)+1`、RSI `p+2`、MACD `slow+signal`、EMA 交叉 `p+1`，
  都與指標第一個有效索引一致。
- **分段切割**：60/20/20 採最大餘數法，全程安全整數運算，TS 與 Rust 相同。
- **SQL**：所有動態 SQL 只拼接常數欄位清單，值一律綁定參數，沒有注入面。
- **Control API**：只 bind 127.0.0.1；檢查 Host；有 `Origin` 就拒絕（防 DNS rebinding／瀏覽器）；
  Bearer token 以常數時間比較；限制 head、body 大小與連線數；Unix 下 token 檔權限為 0600。
- **HTTP 抓取**：只允許 HTTPS 和白名單 host，拒絕 redirect，限制 body 大小，只重試可重試的錯誤；
  Tiingo token 放在 header，錯誤訊息不外洩 token。
- **Binance ZIP**：限制單一 entry，檢查 CRC-32，宣告大小與實際大小必須一致，有 256 MiB 上限，
  時間單位以整份檔案判定。
- **Worker pool**：`catch_unwind` 會把 panic 轉成錯誤；coordinator 的鎖順序是 Control → DB，規則一致。
- **資料集 identity、策略 identity、parity fixture 與遷移交易**：遷移和版本寫入在同一交易內完成。
- **安全運算式直譯器（code mode）**：沒有 eval；`prev`／`cross` 只回看一根，且禁止巢狀。

---

## Findings

嚴重度：**P1** = 會產生錯誤的研究結論，或違反架構契約；**P2** = 邊界錯誤、UX 缺陷或潛在風險；
**P3** = 衛生或文件問題。「已實測」= 用腳本重現過；「推論」= 讀程式得出。

### P1 — 研究有效性與架構契約

#### A1 ETF／股票日線用 365 年化，而且 engine fingerprint 沒記錄年化基準〔新；相關：前次提出未入帳 METRIC-ANNUALIZATION-001、已追蹤 INTERVAL-CONTRACT-001〕

- 位置：`discovery_runner/execution.rs:310` → `benchmarks::bars_per_year`（`"1d" => 365`）；
  TS `services/backtestRunner.ts` 的 `BARS_PER_YEAR`。P08 雖然定義了 `sessionsPerYear`，
  `discovery_core/etf_metrics.rs` 也存在，但 runner 從沒呼叫它。
- 可達性：P09 / P10 的 ingest 會產生 `datasets` 資料列，並以 `market_snapshots.dataset_id` 綁定；
  `load_verified_dataset` 對 dataset 的來源沒有任何限制。
- 影響：Sharpe／Sortino 被放大 √(365/252) ≈ **1.20 倍**，CAGR／Calmar 也會偏離。score-v1 的
  cagr、sortino、calmar 三個分量會系統性偏好 ETF 候選。另外，年化只看 bar 數、不看實際經過時間
  （前次審查已指出），所以有缺口的資料也會失真。
- `engine_fingerprint()`（`execution.rs:64`）沒有記錄年化因子，也沒有 `indicator-v1`／
  `params-signals-v1` 的版本（見 B6）。這代表兩個年化方式不同的 run 在 fingerprint 上無法區分。
- 建議：年化因子改由 snapshot → instrument／calendar 決定；沒有 snapshot 的舊 dataset 維持 365。
  年化因子要寫進 fingerprint 和 record。

#### A2 一個零交易候選就讓整個 run 失敗；預設 Gate 在內建樣本上不可能通過（已實測）〔已追蹤：DECISION-ZERO-TRADE-001，建議提高優先級〕

- 失敗鏈：Random Entry 遇到候選 0 筆交易會回傳 Err（`discovery_core/random_entry.rs:153`）→
  `execute_candidate` 回 Err → coordinator 把**整個 run** 標成 failed（`discovery_runner/mod.rs:1241-1257`）。
  網格搜尋天生容易出現零交易格，例如極端的 RSI 門檻、或長週期均線配短資料，所以一格就能毀掉整個 run。
- 實測：內建 `makeSampleCandles(600)` 加上 UI 預設軸 `fastMA 5..11 step 3`，Validation 段只有 111 根，
  驗證段交易數分別是 **2 / 2 / 1**。預設 `minTrades = 30` 在這份樣本上**結構上不可能**達到，
  UI 也沒有任何調整 Gate 的入口。結果是：使用者照 UI 預設操作，只會看到全部「未通過」，
  或看到 run 整個失敗。
- 建議：先做 DECISION-ZERO-TRADE-001（改成持久化的 Gate 拒絕，status 標成 insufficient-evidence）；
  UI 顯示「以目前資料長度，Validation 段理論可達的交易數」，或給樣本資料一組示範用的 Gate。

#### A3 探索預設「同根收盤成交」＝樂觀的執行假設〔前次提出未入帳：EXECUTION-TIMING-DECISION-001〕

- `defaultStrategy().fillMode = 'close'`。DiscoveryPanel 直接把 workspace 策略當 base preset
  （`services/discoveryRunConfig.ts`），所以**所有探索結果預設都假設能以產生訊號的那根收盤價成交**。
  契約文件（`docs/backtest-execution-contract.md:87`）明確採用這個行為，但沒有標示它是樂觀偏誤。
- 影響：對短週期（1m–1h）和均值回歸類策略，影響特別大。它會系統性抬高 Validation 表現，
  也就是抬高 Gate 通過率。
- 建議：探索預設改用 `nextOpen`，或在 envelope 強制明示 fillMode，並在 UI 顯示警告。
  做成版本化的產品決策，不要當成局部修補。

#### A4 connect 模式下桌面端直接寫 SQLite（第二個 writer）〔新〕

- 位置：`runtime/connect.rs:157`（以讀寫模式開 `open_migrated`）、`runtime/host.rs:71`，以及
  `commands/db_commands.rs` 的 `import_candles`、`save_strategy`、`save_backtest_result`、
  `save_validation_record`。
- 問題：這 4 個寫入指令都沒有走 `ownership::write_transaction(epoch)`，所以：
  (a) 背景 service 持有 lease 時，桌面照樣寫入，違反 P04b 的 "never two writers"；
  (b) 即使在 embedded 模式，也不會 bump `runtime_state.mutation_seq`，但契約寫的是
  "bumped by every committed change to a run/job/result"；
  (c) 沒有 admission guard，hand-over drain 看不到這些寫入。
- 建議：connect 模式改走 proxy，或在契約中明文豁免並附理由與測試。

#### A5 主執行緒阻塞與 DB 鎖持有範圍〔已追蹤：DB-ASYNC-001；本項補新證據〕

DB-ASYNC-001 只提到「大型匯入與結果持久化」。本次另外找到下列路徑：

- `get_discovery_progress`、`get_active_discovery_run` 是同步指令，在主執行緒執行。
  connect 模式下，它們**在主執行緒上對 service 發 HTTP**（`discovery_commands.rs:118-135`）。
  DiscoveryPanel 每收到 Done 或 host 事件就會呼叫一次。
- `start_for_request` 在持有 DB mutex 的期間做這些事：載入整份 candles、重算 dataset hash、
  跑品質檢查、對全部候選做 walk-forward preflight、逐筆 insert strategy（`discovery_runner/mod.rs:463-514`）。
  這段時間任何 UI 的 DB 指令都會卡在主執行緒。
- coordinator 在持有 DB 鎖時寫 artifact 檔（stage → fsync → verify → rename，`mod.rs:1298-1351`），
  檔案 IO 被序列化在 DB 鎖內。
- 建議：把上面三處一起納入 DB-ASYNC-001；artifact 寫檔移到取 DB 鎖之前。

### P2 — 邊界正確性、UX、潛在風險

#### B1 空單虧損超過 100%：CAGR 顯示 0 或 NaN（已實測）〔新；模型限制已在契約記載〕

- 契約寫明 short 是「1× 無清算」模型，所以權益可以變成負數；問題在於指標不一致。
- 實測：價格 100 → 250 → 300，期末權益 **-10000**，`netReturn = -2`，但 `cagr = 0`、`calmar = 0`。
  換一個 bar 數就會得到 NaN（`Math.pow` 或 `powf` 作用在負底數上）。另外，權益 ≤ 0 之後，
  Sharpe 會跳過這些報酬。
- 在 discovery 裡，MDD > 0.35 會被 Gate 擋下，所以不會進入排名；但手動回測和匯出的報告會顯示錯誤數字。
- 建議：`end <= 0` 時把 CAGR 定義為 -1 並附明確 status，或加入 ruin 規則。TS 與 Rust 一起改。

#### B2 DSL `SUSPICIOUS` 子字串誤判正常名稱（已實測）〔新〕

- `"Important breakout"`、`"Window range"`、`"Trend following process"`、`"Documented MA"` 全部被拒。
  位置：`core/strategy-dsl/validator.ts:39-44` 與 Rust `discovery_core/dsl.rs:55`。
- DSL 是白名單樹，不會執行字串，所以這個過濾只帶來誤判。未來 AI 產生名稱時很容易踩到。

#### B3 `runBacktest` 對非法輸入回傳「看似正常」的結果（已實測）〔新〕

- `startEquity: -5` 回傳 0 筆交易、`netReturn 0`；`from > to` 回傳空權益與全 0 指標；
  `signals` 長度短於 candles 也不會報錯。TS 與 Rust 都一樣。
- 建議：比照 `validateNormalizedFractions`，遇到這些情況拋 `RangeError`。

#### B4 手動匯入的 K 線沒有檢查實際間隔是否符合宣告的 interval〔新；相關：前次提出未入帳 METRIC-ANNUALIZATION-001〕

- `core/market-data/quality.ts` 只檢查單根 K 線的合理性。JSON 匯入可以宣告 `"1h"`，實際卻是日線，
  或中間有大段缺口，都會被接受。foundation 的 coverage audit 只套用在 market snapshot。
- 影響：年化誤差可達數十倍；nextOpen 會跨過缺口成交；embargo 以「根數」計算，也會失真。

#### B5 啟動失敗時 release 版會靜默閃退〔新〕

- `main.rs:121` 用 `panic!("cannot use the workspace …")`，再加上
  `windows_subsystem = "windows"`，使用者看不到任何訊息。
- 觸發情境：資料庫由較新的 build 寫入（`SchemaTooNew`）、service 持有鎖但尚未發布端點、
  app data 目錄不可寫。
- 建議：改成原生對話框，或開一個最小錯誤視窗並附上原因。

#### B6 engine fingerprint／validation record 缺少部分契約版本〔新〕

- `engine_fingerprint()` 沒有 `INDICATOR_CONTRACT_VERSION`、`PARAMS_SIGNALS_CONTRACT_VERSION`、
  年化因子；`configContracts` 也沒有 indicator 版本。
- 影響：將來 ATR 改用 RMA、或指標語意升版時，舊 attempt 無法從 fingerprint 判斷當時的語意。

#### B7 Web Worker 未被使用，回測和掃描都在 UI 執行緒〔已追蹤：PERF-001〕

- 全 repo 沒有任何 `new Worker(...)`。`SweepSection` 只用 `setTimeout(20)` 讓出一次，
  接著同步跑最多 256 組回測。

#### B8 Tauri CSP 封鎖 Google Fonts，dev 與 desktop 視覺不一致〔前次提出未入帳：FONT-OFFLINE-001〕

- `ThemeProvider.tsx:51` 注入 fonts.googleapis.com 的樣式表，但 CSP 是
  `style-src 'self' 'unsafe-inline'`，而且沒有 `font-src`，所以在 desktop 上一定被擋。
- 結果：中文 skin 的字型（LXGW WenKai、Chiron 等）在 desktop 版全部 fallback；
  dev 版則會對 Google 發出請求，這違反 local-first 原則，也會暴露 IP。

#### B9 DSL `ParamSpec` 的 min/max 沒有語意〔新；潛在風險〕

- 驗證、執行、lookback、embargo 都只看 `default`（`validator.ts:134`、`dsl.rs:588`）。
  一旦加入 DSL 參數掃描，而某個取值的週期大於 default，embargo 就會被低估。
- 建議：先寫守衛測試，並在文件寫明「min/max 保留、目前未啟用」。

#### B10 Results Explorer 跨 run、跨 dataset、跨 config 直接比較分數〔新〕

- `rankValidationRecords`（`services/resultsExplorer.ts:44`）把所有通過 Gate 的紀錄依 score 全域排序。
  但 score 的 N（testedCombinations）、caps、weights 和 dataset 可能各不相同，
  而 `score.ts` 自己的註解要求 "only like-for-like scores are ever compared"。
- 建議：預設依 run 或 dataset 分組；跨組比較時顯示警示。

#### B11 流程盲點：2026-08-07 審查提出的 8 個 ID 從未入帳〔新（流程）〕

- `handoffs/2026-08-07-project-review-and-skin-002-v1.md` 提出以下 ID，但 `tasks.md` 和
  `docs/improvement-backlog.md` 都搜尋不到：
  `EXECUTION-TIMING-DECISION-001`、`BENCH-RANDOM-CONTRACT-001`、`METRIC-ANNUALIZATION-001`、
  `PERSIST-RESULT-HISTORY-001`、`RUNNER-FAILSAFE-001`、`TEST-HIDDEN-TEST-001`、`FONT-OFFLINE-001`、
  `A11Y-SEMANTICS-001`。
- 這是「handoff 寫了，但沒轉成任務」的系統性漏洞。建議在 `handoffs/README.md` 的 lifecycle
  加一條規則：review 類 handoff 的每個 ID，要嘛進 `tasks.md`，要嘛在 Resolution 寫明捨棄理由。
- 補充：`TEST-HIDDEN-TEST-001` 在 Rust runner 已經因為切片而實質解決；但 TS 參考實作
  `services/validationRun.ts` 仍用完整 candles 建訊號（目前只有 mock seed 使用它）。

#### B12 Control API 的連線計數在 handler panic 時洩漏〔新；低機率〕

- `runtime/control_api.rs:449-452`：`connections.fetch_sub` 寫在 closure 最後，沒有用 Drop guard。
  handler 每 panic 一次就洩漏一個名額，累積 32 次後 control API 會永久回 503。

#### B13 `ChartPopoutWindow` 監聽器洩漏〔新〕

- `components/ChartPopoutWindow.tsx:27-45`：`Promise.all` 如果一邊成功、一邊失敗，成功那邊的
  unlisten 不會被保存，也永遠不會被呼叫。可以比照 DiscoveryPanel 逐一註冊、逐一保存的寫法。

#### B14 `sma`／`ema` 累加和遇到 NaN 會永久污染（已實測）〔新；低風險〕

- `sma([1,2,NaN,4,5,6,7], 2)` 從第 3 根之後全部是 NaN。目前輸入都經過品質檢查，所以風險低；
  但未來如果允許「指標套指標」，就會靜默產生 0 筆交易。

### P3 — 衛生與文件

- `buildStrategyDef` 的預設名稱：code mode 會取 `entrySig → exitSig`（params 訊號名），
  造成誤導（`services/strategyRecord.ts:23`）。
- JSON 匯入沒有 trim symbol/interval。`requireMetadata` 會拒絕前後有空白的值，錯誤訊息對使用者不友善。
  另外，dataset identity 不含 `source`，同樣內容但 source 不同時，會得到令人困惑的
  "hash conflicts" 錯誤。
- `NumberInput` 在 blur 時一律呼叫 `onChange`，所以只要 Tab 經過欄位，就會清掉 sweep 的 applied 標示。
- `runtime_events` 沒有保留期限，會無限成長。`purge_old_requests` 用了會 bump state version 的
  `write_transaction`，與它自己文件寫的「receipt 寫入不 bump」不一致。
- `research/trial_ledger.rs` 模組層級有 `#![allow(dead_code)]`，註解還寫著 "Nothing in the runtime
  calls this yet"，但 runner 已經在呼叫；這個 allow 會掩蓋真正的死碼。
- Artifact 的 rename 之後沒有 fsync 目錄，POSIX 上斷電可能遺失 rename。NTFS 有日誌，影響較小。
- `research/history.rs:471-473`：損壞的 fingerprint JSON 會被 `unwrap_or(Null)` 靜默吞掉。
  這是審計證據，應該要顯示損壞原因。
- `save_report` 的副檔名檢查區分大小寫。Tauri app 指令預設對所有視窗開放。
- `exprInterpreter.ts:228` 用 `name in FN_ARITY` 會查到 prototype，例如 `constructor`。
  最後會因為 arity 不符而被拒，所以安全，但寫法不嚴謹；應改用 `Object.hasOwn`。
- Holdout 在 n=1 時，out-of-sample 段是空的（前次審查第 6 點）。
- TS `atr` 的註解寫 "Wilder smoothing"，實作是 EMA。Rust 註解已寫明這是刻意選擇；TS 註解應同步。
- 文件漂移：
  - `tasks.md` 記 Rust 444，實測 445。
  - AGENTS.md 寫 Rust 1.77+，Cargo.toml 是 1.89（已追蹤：TOOLCHAIN-001）。
  - 多個檔案仍有 `SKELETON` 或 `todo!()` 字樣的標頭（已追蹤：DOC-STATE-002）。
  - Linux 需要的系統庫沒有寫在 README。

---

## Test gaps（建議補的測試）

| # | 缺口 | 對應 | 做法 |
| --- | --- | --- | --- |
| T1 | invoke 參數名稱契約（AGENTS.md 列為高風險，但完全沒有測試） | — | 解析 `#[tauri::command]` 簽名（snake → camel），與 `commands.ts` 的 `invoke` key 比對 |
| T2 | 通用 no-lookahead 性質測試（目前只有 `backtest.fills.test.ts:46` 一例） | 核心風險 | 擾動第 k 根之後的資料，斷言 ≤k 的指標、訊號、成交完全不變；TS 與 Rust 都做 |
| T3 | 混合零交易、全零交易的 runner 測試 | A2 | 依 DECISION-ZERO-TRADE-001 的決策撰寫 |
| T4 | ETF 年化 | A1 | 綁定 252 的 snapshot dataset，斷言 Sharpe 用 √252、fingerprint 有記錄 |
| T5 | connect 模式寫入 | A4 | `discovery_runner/tests/host.rs` |
| T6 | 主執行緒回應性 | A5 | 長時間的 start 或 import 進行中，同時呼叫 `get_active_discovery_run` 並量測延遲 |
| T7 | 空單爆倉 | B1 | TS + Rust + parity fixture |
| T8 | `runBacktest` 非法輸入 | B3 | `backtest.contract.test.ts` |
| T9 | 匯入間隔與宣告 interval 不符 | B4 | 宣告 1h 的日線資料、含缺口的資料 |
| T10 | 回測帳務守恆性質測試 | 契約不變量 | 隨機訊號 × 隨機 K 線，斷言 `finalEquity = start + Σpnl` |
| T11 | DSL validator 的 TS↔Rust 差分模糊測試 | B2、B9 | 隨機合法／非法樹，比較兩邊的 `ok`、`errors`、`maxLookbackBars` |
| T12 | 啟動失敗路徑 | B5 | SchemaTooNew、NotPublished 時不能 panic |
| T13 | Control API 在 handler panic 後仍可服務 | B12 | 注入 panic，確認連線計數有釋放 |
| T14 | React 元件單元測試（目前只有 MetricsTable） | UI 競態 | 選做；若維持只靠 e2e，要在文件寫明理由 |

## Recommended order（決策先於實作）

1. **三個需要決策的議題**，各開獨立 PR：A3 探索預設成交時點；A2 零交易
   （即 DECISION-ZERO-TRADE-001）；A1 年化基準（併入 INTERVAL-CONTRACT-001 或
   METRIC-ANNUALIZATION-001）。三者都會改變研究結論，**任何 Gate 通過的結果在這之前都不應被信任**。
2. **B11 流程修補**：把 8 個遺失的 ID 補進 `tasks.md`。成本最低、槓桿最高。
3. **純新增的測試**：T1、T2、T8、T10。不改變行為，還可能找出其他問題。
4. **A4、A5**：需要架構決策，與 DB-ASYNC-001 一起處理。
5. **B2、B3、B5、B13、P3**：可以合併成 hygiene PR。

## Appendix

- e2e：`npx playwright test --list` 共 78 個測試，分屬 22 個檔案；本地設定 `retries: 0`，所以 exit 0 代表全數通過。
- 一次性探針腳本的內容與輸出記錄在 session 中，repo 內沒有留下任何檔案。
