# AlphaFactorForge Phase A — 本機驗證 Checklist / 故障排除 / 對應表

> 對象：把 Phase A scaffold 在自己機器跑起來的人。
> 圖例：✅ 任何環境可驗 ｜ 🟡 需本機（Node / Rust / Tauri）

---

## 一、本機驗證 Checklist（依序執行）

### 0. 前置工具
- [ ] Node ≥ 18：`node -v`
- [ ] Rust ≥ 1.77：`rustc --version`
- [ ] Tauri CLI v2：`cargo install tauri-cli --version "^2.0.0"`（或 `cargo tauri -V`）
- [ ] 平台依賴（擇一 OS）
  - macOS：`xcode-select --install`
  - Windows：WebView2 Runtime + MSVC build tools（VS Build Tools）
  - Linux：`webkit2gtk-4.1`、`librsvg2`、`libayatana-appindicator3`、`build-essential`

### 1. 前端純邏輯（✅ 不需 Rust）
- [ ] `cd alpha-factor-forge && npm install` 無錯
- [ ] `npm test` → indicators / validator / backtest 測試全綠
- [ ] `npm run typecheck` → 0 型別錯誤
- [ ] `npm run build`（Vite 前端可打包，產出 `dist/`）

> 只有這一段可在「無 Rust」的機器或 CI 驗證。先過這關，代表 core/DSL/hashing 邏輯正確。

### 2. Rust backend 編譯（🟡）
- [ ] `cd src-tauri && cargo check` 編譯通過（允許 unused warning）
- [ ] `cargo clippy`（可選，建議）無 error

### 3. 圖示（🟡 缺了會擋 build）
- [ ] 準備一張 ≥ 512×512 PNG logo
- [ ] `cargo tauri icon path/to/logo.png` → 產生 `src-tauri/icons/*`
- [ ] 確認 `icons/icon.png` 等檔案存在（取代 placeholder README.txt）

### 4. 啟動 app（🟡）
- [ ] `cargo tauri dev` 開出原生視窗
- [ ] 視窗顯示「AlphaFactorForge — Automated Indicator Discovery Workstation」標題
- [ ] status 顯示 `database already initialized at startup`（非 "running OUTSIDE Tauri"）
- [ ] datasets 數量顯示（首次為 0，正常）
- [ ] OS app-data 目錄出現 `alphafactorforge.sqlite3`
  - macOS：`~/Library/Application Support/com.alphafactorforge.desktop/`
  - Windows：`%APPDATA%\com.alphafactorforge.desktop\`
  - Linux：`~/.local/share/com.alphafactorforge.desktop/`

### 5. DB 健檢（🟡，用 sqlite3 CLI 或 DB 工具開檔）
- [ ] `.tables` 列出 9 張表 + `schema_migrations`
- [ ] `SELECT * FROM schema_migrations;` 有 `0001_init`
- [ ] 對 `datasets` 手動 INSERT 一筆，重啟 app，bridge 面板能列出 → 證明讀路徑通

### 6. 收尾項（補完才算 Phase A 完整，見 TODO.md）
- [ ] `repositories::insert_backtest_summary` + list 補上
- [ ] `db_commands::save/get_backtest_result` 接通（目前回 NotImplemented）
- [ ] 現有 AlphaFactorForge 圖表 UI 移植進 `src/`（先能跑單策略回測）

---

## 二、可能會爆的 compile error 與修法

### Rust / Cargo

**E1. `error: failed to run custom build command for tauri-build`**
- 多半是缺 `tauri.conf.json` 對應的圖示或欄位。先補圖示（步驟 3）。確認 `frontendDist: "../dist"` 路徑存在（先 `npm run build` 一次或用 dev 模式）。

**E2. rusqlite link error / `SQLite3 not found`**
- `Cargo.toml` 已用 `features = ["bundled"]`，會自帶 SQLite 原始碼編譯，無需系統 SQLite。若仍失敗，確認有 C 編譯器（macOS Xcode CLT / Windows MSVC / Linux build-essential）。

**E3. `app.path()` / `app_data_dir()` not found**
- Tauri v2 的 path API 需要 `Manager` trait 在 scope。`db/mod.rs` 已 `use tauri::{AppHandle, Manager}`。若你改了檔案，確保 `Manager` 有 import。
- v2 中 `app_data_dir()` 回 `Result`，本骨架已 `.map_err(...)`，勿改回 `.unwrap()` 當 Option。

**E4. `the trait Serialize is not implemented for AppError`**
- `error.rs` 已手動 impl `Serialize`。若新增 command 回傳新型別，該型別也要 `#[derive(Serialize)]`。

**E5. `Connection` is not `Send`/`Sync`（State 編譯錯）**
- 已用 `Mutex<rusqlite::Connection>` 包住，並放進 `AppState`。command 內用 `state.db.lock()`。不要把裸 `Connection` 放進 `State`。

**E6. `cannot borrow conn as mutable`（insert_candles）**
- `insert_candles` 需要 `&mut Connection`（開 transaction）。對應 command 取的是 `let mut conn = state.db.lock()...`。保留 `mut`。

**E7. invoke handler 名稱對不上**
- `generate_handler![]` 內每個函式都要 `#[tauri::command]` 且 `pub`，且 `mod` 有宣告（`commands/mod.rs`）。少一個就 `cannot find function`。

**E8. migration SQL 執行期錯誤（非編譯期）**
- `CHECK` 約束打錯字會在 `apply_migrations` 執行時報錯。錯誤會從 `initialize().expect(...)` 噴出。檢查 `0001_init.sql` 的 enum 字串。

### 前端 / TypeScript

**E9. `Cannot find module '@tauri-apps/api/core'`**
- `npm install` 後才有。版本需 v2（package.json 已指定 `^2.0.0`）。v1 的 import 路徑不同（v1 是 `@tauri-apps/api/tauri`）。

**E10. `isTauri is not exported`**
- `isTauri` 來自 `@tauri-apps/api/core`（v2）。若版本太舊沒有，改用 `'__TAURI_INTERNALS__' in window` 判斷。

**E11. `structuredClone is not defined`（跑測試時）**
- Node ≥ 17 才有。升級 Node，或在 `backtest.test.ts` / `validator.test.ts` 改用 `JSON.parse(JSON.stringify(...))`。

**E12. tsc 報 `noUnusedParameters`**
- 嚴格模式開著。stub 參數請用底線前綴（骨架已這樣命名，如 `_state`、`_result_id`）。新增程式碼沿用此慣例或關閉該規則。

**E13. Vite build 找不到 `dist`（Tauri 端）**
- 先 `npm run build` 產 `dist/`，或用 `cargo tauri dev`（會自動跑 `beforeDevCommand: npm run dev`）。

---

## 三、Command / DB / Frontend bridge 對應表

### 3.1 Tauri commands ↔ 前端 wrapper ↔ DB 物件

| Rust command（`#[tauri::command]`） | 檔案 | 前端 wrapper（`tauri-client`） | 觸及的 DB 表 | Phase A 狀態 |
|---|---|---|---|---|
| `init_database` | db_commands | `db.init()` | —（啟動已建） | 🟡 可用 |
| `run_migrations` | db_commands | `db.runMigrations()` | schema_migrations | 🟡 可用 |
| `get_datasets` | db_commands | `db.getDatasets()` | datasets | 🟡 可用 |
| `get_candles` | db_commands | `db.getCandles(id,from,to)` | candles | 🟡 可用 |
| `import_candles` | db_commands | `db.importCandles(dataset,candles)` / `dbClient.importDataset()` | datasets, candles | 🟡 可用 |
| `save_strategy` | db_commands | `db.saveStrategy(s)` | strategy_def | 🟡 可用 |
| `get_strategies` | db_commands | `db.getStrategies()` | strategy_def | 🟡 可用 |
| `save_backtest_result` | db_commands | `db.saveBacktestResult(json)` | backtest_summary, trades | 🟡 **待補**（NotImplemented） |
| `get_backtest_results` | db_commands | `db.getBacktestResults(id?)` | backtest_summary | 🟡 **待補** |
| `export_report` | file_commands | （未包 wrapper） | — | ⬜ 待補 |
| `generate_strategy_dsl` | ai_commands | `ai.generateDSL(ctx)` | ai_generations | ⬜ Phase C |
| `validate_strategy_dsl` | ai_commands | `ai.validateDSL(dsl)` | — | ⬜ Phase C |
| `save_ai_api_key` | secret_commands | `secrets.saveKey(p,k)` | **OS keychain**（非 DB） | ⬜ Phase C |
| `get_ai_api_key_status` | secret_commands | `secrets.keyStatus(p)` | keychain | ⬜ Phase C |
| `delete_ai_api_key` | secret_commands | `secrets.deleteKey(p)` | keychain | ⬜ Phase C |
| `test_ai_connection` | secret_commands | `secrets.testConnection(p)` | keychain | ⬜ Phase C |
| `start_discovery` | discovery_commands | `discovery.start(cfg)` | discovery_runs, discovery_jobs | ⬜ Phase B |
| `pause_discovery` | discovery_commands | `discovery.pause(id)` | discovery_runs | ⬜ Phase B |
| `resume_discovery` | discovery_commands | `discovery.resume(id)` | discovery_runs, discovery_jobs | ⬜ Phase B |
| `cancel_discovery` | discovery_commands | `discovery.cancel(id)` | discovery_runs | ⬜ Phase B |
| `get_discovery_progress` | discovery_commands | `discovery.progress(id)` | discovery_runs | ⬜ Phase B |

### 3.2 invoke 參數命名對應（易錯點）

Tauri v2 會把 Rust snake_case 參數自動對應前端 camelCase。本 scaffold 的 wrapper 已處理：

| Rust 參數 | 前端傳入 key |
|---|---|
| `dataset_id` | `datasetId` |
| `result_json` | `resultJson` |
| `strategy_id` | `strategyId` |
| `run_id` | `runId` |
| `prompt_context` | `promptContext` |

> 若自行加 command，前端 `invoke('x', { camelCaseKey })` 必須對上 Rust 的 snake_case。對不上 → 執行期 `invalid args`。

### 3.3 事件對應（Phase B job runner）

| 後端 emit | 前端訂閱（`tauri-client/events`） | payload |
|---|---|---|
| `discovery://progress` | `onDiscoveryProgress(cb)` | `DiscoveryProgress` |
| `discovery://result` | `onDiscoveryResult(cb)` | `DiscoveryResultEvent` |
| `discovery://done` | `onDiscoveryDone(cb)` | `{ runId }` |

UI 更新用 `throttle(fn, 300)` 包住，符合「每 300ms / 每 10 筆」節流要求。

### 3.4 DB 表 ↔ core 型別 ↔ 前端型別

| SQLite 表 | Rust DTO（repositories） | 前端型別 | core 來源 |
|---|---|---|---|
| datasets | `Dataset` | `Dataset`（commands.ts） | `datasetHash()` 算 hash |
| candles | `Candle` | `Candle` | `backtest` 的 `Candle`（欄位簡寫 t/o/h/l/c/v，匯入時轉換） |
| strategy_def | `StrategyDef` | `StrategyDef` | `strategyHash()` 算 hash；`type=dsl/ai_dsl` 配 `StrategyDSL` |
| backtest_summary | （待補 DTO） | （待補） | `computeMetrics()` 產欄位 |
| trades | （待補 DTO） | — | `runBacktest()` 的 `ClosedTrade` |

> 注意：`core/backtest` 的 `Candle` 用簡寫欄位（t/o/h/l/c/v），DB / bridge 的 `Candle` 用全名（timestamp/open/...）。匯入或回測前需做一次欄位映射（建議在 `dbClient` 或 store 層集中轉換，避免散落）。
