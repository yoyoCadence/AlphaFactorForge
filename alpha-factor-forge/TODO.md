# AlphaFactorForge — Phase A scaffold TODO / 檔案狀態

> 狀態快照（測試數、slice 進度、環境驗證）以根目錄 `tasks.md`「Current Snapshot」為唯一事實來源；本檔為 Phase A 的**檔案級狀態對照表**（FULL/SKELETON/STUB），非進度板。

圖例：✅ FULL（完整可用）｜🟡 SKELETON（可編譯骨架，待補實作）｜⬜ STUB（佔位，Phase B/C）

> **本生成環境只能驗證 ✅ 的 TS 純邏輯（`npm test` / `npm run typecheck`）。**
> 所有 Rust / Tauri 執行步驟標記「需本機驗證」。

---

## 逐檔狀態

### 前端 core 純函數（✅ 完整，可立即測）
- ✅ `src/core/indicators/index.ts` — SMA/EMA/WMA/RSI/MACD/ATR/BBANDS/STDDEV/HIGHEST/LOWEST/ROC
- ✅ `src/core/metrics/index.ts` — 全部 backtest_summary 指標
- ✅ `src/core/backtest/index.ts` — deterministic 回測引擎（long/short/sizing/fee/slip/SL-TP/fill mode）
- ✅ `src/core/hashing/index.ts` — strategy_hash / dataset_hash（canonical + SHA-256/FNV fallback）
- ✅ `src/core/strategy-dsl/schema.ts` — 指標/運算子白名單、節點型別、限制
- ✅ `src/core/strategy-dsl/validator.ts` — whitelist 編譯器/驗證器（深度/節點上限、可疑字串、$param 檢查）
- ✅ tests：`indicators.test.ts` / `validator.test.ts` / `backtest.test.ts`

### 前端 bridge / shell（🟡）
- ✅ `src/tauri-client/commands.ts` — 所有 command 的 typed wrapper
- ✅ `src/tauri-client/events.ts` — discovery 事件訂閱 + throttle
- ✅ `src/tauri-client/dbClient.ts` — importDataset（含 hash）
- 🟡 `src/main.tsx` — 只是 bridge 驗證殼；**待移植**現有 AlphaFactorForge 圖表 UI
- 🟡 `src/workers/backtest.worker.ts` — 協定骨架，待接 UI

### Rust backend（🟡 / ⬜，全部需本機 `cargo check`）
- 🟡 `src-tauri/src/main.rs` — invoke handler 註冊齊全
- ✅ `src-tauri/src/error.rs` — 錯誤型別
- 🟡 `src-tauri/src/db/mod.rs` — 連線 + migration runner（邏輯完整）
- 🟡 `src-tauri/src/db/repositories.rs` — datasets/candles/strategy/**backtest_summary** CRUD 完整（upsert on strategy+dataset+segment）
- 🟡 `src-tauri/src/commands/db_commands.rs` — 完整；`save/get_backtest_result` 已接通 `insert_backtest_summary` / `list_backtest_summaries`
- ⬜ `src-tauri/src/commands/file_commands.rs` — export_report 待補
- ⬜ `src-tauri/src/commands/ai_commands.rs` — Phase C
- ⬜ `src-tauri/src/commands/secret_commands.rs` — Phase C（keychain）
- ⬜ `src-tauri/src/commands/discovery_commands.rs` — Phase B
- ✅ `src-tauri/migrations/0001_init.sql` — 9 張表全建（schema 已立）

---

## 需本機補完才能跑起來（Phase A 收尾）

1. ✅ **Tauri 圖示**：`src-tauri/icons/icon.png` 已就位（另有 `app-icon-source.png` 1254×1254 方形原圖）。`tauri.conf.json` 只要求 `icons/icon.png`，已滿足。可選：本機 `cargo tauri icon icons/app-icon-source.png` 產生各平台多尺寸（.ico/.icns/PNG set）。
2. ✅ **backtest_summary 持久化**：`repositories::insert_backtest_summary`（upsert）+ `list_backtest_summaries` 已補；`db_commands::save_backtest_result`（改收型別化 `BacktestSummary`）/ `get_backtest_results` 已接通；TS `commands.ts` 同步加 `BacktestSummary` 介面。**仍需本機 `cargo check`**。trade 明細（`trades` 表）延到 UI 移植時再寫。
3. **UI 移植**：把現有 `AlphaFactorForge.dc.html` 的圖表 / 指標 / 單策略回測 / holdout / sweep / replay / 報告匯出，拆進 `src/components` `src/pages` `src/charts`，改用 `core/*` 純函數與 `tauri-client` 存取資料。
   - code mode 保留為 **manual-only / unsafe-for-ai**，與 AI DSL 完全隔離（AI 永不走 code mode）。
   - 存回測結果時，camelCase `Metrics` → snake_case `BacktestSummary` 一律走**單一 helper `metricsToBacktestSummary()`**，勿在各 component inline 映射（PR #1 定案）。
4. **連線狀態**：`@tauri-apps/api` 版本對齊（v2）；`isTauri()` 判斷瀏覽器 dev 模式降級。

驗證指令：
```bash
npm install
npm test            # ✅ 應全綠（純邏輯）
npm run typecheck   # ✅ 型別
cd src-tauri && cargo check   # 🟡 需本機 Rust
cargo tauri dev     # 🟡 需本機 Tauri + 圖示
```

---

## Phase B（下一階段，勿在 Phase A 動）
- Train/Validation/Test split + embargo（`src/core/validation/`）
- Gate + Score（`src/core/scoring/`，權重可設定、breakdown 存 JSON）
- Benchmark：Buy&Hold / SMA / RSI / Bollinger / Random Entry（固定 seed、N 次）（`src/core/benchmarks/`）
- duplicate skip（strategy_hash + dataset_hash + segment）
- Discovery job runner（Rust thread pool）+ pause/resume/cancel/checkpoint
- 事件協定落地 + 前端節流訂閱
- Results Explorer（只顯示 Validation；Test 隱藏）
- lifecycle：candidate / validated / rejected

## Phase C（最小 AI Lab）
- secret_commands：keychain（`keyring` crate）
- test_ai_connection
- generate_strategy_dsl（後端持金鑰呼叫，回 raw）
- validate_strategy_dsl（後端鏡像 validator.ts）
- 人工 approve → 建 strategy_def(source=ai, type=ai_dsl) → 加入 queue
- 記錄 prompt / raw / parsed / validation 於 ai_generations

## Phase D（延後，不在首版）
paper_live / promoted / hidden Test 自動揭露 / clustering 自動細化 / meme risk filter / walk-forward 全自動 / quarantine 自動偵測 / AI 全自動閉環。schema 已預留，但 job runner 與 UI 一律不得啟用。

---

## 不可違反的邊界（回歸測試必查）
1. API key 不進 frontend / localStorage / SQLite 明文（只 keychain）。
2. frontend 不直接呼叫 AI API。
3. AI 只能產 JSON DSL；validator 必須擋下未知運算子與注入字串。
4. 大量 discovery 不跑在 UI 主執行緒（走 Tauri backend）。
5. Test segment 不參與 v1 ranking；Results Explorer 預設隱藏 Test。
6. code mode = manual-only，AI 不可使用。
