# Handoff: P04b 桌面 connect 模式與背景切換（契約 §1.1 desktop-connect、§1.5）

Date: 2026-09-18
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P04a 驗收修正 `757b9f5` 之後續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: 實作完成、本機驗證通過（Rust 243／Vitest 877／Playwright 72／原生 smoke），待 Codex 驗收；P04 至此完整，P05 另行授權

## Summary

Plan §5 P04 的後半。使用者於 P04a 驗收修正 commit 後指示「繼續下個任務」。本次讓桌面成為契約 §1.1 的第二種
宿主：**嵌入**（自己持有工作區，一如既往）或 **connect**（背景 service 持有，桌面只做代理），並實作 §1.5 的
背景切換與其反向。UI 只加一個徽章與一顆按鈕（探索面板標題列）；既有探索面板、事件 parser、feed reducer 都
沒有改，因為 connect 模式下視窗收到的就是同一組事件與 snapshot。

P04 驗收「關 UI 工作持續；重連採用同一 run；嵌入模式 native smoke 保留」在本次的證明：

- **關 UI 工作持續**：桌面內進行中的 run 經「在背景繼續」在 checkpoint 交給 service，桌面轉 connect；此後關閉視窗
  不影響 service 內的 coordinator（Rust `tests/host.rs` hand-over 測試；原生 smoke 見下）。
- **重連採用同一 run**：桌面對執行中的 service 啟動 → `discovery.active` 拿到同一 run → forwarder 從帳本現況
  cursor 轉送（Rust；Playwright `host-mode.spec.ts` 第二個測試以 mock connect 模式採用 paused run 並代理 resume）。
- **嵌入模式 native smoke 保留**：`main.rs` 在空工作區仍自己持有、建庫、migration、recovery；CI lane 未改；
  本機以真實 desktop binary 驗證兩條路徑。

## 執行前驗證

- **桌面在 connect 模式如何讀資料庫**：Phase A／B 的 repository 命令（datasets、strategies、results、validation
  records）需要 SQLite 連線；契約只限制 migration／recovery／runner 寫入給 holder。新增 `db::open_migrated`：
  不遷移開庫、同一組 pragma、schema 必須**恰好**等於本 build（缺 → `SchemaPending`「owner 是舊 build」，
  多 → `SchemaTooNew`，檔案不存在 → owner 尚未建立）。使用者自己的儲存（save_backtest_result 等）走這條
  連線，與 service 併發由 WAL＋busy_timeout 處理；`run_migrations` 在 connect 模式拒絕。
- **命令期間不能持有模式鎖**：代理呼叫是 HTTP 往返（最長 10 s＋long-poll），若在 `Mutex<HostMode>` 內執行會卡住
  所有 DB 讀取。設計為命令先在鎖內複製 `HostSnapshot`（嵌入：db／runner／epoch；connect：`ServiceProxy`
  clone），出鎖後才執行。
- **切換時的競態（對應 P04a 驗收 H1）**：桌面 mutating 命令在通過模式檢查後可能還在執行 `runner.start`；
  hand-over 若只掃描既有 coordinator 會漏掉。新增 `Admission`：每個桌面 mutating 命令整段持有 guard，
  hand-over 先關門並等待 guard 歸零，再要求 coordinator checkpoint。
- **釋放鎖後失敗的回滾**：launch 失敗或 service 沒在 30 s 內發布端點 → 重新取鎖回嵌入（epoch +1）；連取鎖都失敗
  才進 `switching` 並附原因，`get_host_status.detail` 顯示。
- **service exe 位置**：與桌面 binary 同目錄的 `alpha-factor-forge-service(.exe)`（同 package 同 target dir；
  `cargo tauri dev` 與 `tauri build --no-bundle` 皆如此）；不存在時按鈕 disabled 並在 title 顯示路徑。
  安裝包打包屬 P22。
- **detached 啟動**：Windows 以 `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`（std `CommandExt::creation_flags`，
  無 crate），stdout／stderr 導向工作區 `service.log`，桌面不 wait child。
- **forwarder 的 cursor**：從連接當下的帳本尾端開始（`ledger_end` = `events?afterEventId=i64::MAX` 的
  `lastEventId`），歷史交給視窗自己的 `get_active_discovery_run` snapshot；視窗既有 reducer 以 `sequence`
  丟棄早於 snapshot 的事件，所以 cursor 與 snapshot 之間的重疊無害。
- **`AFF_DATA_DIR`**：原生 smoke 需要在不碰使用者 `%APPDATA%` 工作區的情況下啟動真實 desktop binary；
  兩個 binary 都讀同一個環境變數（未設定時行為不變；CI smoke lane 仍看 `%APPDATA%`）。

## 修改檔案

- `src-tauri/src/db/mod.rs`：`open_migrated`；`error.rs`：`AppError::SchemaPending`。
- `src-tauri/src/runtime/connect.rs`（新）：`LedgerEventSink` trait、`ConnectError`、`connect()`（manifest →
  `/v1/info` 核對 instanceId／workspaceId／協定版本 → 不遷移開庫）、`ServiceProxy`（`start/pause/resume/cancel/
  progress/active/dispatch/info/ledger_end`，legacy 命令的 requestId 用 CSPRNG `desktop-<16 hex>`）、
  `ConnectedHost`（proxy＋db＋forwarder）、`EventForwarder`（5 s long-poll、cursor 單調、失聯／恢復通知、1 s 重試）。
- `src-tauri/src/runtime/host.rs`（新）：`HostMode`、`Admission`／`AdmissionGuard`、`HostError`、
  `open_or_connect`、`ServiceLauncher`／`ExecutableLauncher`／`service_executable_beside`、
  `hand_over_to_service`、`take_back_from_service`（`service::stop` → 重取鎖 ≤ 10 s）。
- `src-tauri/src/runtime/control_api.rs`：`random_hex` 抽出（token 與 requestId 共用）。
- `src-tauri/src/runtime/service.rs`：`DATA_DIR_ENV`／`default_data_dir` 讀 `AFF_DATA_DIR`。
- `src-tauri/src/main.rs`：`AppState { host: Mutex<HostMode>, admission, data_dir, in_flight, launcher }`、
  `HostSnapshot`、`AppState::db/mode_kind/snapshot`；setup 改走 `host::open_or_connect`，connect 時啟動 forwarder；
  三個新命令註冊。
- `src-tauri/src/commands/discovery_commands.rs`：每個命令依 `HostSnapshot` 分支（嵌入：原路徑＋admission guard；
  connect：proxy）；`get_discovery_progress`／`get_active_discovery_run` 回傳 JSON（兩模式同 bytes）。
- `src-tauri/src/commands/runtime_commands.rs`：`WorkspaceInfo.host_mode`；`dispatch_research_command` 分支；
  `HostStatus`、`get_host_status`、`enter_background_mode`、`exit_background_mode`（blocking 執行緒內以
  `app.state()` 取 state，無 unsafe）；`announce_host` 發 `runtime://host`。
- `src-tauri/src/commands/db_commands.rs`：`state.db()?`；`run_migrations` 在 connect 模式拒絕。
- `src-tauri/src/desktop/discovery_events.rs`：`impl LedgerEventSink for TauriDiscoveryEventSink`（只轉送三個
  已知 channel）、`HOST_EVENT`／`HostEvent`／`host_changed`。
- `src-tauri/src/runtime/mod.rs`、`boundary_tests.rs`（新檔入守衛）、`single_instance.rs`（來源守衛錨點改為
  `host::open_or_connect`，並禁止 main 直接呼叫 `open_workspace(`）。
- 測試：`discovery_runner/tests/host.rs`（新，5）、`tests/control_api.rs`（forwarder 順序／一次性）、
  `runtime/mod.rs`（`open_migrated`）。
- 前端：`tauri-client/commands.ts`（`HostMode`、`HostStatus`、`runtime.hostStatus/enterBackgroundMode/
  exitBackgroundMode`、`WorkspaceInfo.hostMode`）、`events.ts`（`RUNTIME_HOST_EVENT`、`parseHostEvent`、
  `onHostChanged`）、`dataClient.ts`（seam 加 `runtime`／`runtimeEvents`）、`mockClient.ts`（模擬兩模式、
  `hostMode=`／`hostSwitchFail=1`）、`components/DiscoveryPanel.tsx`（徽章＋按鈕＋模式變更重讀 snapshot）、
  `events.test.ts`（+3）、`e2e/host-mode.spec.ts`（新，3）。
- 文件：`tasks.md`、`CHANGELOG.md`、契約（狀態、§1.1、§1.5 實作段、§6）、registry（三列）、
  `STRATEGY_DISCOVERY.md` §4、README 三語、本 handoff。

## Verification

- `cargo check --locked` 0 warning（兩個 binary）；`cargo clippy --locked --all-targets` 只剩 core 既存 4 項。
- `cargo test --locked` **243 passed（52 + 189 + 2）**，新增 7：
  - `discovery_runner::tests::host`：`the_desktop_owns_a_free_workspace_and_connects_to_a_published_service`（空工作區
    → 嵌入 epoch 1；有鎖無端點 → `CannotConnect(NotPublished)`；service 執行中 → connect epoch 2、同 workspaceId、
    非 owner 連線可讀、forwarder 在 service 停止後恰好通知一次失聯）；
    `a_hand_over_moves_an_in_flight_run_to_the_service_at_a_checkpoint_and_the_take_back_returns_it`（gated run 在
    candidate 0；hand-over 在 PauseRequested 等待 300 ms 不返回、模式為 switching、admission 拒絕；放行 → connect；
    service 看到 **paused** run 且 completed=1、桌面 coordinator 已退出；proxy resume → forwarder 收到 result 與
    Done(completed)、completed=2；take-back → 嵌入 epoch 3、recovery 報告為零、端點撤下）；
    `a_hand_over_waits_for_admitted_commands_and_a_failed_launch_returns_to_embedded`（持有 guard 時 hand-over 不
    前進且新命令被拒；放行後 launch 失敗 → `SwitchFailed{now: desktop-embedded}`、epoch 2、鎖在桌面手上；
    嵌入模式下 take-back 被拒）；`a_take_back_after_the_service_died_still_re_owns_the_workspace`；
    `the_executable_launcher_hands_over_to_a_real_service_process_when_the_binary_is_built`（**真實 service exe**
    detached 啟動、pid 不同、`service.log` 存在；take-back 後 port 不回應、log 有 shutdown requested／stopped）。
  - `tests::control_api::the_forwarder_replays_the_ledger_after_its_cursor_in_order_and_exactly_once`（第一個 run 的
    歷史不轉送；第二個 run 的事件全在 cursor 後、sequence 嚴格遞增無重複、Done 一筆、與帳本筆數相等）。
  - `runtime::tests::a_non_owner_opens_only_an_exactly_migrated_database`。
- `npm.cmd run typecheck`、`npm.cmd run build` 通過；`npm.cmd test` **877**（+3 `parseHostEvent`／`onHostChanged`）。
- `npm.cmd run e2e`（`--workers=1`、`E2E_PORT=5199`）**72 passed**（+3 `host-mode.spec.ts`：嵌入→背景→收回、
  connect 模式啟動採用 paused run 並代理 resume 到完成、切換失敗回報 fallback 模式且按鈕仍可用）。
- **原生 smoke（隔離工作區 `AFF_DATA_DIR=C:\tmp\aff-native`，真實 debug binaries，已清除）**：
  (1) service 先啟動 → desktop 啟動 stderr `connected to the background service (epoch 1, port 53199)`，兩程序並存，
  `service stop` exit 0；(2) desktop 先啟動（嵌入、建庫）→ `service run` exit 2「another host owns this workspace」。
  桌面視窗因無 dev server 顯示連線錯誤頁，Rust setup 路徑完整執行。**未做**：透過視窗按鈕的原生 hand-over
  （需 `cargo tauri dev` 與人工點擊）；等價路徑由 `the_executable_launcher_…` 測試以真實 service 程序覆蓋。
- 暫存目錄無 `aff-*` 殘留；無殘留 `alpha-factor*` 程序。

## 未完成項目／已知限制

- **`ledgerGap`／`stateVersion` 只前進而無事件**：forwarder 不轉送，視窗也不主動重讀（與嵌入模式的 emit 失敗
  行為一致）；視窗端整合 `needsResnapshot` 屬 P21。
- **service 未包進安裝包**、**Windows 未明確設 ACL**、**無 signal handler**：維持 P04a 揭露；P22。
- **take-back 的重取鎖視窗**：`service stop` 回報後 OS 鎖仍有毫秒級延遲，`relock` 重試 10 s；逾時進 `switching`。
- **switching 狀態的恢復**：若 hand-over 釋放鎖後 launch 失敗且重取鎖也失敗（例如另一宿主趁隙取得），桌面停在
  `switching` 並顯示原因；需重啟桌面。未提供 UI 內重試。
- **connect 模式的 `dispatch_research_command`**：透傳 caller 的 envelope；transport 失敗回 `Busy`（retryable），
  與 P03b 語意一致（未預約）。
- **Playwright 只證明視窗端**：mock 的 runtime 不模擬 service 失聯通知（`serviceReachable=false`）的 UI 路徑；
  該路徑有 Rust 端測試（forwarder 通知一次）與 `parseHostEvent` 測試，未有瀏覽器層測試。

## 對 P05／後續的交接點

- 任何新的桌面命令若在 connect 模式也要能用，需在 `HostSnapshot` 兩支各實作（或明確在 connect 模式拒絕）。
  純 repository 讀寫用 `state.db()?` 即兩模式通用。
- MCP adapter（P16）可直接用 `runtime::connect::ServiceProxy`／`ControlClient`；`AFF_DATA_DIR` 亦適用。
- 新的 runner 事件 channel 必須同時加入 `TauriDiscoveryEventSink::emit`（LedgerEventSink）的白名單，否則 connect
  模式下不會轉送。

## Resolution (added when acted on)

（待補：Codex 驗收結果、push／PR 編號。）
