# AlphaFactorForge Tasks

This is the single active task board for the workspace. The former `task.md` legacy feature map has been merged into this file and should not be recreated as a second task board.

Task lifecycle: **Backlog -> Next -> In Progress -> Done**.

## Current Snapshot

- Legacy prototype: `AlphaFactorForge.dc.html` is a feature-rich browser PWA and remains the UI/behavior reference.
- Target app: `alpha-factor-forge/` is the Tauri Desktop Phase A scaffold.
- Baseline verified: `npm test`, `npm run typecheck`, and `npm run build` pass in `alpha-factor-forge/`.
- Native Tauri verified: Rust 1.96 / Cargo / MSVC build tools / Tauri CLI v2 installed; `cargo check` and `cargo tauri dev` both pass; multi-size icons generated.
- PR #1 (backtest_summary persistence + app icons) merged to `main`; save->read round-trip + upsert self-test PASS in the running app. PR CI runs typecheck/test/build/cargo-check (all green).
- Source-of-truth architecture: `STRATEGY_DISCOVERY.md` v3 and `README.md`.
- Historical context: `HISTORY.md` and `CONVERSATION_HISTORY.md`.

## Next

- [ ] UI port — Slice 4b: code strategy mode (safe whitelist-AST interpreter; no new Function/eval). See the slice plan under In Progress.

## In Progress

- [ ] Port the legacy AlphaFactorForge PWA UI into the React/Tauri structure (incremental)
  - Reality check: `AlphaFactorForge.dc.html` is a custom "dc"-framework export (`{{ }}` bindings, `<sc-for>`/`<sc-if>`, runtime `support.js`), ~1500 lines; app logic + initial state live in the `<script type="text/x-dc">` block (line ~685+). This is a REWRITE in React, not a copy-paste port.
  - Constraints found: webview CSP is `default-src 'self'`, so live exchange `fetch` from the frontend is blocked (the legacy ran as a plain PWA); SQLite currently holds no candles. Data must arrive via import (file/JSON) or a future backend fetch command.
  - Rules: reuse `src/core/*` + `tauri-client`; persist via one `metricsToBacktestSummary()` helper (PR #1 decision); code mode stays manual-only; do NOT mass-port — one small slice per PR.
  - Legacy defaults to mirror (from the script block): symbols `BTCUSDT…` + intervals `1m..1d`; `defStrat()` params (fastMA 9 / slowMA 21 / rsi 14 / fee 0.05% / slip 0.02% / size 100% / fill close / long); 6 preset strategies; localStorage keys `cd_strat` / `cd_stratlib` / `cd_paper`.
  - **Slice plan (small, one PR each):**
    - [x] Slice 1: backtest pipeline service in `src/services/` (`strategy.ts` / `strategySignals.ts` / `backtestRunner.ts` / `metricsMapper.ts`) — params-mode strategy -> entry/exit signals (12 of 14 legacy signals; `stoch*` await a core STOCH indicator) -> `runBacktest` -> `metrics` -> `metricsToBacktestSummary`. 8 unit tests; `npm test` 33/33 + typecheck green. No UI.
    - [x] Slice 2: Backtest panel UI (params mode) — `src/components/BacktestPanel.tsx` + app shell in `main.tsx`. Dataset picker (SQLite) + JSON/sample candle import, strategy params form (12 signals + exec model), run via Slice 1 service, metrics table, save (strategy_def + backtest_summary via `metricsToBacktestSummary`). Helpers: `candleAdapter` (db↔core candle), `sampleData` (seeded synthetic, CSP-safe), `strategyRecord` (StrategyDef + hash). +7 tests (40/40), typecheck + build green. Removed the PR #1 bridge self-test. No chart/sweep/replay/live/library.
    - [x] Slice 3: chart canvas — `src/charts/CandleChart.tsx` (+ pure `scale.ts`, unit-tested). Candlesticks + MA fast/slow + EMA + Bollinger overlays, volume strip, RSI subpanel (30/70 guides); overlay toggles; indicators via core/indicators (computed over full series, drawn over visible window). Wired into BacktestPanel (loads candles on dataset select). Static fit-to-width; pan/zoom + trade markers deferred. +6 tests (46/46), typecheck + build green.
    - [x] Slice 4a: blocks (rule-builder) strategy mode — `mode: 'params' | 'blocks'` on the strategy; `Rule { l: OperandId, op, r }` AND-lists for entry/exit; `buildBlocksSignals` + `buildSignals` dispatcher over a generalized `evalCond` (adds `>=`/`<=`); 15 named operands from core/indicators (stoch/atr/volMa deferred). UI: 參數/積木 tabs + rule-builder rows (operand select · op · operand|const datalist) in `BacktestPanel`. `strategyRecord` type follows `strat.mode`. +4 tests (50/50), typecheck + build green.
    - [ ] Slice 4b (CURRENT): code strategy mode — manual-only. MUST use a SAFE whitelist-AST expression interpreter; NO `new Function` / `eval` / dynamic exec anywhere (STRATEGY_DISCOVERY §0.3). AI may never reach code mode. Split out from 4a so the eval-sensitive part gets its own careful PR.
    - [ ] Slice 5: holdout comparison + parameter sweep.
    - [ ] Slice 6: bar replay + live signals.
    - [ ] Slice 7: strategy library + report (JSON/CSV) export.
  - Carry-over detail (kept from backlog): suggested folders `src/components`, `src/pages`, `src/charts`, `src/stores`, `src/services`; preserve the terminal-like dense visual style; replace prototype localStorage persistence with SQLite via `tauri-client`.

## Backlog

### Phase A: Tauri Foundation

- [ ] Refine `insert_strategy` UPSERT to refresh mutable fields
  - Current `ON CONFLICT(strategy_hash) DO UPDATE SET updated_at` only touches `updated_at`; re-saving a same-hash strategy does NOT update name/lifecycle/source/definition.
  - Acceptable today (hash covers the full strategy), but editing only a non-hashed field (e.g. name) and re-saving silently keeps the old value.
  - Decide intended semantics and update the UPSERT; covered by `repositories::tests::insert_strategy_upserts_on_hash_without_duplicating` (documents current behavior).

- [ ] Implement report/file export through Tauri commands
  - Keep JSON and CSV export behavior from the prototype.
  - Add a typed frontend wrapper for `export_report`.

- [ ] Replace prototype localStorage strategy/data persistence with local-first Tauri storage
  - Store datasets, candles, strategies, summaries, and trades in SQLite.
  - Keep localStorage limited to non-sensitive UI preferences.

- [ ] Resolve prototype issues before or during the port
  - Verify/fix RSI panel refresh after symbol/interval changes.
  - Verify MA period wiring in chart drawing against strategy state.
  - Verify Bar Replay signal-to-bar alignment.
  - Decide whether to add a Service Worker for the legacy PWA or defer because Tauri is now the target.

- [ ] Review npm audit findings without forced breaking upgrades
  - `npm install` reported 5 vulnerabilities.
  - Prefer targeted upgrades after checking Vite/Tauri compatibility.

### Phase B: Discovery And Validation

- [ ] Implement Phase B validation foundations
  - Add Train/Validation/Test split with embargo.
  - Keep Test hidden from ranking and prompts.
  - Add walk-forward only after the basic split is stable.

- [ ] Implement Gate + Score and benchmarks
  - Gate: minimum trades, cost-adjusted average trade, rolling-window consistency, max drawdown, concentration limits, benchmark wins.
  - Score: OOS CAGR, Sortino, Calmar, regime robustness, profit factor, consistency, complexity/turnover/data-mining penalties.
  - Benchmarks: Buy & Hold, SMA, RSI, Bollinger, Random Entry.

- [ ] Add duplicate skip and result reuse
  - Use `strategy_hash`, `dataset_hash`, and segment.
  - Never retest the same strategy/data/segment combination unnecessarily.

- [ ] Implement the Tauri backend discovery job runner
  - Support start, pause, resume, cancel, checkpoint, and progress events.
  - Keep heavy discovery off the UI thread and out of the Web Worker.
  - Persist run/job progress in SQLite.

- [ ] Build Results Explorer UI
  - Show Validation ranking only by default.
  - Provide filters, details, benchmark deltas, DSL tree inspection, and segment comparisons.
  - Keep Test hidden until one-time promotion flow is implemented.

- [ ] Implement minimum strategy lifecycle
  - `candidate -> validated -> rejected`.
  - Defer `paper_live`, `promoted`, and `quarantined` automation to Phase D.

### Phase C: Minimal AI Strategy Lab

- [ ] Implement secure AI key storage
  - Store AI API keys only through OS keychain/secure storage.
  - Frontend may set/check/delete key status but must never read key values back.

- [ ] Add backend AI connection test
  - Route all AI calls through Tauri backend commands.
  - Handle rate limits, retries, and quota errors in backend.

- [ ] Implement JSON Strategy DSL generation and validation
  - AI may output JSON DSL only.
  - Validate via whitelist schema and suspicious-token checks.
  - Mirror validator behavior in Rust for defense in depth.

- [ ] Add manual approval before AI strategies enter the queue
  - Save prompt/raw/parsed/validation records in `ai_generations`.
  - Approved strategies become `strategy_def(source=ai, type=ai_dsl)`.

### Deferred / Optional Product Work

- [ ] Walk-forward analysis beyond the initial split.
- [ ] Multi-asset portfolio backtesting.
- [ ] Alerts and webhooks.
- [ ] Paper-live forward test flow.
- [ ] Hidden Test one-time reveal and promotion flow.
- [ ] Strategy clustering and family refinement.
- [ ] Meme/low-liquidity risk filters and dynamic slippage.
- [ ] Full closed-loop AI automation.

## Done

- [x] Automate the blocks-save verification (Slice 4a follow-up; replaces manual SQLite checks).
  - TS: strengthened `buildStrategyDef` tests — a blocks rules strategy persists `type='blocks'`, `JSON.parse(original_definition_json).mode === 'blocks'` with the rules intact, and params/blocks `strategy_hash` differ.
  - Rust: `repositories::tests` integration tests on an in-memory migrated DB — `insert_strategy` round-trips `type='blocks'`, and a same-hash re-save does not duplicate (documents the current UPSERT-only-`updated_at` behavior).
  - CI: added `cargo test --locked` to the `cargo-check` job so the Rust test runs on every PR. No schema change; no code mode. Manual SQLite Viewer checks are no longer the acceptance gate.
- [x] Prepare the local Tauri verification environment.
  - Installed Rust/Rustup/Cargo 1.96, MSVC C++ build tools (VS Build Tools 2022), and Tauri CLI v2.
  - Generated multi-size icons (`icon.png`/`.ico`/`.icns` + platform sets) via `tauri icon`.
  - `cd alpha-factor-forge/src-tauri && cargo check` passes; PR CI `cargo-check` job also green.
- [x] Launch the Tauri Phase A bridge locally with `cargo tauri dev`.
  - Native window opens; title bar shows the app + icon.
  - Status reads `database already initialized at startup`; SQLite created in OS app-data; bridge lists datasets (0 initially, expected).
  - Verified end-to-end via the bridge-shell self-test: save->read round-trip + upsert returned PASS.
  - Note: use `npm run tauri -- dev` if `cargo-tauri` is not installed as a cargo subcommand (`cargo install tauri-cli` enables `cargo tauri dev`).
- [x] Complete Phase A backtest result persistence (core).
  - Added `repositories::insert_backtest_summary` (upsert on strategy+dataset+segment) and `list_backtest_summaries`.
  - Wired `save_backtest_result` (now takes a typed `BacktestSummary`, not a raw JSON string) and `get_backtest_results`.
  - Added the `BacktestSummary` interface to `tauri-client/commands.ts`; `npm test` + `npm run typecheck` green.
  - Still needs local `cargo check` (no Rust toolchain in the authoring env). `trades`-table detail deferred to the UI port.
- [x] Add Tauri app icon (`src-tauri/icons/icon.png` + `app-icon-source.png`); `tauri.conf.json` references it.
- [x] Review the project archive and capture initial project context in `AGENTS.md`.
- [x] Copy `HISTORY.md` and `CONVERSATION_HISTORY.md` into the workspace.
- [x] Establish the canonical local source tree from `區塊鏈交易策略PWA.zip`.
  - Preserved the archived `alpha-factor-forge/` scaffold structure intact.
  - Preserved `AlphaFactorForge.dc.html`, `STRATEGY_DISCOVERY.md`, `STRATEGY_GUIDE.md`, screenshots, uploads, and prototype notes as reference material.
  - No same-path file conflicts were found during extraction, so no existing files were overwritten.
  - Root `README.md` and `.gitignore` were integrated as workspace-level files while keeping `alpha-factor-forge/README.md` and `alpha-factor-forge/.gitignore`.
- [x] Run the TypeScript baseline verification for the scaffold.
  - Installed npm dependencies in `alpha-factor-forge/`.
  - `npm test` passed: 3 test files, 25 tests.
  - `npm run typecheck` passed after narrow TypeScript strict-mode fixes.
  - `npm run build` passed and produced `alpha-factor-forge/dist/`.
  - Started local inspection servers: scaffold on `http://127.0.0.1:5173/`, legacy prototype on `http://127.0.0.1:5174/AlphaFactorForge.dc.html`.
- [x] Merge the legacy `task.md` feature map into this active `tasks.md` board.

## Legacy PWA Feature Map

This section preserves the useful planning content from the former `task.md`. It describes what the prototype already demonstrated, not what has been ported to Tauri.

### Market Data

- Historical candles and live prices via Binance / OKX / Coinbase fallback.
- Multiple intervals: 1m / 3m / 5m / 15m / 1h / 4h / 1d.
- Manual data source selection and visible active source.
- Deep historical paging: 500 / 2000 / 5000 / max available.
- Dataset export/import as JSON/CSV for reproducible frozen backtests.

### Strategy Definition

- Params mode.
- Rule-block mode.
- Manual JavaScript expression code mode.
- Built-in example strategies for learning/testing.
- Indicator operands include MA, EMA, RSI, MACD, Bollinger, ATR, stochastic, volume MA, and price fields.

### Execution Model

- Fees/commission.
- Slippage.
- Position sizing.
- Fill assumption: current close vs next open.
- Direction: long, short, both/reversal.
- Stop-loss / take-profit.
- Bar Magnifier for intrabar SL/TP ordering.
- No-future-function discipline by using closed-bar data.

### Performance And Analysis

- Net return, buy-and-hold comparison, win rate, trade count, max drawdown, profit factor, average trade, ending equity.
- Sharpe, Sortino, Calmar.
- Average win/loss, win/loss ratio, expectancy, max win/loss streaks, largest win/loss, average holding bars, time in market.
- Equity curve, buy-and-hold overlay, underwater drawdown.
- Round-trip trade table with MAE/MFE.
- Focus Data mode that collapses the chart and enlarges statistics.

### Robustness And Workflow

- Real date-range backtesting.
- Holdout / sample-out comparison.
- Parameter sweep heatmap.
- Bar Replay.
- Strategy library in localStorage.
- Backtest report JSON export and trade CSV export.

### Remaining Legacy/PWA Options

- Walk-forward analysis.
- Multi-asset portfolio backtesting.
- Alerts/webhooks.
- Service Worker if the PWA line is kept alive.
