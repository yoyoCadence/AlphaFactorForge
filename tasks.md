# AlphaFactorForge Tasks

This is the single active task board for the workspace. The former `task.md` legacy feature map has been merged into this file and should not be recreated as a second task board.

Task lifecycle: **Backlog -> Next -> In Progress -> Done**.

## Current Snapshot

- Legacy prototype: `AlphaFactorForge.dc.html` is a feature-rich browser PWA and remains the UI/behavior reference.
- Target app: `alpha-factor-forge/` is the Tauri Desktop Phase A scaffold.
- Baseline verified: `npm test`, `npm run typecheck`, and `npm run build` pass in `alpha-factor-forge/`.
- Native Tauri is not verified yet: Rust/Cargo were not available on PATH during the latest check.
- Source-of-truth architecture: `STRATEGY_DISCOVERY.md` v3 and `README.md`.
- Historical context: `HISTORY.md` and `CONVERSATION_HISTORY.md`.

## Next

- [ ] Prepare the local Tauri verification environment
  - Confirm Rust 1.77+, Cargo, Tauri CLI v2, WebView2 Runtime, and MSVC build tools on Windows.
  - Current check: Node/npm are available; Rust/Cargo are not currently available on PATH.
  - Icons present: `src-tauri/icons/icon.png` (+ `app-icon-source.png` 1254x1254); optional multi-size via `cargo tauri icon`.
  - Run `cd alpha-factor-forge/src-tauri && cargo check` (verifies the new `backtest_summary` repository + commands compile).

## In Progress

## Backlog

### Phase A: Tauri Foundation

- [ ] Launch the Tauri Phase A bridge locally with `cargo tauri dev`
  - Confirm the native window opens.
  - Confirm SQLite initializes in OS app-data.
  - Confirm the bridge can list datasets.

- [ ] Implement report/file export through Tauri commands
  - Keep JSON and CSV export behavior from the prototype.
  - Add a typed frontend wrapper for `export_report`.

- [ ] Port the legacy AlphaFactorForge PWA UI into the React/Tauri structure
  - Move chart, data controls, strategy editor, execution model, holdout, sweep, replay, strategy library, paper trading, and export flows into React modules.
  - Suggested folders: `src/components`, `src/pages`, `src/charts`, `src/stores`, `src/services`.
  - Reuse `src/core/*` pure functions and `tauri-client` wrappers instead of direct frontend persistence.
  - Persist backtests through ONE mapping helper `metricsToBacktestSummary()` (camelCase `Metrics` -> snake_case `BacktestSummary`); do NOT inline the field mapping per component. Decided in PR #1.
  - Preserve the terminal-like dense visual style from the prototype.

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
