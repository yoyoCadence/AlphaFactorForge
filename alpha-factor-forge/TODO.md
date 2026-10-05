# AlphaFactorForge — 模組對照與未接線邊界

> 狀態快照（測試數、slice 進度、環境驗證）以根目錄 [tasks.md](../tasks.md) 為唯一事實來源；本檔為**模組對照表**，非進度板。2026-10-05 依目前原始碼校正初始 scaffold 說明。

圖例：✅ FULL（完整可用）｜🟡 SKELETON（可編譯骨架，待補實作）｜⬜ STUB（佔位，Phase B/C）

> ✅ 代表指定範圍已有實作，不代表整個產品驗收完成。原始碼的歷史 FULL／SKELETON／STUB 註解不作為進度依據。
> Vitest／mock E2E 與 Rust／原生啟動已有本機及 CI 證據；每次驗證範圍見 tasks.md 與 handoffs。

---

## 逐檔狀態

### 前端 core 純函數（✅ 完整，可立即測）
- ✅ `src/core/indicators/index.ts` — SMA/EMA/WMA/RSI/MACD/ATR/BBANDS/STDDEV/HIGHEST/LOWEST/ROC
- ✅ `src/core/metrics/index.ts` — 全部 backtest_summary 指標
- ✅ `src/core/backtest/index.ts` — deterministic 回測引擎（long/short/sizing/fee/slip/SL-TP/fill mode）
- ✅ `src/core/hashing/index.ts` — versioned strategy／dataset identities；numeric parser 相容性修復仍待 NUMERIC-JSON-002，不能直接改 feature 或重算舊 identity
- ✅ `src/core/strategy-dsl/schema.ts` — 指標/運算子白名單、節點型別、限制
- ✅ `src/core/strategy-dsl/validator.ts` — whitelist 編譯器/驗證器（深度/節點上限、可疑字串、$param 檢查）
- ✅ tests：`indicators.test.ts` / `validator.test.ts` / `backtest.test.ts`

### 前端 bridge / UI
- ✅ `src/tauri-client/commands.ts` — 所有 command 的 typed wrapper
- ✅ `src/tauri-client/events.ts` — discovery-event-v1 版本／payload 驗證與訂閱；UI 以 sequence 處理更新
- ✅ `src/tauri-client/dbClient.ts` — importDataset（含 hash）
- ✅ `src/main.tsx`、`components/`、`charts/`、`services/` — 回測、圖表、Holdout、sweep、replay、策略庫／匯出與研究 UI 已移植
- ✅ `src/workers/backtest.worker.ts` — run/result/error 與 runSweep/sweepResult 的 job id 協定；UI sweep 已接可取消 module worker，保留 context／generation guard；單次 UI 回測仍同步

### Rust backend（需 Rust／Tauri 環境驗證）
- ✅ `src-tauri/src/main.rs` — desktop setup、ownership／service connect 與 invoke handlers
- ✅ `src-tauri/src/error.rs` — 錯誤型別
- ✅ `src-tauri/src/db/mod.rs` — host-independent SQLite／WAL、有序 migrations、新版 schema 拒開
- ✅ `src-tauri/src/db/repositories.rs` — datasets／candles／strategy、摘要與交易、驗證 bundle 持久化；同 hash 策略保留 identity／lifecycle
- ✅ `src-tauri/src/commands/db_commands.rs` — Phase A DB handlers 透過 blocking worker，保留 SQLite mutex／transaction／admission
- ✅ `src-tauri/src/commands/file_commands.rs` — `save_report` 原子檔案保存；舊 `export_report(result_id)` 仍為未使用 stub
- 🟡 `src-tauri/src/commands/ai_commands.rs` — DSL 驗證已實作；provider generation 仍為 stub（P15）
- ⬜ `src-tauri/src/commands/secret_commands.rs` — Phase C（keychain）
- ✅ `src-tauri/src/commands/discovery_commands.rs`、`discovery_runner/` — backend start／pause／resume／cancel／checkpoint，Train／Validation、Gate／Score／benchmarks
- ✅ `src-tauri/src/runtime/`、`service_main.rs` — service CLI、workspace ownership、持久 command／event、desktop hand-over
- ✅ `src-tauri/src/research/`、`market/` — trial ledger、campaign、研究 artifacts 與市場來源；P12 原生 operator 驗收／P09 token 下載仍開放
- ✅ `src-tauri/migrations/` — 以 `db/mod.rs` 的 MIGRATIONS 為順序來源：0001 初始表、0002 validation、0003 runner，追加至 0010 campaign；trial registry 使用獨立 `registry_migrations/`

---

## 初始 Phase A 收尾與現在的接線

1. ✅ **Tauri 圖示**：`src-tauri/icons/icon.png` 已就位（另有 `app-icon-source.png` 1254×1254 方形原圖）。`tauri.conf.json` 只要求 `icons/icon.png`，已滿足。可選：本機 `cargo tauri icon icons/app-icon-source.png` 產生各平台多尺寸（.ico/.icns/PNG set）。
2. ✅ **摘要／交易／驗證持久化**：`saveBacktestResult(summary,trades)` 保存摘要與明細；`saveValidationRecord(...)` 原子保存兩段結果與不可變驗證紀錄。Results Explorer 可重新讀取；歷史缺少 trades 時如實顯示。
3. ✅ **UI 移植**：圖表／指標／回測／Holdout／sweep／replay／報告匯出已拆進 `components`／`charts`／`services`，經 `core/*` 與 typed client 使用資料。legacy prototype 保留為參考。
   - code mode 保留為 **manual-only / unsafe-for-ai**，與 AI DSL 完全隔離（AI 永不走 code mode）。
   - 存回測結果時，camelCase `Metrics` → snake_case `BacktestSummary` 一律走**單一 helper `metricsToBacktestSummary()`**，勿在各 component inline 映射（PR #1 定案）。
4. ✅ **連線狀態**：Tauri v2 typed wrappers；`?mock=1` 為隔離瀏覽器測試 seam，不能證明 SQLite／原生 IPC。

驗證指令：
```bash
npm install
npm test            # Vitest
npm run typecheck   # ✅ 型別
cd src-tauri && cargo check --locked
cargo test --locked
# 回到 alpha-factor-forge/，使用已安裝的 CLI：npm run tauri -- dev
```

---

## Phase B 實作與剩餘邊界（進度見 tasks.md）
- Train/Validation/Test split + embargo（`src/core/validation/`）
- Gate + Score（`src/services/gate.ts`／`src/services/score.ts`，thresholds／breakdown 存 JSON）
- Benchmark：Buy&Hold / SMA / RSI / Bollinger / Random Entry（固定 seed、N 次）（`src/services/benchmarks.ts`／`src/services/randomEntry.ts`）
- duplicate skip（strategy_hash + dataset_hash + segment）
- Discovery job runner（Rust thread pool）+ pause/resume/cancel/checkpoint
- 事件協定落地 + 前端節流訂閱
- Results Explorer（只顯示 Validation；Test 隱藏）
- lifecycle：candidate / validated / rejected

上述 split、Gate／Score、benchmarks、duplicate skip、runner、事件、Results Explorer 與 lifecycle 已有實作／回歸測試。
P12 的 campaign／ledger 與純 confirmation 計算亦已實作；native operator 驗收仍開放。
P13 才接 confirmation 執行、揭露／alpha／freshness fence，不能把純計算通過當成 runtime 已交付。

## Phase C（最小 AI Lab）

目前僅 DSL 驗證／固定 DSL 執行已接通（P11）；下列 provider、認證與人工 approve 流程尚待 P15。首個 provider 的 Codex 訂閱路徑以根目錄 `docs/ai-provider-contract.md` 為準，不能宣稱 secrets stubs 已實作。
- secret_commands：keychain（`keyring` crate）
- test_ai_connection
- generate_strategy_dsl（後端持金鑰呼叫，回 raw）
- validate_strategy_dsl（後端鏡像 validator.ts）
- 人工 approve → 建 strategy_def(source=ai, type=ai_dsl) → 加入 queue
- 記錄 prompt / raw / parsed / validation 於 ai_generations

## Phase D（延後，不在首版）
paper_live／promoted、hidden Test runtime 揭露、clustering、meme risk filter、walk-forward 自動編排與 AI 全自動閉環仍依 active plan 分項交付。P12 的 Train-only walk-forward 計算與 ledger conflict quarantine 已存在；這些不代表完整 Phase D 或 paper 介面可用。

---

## 不可違反的邊界（回歸測試必查）
1. API key 不進 frontend / localStorage / SQLite 明文（只 keychain）。
2. frontend 不直接呼叫 AI API。
3. AI 只能產 JSON DSL；validator 必須擋下未知運算子與注入字串。
4. 大量 discovery 不跑在 UI 主執行緒（走 Tauri backend）。
5. Test segment 不參與 v1 ranking；Results Explorer 預設隱藏 Test。
6. code mode = manual-only，AI 不可使用。
