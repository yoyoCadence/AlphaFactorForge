# Autonomous research capability registry

> P00（契約與相容性預檢）的登記結果，2026-09-16。上游計畫：
> [`plans/active-plan.md`](plans/active-plan.md) §5。本檔是**活的登記簿**：每個
> 後續 phase 完成時更新對應列的狀態與證據；不得在此宣稱未經驗證的能力。
> 唯一任務板仍是根目錄 [`tasks.md`](../tasks.md)。

判定用語：**可用**＝本機已實測；**可規劃**＝依賴已到位、尚未實作；**阻擋**＝有明確
阻擋原因，解除前該能力不得啟用；**未驗證**＝尚未檢查，不得假設可用。

---

## 1. Runtime／宿主

| 能力 | 狀態 | 證據／阻擋原因 | 解除／實作 phase |
| --- | --- | --- | --- |
| 桌面嵌入模式（Tauri 內建 runner、啟動 recovery） | 可用 | `src-tauri/src/main.rs` `setup`；PR #103 native smoke lane | — |
| desktop single-instance | 可用 | RUNNER-OWNERSHIP-001；`single_instance.rs` | — |
| 關閉 UI 後研究繼續 | 可用（service 宿主） | P04a（2026-09-17）：`alpha-factor-forge-service run` 以 host kind `service` 持有工作區，run 由 service 程序內的 coordinator 執行，啟動它的 client 消失後仍繼續；重連 = `discovery.active` → `events.read` cursor（Rust 測試 `a_run_started_over_the_api_continues_without_its_client_and_a_new_client_adopts_it`、`tests/service_smoke.rs`）。P04b（2026-09-18）：桌面「在背景繼續」把進行中的 run 在 checkpoint 交給 service（`host::hand_over_to_service`），桌面轉為 connect 模式後關閉視窗不影響 run；重開桌面對執行中的 service 直接 connect 並採用同一 run（Rust `tests/host.rs`、Playwright `host-mode.spec.ts`、原生 smoke） | — |
| host-agnostic runtime（`runtime::open_workspace`：開庫→migration→runner→孤兒恢復） | 可用 | P02；桌面 `main.rs` 只提供 app data dir 路徑；未提供 headless 排程或 service binary | — |
| 跨宿主 workspace ownership（OS 鎖＋epoch＋heartbeat） | 可用 | P03a（2026-09-17，含 review R1/R2 修正）：`runtime/lease.rs`（std `File::try_lock`，未加 crate）、migration 0004 `workspace_ownership`、`db/ownership.rs`；所有 runner store 寫入（含 claim、strategy／run／progress）在 `BEGIN IMMEDIATE` transaction 內檢查 epoch；雙啟／lease 釋放接手／舊 coordinator claim／檢查後換手再 cancel／跨連線 transaction 排他／heartbeat 有測試。connect 模式待 P04 | — |
| SQLite `busy_timeout` | 可用 | P02：`db::open_at` 設 `BUSY_TIMEOUT = 5 s`，runtime 測試斷言 `PRAGMA busy_timeout = 5000` | — |
| 舊 binary 拒絕較新 schema | 可用 | P03a：`apply_migrations` 遇未知 `schema_migrations` 版本回 `SchemaTooNew`，整個開啟失敗（含讀取，比契約「拒絕寫入」更嚴） | — |
| 冪等命令／持久事件 ledger | 可用 | P03b（2026-09-17）：`research-command-v1` dispatcher（reserve-then-complete by requestId）、`runtime_events` 帳本（AUTOINCREMENT eventId，跨重啟不重用）、`events.read` cursor；桌面命令 `dispatch_research_command`／`get_workspace_info`。UI 尚未改走 envelope（P04） | — |
| 完整研究歷史（假說凍結、每候選 attempt、不可變結果 artifact） | 可用 | P05（2026-09-19）：migration 0007、`research/history.rs`（狀態與 job 同 transaction）、`research/artifacts.rs`（content-addressed、staging→rename→參照、讀取校驗、不刪除）；重跑不覆寫舊明細、失敗可追溯、新舊投影一致皆有 Rust 測試（`tests/history.rs`）；桌面「研究歷史」面板。discovery 自動登記的假說 failure modes 為「未陳述」，有陳述的假說待 P15 | — |
| loopback 控制介面 | 可用 | P04a：`runtime/control_api.rs` std-only HTTP/1.1（未採 hyper／tokio）；`127.0.0.1` 動態 port、manifest `control-endpoint.json`＋`control-token`（`getrandom` CSPRNG、constant-time 比對）、Host／Origin 檢查、body／head 上限、`/v1/info`／`/v1/commands`／`/v1/events` long-poll／`/v1/shutdown`；curl 實測 401／200／403／long-poll。`dirs`、`getrandom` 由 tauri 既有鎖定版本升為直接依賴，無新 crate | — |
| 桌面 connect 模式（另一宿主持有工作區時桌面只做代理） | 可用 | P04b：`host::open_or_connect`（`NotOwner` → manifest → `/v1/info` 核對 → `db::open_migrated`）；discovery 命令與 envelope 經 `ServiceProxy` 代理；帳本事件由 forwarder 轉送到視窗（`LedgerEventSink`）；`runtime://host` 通知模式變更／失聯；失聯後持續重新發現並核對同 workspace 的端點（service 重啟、換 port／token 皆可恢復，其他 workspace 的端點被拒）；`runtime://resnapshot` 讓視窗在帳本缺口／無事件的版本前進／無法交付／重連時重讀（契約 §3）；`get_workspace_info.hostMode`。持有鎖但未發布端點的宿主仍拒絕啟動並說明 | — |
| 背景模式切換（桌面 ⇄ service，契約 §1.5） | 可用 | P04b：`enter_background_mode`／`exit_background_mode`（Admission 關門→checkpoint→釋放→啟動 detached service→連接；反向 stop→重取鎖）；失敗回滾為嵌入。service 由桌面 binary 旁的同名 exe 啟動（`service.log` 在工作區）；未包進安裝包、無 Windows ACL 呼叫、Ctrl+C 仍為崩潰路徑 | 包裝／ACL：P22 |
| VS Code MCP 入口 | 可規劃 | 依賴 P04 控制介面；MCP 協定本身由本專案的 stdio adapter 實作 | P16 |
| Windows 排程／服務包裝 | 未驗證 | 未檢查 Task Scheduler／服務帳戶行為 | P22 |

## 2. AI provider（Codex／ChatGPT 訂閱）

本機事實（2026-09-16，`codex doctor` 與 stdio 握手；未消耗模型額度，未啟動任何 turn）：

| 項目 | 值 |
| --- | --- |
| Codex CLI | `codex-cli 0.140.0`，standalone 安裝（`codex doctor` 提示 0.154.0 可更新） |
| 登入模式 | ChatGPT（`account/read` → `type: chatgpt`, `planType: plus`；`stored API key: false`） |
| 額度（讀取時） | primary 5 小時視窗 68%、secondary 7 天視窗 68%、無 credits、`rateLimitReachedType: null` |
| 可見模型 | `model/list` 回傳 1 個：`gpt-5.5`；全域 config 預設 `model = gpt-6-astra`（**不在列表中**）；app-server 啟動記錄 `failed to load models cache: missing field base_instructions` |
| 全域設定 | `sandbox_mode = workspace-write`、`approval_policy = on-request`、`mcp_servers` 2 個、29 個 feature flag |
| 網路 | 無 proxy；WebSocket 至 ChatGPT backend 握手 101 |

| 能力 | 狀態 | 證據／阻擋原因 | phase |
| --- | --- | --- | --- |
| app-server stdio 初始化、帳戶、額度、模型、設定讀取 | 可用 | 握手成功（方法清單見 [`ai-provider-contract.md`](ai-provider-contract.md) §2） | — |
| protocol schema 產生（`codex app-server generate-json-schema`） | 可用 | 產出 `ClientRequest`／`ServerNotification`／`ServerRequest`／v2 共 261 個 schema 檔（暫存，未入 repo） | P15 用於產生 typed adapter |
| 有界真實生成（`thread/start` + `turn/start` + `outputSchema`） | 可規劃，**未實測** | schema 存在；P00 不啟動 turn | P15 |
| 生成環境隔離（清空 MCP／plugins／web search、read-only、no network） | **阻擋** | 本機全域 config 有 2 個 MCP server；per-thread `config` 覆寫是否能清空尚未驗證；獨立 `CODEX_HOME` 會失去登入且禁止複製 auth | P15 驗證；驗證前 unattended 模式禁用 |
| unattended 自動研究循環 | **阻擋** | 上列隔離未驗證；`UnknownOutcome` 恢復未實作 | P17（且需 P15 解除） |
| 模型 ID 凍結 | **阻擋（相容性問題）** | config 預設模型與 `model/list` 不一致、models cache 載入錯誤；必須以 `model/list` 實際結果為準 | P15 |
| 版本鎖定 | 阻擋（政策） | 0.140.0 為 experimental 子命令；P15 前必須重跑本節檢查並記錄實際版本 | P15 |
| API key provider（keyring） | 不啟用 | `secret_commands.rs` 為 `NotImplemented` stub；`keyring 4.2.0` 在 crates.io 可用；第一 provider 是訂閱，不走此路徑 | 保留 |
| Codex 自動更新／額度購買／帳號輪替 | 禁止 | 產品決策 | — |

## 3. 市場資料

| 能力 | 狀態 | 證據／阻擋原因 | phase |
| --- | --- | --- | --- |
| candles plausibility gate | 可用 | `market-data-quality-v1` | — |
| instrument／venue／calendar／provenance／snapshot 語意 | 可用 | P06（2026-09-20）：migration 0008＋`src-tauri/src/market/`＋雙語純契約 `market-foundation-v1`（`fixtures/rs-core/market-foundation-v1.json`）。instrument 修訂鏈、calendar 版本不可編輯、原件（含被拒絕者）與修訂鏈、coverage 稽核與 snapshot 生成皆有測試；0001 `datasets` 未改，沒有 snapshot 的 dataset 為 `legacy` 且不能自動取得資格。形狀與不變量：[`market-foundation-v1.md`](market-foundation-v1.md)。**未連網、未匯入任何正式資料** | — |
| Binance 封存與 REST | 可規劃 | `data.binance.vision` 封存索引 200、`api/v3/ping` 200（只探查，未下載） | P07 |
| Tiingo US ETF | **阻擋（無帳戶）** | host 可達（301）；無 token，免費權限、dividend／split 欄位未驗證 | P09（使用者先開通） |
| FinMind TW ETF | 未驗證權限 | API host 可達（422 空查詢）；配息／分割事件涵蓋未驗證 | P10 |
| 版本化 calendar（NYSE／TWSE） | 阻擋（資料缺失） | P06 已建立 calendar 註冊、版本不可編輯與「未註冊 calendar 即不得註冊該 instrument」的阻擋；`crypto-24x7-v1` 為唯一可由契約本身定義而內建者。`nyse-v1`／`twse-v1` 需要真實休市資料，**未取得、未捏造** | P09／P10 提供資料，P08 定義盤中 session |
| ETF 交易日年化／配息／分割語意 | 阻擋（語意缺失） | `barsPerYear('1d') = 365`（`backtestRunner.ts`）；metrics 無 dividend／split 概念 | P08（新版本 metrics 契約） |
| 多年 BTC／ETH 完整性 | 未驗證 | 只探查端點；AlphaBTC 歷史限制（缺 13 根、45 天）必須成為回歸案例 | P07 |

## 4. 研究紀律、DSL 與驗證

| 能力 | 狀態 | 證據／阻擋原因 | phase |
| --- | --- | --- | --- |
| Train／Validation／Test＋embargo、Gate、Score、benchmarks、validation records | 可用 | tasks.md Current Snapshot；854 vitest＋155 Rust＋62 e2e（P00 未重跑） | — |
| params-only discovery runner（暫停／恢復／取消／孤兒恢復） | 可用 | RUNNER-EXEC/STORE/CONFIG-001 | — |
| JSON DSL 執行 | **阻擋** | validator 無 runtime 消費者；24 個白名單指標中僅 11 個＋價格來源在兩核心實作（[`ai-provider-contract.md`](ai-provider-contract.md) §8） | P11 |
| hypothesis／attempt／lineage 完整歷史 | 阻擋（覆寫語意） | `backtest_summary` 依 strategy＋dataset＋segment upsert；trades 為最新結果 | P05 |
| 試驗帳本、統計精度預檢、block-bootstrap／Holm | 可規劃 | AlphaBTC 精度反例（`146/1001≈0.145854`）為必備回歸案例 | P12 |
| 一次性 Test 消耗 registry | 可規劃 | 現有 Test 從未執行（split contract）；尚無消耗紀錄 | P13 |
| 引擎封存與備份／還原 | 可規劃 | 只有 contract／identity hashes | P14 |
| Validation／Test 不進 prompt 的自動測試 | 可規劃 | 契約：ai-provider §7 | P13／P15 |

## 5. Paper 與 UI

| 能力 | 狀態 | 證據／阻擋原因 | phase |
| --- | --- | --- | --- |
| 共用成交／帳務核心 | 可規劃 | 既有 `backtest-execution-v1`；paper 模式無 | P18 |
| 單帳戶 paper、風控鎖、checkpoint | 可規劃 | 0001 lifecycle CHECK 限 `candidate|validated|rejected`；`paper_live` 保留 | P19／P20 |
| Results Explorer | 可用 | P01（2026-09-16，驗收修正 2026-09-17）：`ResultsExplorer.tsx`＋`get_backtest_result_detail`；Validation 排名、Test 隱藏、缺漏如實呈現、明細綁定畫面上的摘要列、讀取失敗不自動重試；不含 DSL 樹、服務重連（ABC-12／P21） | — |
| 工作中心、固定通知、接續上次工作 | 可規劃 | 目前操作訊息在頁首 | P21 |

## 6. 工具鏈與依賴（本機）

| 項目 | 值 |
| --- | --- |
| Rust／Cargo | 1.96.0 |
| Node／npm | v24.14.1／11.12.1 |
| Tauri CLI | 2.11.3 |
| crates.io | 可達（`cargo search`）；候選：`fs4 1.1.0`（OS 檔案鎖）、`keyring 4.2.0`（保留） |
| 已在 `Cargo.lock`（間接） | `tokio 1.52.3`、`reqwest 0.13.4`、`hyper 1.10.1`、`uuid 1.23.4`、`windows-sys 0.45.0` |
| 尚未加入 | `fs4`／`fd-lock`、`keyring`、`chrono-tz`、`axum`／`tiny_http` — 於所屬 phase 集中加入並鎖版。P06 未加入任何套件：日線以「交易日的 UTC 午夜」定義、交易日由版本化 calendar 資料列舉，因此不需要 `chrono-tz`（盤中 session 屬 P08，屆時再評估） |
| GitHub | `git fetch` 與 `gh` 皆 401（`GITHUB_TOKEN` 失效）；本機分析不受影響，push／PR 待恢復 |

## 7. 產品決策登記（凍結）

主專案 AlphaFactorForge；第一 provider Codex／ChatGPT 訂閱；Tauri 與 VS Code 共用
同一服務；Windows 背景運行；首批市場 Crypto（1h）、US ETF（1d）、TW ETF（1d）；免費
資料優先；收益／低回撤／平衡三種研究目標分開比較；模擬開戶預設人工確認、可對指定
計畫預先授權；AI 只產生經驗證 JSON DSL；不納入實盤。詳見 active-plan §1。

## 8. P00 結論

- 不依賴 AI 的 phase（P04–P14、P18–P19）依賴皆已到位，可依序規劃；P01、P02、P03a、P03b、P04a、P04b、P05、P06 已完成。
- **AI unattended 功能標為阻擋**，原因：生成環境隔離尚未驗證、模型清單與設定不一致、
  Codex 子命令為 experimental。P15 以一次有界真實生成解除或維持阻擋；不得改為付費
  API 或 GUI 點擊自動化。
- US ETF 接線（P09）在使用者開通 Tiingo 帳戶前為阻擋。
- 本次未啟動研究、未匯入正式資料、未新增程式／migration／依賴。
