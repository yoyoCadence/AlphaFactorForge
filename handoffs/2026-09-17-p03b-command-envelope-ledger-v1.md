# Handoff: P03b 命令 envelope 與事件帳本（`research-command-v1`、`research-event-v1`）

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P03a 驗收修正 `7d7f774` 之後續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: P03b 驗收 R1/R2/R3 已修正並有回歸（2026-09-17，Rust 196／vitest 874）；待重新驗收後再進 P04

## Summary

Plan §5 P03 的後半：契約 §2（命令 envelope、冪等 `requestId`）與 §3（持久 `eventId` 帳本、snapshot＋cursor
重連）。使用者於 P03a 驗收修正後指示「繼續下個任務」。Rust 為主，TypeScript 只加 typed client 與純
builder；**UI 尚未改走 envelope**（P04 bridge 的工作），既有桌面命令行為不變、但其事件也進帳本。

## 執行前驗證

- 契約 §2／§3 在 P03a 之後可直接建於 `ownership::write_transaction`（Codex 修正 R2 後的寫入邊界）：
  預約、完成、帳本 append 都走同一邊界，舊 owner 三者皆被拒。
- 契約要求「同一 `requestId` 重送回傳**第一次**的結果；不重複建立 run」：採 **reserve-then-complete**
  （先插 `pending` 列再執行），第一次未完成就重送 → `Busy`（retryable，訊息說明 outcome 未知、請查狀態後換新 id），
  不會 silently 重跑；這與 Plan §4.2 對 `UnknownOutcome` 的態度一致。
- `eventId` 跨重啟唯一：`runtime_events.event_id INTEGER PRIMARY KEY AUTOINCREMENT`（`sqlite_sequence` 記高水位，
  刪列後也不重用）。
- `workspaceId` 為「工作區資料目錄的穩定識別」：migration 0005 以 `randomblob(16)` 鑄造一次寫入 `app_settings`，
  不由路徑推導（搬目錄不變）。
- 帳本 append 在 runner commit **之後**（sink 內）：commit 與 append 之間崩潰會少一筆帳本列，但快照仍含該結果
  （重連先讀 snapshot），符合「漏失事件不能抹掉已提交結果」；未把 append 塞進每個 store transaction（那會再改所有寫入點）。

## 修改檔案

| 檔案 | 變更 |
| --- | --- |
| `src-tauri/migrations/0005_runtime_ledger.sql` | 新增：`command_requests`、`runtime_events`、`workspace_id` |
| `src-tauri/src/db/runtime_ledger.rs` | 新增：`workspace_id`、`reserve_request`（Fresh／Replay／Conflict）、`complete_request`、`purge_old_requests`（7 天，pending 永不清）、`append_event`、`read_events_after`、`last_event_id`；5 個測試 |
| `src-tauri/src/runtime/commands.rs` | 新增：`CommandEnvelope`（deny_unknown_fields）、`ErrorCode`×9、`CommandError`（含 `From<AppError>` 對映）、`EventEnvelope`、`Command` 白名單、`Dispatcher`、`LedgerSink`；8 個測試 |
| `src-tauri/src/runtime/mod.rs` | `Workspace.workspace_id`；`open_workspace` 於恢復後 purge 舊請求並讀 id |
| `src-tauri/src/commands/runtime_commands.rs` | 新增：`get_workspace_info`、`dispatch_research_command`（以 `CommandError` 拒絕） |
| `src-tauri/src/commands/discovery_commands.rs` | start／resume／cancel 的 Tauri sink 包一層 `LedgerSink` |
| `src-tauri/src/main.rs` | `AppState.workspace_id`；註冊兩個新命令 |
| `src-tauri/src/discovery_runner/tests/commands.rs` | 新增：真實 run 的三次同 requestId 測試＋帳本 cursor 分頁對照 |
| `src-tauri/src/db/mod.rs`、`repositories.rs`、`runtime/boundary_tests.rs`、`discovery_runner/tests.rs` | 註冊 0005、migration 數 4→5、掃描清單、子模組 |
| `src/tauri-client/commands.ts` | `CommandEnvelope`／`CommandError`／`EventEnvelope`／`WorkspaceInfo` 型別；`runtime.info`／`runtime.dispatch` |
| `src/services/researchCommand.ts`（＋`.test.ts`） | 純 builder：`buildCommandEnvelope`、`newRequestId`、`isCommandError`、`shouldRetrySameRequest`；測試以 `?raw` 掃描 Rust 原始碼 pin 版本字串、白名單、mutating 集合、錯誤碼 |
| `tasks.md`、`CHANGELOG.md`、契約狀態／§6、registry、`STRATEGY_DISCOVERY.md` §4 新小節、README 三語 | ABC-01 規格要求的文件同步 |

未動：mock client／dataClient seam（UI 未用）、e2e、既有 migration 0001–0004。

## 設計要點

- **reads 不記帳**（`discovery.progress`／`active`、`events.read`、`ownership.read`）：無副作用可重複。
- **replay 逐字**：成功回同一 result JSON，失敗回同一 `CommandError`（含 code／retryable）。
- **同 id 不同內容 → `DuplicateRequest`**（payload 以排序鍵的 canonical JSON 做 SHA-256；陣列順序敏感）。
- **AppError → 錯誤碼**：`NotOwner`／`StaleOwner` 保留；訊息含 not found → `NotFound`；含 already／busy／lock
  poisoned → `Busy`（retryable）；其餘 `Validation`。這是啟發式，P04 若需要更精確可在 `AppError` 加變體。
- **`LedgerSink` 失敗仍轉發**：帳本寫不進去會 `eprintln!`，但宿主仍收到事件（DB 才是真相）。
- **桌面既有命令也進帳本**：不論 run 由舊命令或 envelope 啟動，`events.read` 同一 cursor 可讀。

## Verification

| 項目 | 結果 |
| --- | --- |
| `cargo check --locked`（一般 build） | 0 warning |
| `cargo clippy --locked --tests` | 新模組無 warning |
| `cargo test --locked` | 52 + 141 = **193 passed**（+14） |
| `npm.cmd run typecheck`、`npm.cmd run build` | 通過 |
| `npm.cmd test` | **873 passed**（+8） |
| 突變檢查 | 預約永遠 Fresh → 回放測試與真實 run 測試失敗；sink 不 append → 帳本測試與真實 run 測試失敗；還原後全綠 |
| Playwright | 未重跑（無 UI 變更） |
| 原生 `cargo tauri dev` | 未執行 |
| 暫存目錄 | 測試後無 `aff-*` 殘留 |

## 對 P04 的交接點

- service binary：`open_workspace(path, HolderKind::Service)` → `Dispatcher::new(db, discovery, epoch, workspace_id, sink)`；
  loopback API 只需把 HTTP body 交給 `dispatch`，錯誤直接序列化 `CommandError`。
- 桌面 connect 模式：`NotOwner` 時改以 loopback 送 envelope；重連 = `discovery.active` → `events.read {afterEventId}`；
  前端 builder 已有（`services/researchCommand.ts`），`runtime.info()` 提供 `workspaceId`。
- §1.5 背景切換：停收新工作 → checkpoint → drop `OwnershipHandle` → 啟動 service；service 取得 epoch+1 後，桌面舊
  runner 的任何寫入都是 `StaleOwner`（P03a），舊請求列因 epoch 記錄可辨識。
- 若 P04 要「事件在同一 transaction 內入帳」，需改 store 寫入點以 `append_event` 併入 `write_transaction`；本次刻意未做。

## Resolution (added when acted on)

（待補：push／PR 編號、review 結果。）

## Resolution — Codex acceptance review（2026-09-17）

Target：`e2e07e1`，base `7d7f774`；分支 `docs/p00-contract-precheck`，驗收開始時
worktree clean。**本次暫不通過**：三項可重現問題如下。P04 的 transport／connect UI
不在本次範圍；未因 UI 尚未改用 envelope 而提出缺漏。

### R1（High）：回條寫入失敗仍回成功，卻無法重播該成功

- 位置：`alpha-factor-forge/src-tauri/src/runtime/commands.rs:299–319`，尤其
  `complete_request` 失敗僅印 log，仍回傳原 `outcome`；`replay`（389 行起）對 pending
  只能永久回 Busy。
- 重現：建立 paused run；以 SQLite `BEFORE UPDATE ON command_requests` trigger
  暫時拒絕回條更新 → 經真實 dispatcher 執行 `discovery.cancel` → 刪除 trigger、以
  相同 requestId 重送。第一次回 `Ok({"runId":1})`，run 已 Cancelled，但 request
  列仍 pending；故障解除後重送仍回 Busy，不能取得第一次的結果。
- 這不只是「程序崩潰造成 outcome 未知」：服務明知回條沒有保存，仍發出普通成功
  回應，違反 §2 的第一次結果重播承諾。pending 不清除且沒有恢復機制，重送也無法修復。
- 修正要求：讓可對外確認的結果有持久重播依據，例如將 domain mutation 與回條一起
  提交，或持久保存 request→domain result 關聯供恢復。回條不能保存時須明確回報
  outcome／durability 未確認，不能只 log 後當成正常成功；不得藉重跑 domain command
  來補回條。加入故障解除後相同 requestId 可恢復／重播且不重做 mutation 的回歸測試。

### R2（High）：帳本 append 失敗後沒有補記或重同步訊號，cursor 讀者永久漏掉終態

- 位置：`alpha-factor-forge/src-tauri/src/runtime/commands.rs:488–494`，
  `LedgerSink::emit` 吞掉 append error，仍轉送宿主；mutation 與 append 是不同提交。
- 重現：cursor 讀者先取得 run 的 paused snapshot 與 cursor 0 → 暫時以
  `BEFORE INSERT ON runtime_events` trigger 拒絕事件 → dispatcher 成功 cancel →
  刪除 trigger → `events.read {afterEventId:0}`。DB 為 Cancelled，宿主收到 1 個
  Done，但頁面一直為 `{"events":[],"lastEventId":0}`，沒有 gap／resync 指示。
- 原交接已揭露 commit→append 的崩潰窗口，但「snapshot 仍有結果」不足以保證
  §3 重連協定：snapshot 在取消之前已讀完，後續只追 cursor 的 reader 不會知道要
  再讀 snapshot。即使程序沒有崩潰、寫入故障已解除，也無法補到這個終態。
- 修正要求：用與 domain commit 同 transaction 的 durable outbox／帳本，或提供
  持久、可檢測的 gap 與強制 resnapshot 協定，確保 append 失敗後最終仍可追上狀態。
  不能僅以 eprintln 或宿主收到舊格式事件作為持久 cursor 的替代。補「snapshot 之後
  發生 mutation、append 失敗、恢復後 reader 可追上」的回歸。

### R3（Medium）：已永久記錄的 Busy 仍標為可用相同 requestId 重試

- 位置：`runtime/commands.rs:97–114` 的錯誤對映、299–308 的 failed 回條保存、
  389 行起的逐字 replay，以及 `src/services/researchCommand.ts:107–113` 的
  `shouldRetrySameRequest`。
- 重現：既有 paused run 佔住全域 slot → 新 requestId 的合法 discovery.start 得到
  `Busy(retryable=true)` 並被記為 failed → 取消舊 run 釋放 slot → 同 id 重送，仍逐字
  回原 Busy；換新 id 才成功建立 run 2。TypeScript helper 對前者持續回 true。
- 修正要求：明確區分「可輪詢的 pending／尚未保留請求」與「已決定、只會重播的
  終局失敗」。後者不能宣稱同 id 重試會取得進展；可在第一次保存失敗前調整 retry
  disposition，或擴充明確的 retry/new-intent 協定並同步 TS helper。不要讓 backend
  的「inspect state and use a new requestId」文字與結構化 retry 語意互相矛盾。

### 驗證與重現證據

- `cargo test --locked`：**193 passed（52 + 141）**。
- `npm.cmd test`：**873 passed**；另外新 builder 測試單獨跑 **8 passed**。
- `npm.cmd run typecheck`、`npm.cmd run build` 通過。
- `cargo check --locked` 無 warning；`cargo clippy --locked --tests` 成功，只見既有
  core 的 4 個 warnings（lib/test 重複列出），新模組無 warning。
- 臨時在既有 runner command tests 中加入 3 條保護性斷言，執行
  `cargo test --locked review_ -- --nocapture`：**3 failed**，關鍵輸出：

  ```text
  receipt: first={"runId":1}, stored=pending, retry=Busy(retryable=true)
  ledger: snapshot="paused", persisted=Cancelled, host_events=1,
          page={"events":[],"lastEventId":0}
  busy: first=Busy(retryable=true), after slot released=同一 Busy,
        fresh request={"runId":2}
  ```

- 臨時 probes 僅使用 in-memory DB／SQLite triggers 與既有真實 dispatcher/runner，
  未替換產品邏輯；已移除，source 回到受驗提交，只留下本驗收文件修改。
- 事件分頁本身可按頁內最後一筆 `eventId` 正確前進；`lastEventId` 是已明載的全域
  high-water mark，不當成 page next cursor。本次未將此設計列為缺陷。
- 未重跑 Playwright／原生 Tauri UI；本次未 commit／push。`tasks.md` 與 ABC-01
  目前的 Done 宣告應在以上問題修正後重新確認。

## Resolution — R1/R2/R3 修正與回歸（2026-09-17，Claude Code）

使用者要求修正；沿用 `docs/p00-contract-precheck`，保留上方驗收紀錄。

- **R1 已關閉（成功結果可重播）**：migration `0006_request_effects` 新增 `request_effects`（`request_id` PK →
  `run_id`／`command`／`epoch`），由 `create_discovery_run_with_effect`、`transition_run_with_effect`、
  `cancel_discovery_run_with_effect` 在**與 domain 變更同一 transaction** 內寫入；pause 的 request id 放在
  `ControlState.pause_request_id`，由 coordinator 的 drain 轉換寫入。`Dispatcher::settle_pending` 對 pending 回條
  依序判定：① 同程序仍在執行（共用 `InFlightRequests`）→ `Busy(retryable=true)`；② 有效果列 → 由效果列還原第一次
  結果（start 且 run 已 failed 時還原為該失敗）、補寫回條、**不重做**；③ 無效果列且無執行中 → 第一次執行。
  回條寫入失敗只 log 並回傳原結果——因效果列已持久，重送可復原（不再是無法重播的成功）。
- **R2 已關閉（不會永久漏讀）**：`runtime_state.mutation_seq` 由每個 domain `write_transaction` 遞增；
  heartbeat／回條／帳本 append 改用 `write_transaction_quiet`，不動版本。`discovery.active`／`progress` 回
  `{ run, stateVersion }`（先讀版本再讀 snapshot），`events.read` 回 `stateVersion`（先讀事件再讀版本）、
  `ledgerGap`（append 失敗時 best-effort 寫入 `app_settings`）與 `ledgerDegraded`。讀者規則（TS `needsResnapshot`）：
  空頁面且版本前進、或 gap 在 snapshot 之後 → 重讀 snapshot。
- **R3 已關閉（重試提示一致）**：`execute_and_record` 對即將記錄的錯誤套 `final_error`（`retryable=false`、訊息附
  「send a new requestId」），第一次回應與重播一致；`retryable=true` 只留給 pending（`pending_error`）。
  TS `CommandError.retryable` 文件與 `shouldRetrySameRequest` 說明同步。
- **回歸（`discovery_runner/tests/commands.rs`，真實 dispatcher／runner／gated executor，SQLite `TEMP TRIGGER`）**：
  `a_success_whose_receipt_failed_is_recovered_on_retry_without_executing_again`（拒絕回條 UPDATE → 第一次 Ok、列 pending、
  效果列存在、Done 事件 1 → 解除後同 id 重送 Ok 相同、Done 仍 1、列變 succeeded、第三次為純 replay；新 id 取消已取消 run
  為終局失敗）；`a_reader_following_the_cursor_detects_a_missed_event_and_resnapshots`（拒絕帳本 INSERT → cancel 成功、
  宿主收到 Done → 解除後 cursor 頁面空但 `stateVersion` 前進、`ledgerGap`／`ledgerDegraded` 標記 → 重讀 snapshot 得 cancelled）；
  `a_recorded_busy_is_final_and_not_labelled_retryable`（paused run 佔 slot → 新 id start 得 `Busy(retryable=false)` 附新 id 提示
  → 取消後同 id 重送逐字相同、run 數仍 1 → 新 id 建立 run 2）。三條各自在拿掉修正後失敗（效果列不查／版本不遞增／不套
  final_error），還原後全綠。
- 另：只剩測試使用的無效果 store wrapper（`create_discovery_run`／`transition_run`／`cancel_discovery_run`）改為
  `#[cfg(test)]`，一般 `cargo check` 0 warning；clippy 新模組無 warning。
- **驗證**：`cargo test --locked` **196 passed（52 + 144）**；`npm.cmd test` **874**；typecheck／build 通過；
  Playwright 未重跑（無 UI 變更）；原生 Tauri 未執行。暫存目錄無 `aff-*` 殘留。
- 已知限制（如實記錄）：效果列覆蓋四個 mutating 命令的 domain 變更；`pause` 在 run 已 Paused／Completed 時不做轉換也不寫效果列，
  此時回條失敗後的重送會再執行一次 pause（對已暫停 run 為 no-op）。帳本 append 仍在 runner commit 之後、非同一 transaction，
  漏讀由 `stateVersion`／`ledgerGap` 補救而非杜絕。
