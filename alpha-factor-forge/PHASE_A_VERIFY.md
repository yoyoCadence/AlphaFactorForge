# AlphaFactorForge Phase A — 本機驗證 Checklist / 故障排除 / 對應表

> 對象：在本機驗證目前 Tauri 工作站的人。2026-10-05 校正初始 scaffold checklist；
> 實際進度／測試證據以 [tasks.md](../tasks.md) 為準。本清單留白供每次驗證記錄，不代表 repo 未實作。
> 圖例：✅ 任何環境可驗 ｜ 🟡 需本機（Node / Rust / Tauri）

---

## 一、本機驗證 Checklist（依序執行）

### 0. 前置工具
- [ ] Node 20（CI 基準，依 lockfile 工具需求）：`node -v`
- [ ] Rust ≥ `src-tauri/Cargo.toml` 的 `rust-version`（目前 1.89）：`rustc --version`
- [ ] Tauri CLI v2：`npm install` 後在 app 目錄 `npm run tauri -- --version`；可沿用已安裝的 `cargo tauri`
- [ ] 平台依賴（擇一 OS）
  - macOS：`xcode-select --install`
  - Windows：WebView2 Runtime + MSVC build tools（VS Build Tools）
  - Linux：`webkit2gtk-4.1`、`librsvg2`、`libayatana-appindicator3`、`build-essential`

### 1. 前端純邏輯（✅ 不需 Rust）
- [ ] `cd alpha-factor-forge && npm install` 無錯
- [ ] `npm test` → indicators / validator / backtest 測試全綠
- [ ] `npm run typecheck` → 0 型別錯誤
- [ ] `npm run build`（Vite 前端可打包，產出 `dist/`）

> 無 Rust 的機器可執行這一段與 Playwright mock E2E；通過僅代表目前測試涵蓋範圍，不能推論原生 IPC 或所有數值 identity 都已無待辦。

### 2. Rust backend 編譯（🟡）
- [ ] `cd src-tauri && cargo check --locked --all-targets` 編譯通過
- [ ] `cargo test --locked` repositories／runner／service 等回歸通過
- [ ] `cargo clippy`（可選，建議）無 error

### 3. 圖示（🟡 缺了會擋 build）
- [x] `icons/icon.png` 已就位（另有 `icons/app-icon-source.png` 1254×1254 方形原圖）
- [x] `tauri.conf.json` 的 `bundle.icon` 指向 `icons/icon.png`，需求已滿足
- [x] 多尺寸圖示已生成；需要更換素材時才重跑 icon 工具

### 4. 啟動 app（🟡）
- [ ] 在 app 目錄 `npm run tauri -- dev` 開出原生視窗
- [ ] 視窗顯示「AlphaFactorForge — Automated Indicator Discovery Workstation」標題
- [ ] status 顯示 `database already initialized at startup`（非 "running OUTSIDE Tauri"）
- [ ] dataset 選擇器可讀取既有資料；新隔離工作區首次為空
- [ ] OS app-data 目錄出現 `alphafactorforge.sqlite3`
  - macOS：`~/Library/Application Support/com.alphafactorforge.desktop/`
  - Windows：`%APPDATA%\com.alphafactorforge.desktop\`
  - Linux：`~/.local/share/com.alphafactorforge.desktop/`

### 5. DB 健檢（🟡，對已停止的隔離工作區做唯讀檢查）
- [ ] `.tables` 有目前 migrations 定義的表（不再只檢查初始 9 張）
- [ ] `SELECT version FROM schema_migrations ORDER BY version;` 與 `src-tauri/src/db/mod.rs` 的 MIGRATIONS 相符
- [ ] 從 UI 匯入 CSV、重啟後可讀取資料與回測結果；不手動寫入資料表繞過 hash／transaction／ownership

| Migration | 實作範圍 |
|---|---|
| `0001_init` | datasets、candles、strategy、摘要／交易及初始 discovery／AI／settings 表 |
| `0002_validation_records` | 不可變驗證紀錄 |
| `0003_discovery_runner` | 可恢復的 backend run／job 欄位與索引 |
| `0004`–`0006` | workspace ownership、持久 command／event 與 request effects |
| `0007`–`0008` | 研究歷史／artifact 指紋與市場資料 foundation |
| `0009`–`0010` | trial ledger workspace binding 與 campaign admission |

Migrations 依程式的有序清單追加，舊版程式拒絕較新 schema；不要刪版本或重跑 SQL。
Trial registry 在工作區外，使用 `registry_migrations/`，不屬於此 workspace 的表清單。

### 6. UI 與原生驗證範圍（見 TODO.md／tasks.md）
- [x] `repositories::insert_backtest_summary` + `list_backtest_summaries` 補上
- [x] `db_commands::save/get_backtest_result` 接通（改收型別化 `BacktestSummary`，不再回 NotImplemented）
- [x] 圖表／單策略回測／Holdout／sweep／replay／策略庫與匯出已移植
- [ ] `npm run e2e` 通過；這個 suite 使用 `?mock=1`，不代表原生持久化
- [ ] 原生匯入、保存、重新讀取與匯出可重現；記錄本次環境與 artifact
- [ ] P12 campaign／service operator checklist 依 handoff 驗收，不由單元或 bridge smoke 代替

---

## 二、可能會爆的 compile error 與修法

### Rust / Cargo

**E1. `error: failed to run custom build command for tauri-build`**
- 多半是缺 `tauri.conf.json` 對應的圖示或欄位。先補圖示（步驟 3）。確認 `frontendDist: "../dist"` 路徑存在（先 `npm run build` 一次或用 dev 模式）。

**E2. rusqlite link error / `SQLite3 not found`**
- `Cargo.toml` 已用 `features = ["bundled"]`，會自帶 SQLite 原始碼編譯，無需系統 SQLite。若仍失敗，確認有 C 編譯器（macOS Xcode CLT / Windows MSVC / Linux build-essential）。

**E3. `app.path()` / `app_data_dir()` not found**
- Desktop 路徑解析在 `main.rs`，共享 DB 模組只接收 path，不能重新引入 Tauri 依賴。呼叫 path API 的 host 需要 `Manager` trait。
- `app_data_dir()` 回 `Result`；若使用 `AFF_DATA_DIR` 隔離工作區，也要依 trial-ledger 文件隔離其 registry。

**E4. `the trait Serialize is not implemented for AppError`**
- `error.rs` 已手動 impl `Serialize`。若新增 command 回傳新型別，該型別也要 `#[derive(Serialize)]`。

**E5. `Connection` is not `Send`/`Sync`（State 編譯錯）**
- 連線由 host snapshot 提供共享 mutex。DB commands 在 blocking worker 內取得鎖；不要在 async future 持有 guard，或繞過 workspace admission／ownership。

**E6. `cannot borrow conn as mutable`（insert_candles）**
- `insert_candles` 需要 `&mut Connection`（開 transaction）。對應 blocking worker 取得 mutable guard；保留 repository transaction 邊界。

**E7. invoke handler 名稱對不上**
- `generate_handler![]` 內每個函式都要 `#[tauri::command]` 且 `pub`，且 `mod` 有宣告（`commands/mod.rs`）。少一個就 `cannot find function`。

**E8. migration SQL 執行期錯誤（非編譯期）**
- SQL 約束錯誤會從共享 workspace opener 的 `apply_migrations` 傳出並使 startup 拒絕。對照有序 migration 清單與原始錯誤，不刪除 migration 記錄或改寫已套用的 SQL。

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
| `save_backtest_result` | db_commands | `db.saveBacktestResult(summary,trades)` | backtest_summary, trades | ✅ 原子 upsert／替換交易 |
| `get_backtest_results` | db_commands | `db.getBacktestResults(id?)` | backtest_summary | 🟡 可用 |
| `get_backtest_result_detail` | db_commands | `db.getBacktestResultDetail(summaryId)` | backtest_summary, trades | ✅ 同一讀取 transaction |
| `save_validation_record` | db_commands | `db.saveValidationRecord(...)` | backtest_summary, trades, validation_records | ✅ 原子保存 Train／Validation bundle |
| `list_validation_records` / `get_validation_record` | db_commands | `db.listValidationRecords(id?)` / `db.getValidationRecord(id)` | validation_records | ✅ 可用 |
| `save_report` | file_commands | `files.saveReport(filename,contents)` | —（檔案） | ✅ 實際匯出路徑 |
| `export_report` | file_commands | （未包 wrapper） | — | ⬜ 待補 |
| `generate_strategy_dsl` | ai_commands | `ai.generateDSL(ctx)` | ai_generations | ⬜ Phase C |
| `validate_strategy_dsl` | ai_commands | `ai.validateDSL(dsl)` | — | ✅ DSL 白名單驗證；不呼叫 provider |
| `save_ai_api_key` | secret_commands | `secrets.saveKey(p,k)` | **OS keychain**（非 DB） | ⬜ Phase C |
| `get_ai_api_key_status` | secret_commands | `secrets.keyStatus(p)` | keychain | ⬜ Phase C |
| `delete_ai_api_key` | secret_commands | `secrets.deleteKey(p)` | keychain | ⬜ Phase C |
| `test_ai_connection` | secret_commands | `secrets.testConnection(p)` | keychain | ⬜ Phase C |
| `start_discovery` | discovery_commands | `discovery.start(cfg)` | discovery_runs, discovery_jobs | ✅ backend runner |
| `pause_discovery` | discovery_commands | `discovery.pause(id)` | discovery_runs | ✅ checkpoint pause |
| `resume_discovery` | discovery_commands | `discovery.resume(id)` | discovery_runs, discovery_jobs | ✅ 從 persisted queue 恢復 |
| `cancel_discovery` | discovery_commands | `discovery.cancel(id)` | discovery_runs, discovery_jobs | ✅ 可用 |
| `get_discovery_progress` | discovery_commands | `discovery.progress(id)` | discovery_runs, discovery_jobs | ✅ DB snapshot |

此表保留 Phase A／runner 常用映射；runtime／research／campaign／pop-out 的完整 signatures 以 `commands.ts` 和 Rust handlers 為準。

### 3.2 invoke 參數命名對應（易錯點）

Tauri v2 會把 Rust snake_case 參數自動對應前端 camelCase。Typed wrapper 已處理：

| Rust 參數 | 前端傳入 key |
|---|---|
| `dataset_id` | `datasetId` |
| `summary`（`BacktestSummary` 物件） | `summary` |
| `strategy_id` | `strategyId` |
| `run_id` | `runId` |
| `prompt_context` | `promptContext` |

> 若自行加 command，前端 `invoke('x', { camelCaseKey })` 必須對上 Rust 的 snake_case。對不上 → 執行期 `invalid args`。

### 3.3 事件對應（Phase B job runner）

| 後端 emit | 前端訂閱（`tauri-client/events`） | payload |
|---|---|---|
| `discovery://progress` | `onDiscoveryProgress(cb)` | `DiscoveryProgress` |
| `discovery://result` | `onDiscoveryResult(cb)` | `DiscoveryResultEvent` |
| `discovery://done` | `onDiscoveryDone(cb)` | `DiscoveryDoneEvent`（含版本、sequence、status、bestStrategyId／errorMessage） |

三種 payload 均以 `discovery-event-v1` 驗證版本、sequence 與必要欄位，對齊 TS／Rust authored fixture。UI 可重讀 DB snapshot；不能把舊 `{ runId }` 或 scaffold 的 throttle 範例當成 wire contract。

### 3.4 DB 表 ↔ core 型別 ↔ 前端型別

| SQLite 表 | Rust DTO（repositories） | 前端型別 | core 來源 |
|---|---|---|---|
| datasets | `Dataset` | `Dataset`（commands.ts） | `datasetHash()` 算 hash |
| candles | `Candle` | `Candle` | `backtest` 的 `Candle`（欄位簡寫 t/o/h/l/c/v，匯入時轉換） |
| strategy_def | `StrategyDef` | `StrategyDef` | `strategyHash()` 算 hash；`type=dsl/ai_dsl` 配 `StrategyDSL` |
| backtest_summary | `BacktestSummary` | `BacktestSummary`（commands.ts） | `computeMetrics()` 產欄位（camelCase → snake_case 映射） |
| trades | `TradeRow` | `TradeRow`（commands.ts） | `tradesMapper` 轉換 `ClosedTrade`，隨摘要原子保存 |

> 注意：`core/backtest` 的 `Candle` 用簡寫欄位（t/o/h/l/c/v），DB / bridge 的 `Candle` 用全名（timestamp/open/...）。匯入或回測前需做一次欄位映射（建議在 `dbClient` 或 store 層集中轉換，避免散落）。
