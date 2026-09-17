# Handoff: P02 Runtime 解耦（ABC-02a）

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`（P00／P01 之後同分支續做；本機 git，GitHub 仍鎖）
PR: 尚未建立
Status: P02 完成；下一個可執行 phase 為 P03（跨宿主 ownership），須另行明確授權

## Summary

執行 `docs/plans/active-plan.md` §5 的 P02：「runner event sink、DB path initialization、共用
orchestration」，驗收「既有桌面行為、golden、runner 原子性不變；未加入 headless 排程」。
使用者於 P01 驗收收尾後指示「繼續做下個任務」。純 Rust 變更，零 TypeScript／schema／
依賴／命令契約變動。

## 執行前驗證

- Plan §3.1「先移出 runner 的 Tauri event sink，以及 DB 初始化對 `AppHandle` 的依賴」與契約
  §6 P02 列（§0 邊界、busy_timeout、DB path 注入、event sink 抽離）在 codebase 對照成立：
  `discovery_runner/mod.rs` 唯一的 tauri 用法是 `TauriDiscoveryEventSink`；`db::initialize`
  唯一的 tauri 用法是 `app.path().app_data_dir()`；`main.rs` 的 setup 依序做開庫→建 runner→
  孤兒恢復。
- 不可動的既有約束：CI native smoke lane 斷言 `%APPDATA%\com.alphafactorforge.desktop\alphafactorforge.sqlite3`
  存在且 WAL sidecar 有 migration 寫入（依賴「先設 WAL 再 migration」的順序）；
  `single_instance::tests` 斷言 main.rs 的 plugin → setup → recovery 文字順序。兩者都保留。

## 修改檔案

| 檔案 | 變更 |
| --- | --- |
| `src-tauri/src/runtime/mod.rs` | 新增：`Workspace { db, discovery, recovery }`、`open_workspace(path)`（開庫→migration→runner→孤兒恢復）；3 個測試（首次開啟建目錄／migration／pragma、重開冪等、開啟即修復孤兒） |
| `src-tauri/src/runtime/boundary_tests.rs` | 新增：掃描 runtime／db/*／discovery_runner/*／identity／error 原始碼（去除 `//` 註解）不得出現 `tauri`；反向斷言 desktop adapter 是唯一 emit 處 |
| `src-tauri/src/desktop/mod.rs`、`desktop/discovery_events.rs` | 新增：`TauriDiscoveryEventSink` 原樣搬入（事件名、`main` 視窗、payload 不變） |
| `src-tauri/src/db/mod.rs` | `initialize(&AppHandle)` → `open_at(&Path)`；新增 `DB_FILE_NAME`、`BUSY_TIMEOUT = 5 s`；pragma 順序 WAL → foreign_keys → busy_timeout；移除 tauri import |
| `src-tauri/src/discovery_runner/mod.rs` | 移除 sink 實作與 `use tauri`；只保留 `DiscoveryEventSink` trait 與事件常數 |
| `src-tauri/src/commands/discovery_commands.rs` | sink 改從 `crate::desktop::discovery_events` 匯入 |
| `src-tauri/src/main.rs` | 宣告 `desktop`／`runtime` 模組；setup 只解析 app data dir 路徑，交給 `runtime::open_workspace`；非零 recovery 報告寫 stderr |
| `src-tauri/src/single_instance.rs` | 順序測試改錨定 `runtime::open_workspace(&db_path)`，並斷言 main.rs 不再自行 `db::initialize(`／`.recover_orphans(` |
| `.github/workflows/ci.yml` | 註解 `db::initialize` → `db::open_at`（無行為變更） |
| `tasks.md`、`CHANGELOG.md`、`docs/research-runtime-contract.md`、`docs/autonomous-research-capability-registry.md` | 狀態同步 |

未動：`discovery_runner/tests.rs`、`event_contract_tests.rs`、`execution.rs`、所有 migration、
TypeScript、`Cargo.toml`。

## Verification

| 項目 | 結果 |
| --- | --- |
| `cargo check --locked` | 通過，無 warning |
| `cargo clippy --locked` | 無新增 warning |
| `cargo test --locked`（`CARGO_TARGET_DIR=C:\tmp\aff-target`） | 52 + 110 = **162 passed**（+6：runtime 3、boundary 3） |
| 突變檢查 | 在 `discovery_runner/mod.rs` 塞入 `use tauri::Manager as _;` → `host_agnostic_modules_never_name_the_desktop_framework` 失敗並列出位置；還原後通過 |
| 前端 suites | 未重跑（無 TypeScript 變更） |
| 原生 `cargo tauri dev` | **未執行**；CI native smoke lane 覆蓋啟動與 DB／WAL 斷言，順序測試與 runtime 測試覆蓋其斷言內容 |

GitHub：仍 401，僅本機 commit。

## 對 P03／P04 的交接點

- P03 的 lease：依契約 §1.2，OS 鎖須在 `db::open_at` **之前**取得；建議放在 `runtime::open_workspace`
  開頭（新增參數或包一層 `open_owned_workspace`），epoch 檢查加在 `db::discovery::commit_candidate_assessment`
  的同一 transaction 內。
- P04 的 service binary：以 `runtime::open_workspace(path)` 取得 `Workspace`，實作自己的
  `DiscoveryEventSink`（例如寫入事件帳本），不需碰 `desktop/`。
- 邊界測試的檔案清單（`HOST_AGNOSTIC_SOURCES`）新增 host-agnostic 模組時要一併登記。

## Resolution (added when acted on)

（待補：push／PR 編號、review 結果。）
