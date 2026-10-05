# AlphaFactorForge - Automated Indicator Discovery Workstation

自動因子鍛造與驗證工作站。本機優先的策略研究環境，用於技術指標回測、策略探索、AI 生成 Strategy DSL、SQLite 本機資料庫、長時間 discovery job、結果審計與防過擬合驗證。

> 架構定案見 `../STRATEGY_DISCOVERY.md`（v3, Tauri）。本 repo 為其實作。

> 2026-10-05 文件校正：下方描述目前實作；進度、測試數及驗收以 [tasks.md](../tasks.md)
> 為唯一來源。P12 已有 campaign／ledger 與純統計計算，原生 operator 驗收仍開放；
> P13 confirmation runtime 與 P15 AI provider／approve 尚未完成。

> 2026-09-22：P11 新增可執行 `strategy-dsl-v1`：TypeScript／Rust evaluator、共享 parity fixture、
> 嚴格白名單／型別／arity／lookback／因果驗證，以及 `discovery-config-v2` backend runner admission。
> DSL、參數 schema 與 P05 lineage 會在執行前保存；AI provider／approve 仍待 P15。
> 見 [DSL 契約](../docs/strategy-dsl-contract.md)。

> 2026-09-21：P09 新增 service CLI `fetch-tiingo`／`tiingo-status`、Windows 認證管理員讀取、
> 五檔 US ETF 資格報告與不可變原件。見 [Tiingo 使用說明](../docs/market-source-tiingo-v1.md)。
> 真實認證下載驗收待本機 token 配置；fixtures 不代表真實來源涵蓋率。

> 2026-09-20：P08 新增純 `core/market-data/etf.ts`／`core/metrics/etf.ts` 與 Rust 對應模組。
> 使用方式與範圍見 [ETF 日線語意](../docs/etf-semantics-v1.md)；這是後續 ETF adapters／成交核心的介面，
> 不會切換現有 UI 回測的計算契約。初始 scaffold 已逐步實作，歷史交接見根目錄 `HISTORY.md`。

---

## 實作範圍（請先讀）

這是由 Phase A scaffold 演進的本機研究工作站；目前已可使用回測 UI、SQLite 持久化與 backend Discovery，完整 AI／confirmation／paper 流程仍依計畫逐項交付。

- ✅ **純邏輯**：`src/core/*`（indicators / backtest / metrics / hashing / Strategy DSL），以 Vitest 與 TS／Rust fixtures 驗證；不代表所有輸入或契約皆無待辦。
- ✅ **原生實作**：Rust commands、repositories、Discovery runner、獨立 service、workspace ownership、事件／研究歷史及市場 snapshots；需 Rust／Tauri 環境驗證。
- ✅ **已移植 UI**：圖表、回測、Holdout、sweep、replay、策略庫、報告匯出、Discovery、Results Explorer、研究歷史與 campaign。瀏覽器 `?mock=1` 使用隔離記憶體 seam，不能替代原生驗收。

檔案頂部的 FULL／SKELETON／STUB 可能保留歷史註解；目前模組對照與未接線邊界見 [TODO.md](TODO.md)。

---

## 需要的本機工具

| 工具 | 版本 | 用途 |
|---|---|---|
| Node.js | 20（CI 基準；依 `package-lock.json` 的工具需求） | 前端 build／tests |
| Rust | ≥ 1.89（stable；以 `src-tauri/Cargo.toml` 的 `rust-version` 為準） | Tauri backend |
| Tauri CLI | v2 | `cargo tauri dev/build` |
| 平台依賴 | 見 Tauri 官方 | macOS: Xcode CLT；Windows: WebView2 + MSVC；Linux: webkit2gtk 等 |

安裝 Tauri 前置依賴：照 https://v2.tauri.app/start/prerequisites/ 對應你的 OS。

---

## 首次安裝與啟動

```bash
# 1. 安裝前端依賴
cd alpha-factor-forge
npm install

# 2. 使用 package.json 已安裝的 Tauri v2 CLI
# 開發模式會編譯 Rust backend、啟動 Vite 並開原生視窗
npm run tauri -- dev
```

> 首次原生啟動會編譯 Rust 依賴，需數分鐘。
> SQLite 資料庫會在首次啟動時於 app data 目錄建立並跑 migration。

---

## 逐步驗證（Phase A）

| 步驟 | 指令 | 預期 | 環境 |
|---|---|---|---|
| 1. 純邏輯單元測試 | `npm test`（vitest） | indicators / dsl / hashing / backtest 測試通過 | ✅ 任何環境 |
| 2. TypeScript 型別檢查 | `npm run typecheck` | 無型別錯誤 | ✅ 任何環境 |
| 3. Rust 編譯檢查 | `cd src-tauri && cargo check` | 編譯通過（可能有 unused warning） | 🟡 需本機 Rust |
| 4. Rust 回歸 | `cd src-tauri && cargo test --locked` | repositories、runner、ledger／service 等通過 | 🟡 需本機 Rust |
| 5. 啟動 app | 在 app 目錄 `npm run tauri -- dev` | 原生工作站與 DB 初始化成功 | 🟡 需本機 Tauri |
| 6. 匯入與回測 | app 內匯入 CSV，再執行／保存單策略回測 | dataset、摘要與交易可讀回 | 🟡 需本機 |
| 7. UI 回歸 | `npm run e2e`（先安裝 Playwright Chromium） | mock seam 操作通過 | ✅ Node／瀏覽器環境 |

> 本機與 CI 已有原生 build／啟動驗證；每次工作的實際證據與剩餘 operator checklist 見 tasks.md。單元、mock E2E、原生 smoke 與 campaign 驗收各有範圍，不能互相代替。

---

## 目錄結構

```
alpha-factor-forge/
  src/                        前端（React/Canvas）+ 共用 core 純函數（TS）
    core/                     ✅ 純函數，無 React/DOM/IO 依賴
      indicators/               技術指標
      backtest/                 回測引擎（deterministic）
      metrics/                  績效指標
      market-data/              市場資料／ETF 純契約
      strategy-dsl/             DSL schema + whitelist validator + evaluator
      validation/               Train/Val/Test split（Phase B）
      hashing/                  strategy_hash / dataset_hash
    tauri-client/             前端 → backend 的正式橋接
    workers/                  回測與掃描 worker 協定；UI sweep 使用可取消的 module worker
    components/ charts/ theme/     已移植 UI
    services/                 純服務、Gate／Score／benchmarks、DTO 映射
  src-tauri/                  Rust backend
    src/
      main.rs
      commands/               db / file / discovery / runtime / research / campaign / window
      db/                     連線、repositories、持久事件與 ownership
      discovery_runner/       backend 工作執行、checkpoint、驗證
      runtime/ research/ market/ rs_core/     service、ledger、snapshots、純核心
    migrations/               有序追加的 workspace SQL migrations
    registry_migrations/      工作區外 trial registry 的獨立 migrations
    Cargo.toml
    tauri.conf.json
```

Schema 由 `src-tauri/src/db/mod.rs` 的 MIGRATIONS 順序及 `migrations/` 共同定義：
0001 初始表、0002 驗證紀錄、0003 runner；後續追加 ownership、事件／請求、研究歷史、市場、ledger binding、campaign（目前至 0010）。
詳見 [TODO.md](TODO.md) 與 [PHASE_A_VERIFY.md](PHASE_A_VERIFY.md)；API keys 不進 SQLite。
