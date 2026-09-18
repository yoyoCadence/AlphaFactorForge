# Handoff: P04b 桌面 connect 模式與背景切換（契約 §1.1 desktop-connect、§1.5）

Date: 2026-09-18
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P04a 驗收修正 `757b9f5` 之後續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: H1／M1 已關閉；H2 首頁競態已修正並驗證（2026-09-19，見最後 Resolution）；修正未 commit／push，P05 尚未開始

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

- ~~`ledgerGap`／`stateVersion` 只前進而無事件不重讀~~：驗收 H2 修正，見下方 Resolution。
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

## Review — P04b 驗收未通過（2026-09-18，Codex）

受驗 commit `2afdcb5`；分支 `docs/p00-contract-precheck`，驗收起始 worktree 乾淨。
P04a 四項修正已在 `757b9f5` 提交；本次檢查 P04b 桌面接線、切換失敗回復與事件同步。
既有 Rust 243 項測試全過，但三條隔離工作區故障 probes 均失敗。

### H1（High）— 收回桌面失敗後，Connected 模式永久失去事件轉送

- 位置：`src-tauri/src/runtime/host.rs:320–328`。`service::stop` 的結果尚未分支處理，就呼叫
  `connected.stop_following()`；Err 分支把同一個 connected 放回 slot，但不重新啟動 forwarder。
- 重現：啟動真實 service lifecycle、連接並啟動 forwarder；僅把隔離 workspace 的 token 檔暫時換成
  不相符的合法格式 token，讓 take-back 的 stop 回 401。桌面正確回 `SwitchFailed(now=desktop-connect)`；
  還原檔案後，原本已驗證的 proxy（仍持有正確 token）成功啟動 run，DB 正常 completed；等待 6 秒
  （超過 forwarder 的 5 秒 poll），仍完全沒有 Done 被轉送。
- 影響：看似已回復可用的背景模式，命令能執行，進度與結果卻不再更新；使用者沒有事件失聯提示。
  timeout／其他 stop error 也會經過同一條錯誤回復路徑。
- 修正要求：stop 失敗時保留正在工作的 forwarder，或在回復 Connected 前以原 cursor 恢復它；
  加入「take-back 失敗 → service 仍能執行 → 桌面仍收到完成」回歸。
- Probe：`review_failed_take_back_must_keep_forwarding`，失敗訊息：
  `restored Connected mode must still forward the service's Done event`。

### H2（High）— 帳本缺口資訊在 bridge 被丟棄，終態可永久漏讀

- 位置：`src-tauri/src/runtime/connect.rs:280–300` 的 `forward_loop` 只處理 events，忽略
  stateVersion／ledgerGap／ledgerDegraded；`ServiceProxy::active/progress` 也剝掉版本只回 run。
  視窗沒有其他定期 snapshot reconciliation。這違反契約 §3 已定案的缺口重讀規則，不能視為
  單純 P21 UX 整合；P04b 現在就是這個帳本的實際讀者。
- 重現：在隔離 DB 加 BEFORE INSERT trigger，只拒絕 `channel='discovery://done'` 的帳本 append，
  真實 service 完成一個 run。讀回 DB 是 completed，events API 有
  `ledgerGap={epoch:2,stateVersion:9}`；forwarder 只交出 progress／result／progress，
  沒有 Done，沒有任何要求重讀或標示 stale 的通知（lost=0、restored=0）。
- 影響：已提交的完成結果沒有遺失，但桌面停留舊狀態；必須由使用者自行刷新才會恢復。
  手動記錄限制並不滿足 P03b 已建立的「帳本 append 失敗不得讓 cursor 讀者永久漏讀」。
- 修正要求：bridge 保留 snapshot 版本並處理缺口／空頁版本前進，確實觸發視窗重讀（避免同一 gap
  在已涵蓋的 snapshot 上無限觸發）。同時注意 `DiscoveryPanel.tsx:230–235` 的 host 通知只讀 active：
  run 已終止時 active=null，目前會直接略過；應能重讀當前 runId 的 progress 來解決舊 running 狀態。
  另須處理 sink.emit 失敗仍推進 cursor 的情形，不能默默略過。
- Probe：`review_forwarder_must_signal_a_missing_terminal_ledger_event`，輸出：
  `db=completed, gap={epoch:2,stateVersion:9}, forwarded=[progress,result,progress], lost=0, restored=0`。

### M1（Medium）— service 重啟後，桌面永遠重試舊端點

- 位置：`src-tauri/src/runtime/connect.rs:250–317`。forwarder 捕獲固定 ControlClient，錯誤後只睡眠
  再對同一 port/token 重試；不重新讀 manifest／核對身分，也不更新 ConnectedHost 的 proxy。
  `DiscoveryPanel.tsx:228` 卻顯示「重試連線中」。
- 重現：service epoch 2 正常停止，等 forwarder 發出 lost；在同一 workspace 重啟 service 到 epoch 3。
  新端點與新 proxy 的 info 正常，舊桌面等 6 秒仍 `restored=0`、`desktop proxy live=false`。
  新服務已 ready；延長等待也不會改變固定 client 的端點。若 port 被重用但 token 改變，HTTP 401
  也只會寫 log，並非真正重新連接。
- 修正要求：失聯後重新發現端點、核對 workspace／instance／協定與 schema，再更新 host proxy 與
  forwarder；從 snapshot 與持久 cursor 恢復，不能只讓徽章變回可用。補同 workspace 重啟後
  讀取／命令／事件均恢復的回歸，並拒絕其他 workspace 的替代端點。
- Probe：`review_forwarder_must_reconnect_after_the_service_restarts`，失敗訊息：
  `new service is healthy, but restored=0, desktop proxy live=false`。

### 驗證與工作目錄

- 基線 `cargo test --locked`：**243 passed（52 + 189 + 2）**，含真實 service exe 的交接測試與 smoke。
- `npm.cmd test`：**877 passed**；typecheck／build 通過。`cargo check --locked` 無 warning；
  `cargo clippy --locked --all-targets` 成功，只有 core 既存 4 項 warning。
- Playwright 全套首輪 **71 passed／1 failed**：第一個 `code-validation.spec.ts` 在 `page.goto` 等待
  domcontentloaded 時超過 60 s，尚未執行功能斷言；P04b 三個 host-mode 案例皆過。僅重跑該失敗檔案後
  **1 passed（44.5 s）**，未修改測試或產品。不能把首輪描述成 72 項一次全綠。
- 臨時 probes：上述三條各自失敗；全部使用隔離 TEMP workspace，trigger／token 故障均還原，
  service 正常停止，DB handle 關閉後刪除測試目錄。未碰使用者的 app-data workspace。
- Probes 已移除，產品程式碼維持受驗 commit；僅本 handoff 修改。未 commit／push。
- `git diff --check` 通過；臨時 service 程序、aff-host-test 目錄與 E2E 的 5199 listener 均無殘留。
- 未重跑原生桌面視窗的按鈕操作。原作者已揭露 native smoke 是載入錯誤頁時的 Rust setup 檢查，
  不把它解讀為 WebView 到真實 service 的完整 UI hand-over 驗證。
- 下一步修正 H1／H2／M1，再複驗 P04b；目前不應把 P04 標為完整通過或進入 P05。

## Resolution — 驗收 H1／H2／M1 修正與回歸（2026-09-18，Claude Code）

使用者要求修正；沿用 `docs/p00-contract-precheck`，保留上方驗收紀錄。

- **H1 已關閉**：`take_back_from_service` 只在 `service::stop` 成功（或端點已不存在）後才 `stop_following()`；
  stop 失敗（401、逾時等）時放回 slot 的 `ConnectedHost` 帶著原本仍在跑的 forwarder。新增 `ConnectedHost::is_following`。
  回歸 `a_failed_take_back_keeps_forwarding_the_services_events`（把 token 檔換成合法格式但不符的 token → stop 401 →
  `SwitchFailed{now: desktop-connect}`；還原後以原 proxy 啟動 run → sink 收到 Done(completed)；再 take-back 成功）。
- **H2 已關閉**：`LedgerEventSink` 新增必要方法 `resnapshot_needed(reason, state_version)`（無預設實作）。forwarder 以
  第一頁的 `stateVersion`／`ledgerGap` 為「視窗已涵蓋」基準，之後：出現**新的** gap 標記、無事件但版本前進、`emit` 失敗
  （cursor 仍前進但不再靜默）、或重連成功，各通知一次。桌面 sink 發 `runtime://resnapshot`；前端 `parseResnapshotEvent`／
  `onResnapshotNeeded` 進 seam；`DiscoveryPanel` 的重讀改為「`getActiveRun` → 若 null 且有跟隨中的 runId 則 `progress(runId)`」，
  host 通知也走同一條。回歸 `a_missing_terminal_ledger_row_makes_the_window_re_read_once`（持久 trigger 拒絕 Done 列 append；
  DB completed、頁面有 `ledgerGap`、無 Done 轉送、恰一次 resnapshot 且 stateVersion 相符、`progress` 回 completed；再等一個
  poll 週期不重複通知）；Playwright `a missing terminal event is recovered by re-reading the run`（mock `discoveryDropDone=1`
  依真實 runner 只以 Done 宣告完成，不發終態 progress；面板經重讀顯示已完成、控制列與 DB 一致）。
- **M1 已關閉**：`verify_endpoint(data_dir, expected_workspace)` 抽出並由首次連接與每次重新發現共用（manifest → 期望
  workspaceId → `/v1/info` instanceId／workspaceId → 協定版本 → `db::open_migrated` schema）。forwarder 對任何讀帳本失敗
  （Io、401、protocol）都進入 lost 並每 1 s 重新發現；成功即替換 `ConnectedHost` 內 `Arc<Mutex<ServiceProxy>>`（命令端
  `proxy()` 取 clone）、`connection_restored` ＋ `resnapshot_needed("reconnected…")`；cursor 沿用。回歸
  `a_restarted_service_is_rediscovered_and_another_workspaces_endpoint_is_refused`（service 停止 → lost 一次；另一 workspace
  的 live manifest／token 複製進來 3 s 內 `restored=0`；移除後同 workspace 重啟 epoch 2 → `restored=1`、proxy 換成新
  instance、info 可用、有 reconnected 重讀、舊端點不可用；新 proxy `active`／`start` → Done 轉送）。
- **突變檢查**（各自單獨執行）：H1 把 `stop_following()` 移回 stop 之前 → 回歸紅（`the forwarder must survive a failed
  take-back`）；H2 停用 gap 分支 → 紅（`timed out waiting for the window to be told to re-read`）；M1 讓重新發現永遠失敗 →
  紅（`timed out waiting for the restored notification`）；面板停用 `onResnapshotNeeded` → Playwright 紅（`Received
  string: "mock run #2 · 執行中"`）。還原後全綠。
- 測試工具：`tests/host.rs` 的 `TempDir` 在 panic 展開中只回報不再 panic（失敗的測試留有 service 執行緒時，第二次 panic 會
  abort 整個測試 binary 並吞掉第一個訊息）；綠跑不留殘留。
- **驗證**：`cargo test --locked` **246 passed（52 + 192 + 2）**；`cargo check` 0 warning；clippy 只剩 core 既存 4 項；
  typecheck／build 通過；`npm.cmd test` **879**；Playwright 全套 **73 passed**（本輪一次全綠；上一輪驗收所見
  `code-validation.spec.ts` 首次導覽逾時為 dev server 冷啟動，非產品問題，未改測試）。原生桌面視窗操作未重跑。
- 範圍說明：`ServiceProxy::progress/active` 仍回傳 legacy snapshot 形狀（不含 stateVersion）；版本比對由 bridge 執行並以
  `runtime://resnapshot` 通知，視窗不需知道版本。嵌入模式的 emit 失敗行為未改（非本次範圍）。

## Review — 第二次驗收：H1／M1 通過，H2 尚未關閉（2026-09-18，Codex）

受驗 commit `ea9fa2b`，分支 `docs/p00-contract-precheck`，開始時 worktree 乾淨。

- **H1 關閉**：stop 失敗分支在停止 forwarder 前即回復 Connected，原轉送器仍運作。
  `a_failed_take_back_keeps_forwarding_the_services_events` 實跑通過，含失敗後新的 run 完成與 Done 轉送。
- **M1 關閉**：重新發現使用預期 workspaceId 驗證 manifest／info／協定／schema，成功後替換
  命令端共用的 proxy，沿用持久 cursor；另一 workspace 的端點會被拒絕。
  `a_restarted_service_is_rediscovered_and_another_workspaces_endpoint_is_refused` 實跑通過，
  包含重啟後的讀取、命令與 Done 轉送。

### H2（High，仍開啟）— 第一頁不等於視窗已涵蓋的 snapshot

- 位置：`alpha-factor-forge/src-tauri/src/runtime/connect.rs:353–395`。`known_version/known_gap`
  初始為 None，第一頁在 `primed=false` 時跳過所有 gap／版本檢查，然後把該頁版本與 gap 記成已知。
  程式註解假設視窗「around now」的 snapshot 已包含它們，但兩者沒有順序或版本交握。
- 視窗可先讀到 running，接著 coordinator 完成且 Done append 失敗；forwarder 第一頁才讀到該缺口。
  由於第一頁被當成已涵蓋，後續同一 gap／版本也不再通知，視窗仍可永久停在 running。
  這是前次 H2 的首頁時序變體，並非新增其他 phase 的需求。
- **確定性重現**（真實 runner、Dispatcher、HTTP、gated executor；無產品路徑修改）：
  1. 啟動單 candidate run，worker 暫停；讀取真實 `discovery.active` 得 running、stateVersion=5，保存 cursor。
  2. SQLite TEMP TRIGGER 只拒絕 Done 帳本列；放行 worker，DB completed，gap={epoch:1,stateVersion:7}。
  3. 讓 forwarder 以保存的 cursor 讀第一頁，代表其背景執行緒第一次讀取晚於視窗 snapshot。
     它轉送 result／progress，但沒有 Done；再等超過一個 5 s poll，仍沒有重讀通知。
  4. `review_first_page_gap_after_the_window_snapshot_must_not_be_assumed_covered` 失敗：

     ```text
     snapshotVersion=5, gap={"epoch":1,"stateVersion":7}, rereads=[]
     a gap newer than the real window snapshot must request reconciliation even on the first page
     ```

- **修正要求**：不能由第一次 events page 推斷視窗已涵蓋的版本。建立初始化的 snapshot／事件訂閱交握，
  或在無法證明涵蓋時保守要求重讀（須確保視窗已能收到通知）；以實際 snapshot 涵蓋的版本決定 gap
  是否已解決，再做同一 marker 的去重。保留既有「後續同一 gap 不無限重讀」回歸，另補本次首頁時序。
  請勿只靠延長 sleep 讓第一頁先回來，使測試避開競態。

### 本次驗證與工作目錄

- 基線 `cargo test --locked` **246 passed（52 + 192 + 2）**；原作者三條修正回歸皆過，
  但上述新增首頁 probe 失敗。probe 的 worker／forwarder 已停止、trigger 已移除，臨時測試已移除。
- `npm.cmd test` **879 passed**；typecheck／build 通過；`cargo check --locked` 0 warning；
  `cargo clippy --locked --all-targets` 成功，僅 core 既存 4 項 warning。
- Playwright 聚焦 `e2e/host-mode.spec.ts` **4 passed**，含缺失 Done 後的視窗重讀；本次沒有重跑其餘 69 項。
- 產品程式碼回到受驗 commit；僅本 handoff 修改，未 commit／push。原生 Tauri 視窗操作未重跑。
- 下一步只需處理仍開啟的 H2 並複驗；本次不重開已通過的 H1／M1，P05 尚未開始。

## Resolution — H2 首頁競態與初始化訂閱順序（2026-09-19，Codex）

依使用者「請你修正」處理本輪唯一未關閉的 H2。H1／M1 的修正保留；P05 未開始。

- **bridge**：移除第一頁 `primed=false` 跳過 gap 檢查的假設。第一頁若有缺口同樣通知，沒有缺口也因不知
  視窗涵蓋範圍而保守要求一次同步。`known_version/known_gap` 僅表示已觀察／通知的帳本資訊，並非視窗
  已讀取的版本；同一 marker 後續不重複通知。
- **視窗**：合併探索事件與 runtime 事件的初始化，全部訂閱完成後才讀初始 snapshot。因此早於 listener
  的通知由初始讀取涵蓋，之後的通知會排入重讀。重讀依序執行；讀取途中收到新通知時不採用過時回應，
  再讀一次。保留剛讀到的 runId，即使第二次 active 已為空，也能讀該 run 的 progress，取回 terminal 狀態。
  Done 同樣排入此流程，仍立即取消節流並更新終態；只有讀取完成才顯示「已重新讀取進度」。失敗顯示錯誤
  與資料可能過時提示，僅有排隊中的新通知才再讀，不因失敗自行重試。
- **Rust 回歸**：`a_first_page_gap_after_the_window_snapshot_requests_reconciliation` 使用真實 Dispatcher／
  HTTP／runner 與 gated executor：先讀 running snapshot，TEMP TRIGGER 拒絕 Done append，完成後才啟動
  forwarder。第一頁即要求重讀。原有後續相同 gap 不重複通知測試保留，允許獨立的一次初始同步。
- **UI 回歸**：DEV mock `discoveryStartupGap=before-listener|during-snapshot` 固定注入兩種時序，沒有 timer
  或後續事件替畫面補救。前者通知在 listener 安裝前遺失，畫面仍讀到完成數 **2/4**；後者先捕捉 paused
  snapshot，再完成 run 並只通知缺口，畫面最後讀到 **completed、4/4**，而非停在 paused。
- **突變檢查**：只還原 bridge → 新 Rust 回歸紅（通知數 **0 != 1**）；只還原面板 → 兩條新 Playwright
  分別紅於 **1/4 != 2/4**、**已暫停 != 已完成**。UI 首次突變執行的第一條曾開頁逾時，未將逾時算作
  證據；單獨重跑後取得上述實際狀態斷言失敗。所有突變均以 finally 還原修正。
- **驗證**：`cargo test --locked` **247 passed（52 + 193 + 2）**；`cargo check --locked` 0 warning；
  `cargo clippy --locked --all-targets` 僅既有 core 4 項；Vitest **879 passed**；typecheck／build 通過。
  聚焦 host-mode Playwright **6 passed**；修正版全套 **74 passed + 1 開頁逾時**（`code-validation.spec.ts`
  的首次 `page.goto`，尚未到產品斷言），使用同一已就緒 dev server 單獨重跑該項 **1 passed**，合計
  **75 項皆取得通過結果**，不是宣稱全套一次全綠。沒有修改該項測試或放寬 timeout。
- 文件同步：契約 §1.5、`tasks.md`、`CHANGELOG.md`。本次不變更 wire protocol／schema，採保守初始化同步；
  原生 Tauri 視窗操作未重跑。修改尚未 commit／push。
