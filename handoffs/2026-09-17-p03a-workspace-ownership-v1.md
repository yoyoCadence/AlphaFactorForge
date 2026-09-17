# Handoff: P03a Workspace ownership（`ownership-lease-v1` §1、§5.2）

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P02 之後同分支續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: P03a 完成；下一個可執行 phase 為 P03b（冪等請求、持久事件帳本），須另行明確授權

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
