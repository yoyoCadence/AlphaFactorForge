# AlphaFactorForge Workspace

**Automated Indicator Discovery Workstation**  
中文定位：**自動因子鍛造與驗證工作站**  
日本語：**自動インジケーター発見・検証ワークステーション**

Repository: `https://github.com/yoyoCadence/AlphaFactorForge`

Languages:

- [中文](#中文)
- [English](#english)
- [日本語](#日本語)

---

## 中文

### 專案概述

AlphaFactorForge 最初來自 Claude Design 產出的單檔 PWA，用於加密貨幣行情、策略回測與 paper trading。現在的產品方向更聚焦：**自動設計新的技術指標與策略假說，並在信任任何結果前，用可重現、可審計、避免過擬合的流程進行驗證**。

目前工作區同時保留兩個層次：

- `AlphaFactorForge.dc.html`：既有 browser-only PWA prototype，是 UI 與功能行為的參考。
- `alpha-factor-forge/`：Tauri v2 + React + TypeScript + Rust + SQLite 的 Phase A desktop scaffold，是長期本機優先 app 的目標。

目前方向：保留既有 Web UI 概念，把耐久資料、長時間任務與安全敏感操作移到 Tauri/Rust；SQLite 作為主要資料庫；AI/API key 由 backend 與 OS keychain 管理，不能留在 frontend。

### 目前狀態

> **目前狀態的唯一事實來源是根目錄 `tasks.md` 的「Current Snapshot」。** 本段僅為概述；數字類進度（測試數、slice 進度）一律以 `tasks.md` 為準，避免多處敘述分歧。

- 原始壓縮檔 `區塊鏈交易策略PWA.zip` 已解壓並整合到此工作區。
- 專案已初始化 Git，並持續透過 PR 在 `yoyoCadence/AlphaFactorForge` 開發。
- 前端 baseline 驗證指令皆通過：`npm install` → `npm test` → `npm run typecheck` → `npm run build`（實際測試數見 `tasks.md`）。
- Native Tauri 已在本機驗證：Rust/Cargo 就緒，`cargo check` 與 `cargo tauri dev` 皆通過，多尺寸 icon 已生成。
- CI 於每個 PR 執行 typecheck / test / build / cargo-check（含 `cargo test`）/ e2e。
- 2026-07-16 的 [npm audit 盤點與修復](docs/security-audit-npm.md) 已以 Vite 6.4.3 + Vitest 3.2.6 清除 5 個 dev-tool findings；full / production audit 皆為 0。後續仍**不要直接跑 `npm audit fix --force`**。

### 工作區內容

- `AlphaFactorForge.dc.html`, `Canvas.dc.html`, `support.js`, `manifest.webmanifest`：legacy PWA prototype 與 runtime files。
- `alpha-factor-forge/`：Tauri desktop scaffold。
- `STRATEGY_DISCOVERY.md`：Strategy Discovery Engine v3 設計，Tauri Desktop 架構已定案。
- `STRATEGY_GUIDE.md`：策略編輯器使用指南，包含 params、rule blocks、code mode。
- `HISTORY.md`：前次 agent 工作的高層次交接摘要。
- `CONVERSATION_HISTORY.md`：更完整的歷史對話脈絡。
- `tasks.md`：唯一 active task board；舊 `task.md` 已合併，不應重建第二份任務板。
- `AGENTS.md`：Codex / Claude Code / human contributors 的協作契約與專案上下文。
- `screenshots/`, `uploads/`：prototype 截圖與支援圖片。

### 架構摘要

Legacy prototype `AlphaFactorForge.dc.html` 目前是 browser-only：

- Canvas K 線圖，包含 zoom、pan、hover OHLC tooltip、MA/EMA/BB/RSI/VOL overlays、buy/sell markers。
- Binance、OKX、Coinbase market data fallback。
- 可匯出/匯入 frozen dataset，確保回測可重現。
- 三種策略編輯模式：params、rule blocks、manual JavaScript expression code mode。
- 回測支援 fees、slippage、position sizing、fill mode、long/short/both、stop-loss/take-profit、Bar Magnifier、holdout comparison、parameter sweep heatmap、report export、Bar Replay。
- Browser paper trading simulation。
- prototype 階段使用 localStorage 保存策略與 paper state。

Target desktop app `alpha-factor-forge/`：

- Frontend：Vite, React 18, TypeScript。
- Core logic：`src/core/*` 純 TypeScript modules，不依賴 React/DOM/IO。
- Bridge：frontend 透過 typed `src/tauri-client/*` wrappers 呼叫 backend。
- Backend：Tauri v2, Rust 1.77+, `rusqlite` bundled SQLite。
- Storage：SQLite 由 Rust/Tauri commands 管理。
- Worker：frontend Web Worker 只負責輕量 interactive backtests、short sweeps、indicator precompute。
- Heavy jobs：Strategy Discovery 應在 Rust backend job runner 執行。
- AI：由 backend 管理 keychain/secure storage；frontend 不可儲存或讀取 API keys。

SQLite schema 來源：`alpha-factor-forge/src-tauri/migrations/0001_init.sql`

- `datasets`
- `candles`
- `strategy_def`
- `backtest_summary`
- `trades`
- `discovery_runs`
- `discovery_jobs`
- `ai_generations`
- `app_settings`

### 不可妥協的邊界

- API keys 不可進 frontend code、localStorage、SQLite 或 plain config files。
- Frontend 不可直接呼叫 AI APIs。
- AI 只能產生通過 whitelist validator 的 JSON Strategy DSL。
- Manual code mode 僅供人類使用；AI 不可使用 code mode。
- Test data 不可用於 generation、tuning、ranking 或 AI prompts。
- Long-running Strategy Discovery 不可跑在 UI thread。
- Tauri migration 期間，`localStorage` 只可保存非敏感 UI preferences。

### Roadmap

Phase A: Tauri Foundation

- 驗證本機 Rust/Tauri prerequisites。
- 補齊 Tauri icons。
- 執行 `cd alpha-factor-forge/src-tauri && cargo check`。
- 啟動 `cargo tauri dev`，確認 SQLite 會在 OS app data 初始化。
- 完成 `backtest_summary` / `trades` persistence。
- 將 PWA UI 移植到 React/Tauri structure，並移除 frontend direct persistence。

Phase B: Discovery And Validation

- Train/Validation/Test split with embargo。
- Gate + Score。
- Benchmarks：Buy & Hold, SMA, RSI, Bollinger, Random Entry。
- 使用 `strategy_hash` + `dataset_hash` 做 duplicate skip。
- Rust backend discovery queue，支援 pause/resume/cancel/checkpoint。
- Tauri event protocol：progress/result/done。
- Results Explorer 只用 Validation ranking，隱藏 Test。
- Lifecycle minimum：`candidate -> validated -> rejected`。

Phase C: Minimal AI Strategy Lab

- AI keys 存入 OS keychain/secure storage。
- Backend AI connection test。
- 僅生成 JSON Strategy DSL。
- 使用 whitelist validator 驗證 DSL。
- AI strategies 進 queue 前必須人工批准。

Phase D: Deferred Automation

- paper live / promoted / quarantined lifecycle。
- Hidden Test one-time reveal flow。
- Clustering and family refinement。
- Meme/low-liquidity risk filters。
- Fully automatic walk-forward。
- Full closed-loop AI automation。

Phase D 明確不屬於第一輪實作範圍。

### 已知問題與待確認

Legacy PWA：

- RSI panel 在 symbol/interval 變更後偶爾可能未刷新，移植前或移植中需驗證。
- MA period wiring 需確認 chart drawing 是否與 strategy state 一致。
- Bar Replay UI 已存在，但 signal-to-bar alignment 仍需再檢查。
- `manifest.webmanifest` 已存在；Service Worker 尚未實作。
- Optional product features：walk-forward analysis、multi-asset portfolio backtesting、alerts/webhooks。

Tauri scaffold：

- `save_backtest_result`／`get_backtest_results` 已實作：持久化到 `backtest_summary`（依 strategy+dataset+segment upsert）。已通過本機 `cargo check` 與 CI `cargo test`。
- `export_report`（依 result_id 產報告）仍是 stub；實際匯出走 Slice 7-2 的 `save_report`。
- AI、secrets、discovery commands 仍是 stubs（Phase B/C）。
- App icon 已就位：`icons/icon.png`（另有 `app-icon-source.png` 1254×1254 原圖）。
- Rust/Cargo 已就緒；`cargo check` 與 `cargo tauri dev` 均已通過。

### 驗證指令

Frontend/core baseline:

```bash
cd alpha-factor-forge
npm install
npm test
npm run typecheck
npm run build
```

Native/Tauri baseline:

```bash
cd alpha-factor-forge/src-tauri
cargo check
```

Rust/Tauri prerequisites 與 icons 準備好後再啟動 desktop app：

```bash
cd alpha-factor-forge
cargo tauri dev
```

---

## English

### Overview

AlphaFactorForge began as a Claude Design single-file PWA for crypto market data, strategy backtesting, and paper trading. The product direction is now sharper: **automatically design new indicators and strategy hypotheses, then validate them through reproducible, auditable, anti-overfitting workflows before trusting any result**.

This workspace keeps two layers:

- `AlphaFactorForge.dc.html`: the existing browser-only PWA prototype, used as the UI and behavior reference.
- `alpha-factor-forge/`: the Phase A desktop scaffold built with Tauri v2, React, TypeScript, Rust, and SQLite.

Current direction: preserve the useful Web UI concepts, move durable storage, long-running jobs, and security-sensitive operations into Tauri/Rust, use SQLite as the main database, and keep AI/API-key handling in the backend and OS keychain.

### Current Status

> **The single source of truth for current status is the "Current Snapshot" section in the root `tasks.md`.** This section is an overview only; for progress numbers (test counts, slice progress) defer to `tasks.md` so claims never diverge.

- The original archive `區塊鏈交易策略PWA.zip` has been unpacked and integrated into this workspace.
- The project is a Git repository developed via PRs in `yoyoCadence/AlphaFactorForge`.
- The frontend baseline commands all pass: `npm install` → `npm test` → `npm run typecheck` → `npm run build` (see `tasks.md` for the current test count).
- Native Tauri has been verified locally: Rust/Cargo are set up, `cargo check` and `cargo tauri dev` both pass, and multi-size icons are generated.
- CI runs typecheck / test / build / cargo-check (incl. `cargo test`) / e2e on every PR.
- The [2026-07-16 npm audit triage and remediation](docs/security-audit-npm.md) cleared all five dev-tool findings with Vite 6.4.3 + Vitest 3.2.6; both full and production audits now report zero. **Do not run `npm audit fix --force` for future advisories.**

### Workspace Contents

- `AlphaFactorForge.dc.html`, `Canvas.dc.html`, `support.js`, `manifest.webmanifest`: legacy PWA prototype and runtime files.
- `alpha-factor-forge/`: Tauri desktop scaffold.
- `STRATEGY_DISCOVERY.md`: Strategy Discovery Engine v3 design with the Tauri Desktop architecture finalized.
- `STRATEGY_GUIDE.md`: strategy editor guide for params, rule blocks, and code mode.
- `HISTORY.md`: handoff summary from previous agent work.
- `CONVERSATION_HISTORY.md`: fuller chronological conversation history.
- `tasks.md`: the single active task board. The former `task.md` has been merged and should not be recreated.
- `AGENTS.md`: collaboration contract and project context for Codex, Claude Code, and humans.
- `screenshots/`, `uploads/`: prototype screenshots and supporting images.

### Architecture

The legacy prototype `AlphaFactorForge.dc.html` is browser-only:

- Canvas candlestick chart with zoom, pan, hover OHLC tooltip, MA/EMA/BB/RSI/VOL overlays, and buy/sell markers.
- Market data fallback across Binance, OKX, and Coinbase.
- Dataset export/import for reproducible frozen datasets.
- Three strategy editor modes: params, rule blocks, and manual JavaScript expression code mode.
- Backtesting with fees, slippage, position sizing, fill mode, long/short/both direction, stop-loss/take-profit, Bar Magnifier, holdout comparison, parameter sweep heatmap, report export, and Bar Replay.
- Browser paper trading simulation.
- Prototype-stage localStorage persistence for strategy and paper-trading state.

The target desktop app is `alpha-factor-forge/`:

- Frontend: Vite, React 18, TypeScript.
- Core logic: pure TypeScript modules under `src/core/*`, with no React/DOM/IO dependency.
- Bridge: the frontend calls the backend through typed `src/tauri-client/*` wrappers.
- Backend: Tauri v2, Rust 1.77+, `rusqlite` with bundled SQLite.
- Storage: SQLite managed by Rust/Tauri commands.
- Worker: frontend Web Worker only for light interactive backtests, short sweeps, or indicator precompute.
- Heavy jobs: Strategy Discovery belongs in the Rust backend job runner.
- AI: backend-managed keychain/secure storage only; the frontend must never store or read API keys.

SQLite schema source: `alpha-factor-forge/src-tauri/migrations/0001_init.sql`

- `datasets`
- `candles`
- `strategy_def`
- `backtest_summary`
- `trades`
- `discovery_runs`
- `discovery_jobs`
- `ai_generations`
- `app_settings`

### Non-Negotiable Boundaries

- API keys never go into frontend code, localStorage, SQLite, or plain config files.
- The frontend must not call AI APIs directly.
- AI may only produce validated JSON Strategy DSL.
- Manual code mode is for humans only; AI must never use code mode.
- Test data must not drive generation, tuning, ranking, or AI prompts.
- Long-running Strategy Discovery must not run on the UI thread.
- During the Tauri migration, `localStorage` is acceptable only for non-sensitive UI preferences.

### Roadmap

Phase A: Tauri Foundation

- Verify local Rust/Tauri prerequisites.
- Add required Tauri icons.
- Run `cd alpha-factor-forge/src-tauri && cargo check`.
- Launch `cargo tauri dev` and confirm SQLite initializes in OS app data.
- Complete `backtest_summary` / `trades` persistence.
- Port the PWA UI into the React/Tauri structure without direct frontend persistence.

Phase B: Discovery And Validation

- Train/Validation/Test split with embargo.
- Gate + Score.
- Benchmarks: Buy & Hold, SMA, RSI, Bollinger, Random Entry.
- Duplicate skip via `strategy_hash` + `dataset_hash`.
- Rust backend discovery queue with pause/resume/cancel/checkpoint.
- Tauri event protocol for progress/result/done.
- Results Explorer that ranks Validation only and hides Test.
- Lifecycle minimum: `candidate -> validated -> rejected`.

Phase C: Minimal AI Strategy Lab

- Store AI keys through OS keychain/secure storage.
- Backend AI connection test.
- Generate JSON Strategy DSL only.
- Validate DSL through the whitelist validator.
- Require manual approval before queueing AI strategies.

Phase D: Deferred Automation

- paper live / promoted / quarantined lifecycle.
- Hidden Test one-time reveal flow.
- Clustering and family refinement.
- Meme/low-liquidity risk filters.
- Fully automatic walk-forward.
- Full closed-loop AI automation.

Phase D is explicitly out of the first implementation pass.

### Known Issues And Open Questions

Legacy PWA:

- RSI panel may occasionally fail to refresh after symbol/interval changes.
- MA period wiring should be verified in chart drawing against strategy state.
- Bar Replay UI exists, but signal-to-bar alignment needs another pass.
- `manifest.webmanifest` exists; Service Worker is not implemented.
- Optional product features: walk-forward analysis, multi-asset portfolio backtesting, alerts/webhooks.

Tauri scaffold:

- `save_backtest_result` / `get_backtest_results` are implemented: they persist to `backtest_summary` (upsert on strategy+dataset+segment). Verified via local `cargo check` and CI `cargo test`.
- `export_report` (render-by-result_id) is still a stub; actual export goes through the Slice 7-2 `save_report` command.
- AI, secrets, and discovery commands are stubs (Phase B/C).
- App icon is in place: `icons/icon.png` (plus `app-icon-source.png`, 1254×1254 source).
- Rust/Cargo are set up; `cargo check` and `cargo tauri dev` both pass.

### Verification

Frontend/core baseline:

```bash
cd alpha-factor-forge
npm install
npm test
npm run typecheck
npm run build
```

Native/Tauri baseline:

```bash
cd alpha-factor-forge/src-tauri
cargo check
```

Run the desktop app only after Rust/Tauri prerequisites and icons are ready:

```bash
cd alpha-factor-forge
cargo tauri dev
```

---

## 日本語

### 概要

AlphaFactorForge は、もともと Claude Design で作られた暗号資産マーケットデータ、戦略バックテスト、paper trading 用の単一ファイル PWA から始まりました。現在のプロダクト方針はより明確です。**新しいインジケーターと戦略仮説を自動生成し、結果を信頼する前に、再現可能で監査しやすく、過剰最適化を避ける検証フローに通すこと**が中心です。

このワークスペースには 2 つの層があります。

- `AlphaFactorForge.dc.html`：既存の browser-only PWA prototype。UI と挙動の参照元です。
- `alpha-factor-forge/`：Tauri v2 + React + TypeScript + Rust + SQLite による Phase A desktop scaffold。長期的な local-first app の本体です。

現在の方向性：既存 Web UI の良い部分を残しつつ、永続データ、長時間ジョブ、セキュリティ上重要な処理を Tauri/Rust に移します。SQLite を主要データベースとし、AI/API key は backend と OS keychain で管理します。

### 現在の状態

> **現在の状態の唯一の情報源はルートの `tasks.md`「Current Snapshot」です。** 本節は概要のみで、数値的な進捗（テスト数・slice 進捗）は `tasks.md` を参照してください。

- 元のアーカイブ `區塊鏈交易策略PWA.zip` は解凍され、このワークスペースに統合済みです。
- Git repository として `yoyoCadence/AlphaFactorForge` で PR ベースに開発中です。
- Frontend baseline コマンドはすべて通過：`npm install` → `npm test` → `npm run typecheck` → `npm run build`（テスト数は `tasks.md` 参照）。
- Native Tauri はローカル検証済み：Rust/Cargo 準備済み、`cargo check` と `cargo tauri dev` が通り、マルチサイズ icon も生成済みです。
- CI は各 PR で typecheck / test / build / cargo-check（`cargo test` 含む）/ e2e を実行します。
- [2026-07-16 の npm audit 調査と修正](docs/security-audit-npm.md)では Vite 6.4.3 + Vitest 3.2.6 により dev-tool findings 5 件を解消し、full / production audit はともに 0 件になりました。今後も **`npm audit fix --force` は実行しないでください**。

### ワークスペース内容

- `AlphaFactorForge.dc.html`, `Canvas.dc.html`, `support.js`, `manifest.webmanifest`：legacy PWA prototype と runtime files。
- `alpha-factor-forge/`：Tauri desktop scaffold。
- `STRATEGY_DISCOVERY.md`：Strategy Discovery Engine v3 設計。Tauri Desktop architecture は確定済みです。
- `STRATEGY_GUIDE.md`：params、rule blocks、code mode を含む strategy editor guide。
- `HISTORY.md`：以前の agent 作業の handoff summary。
- `CONVERSATION_HISTORY.md`：より完全な会話履歴。
- `tasks.md`：唯一の active task board。旧 `task.md` は統合済みで、再作成しないでください。
- `AGENTS.md`：Codex、Claude Code、人間の contributor 向け協業ルールと project context。
- `screenshots/`, `uploads/`：prototype screenshots と supporting images。

### アーキテクチャ

Legacy prototype `AlphaFactorForge.dc.html` は browser-only です。

- Canvas candlestick chart：zoom、pan、hover OHLC tooltip、MA/EMA/BB/RSI/VOL overlays、buy/sell markers。
- Binance、OKX、Coinbase の market data fallback。
- 再現可能な frozen dataset の export/import。
- 3 つの strategy editor modes：params、rule blocks、manual JavaScript expression code mode。
- Fees、slippage、position sizing、fill mode、long/short/both、stop-loss/take-profit、Bar Magnifier、holdout comparison、parameter sweep heatmap、report export、Bar Replay に対応した backtesting。
- Browser 上の paper trading simulation。
- Prototype 段階では localStorage に strategy / paper state を保存。

Target desktop app は `alpha-factor-forge/` です。

- Frontend：Vite, React 18, TypeScript。
- Core logic：`src/core/*` 配下の pure TypeScript modules。React/DOM/IO に依存しません。
- Bridge：frontend は typed `src/tauri-client/*` wrappers を通して backend を呼びます。
- Backend：Tauri v2, Rust 1.77+, bundled SQLite 付き `rusqlite`。
- Storage：SQLite は Rust/Tauri commands が管理します。
- Worker：frontend Web Worker は軽量な interactive backtests、short sweeps、indicator precompute のみに使用します。
- Heavy jobs：Strategy Discovery は Rust backend job runner で実行します。
- AI：keychain/secure storage は backend 管理のみ。frontend は API keys を保存・閲覧してはいけません。

SQLite schema source：`alpha-factor-forge/src-tauri/migrations/0001_init.sql`

- `datasets`
- `candles`
- `strategy_def`
- `backtest_summary`
- `trades`
- `discovery_runs`
- `discovery_jobs`
- `ai_generations`
- `app_settings`

### 譲れない境界

- API keys は frontend code、localStorage、SQLite、plain config files に置かない。
- Frontend は AI APIs を直接呼ばない。
- AI は whitelist validator を通過する JSON Strategy DSL のみ生成できる。
- Manual code mode は人間専用。AI は code mode を使わない。
- Test data は generation、tuning、ranking、AI prompts に使わない。
- Long-running Strategy Discovery は UI thread で実行しない。
- Tauri migration 中の `localStorage` は、非センシティブな UI preferences のみに限定する。

### Roadmap

Phase A: Tauri Foundation

- ローカル Rust/Tauri prerequisites を確認する。
- 必要な Tauri icons を追加する。
- `cd alpha-factor-forge/src-tauri && cargo check` を実行する。
- `cargo tauri dev` を起動し、SQLite が OS app data に初期化されることを確認する。
- `backtest_summary` / `trades` persistence を完成させる。
- PWA UI を React/Tauri structure に移植し、frontend direct persistence をなくす。

Phase B: Discovery And Validation

- Embargo 付き Train/Validation/Test split。
- Gate + Score。
- Benchmarks：Buy & Hold, SMA, RSI, Bollinger, Random Entry。
- `strategy_hash` + `dataset_hash` による duplicate skip。
- Pause/resume/cancel/checkpoint 対応の Rust backend discovery queue。
- Progress/result/done 用 Tauri event protocol。
- Validation のみで ranking し、Test を隠す Results Explorer。
- 最小 lifecycle：`candidate -> validated -> rejected`。

Phase C: Minimal AI Strategy Lab

- AI keys を OS keychain/secure storage に保存する。
- Backend AI connection test。
- JSON Strategy DSL のみ生成する。
- Whitelist validator で DSL を検証する。
- AI strategies を queue に入れる前に manual approval を必須にする。

Phase D: Deferred Automation

- paper live / promoted / quarantined lifecycle。
- Hidden Test one-time reveal flow。
- Clustering and family refinement。
- Meme/low-liquidity risk filters。
- Fully automatic walk-forward。
- Full closed-loop AI automation。

Phase D は最初の実装範囲には含めません。

### 既知の問題と確認事項

Legacy PWA：

- Symbol/interval 変更後、RSI panel が更新されない場合があるため確認が必要です。
- MA period wiring が chart drawing と strategy state に対して正しいか確認が必要です。
- Bar Replay UI は存在しますが、signal-to-bar alignment は再確認が必要です。
- `manifest.webmanifest` はありますが、Service Worker は未実装です。
- Optional product features：walk-forward analysis、multi-asset portfolio backtesting、alerts/webhooks。

Tauri scaffold：

- `save_backtest_result` / `get_backtest_results` は実装済みです：`backtest_summary` に永続化します（strategy+dataset+segment で upsert）。ローカル `cargo check` と CI `cargo test` で検証済みです。
- `export_report`（result_id からのレポート生成）は依然 stub です。実際のエクスポートは Slice 7-2 の `save_report` を使います。
- AI、secrets、discovery commands は stubs です（Phase B/C）。
- App icon は配置済みです：`icons/icon.png`（`app-icon-source.png` 1254×1254 原図あり）。
- Rust/Cargo は準備済みで、`cargo check` と `cargo tauri dev` は通過します。

### 検証コマンド

Frontend/core baseline:

```bash
cd alpha-factor-forge
npm install
npm test
npm run typecheck
npm run build
```

Native/Tauri baseline:

```bash
cd alpha-factor-forge/src-tauri
cargo check
```

Rust/Tauri prerequisites と icons の準備後に desktop app を起動してください。

```bash
cd alpha-factor-forge
cargo tauri dev
```
