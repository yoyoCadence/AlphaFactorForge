# Handoff: P03a Workspace ownership（`ownership-lease-v1` §1、§5.2）

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P02 之後同分支續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: P03a review R1/R2 已修正並通過回歸（2026-09-17，Rust 179）；依使用者指示納入本次本機 commit，未 push；P03b 仍須另行明確授權

## Summary

Plan §5 的 P03「OS lock、epoch、冪等請求、事件帳本、migration 保護」超過一個工作階段，比照
ABC-02a／02b 拆成兩個可獨立合併的子項。本次 P03a 完成 **lease 順序、epoch 檢查、heartbeat、
migration 保護**；**P03b** 留下 §2 冪等 `requestId` 與 §3 持久 `eventId`／snapshot＋cursor。
使用者於 P02 驗收後指示「繼續下個任務」。純 Rust 變更；前端無變動。

## 執行前驗證

- 契約 §1.2 的固定順序（OS 鎖 → 開 SQLite（busy_timeout）→ migration → ownership row → 孤兒恢復
  → heartbeat）可直接放進 P02 的 `runtime::open_workspace`，桌面 `main.rs` 不需知道細節。
- 契約 §1.3「每個 runner 寫入 transaction 必須帶目前 epoch」：runner 的寫入點共 8 處（start、
  resume、cancel ×2、候選提交、pause checkpoint、complete、fail）＋ recovery，均在同一
  mutex guard 內先檢查；候選提交另在 transaction 內檢查（`CandidateAssessment.epoch`）。
- OS 鎖：std `File::try_lock`（Rust 1.89 穩定）在本機 1.96 實測第二把鎖被擋、釋放後可取得；
  CI 用 `dtolnay/rust-toolchain@stable`，故 `rust-version` 由 1.77.2 提到 1.89 而不加 crate
  （Plan 原預期 `fs4`；registry 已更新）。
- 不可動的既有約束：CI smoke lane 的 DB／WAL 斷言不受影響（多一個 `ownership.lock`）；
  `single_instance::tests` 順序守衛改錨定 `runtime::open_workspace(&db_path,`。

## 修改檔案

| 檔案 | 變更 |
| --- | --- |
| `src-tauri/migrations/0004_workspace_ownership.sql` | 新增：單列 `workspace_ownership`（epoch、holder_kind、holder_instance_id、pid、acquired_at、heartbeat_at、heartbeat_seq），seed epoch 0 |
| `src-tauri/src/db/ownership.rs` | 新增：`acquire`、`assert_epoch`（`StaleOwner`）、`heartbeat`、`read`、`lease_is_stale`、`HEARTBEAT_PERIOD=5s`／`STALE_AFTER=30s`；5 個測試 |
| `src-tauri/src/runtime/lease.rs` | 新增：`try_lock_workspace(data_dir)` → `OsLock`（`NotOwner` 於 WouldBlock）；2 個測試 |
| `src-tauri/src/runtime/mod.rs` | `open_workspace(path, HolderKind)` 依契約順序；`Workspace.ownership: OwnershipHandle`（鎖＋epoch＋heartbeat 執行緒，drop 先停 beat 再放鎖）；`open_workspace_with(.., period)` 供測試；4 個新測試、既有 3 個改為 4 個 migration |
| `src-tauri/src/db/mod.rs` | 註冊 0004；`apply_migrations` 遇未知版本回 `SchemaTooNew`（整個開啟失敗） |
| `src-tauri/src/db/discovery.rs` | `CandidateAssessment.epoch`；`assert_owner(conn, Option<i64>)`；commit transaction 內先檢查 |
| `src-tauri/src/discovery_runner/mod.rs` | `DiscoveryRunner.epoch`、`with_epoch`、`epoch()`；`ControlState.epoch`；8 個寫入點＋recovery 先 `assert_owner` |
| `src-tauri/src/error.rs` | `NotOwner`、`StaleOwner`、`SchemaTooNew` |
| `src-tauri/src/main.rs` | `AppState.ownership`；`open_workspace(.., DesktopEmbedded)`；失敗 panic 附原因（connect 模式待 P04） |
| `src-tauri/src/single_instance.rs`、`runtime/boundary_tests.rs`、`db/discovery_tests.rs`、`db/repositories.rs` | 順序錨點、掃描清單、3 個 literal 補 `epoch: None`、migration 數 3→4、新測試 |
| `src-tauri/Cargo.toml` | `rust-version = "1.89"` |
| `tasks.md`、`CHANGELOG.md`、契約 §狀態／§6、registry | 狀態同步 |

未動：TypeScript、e2e、`execution.rs`、runner 事件測試、既有 migration 0001–0003。

## 設計要點

- **鎖是權威，heartbeat 是顯示**：epoch 只在取得 OS 鎖後才會變；heartbeat 過期只供 UI 標「失聯」，
  不授權搶占（§1.4）。休眠中的 owner 仍是 owner。
- **`heartbeat_seq` 而非 wall clock**：讀者以自己的單調時間區間看計數器是否前進（`lease_is_stale`），
  跨程序不比時鐘。
- **同 transaction 檢查**：候選提交在 `commit_candidate_assessment` 的 transaction 內先
  `assert_owner`；其餘寫入在同一 mutex guard 內先檢查——單連線模型下等價，因為只有鎖的持有者能改 epoch。
- **`epoch: None`** 只給測試與無 lease 的舊路徑；production 一律由 `open_workspace` 給 `Some`。
- **SchemaTooNew 比契約嚴**：契約要求「拒絕寫入」，實作連讀取都拒絕（整個開啟失敗），避免舊 binary
  對沒見過的 schema 做任何事。

## Verification

| 項目 | 結果 |
| --- | --- |
| `cargo check --locked --tests`、`cargo clippy --locked --tests` | 無新 warning |
| `cargo test --locked`（`CARGO_TARGET_DIR=C:\tmp\aff-target`） | 52 + 122 = **174 passed**（+12） |
| 必測情境對照 | 雙啟（`a_second_host_cannot_open…`）、owner crash／接手（同上，drop 後 epoch 2）、舊 worker 提交（`a_candidate_from_a_previous_epoch_is_refused_before_any_write`＋`a_runner_left_over…`）、休眠／時鐘（`lease_is_stale` 純函式＋heartbeat 停止不放鎖）、新 schema 拒絕（`a_database_written_by_a_newer_build…`） |
| 突變檢查 | 移除 commit 內 `assert_owner` → 提交測試失敗；假裝鎖永遠空閒 → 雙啟測試失敗；移除 SchemaTooNew → 新 schema 測試失敗；還原後全綠 |
| 暫存目錄 | 測試後 `%TEMP%` 無 `aff-runtime-test-*`／`aff-lease-test-*` 殘留 |
| 前端 suites | 未重跑（無 TS 變更） |
| 原生 `cargo tauri dev` | **未執行**；smoke lane 覆蓋啟動 |

## 對 P03b／P04 的交接點

- P03b：`research-command-v1` 的 `requestId` 冪等表（≥24h）與 `research-event-v1` 的持久
  `eventId`（0005 migration）；事件在 commit 後寫入帳本再發布，`DiscoveryEventSink` 可包一層
  ledger sink；重連 = snapshot ＋ `afterEventId`。
- P04：service binary 呼叫 `open_workspace(path, HolderKind::Service)`；桌面收到 `NotOwner`
  時改走 connect 模式（讀 `ownership::read` 顯示 holder，以 `lease_is_stale` 標失聯）；
  §1.5 背景切換順序：停收新工作 → checkpoint → drop `OwnershipHandle` → 啟動 service。
- `OwnershipHandle::lost()` 目前只有 heartbeat 收到 `StaleOwner` 才會為 true（鎖檔被替換的異常情況）；
  P04 可據此讓 runner 自行降級。

## Resolution (added when acted on)

（待補：push／PR 編號、review 結果。）

## Resolution — Codex acceptance review（2026-09-17）

Review target：`63567c2`（P02 收尾）＋ `9afac5a`（P03a），分支
`docs/p00-contract-precheck`；驗收開始時 worktree clean。

**結論：P02 收尾通過；P03a 需修正以下兩項 High，暫不通過。**
P03a／P03b 拆分本身合理，未將 requestId、eventId 或 connect 模式算作本次缺漏。

### R1（High）：舊 coordinator 的 claim 沒有 epoch 防護

- 位置：`alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs:782`，呼叫
  `db/discovery.rs:325` 的 `claim_candidate_jobs`。呼叫端未 `assert_owner`，該函式的
  transaction 亦未接收／檢查 epoch；只確認 run 為 running、job pair 為 queued。
- 可重現順序：A（epoch 1）完成 start 的資料寫入、尚未啟動 coordinator → 在初始
  Progress sink 暫停交接，drop A 的真實 `OwnershipHandle` → B 經
  `open_workspace_with(..., Service, ...)` 取得同一路徑的 OS 鎖與 epoch 2，recovery
  將 run 暫停 → 使用既有 `transition_run` 模擬 B 的明確 resume（Paused → Running）
  → 讓 A 的 coordinator 繼續。B 不派自己的 worker，以隔離舊 coordinator 的影響。
- 實測：A 將 Train／Validation **兩筆 queued job 改為 running 並派工**；直到候選
  assessment commit 才被 StaleOwner 拒絕。兩筆 job 留在 running，會阻擋新 owner 的
  claim／completion。候選結果提交的防護不能補救已經提交的 claim。
- 修正要求：claim 必須帶 coordinator 的 epoch，並在更動 job 的同一 transaction 內
  檢查；失去 ownership 時停止派工。補上述跨 epoch coordinator 回歸測試，斷言 job
  狀態不變且未派出舊工作。同步更正本文前段「8 個寫入點已完整覆蓋」的交接說明。

### R2（High）：transaction 外的檢查擋不住檢查後才發生的換手

- 位置：`alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs:646`（無 control 的
  cancel；另一 cancel 分支與 start／resume／pause／complete／fail／recovery 亦採相同
  transaction 外檢查方式）。`db/discovery.rs:889` 的 cancel transaction 沒有 epoch
  檢查。本文前段稱「同一 mutex guard 等價同 transaction」不成立：接手 owner 有另一條
  SQLite connection／另一個 mutex，舊 runner 的 mutex 不會阻擋它。
- 可重現順序：建立 A epoch 1 與 paused run → 呼叫真實 `runner.cancel` → 用臨時
  `#[cfg(test)]` callback，僅在 `assert_owner` 通過後、讀取 run／開啟取消 transaction
  前暫停 → 保持舊 DB mutex guard，drop A 的 lease → B 透過正常 runtime 開啟同一
  workspace，取得 epoch 2 → 繼續 A 原本的 cancel。
- 實測：A 回傳 **`Ok(())`**，epoch 已為 2 的 DB 中 run 被改成 **Cancelled**；預期
  StaleOwner 且 paused run 不變。這個測試沒有替換 epoch 檢查或取消實作，插樁只控制
  執行順序；使用真實檔案鎖、不同 SQLite connections 與正常 ownership acquisition。
- 修正要求：依契約 §1.3，讓每一個 runner 寫入 transaction 都在內部檢查其 epoch，
  包括 start 中 strategy／run／progress 的各次寫入及錯誤路徑；不要只把 R1 補成
  transaction 外檢查。補 check→takeover→write 的確定性回歸，確認整個寫入 rollback。

### 驗證與範圍

- `cargo test --locked`：現有 **174 passed（52 + 122）**。
- 臨時驗收 probes：`cargo test --locked review_ -- --nocapture --test-threads=1`，
  **兩條保護性斷言均失敗**，輸出如下：

  ```text
  REVIEW claim: current epoch=2, old runner epoch=Some(1), running jobs=2
  REVIEW cancel: current epoch=2, old runner epoch=Some(1), result=Ok(()), persisted status=Cancelled
  ```

- 臨時 probes 與 callback 已移除，Rust source diff 為空；兩個 probe 的工作目錄均已由
  guard 在關閉 workspace／connections 後移除，無 `aff-review-epoch-*` 殘留。
- `cargo check --locked`、`cargo clippy --locked` 成功。一般 bin build 新增 **6 個
  dead_code warnings**（`STALE_AFTER`、`OwnershipRow`、`read`、`lease_is_stale`、
  `OsLock.path`、`OsLock::path`）；clippy 另有既有 core 的 4 個 warnings。原交接使用
  `--tests`，這些項目被測試使用，故不能以該結果代表一般 build 無新 warning。
  此項為非阻擋清理／驗證文字修正，不要求處理既有 core warnings。
- P02 TempRoot drop 順序與狀態文字修正已確認；本次測試未遇到暫存目錄刪除失敗。
- 未跑前端（無 TS 變更）、原生 Tauri 啟動、真正跨 OS process 的強制終止測試；既有
  crash 測試以 drop 模擬 lease 釋放，勿擴大解讀為實測 kill-process。
- 本次只記錄驗收；未改產品實作，未 commit／push。修正後應更新 `tasks.md`、契約
  狀態與本文 Resolution，再重新驗收 P03a；P03b 不應以目前 Done 標記直接起跑。

## Resolution — R1/R2 修正與回歸（2026-09-17）

使用者明確要求修正上述驗收問題；沿用 `docs/p00-contract-precheck`，保留前次驗收紀錄。

- **R1 已關閉**：`claim_candidate_jobs` 明確接收 `ControlState.epoch`，在 claim 的
  transaction 內先檢查。舊 coordinator 在派工前即遭拒絕；兩筆 job 保持 queued，
  executor 呼叫次數為 0，且不發布成功事件或改寫新 owner 的 run。
- **R2 已關閉**：新增 `ownership::write_transaction(conn, epoch)`：先
  `BEGIN IMMEDIATE` 取得 SQLite 寫入保留鎖，再檢查 epoch；錯誤 drop rollback。
  所有 runner store 寫入均改用此邊界：claim、strategy admission、run create/start、
  progress、transition（resume／pause）、cancel、fail、complete、recovery、candidate
  assessment。heartbeat 的更新與回傳 sequence 也共用此邊界。
  原 transaction 外 `assert_owner` 保留為 preflight，不能取代 store 內的檢查；
  start／resume checkpoint 遇到 StaleOwner 保留該錯誤型別，不包成一般錯誤。
- **更正原交接**：「8 個寫入點」漏列 claim 與 admission 的細項；「同 mutex 等價
  同 transaction」不成立。以本節的 store transaction 清單為準。
- **新測試共 5 個**：`discovery_runner/tests/ownership.rs` 的 3 個測試使用真實
  OS lease 與不同 SQLite connections，涵蓋 stale claim／dispatch、cancel preflight
  後換手、admitted transaction 阻止 successor 推進 epoch；store 測試逐一驗證
  9 種 run mutation 在舊 epoch 返回 StaleOwner 且所有列不變、同一輸入在新 epoch
  成功；repository 測試驗證 strategy insert 的相同正反例。既有 assessment fence
  與 rollback 測試仍通過。
- 兩個重現案例先在原實作執行，皆紅（queued 變 0；cancel 回傳 Ok）；修正後皆綠。
  cancel 的執行順序由 `#[cfg(test)]` thread-local callback 控制，不使用 sleep 猜時序，
  不編入一般 application。資料目錄以 guard 在 handles／connections 關閉後刪除。
- **一般 build warnings**：4 個 P04 read/liveness 預留項加上附理由的局部
  `cfg_attr(not(test), allow(dead_code))`；僅供測試的 lock path 欄位／accessor 改為
  `cfg(test)`。未用全域 allow，也未改動既有 core 的 4 個 clippy warnings。
- **驗證**：`cargo test --locked` **179 passed（52 + 127）**；
  `cargo check --locked` 無 warning；`cargo clippy --locked` 成功，僅上述既有 4 個
  core warnings。前端無變更，未重跑前端 suites／原生 Tauri UI，也未測 kill-process。
- 同步 `tasks.md`、CHANGELOG、runtime 契約與 capability registry。使用者於修正完成後
  明確要求 commit，以上修改納入本次本機提交；未 push、未開始 P03b。
