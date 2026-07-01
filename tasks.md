# AlphaFactorForge Tasks

This is the single active task board for the workspace. The former `task.md` legacy feature map has been merged into this file and should not be recreated as a second task board.

Task lifecycle: **Backlog -> Next -> In Progress -> Done**.

## Current Snapshot

- Legacy prototype: `AlphaFactorForge.dc.html` is a feature-rich browser PWA and remains the UI/behavior reference.
- Target app: `alpha-factor-forge/` is the Tauri Desktop Phase A scaffold.
- Baseline verified: `npm test`, `npm run typecheck`, and `npm run build` pass in `alpha-factor-forge/`.
- Native Tauri verified: Rust 1.96 / Cargo / MSVC build tools / Tauri CLI v2 installed; `cargo check` and `cargo tauri dev` both pass; multi-size icons generated.
- Progress (PRs #1–#8 merged): backtest_summary persistence + app icons; UI port Slice 1 (backtest pipeline service), Slice 2 (Backtest panel), Slice 3 (chart canvas), Slice 4a (blocks rule-builder mode); plus a save-path test-automation PR. `npm test` ~53 green.
- Next: UI port Slice 6 — bar replay + live signals. Slice 5c (clickable "?" help markers via a reusable `HelpTip`) done; Slice 5d (chart buy/sell trade markers ▲/▼ from `result.trades`) done; Slice 5b (parameter sweep + interactive heatmap + applied highlight) done; Slice 5a (holdout) done; strategy editor has params/blocks/code modes (code via the safe interpreter, manual-only). Then Slice 7 (strategy library + report export).
- PR CI runs typecheck / test / build / cargo-check (now incl. `cargo test`) — green per PR; `main` requires branches up to date before merge.
- Source-of-truth architecture: `STRATEGY_DISCOVERY.md` v3 and `README.md`.
- Historical context: `HISTORY.md` and `CONVERSATION_HISTORY.md`.

## Next

- [ ] UI port — Slice 7: strategy library + report (JSON/CSV) export. Slice 6 (bar replay + live signals) done across 6-1/6-2/6-3; later Slice 8b (real Tauri window, if multi-monitor wanted). See the slice plan under In Progress.

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
    - [x] Slice 4b-1: code-mode safe interpreter + signals + tests (no UI/AI/eval) — `src/services/exprInterpreter.ts` (tokenizer → recursive-descent parser → restricted AST evaluator) + `buildCodeSignals` + `mode:'code'`/`entryCode`/`exitCode`. Whitelist: ops `+ - * / > < >= <= == != && || !` + parens; variables = blocks operands; functions `prev(x)` (1-bar), `crossUp(a,b)`, `crossDown(a,b)`; finite numeric literals only. Rejects member access/indexing/assignment/strings/ternary/unknown ids+calls/non-finite; caps source ≤1000, nodes ≤128, depth ≤16, no nested time-shift. No `eval`/`Function`; AI never reaches code mode. +37 tests (87 total), typecheck + build green.
    - [x] Slice 4b-2: code-mode UI — `BacktestPanel` 參數/積木/程式碼 mode tabs + a `CodeField` (entry/exit textareas with live interpreter validation + red error border) + whitelist variable/function hint + manual-only note. No backtest-logic change (4b-1 already wired `buildCodeSignals`/dispatch). typecheck + `npm test` 87 + build green.
    - [x] Slice 5a: holdout (out-of-sample) comparison — `BacktestPanel` Holdout toggle + split % (last N% = out-of-sample). `run()` reuses `runParamsBacktest` `from`/`to` to backtest in-sample [0,split) and out-of-sample [split,n) over the same candles (full indicator history); metrics table gains 全期 / 樣本內 / 樣本外 columns. Save still uses the full-period result. typecheck + `npm test` 87 + build green.
    - [x] Slice 5b-1: parameter-sweep engine + tests (no UI) — `src/services/paramSweep.ts`. Vary 1–2 numeric params (`SWEEP_PARAM_KEYS`: fastMA/slowMA/emaPeriod/rsiPeriod/rsiBuy/rsiSell/macdFast/macdSlow/bbPeriod) over min/max/step ranges, run `runParamsBacktest` per combo, score by a `SweepMetricId` (`net`/`sharpe`/`pf`/`winRate`/`calmar`/`dd`, dd stored as -maxDrawdown). Mirrors legacy runSweep: axis cap 64, combo cap 256 (throws), best requires trades>0, pf/calmar non-finite guards (Inf→99); reuses `from`/`to` to sweep a sub-range. Pure/deterministic; a single failing combo yields a null cell. +16 tests (103 total), typecheck + build green. No UI/AI/eval.
    - [x] Slice 5b-2: parameter-sweep UI — a collapsible 「參數掃描」 section in `BacktestPanel` (X param + min/max/step, optional 2-D Y, optimisation metric; live combo-count + dup/over-cap guards). Runs the 5b-1 `runParamSweep` (yields a frame so 「掃描中…」 paints), renders a red→yellow→green `SweepHeatmap` (value + trade count per cell, best cell outlined) and 「套用最佳」 which patches the strategy. data-testid hooks (`sweep-toggle`/`sweep-2d`/`sweep-combos`/`run-sweep`/`apply-best`/`sweep-best-cell`) + a new `e2e/sweep.spec.ts` flow. No backtest-logic change. typecheck + `npm test` 105 + build + e2e (2 specs) green.
    - [x] Slice 5b-3: interactive sweep heatmap — fix the 2-D layout overlap (axes now render as full-width wrap-safe rows; metric/2-D/combo count sit in their own controls bar), make **every heatmap cell clickable** to apply that combo (`applySweepCombo`), and add an **applied-cell highlight** (blue ✓ ring via `appliedCell` state) distinct from the best-cell ★ outline; `套用最佳` reuses `applySweepCombo`. **Also highlights the applied params in the strategy form + chart quick row** (blue ✓ accent via `appliedKeys`; a param drops out the moment it is hand-edited). data-testid: per-cell `sweep-cell-<x>[-<y>]`, `sweep-best-marker`, `sweep-applied-marker`, form `applied-<key>` / chart `quick-applied-<key>`. +1 e2e (cell-click apply + form highlight) and updated 2 existing specs. typecheck + `npm test` 105 + build + e2e (4 specs) green.
    - [x] Slice 5d: chart buy/sell trade markers — draw entry/exit markers on `CandleChart` from the latest backtest `result.trades`, like a trading terminal: buy ▲ below the low (green), sell ▼ above the high (red); LONG = buy@entry/sell@exit, SHORT flips. Pure `tradeLegs(trades, timeToIndex)` in `scale.ts` (maps trade `entryTime`/`exitTime` → bar index + buy/sell, drops unknown times); a new `trades` overlay toggle (default on) in `OverlayToggles`. +4 tests (109 total), typecheck + build + e2e (4 specs) green. Canvas pixels aren't E2E-assertable; geometry is unit-tested.
    - [x] Slice 5c: clickable "?" help markers — reusable `src/components/HelpTip.tsx` (small circular `?` that toggles a short explanation popover; closes on click-again / Escape / outside-click; `align` left|right anchor avoids container-edge overflow; `role="tooltip"` + `aria-label`/`aria-describedby`, `type="button"` + stopPropagation so it never triggers an enclosing control). Wired via a central `HELP` copy map onto the 資料集 / 策略 / 執行模型 / Holdout / 回測績效 / 參數掃描 headers and the 執行回測 / 儲存結果 / 執行掃描 / 套用最佳 actions. The Holdout row was rewrapped so the tip sits outside its `<label>` (can't toggle the checkbox). UI-only; no logic/backtest change. +3 e2e (`e2e/help.spec.ts`: toggle · Escape+outside-close · single-open-at-a-time); typecheck + `npm test` 109 + build + e2e (7 specs) green.
    - [x] Slice 6: bar replay + live signals — step through candles bar-by-bar to watch the strategy trigger. Done across 6-1/6-2/6-3:
      - [x] Slice 6-1: replay cursor on the chart (no autoplay) — pure `replayWindow(total, upto, maxBars)` in `scale.ts` (inclusive [start,end] window ending at the cursor; `upto=null` keeps the pre-replay latest-`maxBars` behaviour) drives a new optional `upto` prop on `CandleChart` (clips every pane + trade markers to [start,end]; indicators still computed over the full series; dashed blue playhead at the cursor). `BacktestPanel` gains a 「回放模式」 toggle + ⏮/◀ scrubber(range)/▶ + 「第 i/n 根」 readout (cursor clamps to the latest bar whenever candles change). +4 unit tests (113 total) + `e2e/replay.spec.ts` (step · scrub · reset · hide). typecheck + build + e2e (8 specs) green.
      - [x] Slice 6-2: autoplay — ⏵/⏸ play + a 1×/2×/4× speed select driving the 6-1 cursor via a `setInterval` (400/speed ms). Pure `replayTick(cursor,total)` in `scale.ts` (advance one bar, clamp, `atEnd`) does the step; a separate effect stops autoplay at the last bar (kept out of the state updater → StrictMode-safe); play from the end restarts at bar 0; autoplay stops when 回放模式 is turned off or candles change. +2 unit tests (115 total) + `e2e/replay.spec.ts` autoplay flow (⏵ → advances → auto-stops at 600/600). typecheck + build + e2e (11 specs) green.
      - [x] Slice 6-3: live signal readout — under the replay controls, a 「此根訊號」 row shows for the cursor bar whether 進場/出場 conditions are TRUE (via the same `buildSignals` the backtest uses, memoized over candles+strat so it's not recomputed per autoplay tick; code-mode parse errors hide the row) plus 持倉 多/空/空手 from the last backtest's trades via pure `positionAtTime(trades,t)` in `scale.ts` (inclusive bounds; '—（回測後顯示）' until a run). No live exchange fetch (CSP-blocked; replay-driven only). +2 unit tests (117 total) + `e2e/replay.spec.ts` (readout labels + position resolves after a backtest). typecheck + build + e2e (12 specs) green.
    - [ ] Slice 7: strategy library + report (JSON/CSV) export.
    - [ ] Slice 8 (user-requested 2026-07-01): pop-out 圖表 / 回測績效 into an enlarge-able view via a button, non-modal so the other sections stay usable. **Decision 2026-07-01: do (a) now; keep (b) as a future advanced version.**
      - [x] Slice 8a: in-app floating resizable/draggable panel — reusable `src/components/FloatingPanel.tsx` (title-bar drag + bottom-right corner resize + ✕/Esc close, `position:fixed`, `role=dialog aria-modal=false`, NON-modal — no backdrop — with a render-prop giving children the inner size so the chart canvas fills it). `BacktestPanel` factors chart + metrics into `renderChart(h)` / `renderMetricsTable(fontSize)` and adds an 「放大/收合」 button on the 圖表 and 回測績效 headers; when popped the section shows a `PoppedOutNote` inline and the content renders enlarged in the panel, still driven by the same React state so left-column edits reflow live. Chart pop-out defaults over the results area so strategy controls stay clear. UI-only; no backtest/logic change. +`e2e/popout.spec.ts` (chart: open → run backtest from the still-usable left column → close; metrics: open → Esc close). typecheck + `npm test` 113 + build + e2e (10 specs) green.
      - [ ] Slice 8b (future / advanced): real Tauri second OS window for true multi-monitor pop-out. Needs a `?window=…` route mounting just the chart/metrics + Rust `WebviewWindowBuilder` + cross-window state sync via Tauri events; NOT browser-e2e-testable (cargo tauri dev smoke owns it). Only if multi-monitor is wanted — benefit is OS-level window management (drag to another screen), which (a) can't do.
  - Carry-over detail (kept from backlog): suggested folders `src/components`, `src/pages`, `src/charts`, `src/stores`, `src/services`; preserve the terminal-like dense visual style; replace prototype localStorage persistence with SQLite via `tauri-client`.

## Backlog

### Phase A: Tauri Foundation

- [ ] Refine `insert_strategy` UPSERT to refresh mutable fields
  - Current `ON CONFLICT(strategy_hash) DO UPDATE SET updated_at` only touches `updated_at`; re-saving a same-hash strategy does NOT update name/lifecycle/source/definition.
  - Acceptable today (hash covers the full strategy), but editing only a non-hashed field (e.g. name) and re-saving silently keeps the old value.
  - Decide intended semantics and update the UPSERT; covered by `repositories::tests::insert_strategy_upserts_on_hash_without_duplicating` (documents current behavior).

- [ ] Code-mode UX polish: disable Run while an entry/exit expression is invalid
  - Today invalid code only shows a red border + error; pressing 執行回測 still goes through the existing error handling. Non-blocking (noted at Slice 4b-2).

- [ ] Add more browser E2E flows for BacktestPanel (Playwright harness foundation landed — see Done)
  - Goal: let Playwright exercise the same React UI behavior in Vite/browser mode without requiring manual Tauri WebView clicks.
  - Foundation done: `dataClient` seam (prod = tauri-client; dev `?mock=1` = in-memory mock) + Playwright (chromium) + CI `e2e` job.
  - [x] First regression done: Holdout stale-UI flow (load sample -> enable Holdout -> run -> 3 columns -> disable -> single column) — `e2e/holdout.spec.ts`.
  - Remaining flows: params/blocks/code tab switching, invalid code expression error display, valid code run path, save-button UI messaging via the mock. (Keep one flow per PR.)
  - Must stay test-only/dev-only. Do not route production code around Tauri security boundaries, do not replace typed `tauri-client` wrappers, and do not store real data in browser localStorage as a product path.
  - This does NOT replace true Tauri verification. It validates frontend interaction and React state only; real Rust command wiring, SQLite persistence, migrations, AppData paths, and Tauri/WebView behavior still require Rust integration tests plus a small `cargo tauri dev` smoke checklist.
  - Suggested shape: a small test harness entry point or dependency-injected client seam, seeded sample data fixtures, and Playwright tests that run against `npm run dev` in CI/local dev.

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

- [x] Browser E2E harness foundation + first regression (reduces manual UI testing).
  - `dataClient` seam: production/Tauri uses the real `tauri-client`; in Vite DEV only, `?mock=1` swaps in an in-memory mock (`mockClient`, seeded sample candles — no localStorage, no real DB; dead-code-eliminated from prod).
  - Playwright (chromium) running against `npm run dev`; CI `e2e` job; `npm run e2e` locally. Vitest scoped to `src` so it ignores `e2e/`.
  - First test `e2e/holdout.spec.ts`: Slice 5a Holdout stale-UI flow (load sample -> enable -> run -> 3 columns -> disable -> single column) + `data-testid` hooks.
  - Second test `e2e/sweep.spec.ts` (Slice 5b-2): load sample -> expand 參數掃描 -> combo count -> run -> heatmap best cell -> apply best.
  - Explicitly does NOT replace real Tauri/Rust/SQLite verification (Rust integration tests + `cargo tauri dev` smoke still own that).
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
