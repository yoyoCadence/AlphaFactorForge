# AlphaFactorForge - Automated Indicator Discovery Workstation

自動因子鍛造與驗證工作站。本機優先的策略研究環境，用於技術指標回測、策略探索、AI 生成 Strategy DSL、SQLite 本機資料庫、長時間 discovery job、結果審計與防過擬合驗證。

> 架構定案見 `../STRATEGY_DISCOVERY.md`（v3, Tauri）。本 repo 為其實作。

---

## ⚠️ 本交付物的狀態（請先讀）

這是 **Phase A scaffold**。它是**可交接、可逐步驗證**的起點，**不是**完成品。

- ✅ **完整且正確**（純邏輯，無需原生環境即可測）：`src/core/*`（indicators / backtest / metrics / hashing / strategy-dsl schema + validator）。
- 🟡 **可編譯骨架**（結構正確，部分函式為 `todo!()` 或最小實作，需本機補完）：`src-tauri/*`（Rust commands / db / repositories）。
- 🟡 **最小可跑前端**：`src/main.tsx` 是一個驗證 command bridge 的薄殼，**現有 AlphaFactorForge 圖表 UI 尚未移植進來**（見 TODO.md）。

每個檔案頂部的註解會標明：`FULL`（完整）/ `SKELETON`（骨架待補）/ `STUB`（佔位）。

---

## 需要的本機工具

| 工具 | 版本 | 用途 |
|---|---|---|
| Node.js | ≥ 18 | 前端 build（Vite） |
| Rust | ≥ 1.77（stable） | Tauri backend |
| Tauri CLI | v2 | `cargo tauri dev/build` |
| 平台依賴 | 見 Tauri 官方 | macOS: Xcode CLT；Windows: WebView2 + MSVC；Linux: webkit2gtk 等 |

安裝 Tauri 前置依賴：照 https://v2.tauri.app/start/prerequisites/ 對應你的 OS。

---

## 首次安裝與啟動

```bash
# 1. 安裝前端依賴
cd alpha-factor-forge
npm install

# 2. 安裝 Tauri CLI（若尚未安裝）
cargo install tauri-cli --version "^2.0.0"

# 3. 開發模式啟動（會編譯 Rust backend + 啟動 Vite + 開原生視窗）
cargo tauri dev
```

> 首次 `cargo tauri dev` 會編譯 Rust 依賴，需數分鐘。
> SQLite 資料庫會在首次啟動時於 app data 目錄建立並跑 migration。

---

## 逐步驗證（Phase A）

| 步驟 | 指令 | 預期 | 環境 |
|---|---|---|---|
| 1. 純邏輯單元測試 | `npm test`（vitest） | indicators / dsl / hashing / backtest 測試通過 | ✅ 任何環境 |
| 2. TypeScript 型別檢查 | `npm run typecheck` | 無型別錯誤 | ✅ 任何環境 |
| 3. Rust 編譯檢查 | `cd src-tauri && cargo check` | 編譯通過（可能有 unused warning） | 🟡 需本機 Rust |
| 4. 啟動 app | `cargo tauri dev` | 開出原生視窗、DB 初始化、bridge 面板可列出 datasets（空） | 🟡 需本機 Tauri |
| 5. 匯入 K 線 | app 內 Data Manager → import CSV | dataset 落 SQLite、candle_count 正確 | 🟡 需本機 |
| 6. 單策略回測 | app 內跑一次回測 | backtest_summary 落 SQLite、結果可讀回 | 🟡 需本機 |

> 本生成環境（瀏覽器預覽）**只能驗證步驟 1–2**。步驟 3–6 標記為「需本機驗證」。

---

## 目錄結構

```
alpha-factor-forge/
  src/                        前端（React/Canvas）+ 共用 core 純函數（TS）
    core/                     ✅ 純函數，無 React/DOM/IO 依賴
      indicators/               技術指標
      backtest/                 回測引擎（deterministic）
      metrics/                  績效指標
      scoring/                  Gate + Score（Phase B）
      strategy-dsl/             DSL schema + whitelist validator
      validation/               Train/Val/Test split（Phase B）
      benchmarks/               benchmark（Phase B）
      hashing/                  strategy_hash / dataset_hash
    tauri-client/             前端 → backend 的正式橋接
    workers/                  輕量 Web Worker（Phase A 佔位）
    components/ pages/ charts/ stores/ services/     UI（待移植現有 AlphaFactorForge）
  src-tauri/                  🟡 Rust backend
    src/
      main.rs
      commands/               db / file（Phase A）；ai / secret / discovery（stub）
      db/                     連線、migrations、repositories
      ai/ secrets/ jobs/ sidecar/     Phase C/B stub
    migrations/               SQL migration 檔
    Cargo.toml
    tauri.conf.json
```

詳見 `TODO.md` 的逐檔狀態與待補清單。
