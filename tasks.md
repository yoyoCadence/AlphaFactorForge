# AlphaFactorForge Tasks

This is the single active task board for the workspace. The former `task.md` legacy feature map has been merged into this file and should not be recreated as a second task board.

Task lifecycle: **Backlog -> Next -> In Progress -> Done**.

## Current Snapshot

- Legacy prototype: `AlphaFactorForge.dc.html` is a feature-rich browser PWA and remains the UI/behavior reference.
- Target app: `alpha-factor-forge/` is the Tauri Desktop Phase A scaffold.
- Baseline verified: `npm test`, `npm run typecheck`, and `npm run build` pass in `alpha-factor-forge/`.
- Native Tauri verified: Rust 1.96 / Cargo / MSVC build tools / Tauri CLI v2 installed; `cargo check` and `cargo tauri dev` both pass; multi-size icons generated.
- Progress (through code-mode UX polish + REF-004 + BUG-004 + UI-port Slice 8b-2): Phase A backtest pipeline; chart (canvas + overlays + trade markers + wheel-zoom + drag-pan + hover + bar replay); params/blocks/code strategy modes with invalid-expression Run guard; holdout; parameter sweep + interactive heatmap; report export (Slice 7-2); SQLite strategy library (Slice 7-3); native chart + metrics OS windows (Slice 8b); mutable-field strategy UPSERT semantics (REF-004); plus the 2026-07-07 project audit (`docs/` blueprint) and its backlog work: DOC-001, BUG-001, REF-001→004, TEST-002 (golden lock + legacy parity), and Backtest Correctness Phases 1–3 (fee-inclusive accounting, settled metrics, execution-bar/risk fills, legacy `both` reversal, and normalized-fraction validation). Current tests: 193 vitest + 23 Playwright e2e.
- Next: no task is currently staged; select the next small slice from Backlog after the code-mode UX polish merges.
- PR CI runs typecheck / test / build / cargo-check (now incl. `cargo test`) — green per PR; `main` requires branches up to date before merge.
- Source-of-truth architecture: `STRATEGY_DISCOVERY.md` v3 and `README.md`.
- Historical context: `HISTORY.md` and `CONVERSATION_HISTORY.md`.

## Next

- None.

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
    - [x] Slice 7: strategy library + report (JSON/CSV) export. Split into small PRs:
      - [x] Slice 7-1: pure report/export formatters (no UI/IO) — `src/services/reportExport.ts`: `buildReport`/`reportToJson` (a schema-versioned JSON snapshot: app + exportedAt ISO + strategyName + full strategy + dataset meta + metrics + trades), `tradesToCsv` (header + one round-trip-trade per row, +ISO times, RFC-4180-ish quoting), `suggestedFilename` (fs-safe `AlphaFactorForge_<symbol>_<interval>_<date>.<ext>`). +5 unit tests (125 total). typecheck + build green. No UI; module unused until 7-2 wires it.
      - [x] Slice 7-2: export UI + file write — 「匯出 JSON / 匯出 CSV」 buttons on 回測績效 call the 7-1 formatters (`reportToJson` / `tradesToCsv`) and a typed `files.saveReport` wrapper. Tauri command `save_report` writes sanitized `.json` / `.csv` filenames to the OS Downloads directory and avoids overwriting existing files; dev/mock uses a browser Blob download fallback. +2 Rust helper tests + `e2e/export.spec.ts`; typecheck + `npm test` 125 + build + `cargo check --locked` + `cargo test --locked` 4 + `npm run e2e` 14 green. Playwright now uses one worker by default to avoid Windows/Vite cold-load flakes in the mock browser suite.
      - [x] Slice 7-3: strategy library — list SQLite-saved strategies through the existing `get_strategies` Tauri command + typed client; validate persisted definitions before loading them into the params/blocks/code form; refresh after save; unsupported DSL rows remain read-only in the list. Strategy name now lives with the editor so it remains visible after loading clears stale backtest results. +3 unit tests and `e2e/strategy-library.spec.ts`; typecheck + `npm test` 128 + build + cargo check/test + e2e 15 green. Replaces the prototype's localStorage `cd_stratlib`; delete remains optional/deferred.
    - [x] Slice 10 (user-requested 2026-07-01; deferred pan/zoom, low priority): chart pan/zoom in the ported app (the legacy prototype had it; the port had been static fit-to-width since Slice 3). Introduce a visible-window state and reconcile it with replay `upto` + `maxBars`. Touches `CandleChart` heavily; done after Slice 7.
      - [x] Slice 10-1: cursor-anchored wheel zoom + reset-to-fit — `CandleChart` owns an inclusive visible-bar window; negative/positive wheel deltas zoom in/out by 0.8×/1.25× around the bar under the mouse, clamped to 10–`maxBars` bars and dataset bounds. During replay, `reconcileBarWindow` preserves the zoom count while following `upto`, so future candles remain hidden and the playhead stays visible. Dataset changes and replay enter/exit reset to fit. Overlay shows 「顯示 N 根」 + a reset button. The canvas uses a native `{ passive: false }` wheel listener so zoom never scrolls the surrounding page. +5 pure scale tests (133 total) + `e2e/zoom.spec.ts` (normal zoom/reset/max-fit + page position lock + replay boundary). typecheck + build + cargo check/test + e2e 17 green.
      - [x] Slice 10-2: drag-pan — pointer-capture drag on a zoomed visible window with a 4px movement threshold, so a short press remains hover/click and only a true drag hides the crosshair. Pure `panBarWindow` preserves bar count and clamps to dataset bounds or the replay cursor; dragging right reveals older bars, dragging left reveals newer. Replay resumes follow mode when panned back to its right boundary; a historical panned window never paints a false playhead at its right edge. Canvas exposes grab/grabbing cursors plus diagnostic start/end data attributes. +3 scale tests (136 total) + `e2e/pan.spec.ts` (click-vs-drag + index shift/count preservation + replay boundary). typecheck + build + cargo check/test + e2e 19 green.
    - [x] Slice 8 (user-requested 2026-07-01): pop-out 圖表 / 回測績效 into an enlarge-able view via a button, non-modal so the other sections stay usable. **Decision 2026-07-01: do (a) now; keep (b) as a future advanced version.**
      - [x] Slice 8a: in-app floating resizable/draggable panel — reusable `src/components/FloatingPanel.tsx` (title-bar drag + bottom-right corner resize + ✕/Esc close, `position:fixed`, `role=dialog aria-modal=false`, NON-modal — no backdrop — with a render-prop giving children the inner size so the chart canvas fills it). `BacktestPanel` factors chart + metrics into `renderChart(h)` / `renderMetricsTable(fontSize)` and adds an 「放大/收合」 button on the 圖表 and 回測績效 headers; when popped the section shows a `PoppedOutNote` inline and the content renders enlarged in the panel, still driven by the same React state so left-column edits reflow live. Chart pop-out defaults over the results area so strategy controls stay clear. UI-only; no backtest/logic change. +`e2e/popout.spec.ts` (chart: open → run backtest from the still-usable left column → close; metrics: open → Esc close). typecheck + `npm test` 113 + build + e2e (10 specs) green.
      - [x] Slice 8b: real Tauri second OS windows for true multi-monitor pop-out. Split to preserve one-small-slice-per-PR:
        - [x] Slice 8b-1: chart OS window — async Rust `open_popout_window("chart")` uses a stable single-instance label, focuses an existing window, or builds resizable `index.html?window=chart` via `WebviewWindowBuilder` (async avoids the documented Windows WebView2 deadlock). `ChartPopoutWindow` mounts without the main workspace; a typed `windowBridge` ready handshake + targeted snapshot/cursor events sync dataset candles, strategy, overlays, trades, and replay while preserving child-local zoom/pan. Full candles are not resent on replay ticks. A least-privilege Tauri capability grants `listen`/`unlisten`/`emitTo` only to `main` and `chart-popout-window`. +2 TS tests (138 total), +3 Rust tests (7 total), +`e2e/native-window.spec.ts` (20 E2E total); typecheck + build + cargo check/test green. Native click/open smoke remains a PR manual checklist because the Windows Computer Use helper pipe was unavailable after the required retry.
        - [x] Slice 8b-2: metrics OS window — extracted `MetricsTable` for the inline, floating, and native views; added a standalone `index.html?window=metrics` child mount and typed ready/snapshot events that keep full/Holdout results synchronized and clear stale child results when the main result resets. Rust opens or focuses one stable `metrics-popout-window`; a separate least-privilege capability grants event access only to `main` + that window. +3 TS tests (193 total), +2 Rust tests (9 total), +1 child-route E2E (22 total); typecheck + build + cargo check/test + full E2E green. Native click/open/focus/snapshot smoke remains a PR manual checklist because the Windows Computer Use native pipe was unavailable.
    - [x] Slice 9 (user-requested 2026-07-01): chart hover crosshair + unified 「此根資訊」 readout — extends the Slice 6-3 row so pointing at ANY bar shows its info in ANY mode (not just at the replay cursor). Pure `barAtX(x,padL,plotW,start,n)` in `scale.ts` (mouse-x→bar index, clamped) + `CandleChart` reports the hovered bar via `onHoverBar` (mouse handlers read a `layoutRef` written by `draw()`, which now returns its geometry) and draws a dashed crosshair; canvas gets `cursor:crosshair` + `data-testid`. `BacktestPanel`: `hoverBar` state; `activeBar = hovered ?? (replay cursor if on)`; the row (renamed `bar-info`/`bar-position`) shows 第N根 · 開高低收·量 · 進場/出場 · 持倉, gated on `activeBar != null` so it appears on hover even without replay. +3 unit tests (120 total) + `e2e/hover.spec.ts` (hover shows row w/ OHLC, leave hides) + renamed replay readout testids. typecheck + build + e2e (13 specs) green.
  - Carry-over detail (kept from backlog): suggested folders `src/components`, `src/pages`, `src/charts`, `src/stores`, `src/services`; preserve the terminal-like dense visual style; replace prototype localStorage persistence with SQLite via `tauri-client`.

## Backlog

### Phase A: Tauri Foundation

- [ ] Add more browser E2E flows for BacktestPanel (Playwright harness foundation landed — see Done)
  - Goal: let Playwright exercise the same React UI behavior in Vite/browser mode without requiring manual Tauri WebView clicks.
  - Foundation done: `dataClient` seam (prod = tauri-client; dev `?mock=1` = in-memory mock) + Playwright (chromium) + CI `e2e` job.
  - [x] First regression done: Holdout stale-UI flow (load sample -> enable Holdout -> run -> 3 columns -> disable -> single column) — `e2e/holdout.spec.ts`.
  - [x] Code validation flow: invalid entry/exit expressions show accessible errors and disable Run; params mode remains usable; repairing both expressions restores a successful code-mode run — `e2e/code-validation.spec.ts`.
  - Remaining flows: full params/blocks/code tab-state switching and save-button UI messaging via the mock. (Keep one flow per PR.)
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

- [x] Code-mode UX polish — disable Run while an entry/exit expression is invalid.
  - Entry and exit fields now share one synchronous validation result with the Run button, so either invalid expression blocks code-mode execution before the existing runtime fallback; dormant code expressions do not block params or blocks mode.
  - Added `aria-invalid` plus error-description links to both fields and a browser/mock regression covering invalid entry, mode switching, invalid exit, repairs, and a successful code-mode backtest.
  - `npm run typecheck`, 193 vitest, production build, and all 23 Playwright e2e tests pass. No Rust/SQLite paths changed, and no manual checklist is required because the browser flow owns the interaction.

- [x] REF-004 — refine `insert_strategy` UPSERT mutable-field semantics.
  - Same-hash re-saves now refresh `name`, `source`, and `updated_at`, while preserving the existing row id, definition-owned fields, and validation-owned `lifecycle` so a routine frontend save cannot demote a validated/rejected strategy to `candidate`.
  - Updated the existing no-duplicate test to cover rename/source refresh plus lifecycle preservation, and added a focused rename-persistence regression. No migration, hash, TypeScript, or UI changes.
  - `cargo check --locked`, 10 Rust tests, `npm run typecheck`, 193 vitest, production build, and all 22 Playwright e2e tests pass. No manual checklist is required because migrated in-memory SQLite tests own this repository behavior.

- [x] UI port — Slice 8b-2: real Tauri metrics OS window.
  - Extracted the shared full/Holdout metrics renderer so the existing inline and floating views and the new native child window use one formatter and column model.
  - Added a typed metrics ready/snapshot bridge, standalone child route, stable single-instance Rust window spec, and a separate least-privilege event capability scoped to `main` + `metrics-popout-window`.
  - `npm run typecheck`, 193 vitest, production build, `cargo check --locked`, 9 Rust tests, and all 22 Playwright e2e tests pass. Native window interaction remains a PR manual checklist because the Windows Computer Use native pipe was unavailable.

- [x] BUG-004 — backtest direction/input contract (Backtest Correctness Phase 3).
  - Restored legacy `both` reversal semantics for close and `nextOpen`: entry requests long, exit requests short, opposing positions close before opening the requested side, same-side signals hold, and entry wins a simultaneous entry/exit bar.
  - Core now rejects non-finite/out-of-range normalized sizing, fee, slippage, SL, and TP fractions instead of clamping them; UI/service percentage conversion and legacy fallbacks remain exclusively in `backtestRunner`.
  - Added 24 focused direction/validation tests and intentionally updated only the affected `both` golden trade count, last trade, and metrics. `npm run typecheck`, 190 vitest, production build, and all 21 Playwright e2e tests pass.

- [x] BUG-003 — backtest fill timing + risk exits (Backtest Correctness Phase 2).
  - `nextOpen` signals now create pending orders that execute on the following tested candle, so fills use the execution bar's timestamp/index and its open no longer leaks into the signal-bar equity point; final-bar signals do not fill beyond the tested range.
  - SL/TP exits now use gap-aware open/threshold prices plus the correct closing-side slippage for long and short positions, retain conservative SL-first handling without sub-bars, and apply normal exit slippage to EOD settlement.
  - Added 10 hand-calculated timing/risk/EOD tests and intentionally updated affected golden timestamps, risk-exit prices, and derived metrics. `npm run typecheck`, 166 vitest, production build, and all 21 Playwright e2e tests pass.

- [x] BUG-002 — backtest accounting + EOD settlement contract (Backtest Correctness Phase 1).
  - Adopted `docs/backtest-execution-contract.md`: normalized core units, fee-inclusive entry budget, long accounting, unleveraged 1× short collateral, settled metrics baseline/endpoint, and the approved BUG-003/004 follow-ups.
  - Corrected `ClosedTrade.pnl`/`pnlPct` to include both fees; 100% sizing now budgets entry fee without negative free cash; EOD replaces the final mark with settled equity; net return/CAGR/Sharpe/drawdown include configured starting equity.
  - Added six hand-calculated long/short/partial-size/multi-trade/EOD reconciliation tests and intentionally updated golden metrics. Trade count, fill timestamps, and fill prices remain unchanged in this phase.
  - `npm run typecheck`, 156 vitest, production build, and all 21 Playwright e2e tests pass.

- [x] TEST-002 — backtest engine golden tests + legacy parity report (audit backlog; no product-code edits).
  - Added four hard-coded golden configurations over `makeSampleCandles({ seed: 42, count: 300 })`, locking trade count, first/last trade time + price, net return, max drawdown, and Sharpe; added five boundary cases for same-bar signals, one candle, `from === to`, zero UI size, and negative UI costs.
  - Added `docs/engine-parity-report.md` with seven evidence-linked current/legacy comparisons and a follow-up BUG task template. It records recommendations only; maintainer decisions on engine semantics remain open.
  - `core/backtest/index.ts` is unchanged. `npm run typecheck` and all 150 vitest tests pass.

- [x] REF-003b — extract StrategySection; BacktestPanel becomes the orchestrator (PR #41; audit backlog, move-only).
  - Moved the strategy card (mode tabs, library picker, params/blocks/code editors, indicator/exec grids, Holdout toggle, Run) into `components/StrategySection.tsx`. **`BacktestPanel` 648 → 385 lines — the REF-003 `< 400` acceptance criterion is now met (finished per the REF-003 ultrareview).** This closes the audit refactor phase: the panel now only holds shared state + handlers and composes Chart / Dataset / Strategy / Results / Sweep sections.
  - Zero behaviour change: typecheck / 141 vitest / build / 21 e2e green; every strategy `data-testid` preserved.

- [x] REF-003 — extract Dataset/Results sections (PR #40; audit backlog, move-only).
  - `components/DatasetSection.tsx` (資料集 card) + `components/ResultsSection.tsx` (metrics table + export + save + metrics pop-out). `BacktestPanel` 811 → 648 lines. The `< 400` target was completed by the REF-003b follow-up (the strategy form + embedded library was the remaining large block).
  - Zero behaviour change: typecheck / 141 vitest / build / 21 e2e green.

- [x] REF-002 — extract ChartSection (PR #39; audit backlog, move-only).
  - Moved the chart concern (canvas + overlays + bar replay + hover 此根資訊 readout + quick param row + Slice 8a pop-out + Slice 8b native-window snapshot/cursor sync) into `components/ChartSection.tsx`; shared `components/PoppedOutNote.tsx` + `components/panelTypes.ts`. `BacktestPanel` 1047 → 811 lines. Rendered unconditionally so the always-on native-window "ready" listener registers exactly as the inline code did.
  - Zero behaviour change: typecheck / 141 vitest / build / 21 e2e green.

- [x] REF-001 — extract SweepSection from BacktestPanel (PR #37; audit backlog, move-only).
  - Moved the parameter-sweep block (state/handlers/`AxisEditor`/`SweepHeatmap`/JSX) into `components/SweepSection.tsx`; extracted shared `components/panelStyles.ts` (`S`), `components/NumberInput.tsx`, and `services/holdout.ts`. `BacktestPanel` 1382 → 1047 lines; every sweep `data-testid` preserved.
  - Zero behaviour change: typecheck / 141 vitest / build / 21 e2e green. (The rest of the decomposition — REF-002 ChartSection #39, REF-003 Dataset/Results #40, REF-003b StrategySection #41 — is now complete; see the entries above.)

- [x] BUG-001 — parameter sweep respects Holdout (PR #34; audit backlog `docs/improvement-backlog.md`).
  - When Holdout is on, the sweep now optimises on the in-sample segment only (shared `holdoutSplitIndex` with `run()`); Holdout-off keeps full-period behaviour unchanged.
  - Added a reactive in-sample scope note (`data-testid="sweep-scope"`) and a sweep e2e flow. typecheck / 141 vitest / build / 21 e2e green.
  - Follow-up (from ultrareview, low; PR #36): the sweep e2e now runs a Holdout-on sweep and asserts its winning combo differs from the full-period sweep, guarding the in-sample `from/to` wiring against silent removal.

- [x] DOC-001 — status single source of truth (PR #33; audit backlog).
  - Removed stale/contradictory status claims from README (中/EN/JP), AGENTS.md §0.1, and `alpha-factor-forge/TODO.md`; status now points here (Current Snapshot). Kept the "never `npm audit fix --force`" warning.

- [x] Project audit blueprint (PR #32).
  - Added `docs/project-audit-masterplan.md`, `docs/improvement-backlog.md`, `docs/creative-feature-roadmap.md`, `docs/agent-execution-protocol.md`. Analysis + agent-ready task specs; the backlog is a spec library, not a second task board (this file stays the single board).

- [x] Fix legacy saved-strategy loading compatibility.
  - Restored params rows saved before rule/code fields existed and blocks rows saved before code fields existed by filling only those historically absent, inactive-mode fields from current safe defaults.
  - Kept strict rejection for missing active-mode fields, partially missing field pairs, and malformed persisted values.
  - Added three regression cases; typecheck, 141 unit tests, build, 7 Rust tests, and 20 browser E2E tests pass on the post-Slice-8b-1 baseline.

- [x] UI port — Slice 8b-1 real Tauri chart OS window.
  - Added an async Rust single-instance/focus command for a resizable native chart window, avoiding the documented synchronous-command WebView2 deadlock on Windows.
  - Added a chart-only child mount and typed ready/snapshot/cursor event bridge; replay ticks send cursor-only updates instead of the full candle dataset.
  - Preserved child-local chart zoom/pan by retaining candle identity for same-dataset snapshot updates.
  - Added a least-privilege Tauri capability for the main/chart event handshake and a regression test covering both window labels and all required event permissions.
  - Added frontend/Rust tests and child-route E2E; real second-window click/open remains a manual Tauri checklist because Windows UI automation was unavailable.

- [x] UI port — Slice 10-2 chart drag-pan.
  - Added pointer-captured horizontal panning for zoomed charts with whole-bar clamping at dataset and replay boundaries.
  - Preserved Slice 9 hover/crosshair behavior for short clicks by requiring a 4px drag threshold; grab/grabbing cursors communicate the interaction.
  - Corrected replay playhead rendering for historical panned windows and resume-follow behavior at the cursor boundary.
  - Added pure pan-window tests and real pointer-drag Playwright coverage without canvas pixel assertions.

- [x] UI port — Slice 10-1 chart wheel zoom.
  - Added a cursor-anchored visible-window zoom with explicit visible-bar count and reset-to-fit control.
  - Kept bar replay bounded at its cursor while preserving the selected zoom level; dataset/replay mode changes return to a predictable fit window.
  - Fixed wheel event handling with a non-passive native listener so zooming does not move the surrounding page.
  - Added pure window-math unit coverage and real wheel-input Playwright coverage without relying on canvas pixel assertions.

- [x] UI port — Slice 7-3 strategy library.
  - Added an SQLite-backed saved-strategy picker with refresh and load actions; saving a backtest refreshes and selects the saved row.
  - Added strict persisted-definition validation before restoring params, blocks, or manual code strategies into the editor.
  - Added unit and browser E2E coverage; Playwright accepts an `E2E_PORT` override with strict port binding so unrelated local dev servers cannot be reused accidentally.

- [x] Improve button press feedback and export download status.
  - Added global button hover/active/focus/disabled feedback for the React app.
  - Added explicit JSON/CSV export status messaging (`正在準備...` / `下載完成...`) beside the export buttons.
  - Updated `e2e/export.spec.ts`; `npm run typecheck`, `npm test` 125, `npm run build`, and `npm run e2e` 14 passed.
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
