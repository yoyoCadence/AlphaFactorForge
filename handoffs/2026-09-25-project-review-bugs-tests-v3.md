# Handoff: 全專案深度審查 v3 — 補齊「全部完整檢視」（僅記錄，未修改程式）

Date: 2026-09-25
Repo: yoyoCadence/AlphaFactorForge
Branch: claude/project-review-bugs-tests-3p4w8h（基底 `main` @ `f747131`）
PR: 無
Extends: `2026-09-25-project-review-bugs-tests-v2.md`（v2 的 A1–A5、B1–B14、T1–T14 **仍然有效**；本版只補 v2 沒有深讀的部分，並附兩項勘誤）
Status: open — 待使用者逐項確認後再排入 `tasks.md`；本次**沒有改動任何程式碼或測試**

## Summary

使用者要求「不能讀得比較淺，請全部完整檢視」，所以本版把 v2 讀得較淺、或還沒讀的檔案逐行讀完：

- Rust：trial ledger（core／workspace／binding／transfer）、market 層全部（registry、provenance、snapshot、
  ingest、Tiingo、TW ETF、TWSE／FinMind／Binance 解析）、discovery_core 其餘模組（config、enumerate、gate、
  score、walk-forward、precision、dsl、identity、etf、etf_metrics、market_data）、`discovery_runner/mod.rs` 後半、
  `db/{discovery,validation_record,runtime_ledger}.rs`、`repositories.rs` 後半、runtime 全部（service、commands、
  host、connect、control_api、control_client）、research history、Tauri commands、desktop 事件、
  `main.rs`、`identity.rs`，以及 migrations 0001–0009。
- TS：charts／theme、benchmarks／randomEntry、metricsCodec／validationRecord、discoveryFeed、mockClient、
  mockHistorySeed、ResultsExplorer／ResearchHistory、market-data foundation／etf、split、StrategySection。

新結論有三點：

1. **研究有效性仍是最大風險，而且比 v2 描述的更廣。** 基準與 Random Entry 永遠是「做多、100% 部位、當根收盤」，
   而 Gate 只比 net return。所以候選策略只要縮小部位或改成做空，就能在下跌行情中「通過 alpha 測試」（BM-1）。
   另外，Gate 的 95 百分位門檻沒有做多重檢定校正（GT-M）。
2. **Trial ledger 有兩個會隨時間惡化的問題。** 一是 registry 的目錄與環境變數跟規格文件不一致（TLW-1）；
   二是每次 claim 候選都要全量驗證整條 hash chain（TLB-1）。後者的成本只會越來越高，因為 registry 永不縮小。
3. **ETF 管線「資格 OK」但實際不可研究。** TW ETF 單次最多 31 天、Tiingo 最多 366 天，而且不會合併成長序列
   （TW-1）。raw 價格也沒接上 split／dividend 語意（ETF-1）。

標記沿用 v2：〔新〕本次首次發現；〔已追蹤：ID〕已在 `tasks.md`；〔v2：ID〕v2 已列、本版補證據；
〔推論〕由程式碼推得、尚未實跑重現。

## 勘誤（對 v2 與本版筆記）

- **ETF 以 365 年化**：v2 的 **A1** 已涵蓋，本版不另立項，只補三點證據。
  (a) Score-v1 直接用 `cagr` 與 `sortino` 並套 cap（`discovery_core/score.rs:642-660`），ETF 候選因此較早撞到上限。
  (b) `compute_etf_metrics`（sessionsPerYear）沒有任何 production caller。
  (c) Gate 本身不使用任何年化指標，所以 A1 的影響集中在 Score 與顯示數值。
- **ledger gap marker 永不清除**：我原本想列為 P2。後來讀完 `connect.rs:382-395` 才發現 forwarder
  只在 marker「改變」時才要求 resnapshot，所以降為 P3 衛生問題（RL-2）。

## Findings

### P1 — 研究有效性與稽核完整性

#### BM-1 基準與 Random Entry 沒有對齊候選的方向、部位、成交時點；Gate 只比 net return〔新；慣例已記載於契約，但後果沒有〕
- 證據：`discovery_core/benchmarks.rs:4,129-134,181-183`、`random_entry.rs:7,169-174`，以及 TS 的 `services/benchmarks.ts`
  （benchmarkBase）、`services/randomEntry.ts`（cfg）。這些地方一律寫死 `Direction::Long`、sizing 1.0、`FillMode::Close`、
  無 SL/TP。另一方面，`gate.rs:605-635` 的 `benchmarkWins` 只看 `candidate.net_return > benchmark.net_return`，
  `randomEntryPercentile` 也只看 net return 的百分位。
- 契約 `docs/benchmark-suite-contract.md:12,22,25` 宣稱 Random Entry 用「same exposure and holding time」，
  但實際只對齊持有根數。部位大小與方向都沒有對齊。
- 後果：
  - 下跌行情：做空候選，或部位只有 1% 的做多候選，本來就會贏過 buy & hold 和大多數隨機做多，
    完全不需要擇時能力就能通過「關鍵 alpha 測試」。
  - 上漲行情則反過來：低部位候選永遠通不過。
- 建議：Random Entry 對齊候選的 direction／sizePct／fillMode／SL-TP；基準改比風險調整後或曝險正規化後的報酬。
  這是契約版本升級（random-entry-v2、benchmark-suite-v2），需要決策。
- 測試：T15。

#### TLW-1 Trial registry 目錄與覆寫環境變數和規格不一致〔新〕
- 證據：
  - 規格 `docs/trial-ledger-v1.md:54-63` 與 `research/trial_ledger.rs` 定義 `REGISTRY_DIR_NAME = "com.alphafactorforge.evidence"`、
    `REGISTRY_DIR_ENV = "AFF_REGISTRY_DIR"`，並要求「isolated smoke 必須同時設 AFF_DATA_DIR 與 AFF_REGISTRY_DIR」。
  - 實際執行路徑 `research/trial_ledger_workspace.rs::registry_dir()` 用的是
    `dirs::data_local_dir()/…/"com.alphafactorforge.trial-ledger"`。它不讀 `AFF_REGISTRY_DIR`，只接受 debug-only 的
    `AFF_TEST_TRIAL_REGISTRY_DIR`，而且 workspace 必須位於名為 `aff-*` 的暫存目錄。
  - `default_registry_dir()`、`REGISTRY_DIR_NAME`、`REGISTRY_DIR_ENV` 都是死碼。
- 後果：有人照文件用 release build 跑隔離 smoke 時，trial 會寫進使用者真正的 registry。
  依規格 §4，計數永不退還，所以 family 計數會被永久灌水。使用者照文件也找不到自己的 registry。
- 建議：統一一個目錄名稱與一個 env；刪除死碼；加 T16。

#### TLB-1 每次 claim 候選都全量驗證 registry hash chain，而且持有 DB 鎖〔新〕
- 證據：`research/trial_ledger_binding.rs::check_binding()` 會呼叫 `verify_chain()`。這個函式逐一處理每筆 event，
  包括 JSON parse、兩次 SHA-256，以及 batch members。`require_current()` 在 `discovery_runner/mod.rs:411`
  （每個 candidate claim，位於 `lock(&db)` 內）以及 `mod.rs:1773`、`mod.rs:1809`（每次 start）被呼叫。
- 後果：成本是 O(候選數 × 歷來所有 event 數)，而且 registry 跨 workspace 共用、永不縮小。
  驗證期間持有 workspace DB mutex，所以 UI 命令也會被卡住（與 v2 A5 疊加）。
- 建議：只在 open 時驗證一次，之後從已驗證的 seq 做增量驗證，或快取已驗證的 head。加 T17（效能回歸）。

### P2 — 邊界正確性、統計紀律、UX

| ID | 問題 | 證據 | 建議 |
| --- | --- | --- | --- |
| GT-M〔新〕 | Gate 的 `randomEntryPercentile ≥ 95` 是逐候選的原始門檻，沒有多重檢定校正。4096 個純雜訊候選中約 5% 會僅憑運氣過這一關。Score 的 dataMining 懲罰 `w·log10(N)/cap` 對同一 run 的所有候選都是**同一個常數**，完全不影響排序和 bestStrategy。Holm／precision（`precision.rs`）只出現在 trial ledger 的測試與確認協議，不在 Gate 中 | `gate.rs:628-635`、`score.rs:710-722`、`db/discovery.rs:970-996` | UI 標明「未經搜尋規模校正」；或讓門檻依 N 調整；或規定 best 必須經過確認 |
| TLB-2〔新／推論〕 | discovery 登錄 trial 時寫死 `tests_per_trial: 1`，但每個候選實際接受 ≥2 種檢定（4 個 benchmark wins 加 random-entry 百分位；`docs/research-precision-v1` 的範例用 2 tests/trial）。family 的檢定數 m 因此被低估，後續 Holm 門檻偏寬 | `discovery_runner/mod.rs:1803-1807` | 在 trial-ledger-v1 明確定義「test」並據此設定 |
| LC-1〔新〕 | 任一 run 只要 Gate 通過，`strategy_def.lifecycle` 就變成 `validated`，而且「永不降級」。strategy_def 以 hash 為唯一鍵，跨 dataset 共用，所以在 A 資料集幸運通過一次（見 GT-M），之後在 B、C 失敗也仍然是 validated | `db/discovery.rs:1244-1256`、migration 0001 `UNIQUE(strategy_hash)` | lifecycle 改為以 (strategy, dataset) 為單位，或改由 records 推導 |
| ETF-1〔新；限制已部分記載〕 | Tiingo 與 TW 的 dataset 都是 raw 價。split／dividend 只記在 report 裡，P08 的 `split_adjusted_signals` 與 dividend accrual 只有測試在呼叫。範圍內若有 split，會出現假的 -50%／-75% K 棒；除息缺口會讓做多被記成虧損、做空白賺（TLT 每年約 3–4%，SPY 約 1.3%）。snapshot 在 actions 確認後仍然是 OK | `tiingo_ingest.rs:479-483,590`、`tw_etf_ingest.rs:844`、`docs/market-source-tiingo-v1.md:91,111` | 範圍內有 split 時把 qualification 降為 Degraded；或在 P18 前要求 adjusted-split 才能進入 discovery |
| TW-1〔新〕 | TW ETF 單次請求最多 31 天（`tw_etf_ingest.rs:315`），每次都各自匯入一個約 20–22 根的 dataset 與 snapshot，而且沒有合併或 append 機制。22 根日線撐不起 60/20/20 切割加 embargo，所以 TW ETF 實質上無法做 discovery，卻被標成 qualification OK。Tiingo 的 366 天上限（約 252 根）是較輕微的同類問題 | 同左、`tiingo_ingest.rs:235` | 提供連續視窗的合併，或允許更長範圍分段抓取後再組合 |
| TW-2〔新〕 | 兩個 ETF ingest 在已註冊 instrument 與設定不一致時，都回傳 `registered_market_requires_reconciliation`，但沒有任何命令能註冊新 revision。TW 設定只要新增一筆停牌紀錄，之後該代號就永遠無法再匯入 | `tw_etf_ingest.rs:771-782`、`tiingo_ingest.rs:520-531` | 新增 reconcile／register-revision CLI |
| RG-1〔新〕 | `register_instrument` 以 content_hash 去重（UNIQUE，不含 revision）。A(rev1)→B(rev2)→A 會回傳 rev1、created=false，而 `latest_instrument()` 仍然是 B，所以「改回舊內容」永遠無法成為最新版 | `market/registry.rs:289-304` | hash 納入前一版 revision，或允許與最新版不同時新增 revision |
| MF-1〔新〕 | `MAX_EXPECTED_BARS = 1,000,000` 被註解成「約 114 年小時線」，但換算到 1m 只有約 1.9 年、3m 約 5.7 年。更長的請求會得到 `RangeTooLarge`，整個 fetch 失敗，長期的 1m 加密貨幣歷史因此無法匯入 | `market_foundation.rs`、`foundation.ts:410` | 上限依 interval 調整，或分段 audit |
| IN-3〔新／推論〕 | 所有 coverage 缺陷都是 Blocking，內建加密貨幣標的的 `suspensions: []`（`registry.rs:464`），也沒有記錄交易所停機的命令或 UI。跨越真實維護缺口的多月範圍會永遠 Blocked，拿不到 snapshot，在 P12d 之後也就永遠無法 admission | `market/registry.rs:449-468` | 需要用真實資料驗證；並提供記錄 venue suspension 的路徑 |
| TL-G〔新〕 | 沒有 market snapshot 的 dataset（手動 JSON 匯入、sample）其 family_id 為 None，會得到 `AdmissionBlocked::FamilyUnknown`。P12d 啟用 admission 阻擋後，這類 dataset 將永遠無法 admission，UI 沒有任何說明 | `research/trial_ledger.rs` | UI 預先揭露 |
| CH-2〔新〕 | 價格軸、最新價標籤、bar-info 的 OHLC 都寫死 `toFixed(2)`，小於一分的資產全部顯示為 0.00 | `CandleChart.tsx:332,440`、`ChartSection.tsx`（CS-2） | 依價格量級決定精度 |
| RS-1〔新〕 | Holdout 開啟時畫面上有 in-sample 與 out-of-sample，但 Save 只存 `segment='full'`（混合了 IS 與 OOS），JSON 匯出也沒有 range／holdout 資訊。使用者剛看到的 OOS 證據既沒保存也沒匯出 | `ResultsSection`／`BacktestPanel` | 保存並匯出 OOS 段 |
| RE-1〔新；與 v2 A5 疊加〕 | Results Explorer 每次開啟都會一次讀取所有 strategies、datasets、summaries、validation records（每筆都含完整 `record_json`，約 10–30 KB），沒有分頁也沒有虛擬化。數萬筆就會造成數百 MB 的 IPC 傳輸與大量 DOM | `ResultsExplorer.tsx` load()、`repositories.rs:1127-1156` | list 端點不回傳 record_json 並分頁，選取時才讀明細 |
| MK-3〔新〕 | mock runner 讓每隔一個候選就以 0.5 以上分數通過 Gate，而且從不失敗；真實 runner 在樣本資料上通常全數不通過，而且一個零交易候選就會讓整個 run 失敗（v2 A2）。e2e 因此從未跑過 run failed、長時間後才出現 errorMessage、全數 gate-fail 的結果清單。mock 的 bestStrategyId 是最後一個通過者，不是最高分 | `tauri-client/mockClient.ts` | mock 行為對齊 runner 語意 |
| CA-3〔新／推論〕 | 服務 drain 期間（每輪最多 60 秒，coordinator 活著就一直重試），`events_long_poll` 會立即返回（`control_api.rs:819`），而桌面的 `forward_loop` 在 Ok 分支沒有 sleep（`connect.rs:360-399`）。結果是連續發出 HTTP 請求，每次都產生一個 server thread 並透過 `events_page` 取 DB mutex，與正要寫 checkpoint 的 coordinator 搶鎖 | 同左 | shutdown 時只提早返回一次，或讓 client 看到 `shuttingDown` 時退避 |
| CC-1〔新／推論〕 | `ControlClient` 的 timeout 是 10 秒（`control_client.rs:74`），但 `discovery.start` 在回應前要同步完成 enumeration、儲存策略、登錄 lineage 與 trial（含 TLB-1 的全量驗證）。慢的 start 會在桌面端逾時並顯示「background service unreachable」，服務端卻完成了。`ServiceProxy::call` 每次產生新的 requestId（`connect.rs:269`），所以使用者重試不是冪等 replay，只會收到 Busy「already active」 | 同左 | 可變命令延長 timeout，並在重試時沿用同一個 requestId |
| HO-2〔新；v2 B5 的新觸發條件〕 | `open_or_connect` 遇到「非服務的持鎖者」就失敗，例如執行中的 `alpha-factor-forge-service fetch / fetch-tiingo / fetch-tw-etf`，它們在整段受網路限制的匯入期間都持有 workspace。`main.rs` 此時會 panic，而 release 版的 `windows_subsystem=windows` 讓 App 直接消失。市場匯入只有 CLI、而且是獨佔的 | `runtime/host.rs:162-171`、`main.rs` setup | 顯示可理解的錯誤畫面；或讓 ingest 走服務命令 |

### P3 — 衛生、文件、效能小項

- **SN-1** 只要存在任何一筆不可變、也無法刪除的舊式 forward-observed snapshot，`list_snapshots()` 和 `dataset_market_status()` 就對整個清單回 Err（`ingest.rs:1304` 的測試固定了這個行為）。未來 P16／P21 的讀者會被永久卡住，應改為過濾並回報。
- **MOD-1** `market/mod.rs` 中 `is_blocked()` 的註解語意相反（寫成「每個 blocking event 都不存在時為 true」）；**MOD-2** 整個 market 模組都加了 `#![allow(dead_code)]`，會掩蓋真正的死碼。
- **PV-1** `record_raw` 的 fork 檢查是先查後寫，沒有 `UNIQUE(revision_of)`；目前靠單一 writer 保證正確，可以改用 partial unique index。
- **TI-1** Tiingo／TW 的 `ingest()` 對每個 symbol 都用 `?`，其中一個出錯就中止整個 batch，已產生的報告也會從 CLI 輸出消失；**TI-2／IN-5** 每個 endpoint 或 unit 都呼叫一次 `list_provenance`（讀全表），成本隨歷史線性成長。
- **FM-1** FinMind 的回應中只要有任何一筆股票股利，就在區間過濾之前整筆拒絕。event 以整年抓取，所以會阻擋該年所有 31 天視窗。
- **BN-1** Binance 時間欄以 f64 解析；奈秒量級（約 1.7e18）超過 2^53，「精確轉換」的保證不成立。目前不會觸及，但應改用 i64 解析。
- **EN-1** 對訊號沒讀到的參數做 sweep（例如 MA 交叉策略卻掃 bbPeriod）會產生行為完全相同、hash 卻不同的候選，浪費算力、灌大 N 與 ledger 計數，而且同分爭 best。validity 規則也不管訊號是否用到該參數。
- **SC-1** consistency 分量用「絕對」月報酬的 σ，低曝險策略幾乎拿滿分；+∞ 的 profitFactor／sortino 也拿滿權重。這與 BM-1 是同一類「曝險縮放」偏誤。
- **DSL-H** DSL 的 `HIGHEST`／`LOWEST` 視窗包含當根（`indicators.rs:266-279`），但文件沒寫，所以常見的突破寫法 `GT(CLOSE, HIGHEST(CLOSE,n))` 永遠不會觸發。加上 v2 A2（零交易會讓整個 run 失敗），一個 AI 或手寫的突破 DSL 就會拖垮整個 run。
- **VRV-1** 驗證紀錄 validator 接受 Gate 門檻落在 [0,1]，但 `resolve_gate_config` 要求 (0,1]；split plan 也沒有與 dataset 的根數交叉比對。這是 `save_validation_record` 公開寫入面（v2 A4／VR-1）的縱深防禦缺口。
- **VR-1** TS 的 composer 管線與 `save_validation_record` 命令沒有 UI 呼叫者，卻仍對外開放寫入。**RC-1** `researchCommand.ts` 的註解已過時。
- **UP-1** `backtest_summary` 依 (strategy, dataset, segment) upsert。之後的 run 或手動驗證存檔會覆寫舊 run 的 `discovery_jobs.result_id` 與 attempt outcome 所指向的數字。不可變的真相仍在 record_json 與 P05 artifact 中，但 `result_id` 的語意會悄悄改變。
- **TS-1** `save_backtest_result`（webview IPC）接受 `segment='test'`（`repositories.rs:629`）。Test 紀律只靠 UI 自律，建議在 IPC 路徑拒絕。
- **RL-1** `runtime_events` 沒有保留期限；**RL-2** `ledger_gap` marker 永不清除（見勘誤）。
- **HB-1** `heartbeat_seq` 在 production 中沒有任何讀取者，卻每個週期寫一次；ingest CLI 期間 DB mutex 被長時間持有，所以 heartbeat 根本不會跳。
- **HO-3** 服務啟動超過 30 秒時，`wait_for_service` 會失敗，relock 又拿不到鎖，桌面就停在 Switching，而服務其實正在運作。
- **CMD-1** `CommandError::from(AppError)` 用子字串分類（`already`、`busy` 視為可重試）；**CMD-3** `events.read` 的 payload 解析寬鬆（afterEventId 若不是整數就當作 0，等於整個重播）；`InFlightRequests.claim` 等待時沒有 timeout。
- **SV-1** `service.log` 沒有輪替；`tiingo-status` 未設定時回傳 exit 5，混用了「range unusable」的意義。
- **FD-3** `sourceOrigin()` 取第一個 `-` 之前的字串，例如 `binance-us-rest` 會與 `binance-archive` 被視為同一個 origin，這條 splice 規則依賴命名紀律。
- **HY-1** 註解漂移：`HypothesisDraft.strategy_hash` 標註為 `strategy-v2:`，實際存的是 `strategy-doc-v1:`；`research/mod.rs` 提到的 `backtest_trades` 實際表名是 `trades`。
- **TL-I** 不支援的磁碟區偵測只檢查 Windows 磁碟類型與 OneDrive；**TLW-2／3** legacy split 的回填依賴當前機器的核心數，而且失敗時會靜默；**TLT-1／2／3** transfer 沒有接上任何執行路徑、匯入後 `registered_at` 遺失、`json_extract` 全表掃描。
- **CH-4／TH-1／TH-2** chartPaint 寫死 `#000000`；frost-grey 主題的量能對比約 1.1:1，而且對比測試沒涵蓋 vol／grid；字型一次載入約 20 個字族。
- **DF-1／MT-1／RH-1／FP-1／HT-1／SS-1** UI 小項：start 失敗後跟隨中的 run 消失；NaN% 以及 pop-out 與主視窗值不一致；歷史只顯示最新 200 筆；一個 Escape 會關掉所有浮動面板；HelpTip 背景半透明；DSL 策略無法在編輯器載入。
- **MIG** 0001 檔案開頭有 UTF-8 BOM（目前可運作）。

## 已確認健全（本輪新增，避免重工）

- Control API：token 用 CSPRNG 產生並做常數時間比較，Debug 輸出會遮蔽 token；manifest 與 token 以原子寫入，權限 0600；先檢查 Host／Origin 再驗證身分；head／body 有上限；最多 32 條連線；drain 先撤掉 endpoint 再釋放鎖。
- 命令信封：reserve、complete、replay 加上 outcome row（H1／R1／R3）的冪等流程正確。
- runner 狀態機與交易：recover、resume、complete、cancel、fail 都在單一交易內；`select_best_strategy` 會排除非有限分數。
- market：provenance 與 snapshot 的不可變性由 trigger 保證；Tiingo／TWSE／FinMind 解析嚴格，股利雙向對帳；憑證只存在 Windows Credential Manager，不會寫入 log。
- discovery_core：config 的拒絕順序、walk-forward 切割、precision 的 u128 界限（最大約 9e33，遠低於 u128 上限）、DSL 型別檢查與 NaN 安全比較都沒問題。TS 與 Rust 的 foundation、etf、split、etf-metrics 鏡像一致。
- research history 的狀態機與 trigger：凍結欄位不可改、`trial_event_id` 只能寫一次；secrets 與 AI 的 stub 都不會外洩 key。

## Test gaps（接續 v2 的 T1–T14）

| ID | 缺口 | 對應 | 建議 |
| --- | --- | --- | --- |
| T15 | 基準與隨機進場的曝險公平性 | BM-1 | 在下跌合成序列上，sizePct=1% 的做多與純做空候選不應通過 benchmarkWins 或 randomEntry |
| T16 | registry 路徑契約 | TLW-1 | 斷言 `registry_dir()` 與規格的目錄名稱、env 一致 |
| T17 | claim 成本不隨 registry 大小線性成長 | TLB-1 | 預先塞入 N 筆 event，量測每次 claim 的耗時 |
| T18 | 多重檢定揭露 | GT-M | 對 4096 個雜訊候選斷言通過率，並作為回歸基準 |
| T19 | lifecycle 跨 dataset | LC-1 | A 通過、B 失敗後，lifecycle 必須反映 dataset 範圍 |
| T20 | instrument 回復舊內容 | RG-1 | A→B→A 之後 `latest_instrument` 應為 A |
| T21 | 1m 長範圍 | MF-1 | 2 年 1m 範圍不應整體失敗 |
| T22 | drain 期間 forwarder 不空轉 | CA-3 | 計算 drain 期間的請求數 |
| T23 | 慢 start 的逾時與重試冪等 | CC-1 | 注入延遲，重試應 replay 同一個 runId |
| T24 | DSL 突破寫法 | DSL-H | `GT(CLOSE, HIGHEST(CLOSE,n))` 會零觸發，文件與範例要說明，並斷言其行為 |
| T25 | IPC 拒絕 Test 段 | TS-1 | `save_backtest_result(segment='test')` 應被拒絕 |
| T26 | 小價格精度 | CH-2 | 0.00001234 的軸標籤不應為 0.00 |

## Recommended order（決策先於實作）

1. **研究有效性決策**（與 v2 的 A1／A2／A3 並列，各自開 PR）：BM-1（基準曝險對齊）、GT-M（多重檢定揭露或校正）、
   TLB-2（「test」的定義）、LC-1（lifecycle 範圍）。在這些決策完成前，Gate 的通過結果都不應被當作 alpha 證據。
2. **Trial ledger 兩項**：TLW-1 先做，改動小但會持續污染；TLB-1 的效能問題會隨時間惡化。
3. **ETF 可研究性**：TW-1、TW-2、ETF-1。先決定是否需要合併視窗與 adjusted 價格，再決定 qualification 的標示方式。
4. **runtime 連線韌性**：CA-3、CC-1、HO-2（HO-2 可與 v2 的 B5 一起處理）。
5. **純新增測試**：T15、T16、T20、T24、T25，不改變行為。
6. **P3 可合併為 hygiene PR**。

## Resolution

（待使用者確認後補記：哪些項目已排入 `tasks.md`、哪些駁回以及理由。）
