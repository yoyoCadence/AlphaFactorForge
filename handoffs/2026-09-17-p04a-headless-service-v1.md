# Handoff: P04a 無介面 service binary 與 loopback 控制介面（`control-endpoint-v1`）

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P03b 第四次驗收通過 `1030e2f` 之後續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: 實作完成、本機驗證通過（Rust 232／Vitest 874），待 Codex 驗收；P04b（桌面 connect 模式）另行授權

## Summary

Plan §5 P04「Headless 與 bridge」的前半。P04 完整範圍（service binary、loopback API、桌面 connect 模式、
背景切換、native smoke）超過一個 session 可在本專案驗收標準下完成的量，因此比照 P03a／P03b 切成
**P04a（service binary＋控制介面，本次）** 與 **P04b（桌面 connect 模式＋背景切換＋native smoke）**，
`tasks.md` phase 表已改列兩列。使用者於 P03b 第四次驗收後指示「繼續下個任務」。

本次交付：同一 Cargo package 的第二個 binary `alpha-factor-forge-service`，以 host kind `service`
持有工作區（P03a 同一 `open_workspace`），把 P03b 的 `Dispatcher` 掛在 `127.0.0.1` 動態 port 的
HTTP/1.1 JSON 控制介面後面，manifest 與隨機 token 發布在工作區目錄；`stop` 子命令讓它 drain 到
checkpoint 再退出。**桌面 binary 完全未動**（`main.rs` 仍在 `NotOwner` 時停止啟動）；UI 未改。

P04 驗收「關 UI 工作持續；重連採用同一 run；嵌入模式 native smoke 保留」在本次的對應：前兩項在
runtime 層以 Rust 測試證明（啟動 run 的 client 消失後 run 繼續；新 client 以 `discovery.active` 拿到
同一 run、以 `events.read` cursor 續接並被 runner 事件喚醒）；第三項因桌面未改而保留（CI native
smoke lane 以檔名鎖定 `alpha-factor-forge.exe`，第二個 exe 不會被誤測）。桌面「重連採同一 run」的
UI 端證明屬 P04b。

## 執行前驗證

- **計畫 vs 程式碼**：§3.1「在既有 Cargo package 增加 service binary，與桌面共用 orchestration 模組」→
  第二個 `[[bin]]` 宣告同一組 `db`／`discovery_runner`／`runtime` 模組檔（一個 package 只能有一個 lib，
  而 `lib.rs` 必須維持純核心邊界，所以不能把 orchestration 搬進 lib）。代價是共用模組編譯兩次；
  service bin `test = false` 讓共用模組的單元測試只在桌面 target 跑一次。
- **資料目錄一致**：tauri 2.11 `app_data_dir` = `dirs::data_dir().join(identifier)`（已查 registry 原始碼）。
  service 用同一個 `dirs` crate（已在 `Cargo.lock`）＋常數 `APP_IDENTIFIER`，`boundary_tests` 以
  `tauri.conf.json` 釘住常數；Windows 測試斷言等於 `%APPDATA%\com.alphafactorforge.desktop`
  （CI smoke lane 看的路徑）。
- **依賴**：`dirs 6.0.0`、`getrandom 0.3.4` 都是 tauri 的直接依賴、已在 lock；本次只升為本 package
  直接依賴，**沒有新 crate**（`Cargo.lock` 差異只有 package 依賴清單兩行；lock 格式版本保持 3）。
  `hyper`／`tokio` 雖在 lock，但未採用：控制介面只需「一連線一請求、JSON、`Connection: close`」，
  以 `std::net` 手寫可把 keep-alive／pipelining／chunked／upgrade 全部排除在外，規則逐條有測試。
- **契約 §4 二擇一**：事件採 **long-poll**（`waitMs` ≤ 30 s），不採 SSE；理由：client 端不需串流 parser，
  與 `Connection: close` 模型一致，且 §3 重連流程（snapshot → cursor）本來就是分頁語意。
- **契約 §4「檔案權限限目前使用者」**：Unix 以 `mode(0o600)` 建檔；Windows 無法不加 crate 設 ACL，
  依 `%APPDATA%` 使用者 profile 目錄的預設繼承 ACL（已在文件與程式註解揭露，見未完成項目）。
- **Windows Ctrl+C**：std 沒有 signal handler，未加 crate；Ctrl+C／kill 等同崩潰路徑，由 P02／P03a 的
  啟動 recovery 處理（run → paused）；留下的 manifest 以 `/v1/info` 的 `instanceId` 核對判為過期。

## 修改檔案

- `alpha-factor-forge/src-tauri/Cargo.toml`：`default-run = "alpha-factor-forge"`；`[[bin]] alpha-factor-forge-service`
  （`src/service_main.rs`，`test = false`）；直接依賴 `dirs = "6"`、`getrandom = "0.3"`。`Cargo.lock` 僅該兩行。
- `src/service_main.rs`（新）：宣告 `db`／`discovery_runner`／`error`／`identity`／`runtime`，呼叫
  `runtime::service::main`；crate 級 `#![allow(dead_code)]`（共用模組中桌面專用項目在此本來就不用）。
- `src/runtime/service.rs`（新）：`parse_args`（`run|stop|status [--data-dir]|--help`）、`default_data_dir`、
  `run`（lease → dispatcher → bind → 發布 → 服務 → drain → 撤下端點 → 釋放）、`drain`（pause 每個活著的
  coordinator，等其退出，上限 60 s）、`stop`（核對 manifest 與 `/v1/info` 的 `instanceId`，送 shutdown，
  等 port 消失且 manifest 撤下）、`status`、`main`（exit code 0／1／2 NotOwner／3 SchemaTooNew／4 無 service／64 usage）。
- `src/runtime/control_api.rs`（新）：`ControlToken`（CSPRNG、Debug 遮罩、constant-time 比對）、
  `EndpointManifest` 與 `write/read/remove_endpoint_files`（暫存＋原子更名）、`EventNotifier`／`NotifyingSink`、
  `ControlServer`（accept loop、每連線一執行緒、上限 32、lingering close）、請求解析（head ≤ 16 KiB、
  `Content-Length` 必填 ≤ 1 MiB、拒 chunked、重複 Host／Content-Length 400）、Host／Origin／Bearer 檢查、
  路由 `/v1/info`／`/v1/commands`／`/v1/events`（long-poll）／`/v1/shutdown`。
- `src/runtime/control_client.rs`（新）：`ControlClient`（`info`／`dispatch`／`events`／`shutdown`／`request`）、
  `send_raw`（測試用原始請求）。
- `src/runtime/commands.rs`：`Dispatcher::events_page` 由 `events.read` 分支抽出（long-poll 共用，頁面形狀不變）；
  `CommandError::new` 改 `pub(crate)`。
- `src/discovery_runner/mod.rs`：新增 `active_coordinator_run_ids()`（drain 用）。
- `src/runtime/mod.rs`：宣告三個新模組（桌面 bin 內 `#[allow(dead_code)]`，P04b 使用後移除）。
- `src/runtime/boundary_tests.rs`：守衛清單加入四個新檔；新增 `the_service_identifier_is_the_desktop_bundle_identifier`。
- `src/discovery_runner/tests/control_api.rs`（新）、`tests.rs` 註冊；`tests/service_smoke.rs`（新，整合測試，
  spawn 真實 binary）。
- 文件：`tasks.md`（P04 → P04a Done／P04b Next eligible、Done 條目、測試數）、`CHANGELOG.md`、
  `docs/research-runtime-contract.md`（狀態、§1.1、§4 定案＋§4.1 生命週期、§6）、
  `docs/autonomous-research-capability-registry.md`（三列）、`STRATEGY_DISCOVERY.md` §4 小節、README 三語
  （狀態段落＋中文 Windows 啟動說明加 service 用法）、本 handoff。

## 設計要點

- **Drain 順序**：shutdown 後 mutating 命令回 503 `Busy`／`retryable=true`（從未預約，同 requestId 可交給下一
  owner），讀取命令照常（跟隨關閉的 client 仍看得到進度）；pause 所有 coordinator → 等 Paused checkpoint 在
  **本 epoch** 提交並退出 → 停止監聽 → **撤下 manifest／token → 才釋放 lease**（反過來會刪到下一個 owner 剛
  發布的檔案）。超過 60 s 仍活著就釋放，交給下一 owner 的 recovery。
- **過期 manifest**：崩潰的 service 會留下 manifest。任何讀者（`stop`／`status`、之後的桌面 connect）必須先
  `/v1/info` 核對 `instanceId`／`workspaceId`；`stop` 不刪未核對的檔案。新 service 啟動時直接覆寫。
- **Long-poll 不漏喚醒**：先取 notifier 序號、再讀頁面、再等待「序號變動」；`LedgerSink` 先 append 再轉送給
  host sink（即 notifier），所以被喚醒的讀者一定讀得到觸發它的事件；`stateVersion` 前進（無事件的狀態變化）或
  shutdown 也立即返回。
- **Lingering close**：431／413 等在請求讀完前就拒絕的情況，回應後 `shutdown(Write)` 並短暫（≤ 1 s、≤ 1 MiB+16 KiB）
  讀掉 client 仍在送的資料，避免 Windows 上 RST 吃掉回應（測試 `routes_methods_and_bodies_are_bounded` 一開始
  就因此失敗，修正後穩定）。
- **狀態碼對應**：`Validation`／`UnsupportedProtocol`／`WorkspaceMismatch` 400、`Unauthorized` 401、`NotFound` 404、
  `DuplicateRequest`／`NotOwner`／`StaleOwner` 409、`Busy` 503；body 一律 `{"error": CommandError}`，client 以 body 為準。

## Verification

- `cargo check --locked`：兩個 binary 皆 0 warning。`cargo clippy --locked --all-targets`：新模組無 warning
  （lib 既存 4 項未動）。
- `cargo test --locked`：**232 passed**（lib 52＋desktop bin 178＋`service_smoke` 2）。新增 27 個測試（Windows；Unix 另有 1 個 0600 權限測試）：
  - `runtime::control_api`（15）：token 生成／解析／constant-time；manifest 往返、token 不進 manifest、版本不符或
    token 缺／壞拒絕、Unix 0600；head 解析（版本、absolute-form、重複 header）；Host 規則；真實 socket：無 token／
    錯 token／其他 scheme 401、Origin／外部 Host／錯 port 403、缺 Host 400、404／405／411／413／400／431、
    command error 狀態碼與 body；long-poll 等滿 `waitMs`、`waitMs=0` 立即、被 notifier／shutdown 喚醒；shutdown 拒
    mutating 留 reads 並喚醒宿主；stop 後 port 不再回應。
  - `runtime::control_client`（2）、`runtime::service`（5）：參數解析、預設資料目錄；`run` 發布端點→`info`→雙啟 `NotOwner`
    exit 2→`status` 看到 live→`stop` Stopped→`run` 回 Ok→端點撤下→OS 鎖可再取得；無人回應的 manifest 判 Stale
    不刪；較新 schema 在發布前就以 exit 3 停止。
  - `runtime::boundary_tests`（1）：識別碼釘住 `tauri.conf.json`；四個新檔加入「不得引用 tauri」守衛。
  - `discovery_runner::tests::control_api`（2）：**關 UI 工作持續／重連採同一 run**（gated executor：client A 啟動
    2-candidate run 後被 drop；放行 candidate 0 → candidate 1 在無 client 下開始；client B `discovery.active` 得同一
    runId、`completedCandidates=1`；`events` 從 0 讀到 result 無 done；long-poll 掛在 cursor 後 300 ms 不返回，
    放行 candidate 1 後 <10 s 返回且事件 id 全在 cursor 之後；run completed、Done 恰一筆、cursor 之後無事件、
    `active` 回 null）；**drain 到 checkpoint、下一 owner resume**（shutdown 後 start 被拒 `Busy`；`drain` 在
    PauseRequested 等 300 ms 不返回；期間 `discovery.progress` 仍回應；放行後 drain 返回、coordinator 清空、run
    Paused、completed=1；epoch 2 的 runner `recover_orphans` 報告為零、`resume` 後 completed=2）。
  - `tests/service_smoke.rs`（2）：spawn 真實 `alpha-factor-forge-service.exe --data-dir <temp>`：manifest 發布、
    pid 相符、DB 與 lock 檔存在；401 無 token；`/v1/info`；`ownership.read` envelope；WorkspaceMismatch 400；
    events 空頁；第二個 service exit 2 且 stderr 說明；`status` exit 0 且 live pid 相符；`stop` exit 0、程序 exit 0、
    端點撤下、port 不回應、log 有 listening／shutdown requested／stopped 且**不含 token**；之後 `stop`／`status`
    exit 4；再啟動得 epoch 2 後 stop；`--help` 0、未知參數 64。
- 手動：以 `curl.exe` 對真實 binary（隔離目錄 `C:\tmp\aff-svc-manual`，已清除）：無 token 401＋`WWW-Authenticate: Bearer`；
  有 token 200 info；`Origin` 403；`http://localhost:<port>` 200；POST `discovery.active` envelope → `{"result":{"run":null,"stateVersion":1}}`；
  `waitMs=1000` long-poll 實測 1.07 s；`status`／`stop` 子命令與 log 正常。
- 突變檢查：把 `NotifyingSink::emit` 的 `notify()` 拿掉 → `a_run_started_over_the_api_…` 在「the poll returns once the runner emits: Timeout」處紅（10.35 s）；還原後綠（0.35 s）。
- 前端：`npm.cmd run typecheck` 通過、`npm.cmd test` **874**（無 TS 變更）。Playwright 未跑（無 UI 變更）；
  原生 Tauri 桌面未執行（`main.rs` 未動，`cargo test` 已連結 `alpha-factor-forge.exe`）。
- 暫存：測試工作區皆以 guard 移除；`%TEMP%` 無 `aff-control-api-test-*`／`aff-service-*` 殘留。
- 隔離：所有測試與手動操作皆用 `--data-dir` 暫存目錄，未觸碰 `%APPDATA%\com.alphafactorforge.desktop`。

## 未完成項目／已知限制

- **桌面 connect 模式（P04b）**：桌面仍自己持有工作區；桌面開著時 service 拿不到鎖（exit 2），反之桌面啟動會
  panic。桌面經 manifest 連 service、事件轉送到視窗、§1.5 背景切換（停收→checkpoint→drop lease→啟 service）、
  UI 顯示宿主模式、native smoke 都在 P04b。`runtime/control_client.rs` 已可供桌面使用。
- **Windows 檔案 ACL**：manifest／token 只靠 `%APPDATA%` 的繼承 ACL，未呼叫 Windows API 明確設限（需 `windows-sys`
  直接依賴或 FFI）；`--data-dir` 指到非 profile 目錄時權限由該目錄決定。
- **無 signal handler**：Ctrl+C／kill 為崩潰路徑；服務包裝（Task Scheduler／服務帳戶）屬 P22。
- **service 未包進安裝包**：`tauri build` 只 bundle 主 binary；CI native smoke lane 未變。
- **同程序外的重疊重送**：跨程序同 requestId 仍靠 P03b 的 reservation 串行化；控制介面本身不加鎖。
- **`stop` 的等待**：以「port 不回應且 manifest 已撤」判定，之後 `drop(workspace)` 仍有毫秒級視窗才釋放 OS 鎖；
  P04b 的切換需短暫重試取鎖。

## 對 P04b 的交接點

- 桌面 `main.rs` `setup`：`open_workspace` 回 `NotOwner` 時改讀 `control_api::read_endpoint_files(data_dir)`，
  以 `ControlClient::info` 核對 `instanceId`；`AppState` 需一個 `HostMode { Embedded{..}, Connected{ client } }`。
- 連接模式的 discovery 命令：`ControlClient::dispatch(envelope)`（前端 `services/researchCommand.ts` 已有 builder；
  或由 Rust 端把既有 `start_discovery` 等命令轉成 envelope）。事件：一條背景執行緒 `events(after, None, 30 s)`
  long-poll，把 `payload` 以原本的 `discovery://*` event 名稱 emit 到視窗（`research-event-v1` 的 `channel` 即事件名）。
- 連接模式的 DB 讀取（datasets／strategies／results）：需要不跑 migration 的唯讀開啟（`db::open_at` 目前一定
  migrate）；只有 lease holder 可 migration。
- 背景切換：桌面 pause 進行中的 run → drop `OwnershipHandle` → spawn `alpha-factor-forge-service run` → 等 manifest
  → 進入 connect。反向（service → 桌面）：`service::stop` 等價的 client 呼叫 → 取鎖（短暫重試）→ 嵌入模式。

## Resolution (added when acted on)

（待補：Codex 驗收結果、push／PR 編號。）
