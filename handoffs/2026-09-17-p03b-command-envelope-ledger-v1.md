# Handoff: P03b 命令 envelope 與事件帳本（`research-command-v1`、`research-event-v1`）

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P03a 驗收修正 `7d7f774` 之後續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: P03b 完成，ABC-01 隨之完成；下一個可執行 phase 為 P04（headless service、loopback API、connect 模式），須另行明確授權

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
