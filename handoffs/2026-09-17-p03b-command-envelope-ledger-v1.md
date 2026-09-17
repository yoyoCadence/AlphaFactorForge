# Handoff: P03b 命令 envelope 與事件帳本（`research-command-v1`、`research-event-v1`）

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P03a 驗收修正 `7d7f774` 之後續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: 第四次驗收通過（2026-09-17，Codex，受驗 commit `acf8079`）；H1／H2 關閉，Rust 205／Vitest 874；P04 尚未開始，另行授權

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

## Resolution — Codex 複驗 `c5832be`（2026-09-17）

Base：`e2e07e1`；分支 `docs/p00-contract-precheck`，複驗開始時 worktree clean。
**結論：仍不通過。** 原 cancel 回條恢復案例、R2 的漏事件偵測與 R3 的 final Busy
語意已通過；R1 的一般性重播承諾尚未成立，另外重同步 helper 有 M1。

### R1（High，未關閉）：效果列不是第一次命令結果，恢復仍會改寫答案

位置：`runtime/commands.rs:338–380`（`settle_pending`／`outcome_from_effect`）、
`discovery_runner/mod.rs:408–415`（run create 與 start 分開提交），以及同檔 652 行
（pause 遇 Completed 回成功）、1449 行起（僅 Paused transition 記錄 pause effect）。

新增三條確定性驗收 probe，皆使用真實 dispatcher／runner、既有 gated executor、
in-memory SQLite 的 TEMP TRIGGER；沒有替換產品邏輯，三條皆失敗：

1. **成功 start 被後來的 worker 失敗改成失敗**：暫時拒絕 `command_requests` UPDATE，
   呼叫合法 start，第一次得到 `Ok({runId:1})`；再放行會失敗的 worker，等待 run Failed
   與 coordinator 退出；解除回條故障、以相同 requestId 重送。
   `outcome_from_effect` 用目前 run.status=Failed 推斷「start 本身失敗」，重播變為
   `Validation: candidate 0 execution failed...`。這是後續計算結果，不是第一次 start
   的回應，不能拿來覆蓋原成功。
2. **失敗 start 被恢復成成功**：暫時拒絕 `discovery_jobs` INSERT 與回條 UPDATE。
   run create＋effect 已提交，後續 start transaction 失敗，因此第一次回 Validation。
   解除兩個故障後同 id 重送，效果列被解讀成成功，回 `Ok({runId:1})`；DB 的 run
   實際仍為 **idle、沒有 jobs／coordinator**。僅有 create effect 不代表整個 start
   已接受／完成初始化。崩潰停在兩個 transaction 之間也有相同歧義。
3. **completion 勝出的 pause 成功沒有效果列**：一個 candidate 的 run 正在 gated
   executor；拒絕回條 UPDATE，另一執行緒 dispatch pause，確定 ControlPhase 已為
   PauseRequested 後釋放 worker。最後一個 candidate 完成，runner 既有規則讓
   Completed 勝出，pause 第一次回成功；coordinator 退出後解除故障、重送同 id。
   `request_effects` 沒有 pause 的列，dispatcher 再執行 pause，回
   `Validation: discovery run 1 has no active coordinator`。原交接的「再執行 pause
   為 no-op」不符合實作；已 Paused／Completed 的 run 並沒有通用的 no-op 成功路徑。

修正要求：保存可重建**不可變第一次 outcome／命令接受階段**的持久依據，不用會持續
變動的 run.status 推測；區分「run row 建立」與「start 接受成功／初始化失敗」；pause
以 Completed 成功返回等沒有 Paused transition 的成功路徑也必須可恢復。
只刪除 `run.status == Failed` 分支，或將所有 effect 一律當成功，不能修復上述全部反例。
請以三條情境補回歸，並同時檢查 resume 初始化／無 effect 的失敗結果保存。

### M1（Medium）：gap 已包含在新 snapshot 中，仍永久要求重讀

位置：`src/services/researchCommand.ts:122–124` 的 `needsResnapshot` 使用
`ledgerGap.stateVersion >= snapshotVersion`，而 gap marker 是持久的、不會在讀 snapshot
時清除。

確定性 TypeScript probe：舊 snapshotVersion=5，事件頁為
`{events:[], stateVersion:7, ledgerGap:{stateVersion:7}}`，需要 resnapshot 是正確的。
讀者重讀後已取得包含取消狀態的 snapshotVersion=7；再收到相同頁面時 helper 仍回 true。
沒有新 domain write 時版本不再前進，照規則實作的 reader 會反覆重讀；單純讀取與
heartbeat 都不會解除。測試預期已補齊後為 false，實際為 true。

修正要求：只有尚未由 snapshot 覆蓋的 gap 才要求重讀，例如採嚴格較新的 version，
或明確的已確認 gap watermark。補「發現 gap → 讀取同版本 snapshot → 下一頁不再
要求重讀」完整循環，並保留較新 gap 仍會觸發的測試。

### 已確認通過與驗證範圍

- 原 R1 的 cancel 範例：effect 與 cancel transaction 共存，重送不再多發 Done，
  成功補寫 receipt；但不能因此推論四個 mutating command 都完成重播保證。
- 原 R2 的靜態漏事件案例：domain stateVersion 前進可偵測、持久 gap 可提示重新讀取，
  已解決原本 cursor 空頁卻完全無法知道漏掉終態的問題；M1 是讀取後無法停止重同步。
- 原 R3：已記錄 Busy 的第一次與重播均 retryable=false，需新 id，通過。
- 基線全套：`cargo test --locked` **196 passed（52 + 144）**；`npm.cmd test`
  **874 passed**；typecheck／build 通過；一般 cargo check 無 warning；
  `cargo clippy --locked --tests` 成功，僅既有 core 的 4 個 warnings。
- 擴充驗收：`cargo test --locked review_ -- --nocapture` **3 failed**（上述 R1
  三例）；builder 測試加入 M1 後 **9 passed／1 failed**。關鍵輸出：

  ```text
  later failure: first=Ok({runId:1}), retry=Err(Validation, candidate execution failed)
  partial start: first=Err(Validation, review_jobs), retry=Ok({runId:1}), status=idle
  pause/complete: first=Ok({runId:1}), effect=None, retry=Err(no active coordinator)
  needsResnapshot(7, pageVersion=7, gapVersion=7): expected false, received true
  ```

- 臨時 probes 已移除，產品與測試 source 回到 `c5832be`；只留下本複驗文件修改。
  未重跑 Playwright／原生 Tauri UI；未 commit／push。下一步先修 R1／M1，再確認
  P03b Done／ABC-01 Done，尚不應進 P04。

## Resolution — 複驗 R1／M1 修正與回歸（2026-09-17，Claude Code）

使用者要求修正；沿用 `docs/p00-contract-precheck`，保留上方兩次驗收紀錄。

- **R1 已關閉（第一次結果不可變重播）**：`request_effects` 改為 `request_outcomes`（migration 0006 在本分支
  尚未 push 前直接改寫，註明於此），每列記錄命令的**不可變第一次結果**：`begun`（開始改狀態）→ `accepted`
  （`{runId}`）／`rejected`（錯誤訊息），只有 `begun` 可被覆蓋。寫入時機＝決定該結果的 store transaction：
  - `start`：run row 建立 → `begun`；初始 checkpoint（`update_discovery_progress_with_outcomes`）→ `accepted`；
    `start_discovery_run` 失敗 → 獨立 `record_request_rejection`；checkpoint 失敗 → `fail_discovery_run_with_outcomes` 內 `rejected`。
  - `resume`：Paused→Running → `begun`；checkpoint → `accepted`；checkpoint 失敗 → fail tx 內 `rejected`。
  - `pause`：request id 掛在 `ControlState`，由**解決它的那個 transaction** 記錄：drain 的 Paused 轉換或
    completion 勝出 → `accepted`；run failure → `rejected`（`pause_failed_message`）；cancel 搶先 → `rejected`
    （`pause_cancelled_message`）。訊息由同一組函式產生，`pause()` 醒來後回答的就是被記錄的那句。
  - `cancel`：cancel tx 內 `accepted`。
  - coordinator 在 acceptance 之後 spawn 失敗：改為 **run 失敗**（status＋Done 事件）而非命令 Err，避免與已記錄的
    `accepted` 矛盾（thread spawn 失敗僅在 OS 資源耗盡時發生）。
  `Dispatcher::settle_pending`：執行中 → pending；有列 → `outcome_from_row` 逐字重播（accepted→Ok、rejected→同一
  `AppError→CommandError` 對映＋`final_error`、begun→「never completed its admission」終局錯誤）並補回條；無列 → 首次執行。
  無 domain 變更的失敗若回條也寫不進去 → 回覆 `retryable=true` 並註明「could not be recorded」（`begun`-only 不算已記錄）。
- **M1 已關閉**：`needsResnapshot` 改為 gap 版本**嚴格大於** snapshot 版本才要求重讀；補完整循環測試
  （發現 gap → 以該版本重讀 snapshot → 同頁不再要求；更新的 gap 仍觸發）。
- **回歸（`discovery_runner/tests/commands.rs`）**：
  `an_accepted_start_replays_ok_even_after_the_run_fails_later`（拒絕回條；worker 失敗使 run Failed；重送仍 Ok、run 數 1、回條修為 succeeded）；
  `a_start_that_failed_after_creating_its_run_replays_the_failure_not_a_success`（拒絕 jobs INSERT＋回條；第一次 Err；重送逐字同一 Err、run 仍 idle 無 jobs）；
  `a_start_that_only_began_replays_an_honest_failure`（再拒絕 request_outcomes UPDATE；第一次為未記錄的 retryable 錯誤；重送得「never completed its admission」、不建第二個 run）；
  `a_pause_that_completion_won_replays_its_success`（單 candidate；PauseRequested 後放行 → completion 勝出 → pause Ok；重送 Ok）；
  `a_resume_that_failed_after_beginning_replays_the_failure`（`BEFORE UPDATE OF progress_json` 拒絕 checkpoint；第一次 Err；重送同一 Err、不再 resume）。
  突變檢查：accepted 列改為推斷失敗 → case 1 紅；不記錄 partial-start rejection → case 2 紅；completion 不記錄 pause → case 3 紅；還原後全綠。
- 其餘只給測試用的無結果 wrapper（`complete_discovery_run`／`fail_discovery_run`）改 `#[cfg(test)]`，一般 build 0 warning。
- **驗證**：`cargo test --locked` **201 passed（52 + 149）**；`npm.cmd test` **874**；typecheck／build 通過；clippy 新模組無 warning；
  暫存目錄無殘留。Playwright 未重跑（無 UI 變更）；原生 Tauri 未執行。
- 殘餘限制（如實記錄）：coordinator 的 fail／complete／cancel transaction 本身失敗時，`pause` 仍依 phase 回答但沒有持久列
  （DB 層故障）；帳本 append 仍在 runner commit 之後、非同一 transaction，由 `stateVersion`／`ledgerGap` 補救。

## Resolution — Codex 第三次驗收 `d7a4e3e`（2026-09-17）

Base：`c5832be`；分支 `docs/p00-contract-precheck`，開始時 worktree clean。
**仍不通過**：前輪 start 後續失敗、partial start、completion 勝出的 pause 與 M1
均已有回歸且通過；但以下兩項仍會破壞相同 requestId 的結果一致性。

### H1（High）：reserve 與 in-flight claim 分離，重疊重送會再次執行並回不同答案

- 位置：`runtime/commands.rs:317–319` 的 Fresh／pending 分流，338 行起的
  `settle_pending`，363 行起的 `execute_and_record`。
- 確定性重現：A reserve 得 Fresh，**尚未**呼叫 `in_flight.begin` 時暫停；B 以
  相同 envelope／同一 dispatcher／同一 `InFlightRequests` 進來，讀到 pending，
  但 in-flight 為空、沒有 outcome，所以執行 cancel 並保存 accepted／succeeded，
  回 `Ok({runId:1})`。待 B 完成並清掉 in-flight 後，讓 A 繼續。A 仍使用舊的 Fresh
  判定，begin 成功後再次執行 cancel，回 `Validation: ... is cancelled`。
  第三次相同 id 依 receipt 重播 Ok。無需任何 DB 故障即可得到同 id 的不同終局答案。
- 驗收插樁只放在 reservation 區塊結束、match 之前，模擬 A 被排程器暫停而 B
  跑完的合法交錯；未修改 reserve／execute／receipt／outcome 實作。
- 修正要求：對同一 requestId 的「取得執行權 → 讀取並判定 receipt/outcome →
  執行或重播 → 完成」使用完整的互斥／claim 流程。若拿到執行權前曾釋放控制，
  必須在取得後重新讀取持久狀態，不能繼續使用先前的 Fresh 或無 outcome 判定。
  `settle_pending` 的 contains→read→begin 窗口也要一起處理；共享 set 本身不足以
  保證整段流程原子。補上述交錯的回歸，確認只執行一次且已完成後都回同一結果。

### H2（High）：cancel 回滾提前消耗 pause requestId，後續成功仍沒有可重播結果

- 位置：`discovery_runner/mod.rs:757–766`：先 `state.pause_request_id.take()`，
  再進入 cancel transaction；`pause_run_after_drain`、`complete_run_after_commit`、
  `fail_run_after_commit` 也有相同的提前 take 模式。
- 確定性重現：單 candidate gated run，dispatch pause 並等到 PauseRequested；
  暫時拒絕 command receipt UPDATE，另以 TEMP TRIGGER 只拒絕 run 的 Cancelled
  狀態 UPDATE。dispatch cancel，因此 cancel transaction 整筆 rollback。
  此時 run 仍 running、pause 仍等待，但 pause requestId 已從 ControlState 消失。
  移除 cancel trigger、放行 worker，completion 正常提交並令 pause 回成功；
  因 requestId 已丟失，沒有 pause outcome。解除 receipt 故障後同 id 重送，
  回 `Validation: discovery run 1 has no active coordinator`，不是原成功。
- 這不僅是「DB 持續壞掉，無法保存任何結果」：取消已回滾，後續 completion 的
  domain transaction **成功提交**，原本可以同時保存 pause 結果；資訊是在記憶體
  中先被消耗，SQLite rollback 無法替它恢復。
- 修正要求：transaction 成功提交後才清除 pause requestId，或失敗時恢復它。
  讓未解決的 pause 保留到真正決定它的 transition／completion／failure／cancel，
  並檢查所有提前 take 的分支，尤其 complete／pause transition 失敗後轉 fail 的路徑。
  補「cancel 回滾 → pause 後續成功 → receipt 恢復重播成功」的回歸。

### 驗證與工作目錄

- 基線 `cargo test --locked`：**201 passed（52 + 149）**；`npm.cmd test`：
  **874 passed**；typecheck／build 通過；cargo check 無 warning；clippy --tests
  成功，只有原有 core 的 4 個 warnings。
- 臨時 probes：`cargo test --locked review_ -- --nocapture` **2 failed**：

  ```text
  overlap: original=Err(Validation, ... is cancelled), duplicate=Ok({runId:1}), replay=Ok({runId:1})
  cancel rollback: pause=Ok({runId:1}), outcome=None, replay=Err(Validation, ... no active coordinator)
  ```

- H1 使用 cfg(test) 的一次性排程 callback；H2 使用真實 dispatcher／runner、gated
  executor 與 SQLite TEMP TRIGGER。測試與 callback 已移除，source 回到受驗 commit；
  僅本 handoff 有修改。未重跑 Playwright／原生 Tauri UI，未 commit／push。
- migration 0006 的本機未發布改寫已由原作者揭露；本次沒有原生 app-data 升級驗證，
  不把這次 fresh DB 測試解讀成舊版 0006 workspace 的升級證明。
- 下一步先修 H1/H2，再重新確認 P03b／ABC-01 的 Done；P04 尚未開始。

## Resolution — 第三次驗收 H1／H2 修正與回歸（2026-09-17，Claude Code）

使用者要求修正；沿用 `docs/p00-contract-precheck`，保留上方三次驗收紀錄。

- **H1 已關閉（重疊重送只執行一次、同答案）**：`InFlightRequests` 改為 per-request claim（`Mutex<HashSet>`＋`Condvar`，
  `claim(request_id)` 阻塞直到該 id 空閒，`RequestClaim` drop 時釋放並喚醒）。mutating envelope 在 **reserve 之前**取得 claim，
  並持有整段「reserve → 讀 receipt/outcome → 執行或重播 → 完成」；重複 envelope 在 claim 上等待，進入後重新讀取持久狀態
  （已有 receipt → 重播），不再沿用先前的 Fresh／無 outcome 判定。原本分離的 `begin`／`end`／`contains` 與「執行中→pending」回覆移除。
- **H2 已關閉（pause requestId 只在 transaction 提交後清除）**：cancel（含 control 分支）、drain 的 Paused 轉換、completion、
  failure 四處由 `take()` 改為 `clone()`，成功 commit 後才 `state.pause_request_id = None`；transaction 回滾時 pause 仍掛在
  ControlState，交給之後真正解決它的 transition／completion／failure／cancel 記錄結果。
- **回歸（`discovery_runner/tests/commands.rs`）**：
  `a_duplicate_sent_while_the_first_is_between_reservation_and_execution_waits_and_replays`（`#[cfg(test)]` thread-local hook
  `runtime::commands::test_hooks::AFTER_RESERVATION` 把 A 停在 reservation 之後；B 同 id 送入必須在 300 ms 內不返回且無事件；
  放行 A → A 執行、B 重播同一 Ok、Done 只多 1、receipt succeeded）；`overlapping_duplicates_execute_once_and_receive_the_same_outcome`
  （Barrier 同時送入，兩者同 Ok、Done 只多 1）；`a_rolled_back_cancel_leaves_the_pause_request_to_the_transaction_that_answers_it`
  （單 candidate；PauseRequested 後以 `BEFORE UPDATE OF status … WHEN NEW.status = 'cancelled'` 讓 cancel 回滾；run 仍 running、pause 仍等待、
  兩者皆無 outcome 列；放行 worker → completion 勝出 → pause Ok 且 outcome accepted；解除 receipt 故障後同 id 重送 → 重播 Ok）；
  單元測試 `a_duplicate_waits_for_the_claim_and_replays_instead_of_executing`（持有 claim 時 dispatch 阻塞、釋放後重播 receipt）。
  突變檢查：claim 不等待 → 確定性 H1 測試紅；cancel 改回 `take()` → H2 測試紅；還原後全綠。
- **驗證**：`cargo test --locked` **205 passed（52 + 153）**；`npm.cmd test` **874**；typecheck／build 通過；一般 build 0 warning；
  clippy 新模組無 warning；暫存目錄無殘留。Playwright 未重跑（無 UI 變更）；原生 Tauri 未執行。
- 範圍說明：claim 只覆蓋同一程序（同一 `InFlightRequests`）；跨程序的同 id 重送由 lease（只有 owner 能執行）與 `command_requests`
  reservation 的 `BEGIN IMMEDIATE` 串行化，本次未新增測試。migration 0006 的本機改寫維持前次揭露，未做舊 0006 workspace 升級驗證。

## Resolution — 第四次驗收通過（2026-09-17，Codex）

受驗 commit：`acf8079`，分支 `docs/p00-contract-precheck`；驗收開始時工作目錄乾淨。
本次核對修正 diff、呼叫端接線及回歸測試，並重跑下列檢查；未發現新的阻擋問題。

- **H1 關閉**：mutating dispatch 在 reservation 前取得 claim，guard 持有到執行／重播及 receipt
  回寫完成；等待者取得 claim 後重新 reserve／讀取持久結果。桌面 AppState 保存單一
  `Arc<InFlightRequests>`，每次建構 Dispatcher 都傳入其 clone，沒有每個命令另建 registry。
  reservation 與執行之間的重疊重送、Barrier 同時重送，以及直接持有 claim 的等待／重播測試均通過。
- **H2 關閉**：cancel、drain 的 Paused transition、completion、failure 四處均保留 pause requestId，
  在成功提交後才清除；前面步驟出錯時仍可交給後續決定結果的 transaction。
  TEMP TRIGGER 令 cancel 回滾、completion 隨後成功、receipt 恢復後重播原成功的回歸通過。
- **實跑驗證**：`cargo test --locked` **205 passed（52 + 153）**；`npm.cmd test`
  **874 passed（50 files）**；`npm.cmd run typecheck`、`npm.cmd run build` 通過；
  `cargo check --locked` 無 warning。`cargo clippy --locked --tests` 成功，仍有 core 既存的
  4 項 warning（backtest 的兩處 unnecessary_map_or、score 的 manual_range_contains／manual_clamp），
  本次修正模組沒有新增 warning。
- **驗收範圍**：本次沒有重跑作者所述突變檢查；沒有跨程序同 id 重送的新測試，也沒有舊版
  migration 0006 workspace 升級驗證。Playwright／原生 Tauri 未執行。上述結果不延伸為這些範圍的通過證明。
- **交接**：P03b 本次複驗通過；P04 仍待使用者另行授權。本次只更新本 handoff，保留先前驗收歷史，
  未修改產品程式碼，未 commit／push。
