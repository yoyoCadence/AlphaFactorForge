# AlphaFactorForge Tasks

This is the single active task board for the workspace. The former `task.md` legacy feature map has been merged into this file and should not be recreated as a second task board.

Task lifecycle: **Backlog -> Next -> In Progress -> Done**.

## Current Snapshot

- Legacy prototype: `AlphaFactorForge.dc.html` is a feature-rich browser PWA and remains the UI/behavior reference.
- Target app: `alpha-factor-forge/` is the Tauri Desktop Phase A scaffold.
- Baseline verified: `npm test`, `npm run typecheck`, and `npm run build` pass in `alpha-factor-forge/`.
- Native Tauri verified: Rust 1.96 / Cargo / MSVC build tools / Tauri CLI v2 installed; `cargo check` and `cargo tauri dev` both pass; multi-size icons generated.
- Progress (through RS-CORE-005 + IDENTITY-001 + PERSIST-001 + FEAT-002 + code-mode UX polish + REF-004 + BUG-004 + UI-port Slice 8b-2): Phase A backtest pipeline and transactional SQLite persistence (datasets, candles, strategies, summaries, and closed trades); versioned SHA-256-only strategy/dataset content identity with TS/Rust exact fixtures, backend verification, and atomic immutable dataset import; Phase B's pure deterministic Train/Validation/Test + embargo split contract, its Train/Validation segmented backtest runner (Test never executed), usage-aware embargo derivation (max used-signal lookback + explicit holding allowance), the full §6 benchmark suite (Buy & Hold / SMA 50/200 / RSI 14 30-70 / Bollinger 20-2 + seeded Random Entry Monte Carlo with matched holding periods), the §5.1 hard elimination gate (explicit thresholds, fail-closed evidence), corrected Sortino/Calmar + explicit non-finite persistence (METRIC-001), the §5.2 score-v1 ranking breakdown (SCORE-001, per the PR #61 handoff Resolution), immutable validation-record persistence (PERSIST-001, per the PR #64 handoff Resolution: migration 0002, atomic bundle save, shared metrics codec), and committed TypeScript-reference/Rust parity through indicators, backtest/metrics, params signals/split/embargo, deterministic benchmarks, mulberry32/Random Entry, Gate, and params-only Score; chart (canvas + overlays + trade markers + wheel-zoom + drag-pan + hover + bar replay); params/blocks/code strategy modes with invalid-expression Run guard; holdout; parameter sweep + interactive heatmap; report export (Slice 7-2); SQLite strategy library (Slice 7-3); native chart + metrics OS windows (Slice 8b); mutable-field strategy UPSERT semantics (REF-004); plus the 2026-07-07 project audit (`docs/` blueprint) and its backlog work: DOC-001, BUG-001, REF-001→004, TEST-001→002 (browser E2E flows, golden lock, and legacy parity), and Backtest Correctness Phases 1–3 (fee-inclusive accounting, settled metrics, execution-bar/risk fills, legacy `both` reversal, and normalized-fraction validation). Current tests: 339 vitest + 47 Rust + 25 Playwright e2e.
- Security snapshot (2026-07-16): SEC-002 exact-pins Vite 6.4.3 + Vitest 3.2.6; full and production-only audits both report zero. See `docs/security-audit-npm.md`.
- Next discovery slice: RUNNER-CONFIG-001; RUNNER-STORE-001, RUNNER-EXEC-001, and RUNNER-UI-001 remain blocked behind it in order.
- PR CI runs typecheck / test / build / cargo-check (now incl. `cargo test`) — green per PR; `main` requires branches up to date before merge.
- Source-of-truth architecture: `STRATEGY_DISCOVERY.md` v3 and `README.md`.
- Historical context: `HISTORY.md` and `CONVERSATION_HISTORY.md`.

## Next

- [ ] **RUNNER-CONFIG-001** — strict config parsing, params-only candidate enumeration/deduplication, v2 hashes, deterministic seed derivation, and hard caps; pure, with no DB, threads, events, UI, or Test execution.
  - Continue strictly after merge: RUNNER-STORE-001 → RUNNER-EXEC-001 → RUNNER-UI-001. Rust backend computation is final; hidden WebView and Node sidecar are rejected. Cross-run result reuse remains Backlog until a versioned immutable execution cache exists.

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

### Phase B: Discovery And Validation

- [ ] Implement Phase B validation foundations
  - [x] VAL-001: lock the pure deterministic 60/20/20 bar split and equal-embargo range contract.
  - [x] VAL-002: run the split plan through the backtest pipeline (Train + Validation segments only; Test never executed).
  - [x] VAL-003: usage-aware embargo derivation (max used-signal lookback + explicit holding allowance, recordable breakdown).
  - Keep Test hidden from ranking and prompts (holds so far: nothing runs or reads the Test segment).
  - Add walk-forward only after the basic split is stable.

- [ ] Implement Gate + Score and benchmarks
  - Gate: minimum trades, cost-adjusted average trade, rolling-window consistency, max drawdown, concentration limits, benchmark wins.
  - [x] GATE-001: §5.1 hard elimination gate as a pure judgment with explicit thresholds and fail-closed evidence.
  - [x] SCORE-001: §5.2 score-v1 ranking breakdown per the PR #61 handoff Resolution (regime component deferred to REGIME-001; Gate→Score orchestration remains with the runner).
  - Score: OOS CAGR, Sortino, Calmar, regime robustness, profit factor, consistency, complexity/turnover/data-mining penalties.
  - Benchmarks: Buy & Hold, SMA, RSI, Bollinger, Random Entry.
  - [x] BENCH-001: the four deterministic benchmarks (Buy & Hold, SMA 50/200, RSI 14 30/70, Bollinger 20/2) as a pure suite.
  - [x] BENCH-002: seeded Random Entry Monte Carlo with matched trade count + holding periods; returns the distribution + candidate percentile (no pass/fail — Gate owns the threshold).

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

- [x] **RS-CORE-005** — pure Rust Gate + Score structural parity (PR #66 Resolution slice 5).
  - Added `src/parity/gateScoreFixture.ts` + `npm run fixtures:gate-score` + committed `gate-score-parity-v1`: 6 params-only complexity cases covering all 12 supported signal ids, 22 encoded Gate cases, 4 complete Score cases, 16 Gate errors, and 11 Score errors held by the TypeScript reference. The envelope locks exact structure/order/status/evidence/error inventories, explicit non-finite/negative-zero input tags, UTC/invalid-Date boundaries, safe-integer and fractional limits, precise audit details, extreme finite contribution/sigma arithmetic, partial/default config resolution, and finite-weight aggregate overflow; only finite non-integer leaves use the declared tolerance.
  - Added pure Rust `discovery_core/gate.rs` and `score.rs` plus a five-test fixture consumer. Gate emits raw and JSON-safe encoded verdicts; Score is params-only by construction and matches complete `score-v1` breakdowns. TypeScript Gate/Score boundaries now reject duplicate benchmarks, malformed/non-finite evidence, unstable sigma intermediates, negative-zero drift, and non-finite aggregate scores before parity is frozen. No runner, SQLite, threads, events, UI, blocks/code discovery candidates, or hidden Test execution was added.
  - Validation: fixture regeneration SHA-256 is blob-identical; `npm test` (339); `npm run typecheck`; `npm run build`; `cargo check --locked`; `cargo test --locked` (47); targeted `rustfmt --check`; `npm run e2e` (25); and `git diff --check` pass. Clippy was intentionally not run.

- [x] **RS-CORE-004** — pure Rust deterministic-benchmark, mulberry32, and Random Entry parity (PR #66 Resolution slice 4).
  - Added `src/parity/benchmarkFixture.ts` + `npm run fixtures:benchmarks` + committed `benchmark-parity-v1`: 5 exact raw-u32 PRNG cases, 4 deterministic-suite cases, 2 exact planner cases, 6 Random Entry integration cases, and 8 TypeScript-held fail-closed cases. Coverage positively exercises SMA 50/200, full strategy-object structure, trades/equity/metrics and METRIC-001 statuses, unknown/prototype-key interval fallback, clipping/drop behavior, `bars: 0→1`, subranges, default 200 runs, strict percentile ties, and accepted/rejected seed/run boundaries.
  - Added pure Rust `discovery_core/benchmarks.rs`, `prng.rs`, and `random_entry.rs`, with exact benchmark strategy audit records and numeric-leaf tolerance comparisons against the TypeScript reference. Shared TS/Rust parity encoders/comparators now prevent the backtest and benchmark fixture paths from drifting; source provenance includes the shared non-finite codec. No runner, SQLite, threads, events, UI, or hidden Test execution was added.
  - CI portability fix: fixture-only rising paths use iterative multiplication instead of platform-dependent `Math.pow`, preserving exact fixture freshness across Node/OS runtimes without weakening exact inputs or the declared Rust float tolerance.
  - Validation: both fixtures regenerate blob-identically; `npm run typecheck`; `npm run test` (324); `npm run build`; `cargo check --locked`; `cargo test --locked` (42); targeted `rustfmt --check` and `git diff --check` clean; Playwright untouched (no UI/mock surface).

- [x] **RS-CORE-003** — pure Rust params-signals, validation-split, and embargo-derivation parity (PR #66 Resolution slice 3).
  - Added `src/parity/signalsSplitFixture.ts` + `npm run fixtures:signals-split` + committed `signals-split-parity-v1` envelope: 7 signal cases (hand-verified exact MA-cross index + one sample case per family, locking bar-0-never-signals and warm-up-NaN-false), 9 split cases (all five usable-bar residues, zero/non-zero embargo, the JS safe-integer extreme), 8 embargo cases (per-family lookbacks, holding allowance, unused-period usage-awareness, and a success case landing embargoBars exactly on MAX_SAFE_INTEGER), and 11 error cases HELD by the TS reference (generation + freshness execute the real TS functions and require the recorded fragment). Every EXPECTED OUTPUT leaf compares exactly (signal booleans; split/embargo integers — inputs still carry floats); both languages assert the exact success AND error inventories by id.
  - Added pure Rust `discovery_core/signals.rs` (`params-signals-v1`), `split.rs` (`validation-split-v1`), and `embargo.rs` (`embargo-derivation-v1`); per D2 only params mode is ported — blocks/code and the expression interpreter stay TypeScript-only; RUNNER-CONFIG owns non-params mode rejection (recorded), and unsupported ids fail closed with the identical message.
  - PR #70 review fixes: TS `safeLookback` checked arithmetic on every derived lookback and the final embargoBars (IEEE-754 would silently round past MAX_SAFE_INTEGER where i64 would not); Rust rejects raw periods above the safe range BEFORE `usize→i64` conversion (previous `as i64` could wrap) and uses bounded checked adds throughout; boundary cases lock raw-period-above-range, derived-lookback overflow (the reviewer's RSI reproduction), allowance-sum overflow, and the exact-MAX_SAFE success; handoff chronology corrected via append-only record.
  - PR #70 third-round fix: post-hoc `isSafeInteger` cannot catch an intermediate rounding cancelled by later subtraction (`a + b - 1`), so every derived addition now goes through a PRE-checked `safeAdd` and the MACD composite is reassociated as `slowest + (signal − 1)`; the reviewer's blocks `macdHist > 0` reproduction and its code-mode twin are locked as TS unit regressions (blocks/code stay TS-only, outside the Rust fixture).
  - Validation: `npm run typecheck`; `npm test` (321); `npm run build`; `cargo check --locked`; `cargo test --locked` (38); targeted `rustfmt --check` clean; fixture regeneration blob-identical; e2e untouched (no UI/mock surface).

- [x] **RS-CORE-002** — pure Rust backtest engine and metrics parity (PR #66 Resolution slice 2).
  - Added `src/parity/backtestFixture.ts` + `npm run fixtures:backtest` + committed `backtest-parity-v1` envelope: 20 behaviour cases (long/short/both × close/nextOpen, same-bar exit+entry, `both` simultaneous-signal entry-wins, SL/TP incl. gap-through and SL-first ambiguity, fee-budgeted 100% sizing, EOD settlement, from/to sub-range and from==to, the Resolution's empty boundaries — empty candle series and inverted from/to evaluating no bar — zero trades, no-downside +Infinity Sortino/Calmar/PF, and two 180-day sample cases spanning ≥4 calendar months with risk exits) plus 3 fail-closed config error cases. Every case's expected output comes from the real TS engine with generation-time sanity invariants; per the PR #69 review, the error cases are HELD by the TS reference (generation + freshness both run them and require a fragment-matching RangeError) and both languages lock the exact 20-case inventory by id.
  - Added pure Rust `discovery_core/backtest.rs` (`backtest-execution-v1`: fee-inclusive entry budgeting, 1× short collateral, nextOpen pending fills, gap-aware SL-first risk exits, legacy `both` reversal, settled EOD endpoint, identical fail-closed messages) and `discovery_core/metrics.rs` (`metrics-v1`: METRIC-001 downside deviation over all bars, +Infinity Sortino/Calmar/PF, UTC monthly returns). No Tauri/SQLite/thread/event/UI dependency; Test never executes.
  - Rust parity locks trades (times/side/bars exact; prices/pnl within the declared tolerance), full equity curves, all metric leaves incl. exact non-finite statuses and monthly-return keys, and the 3 error messages. Fixture regeneration is blob-identical across runs; the vitest freshness test rebuilds and deep-equals the envelope.
  - Validation: `npm run typecheck`; `npm test` (315, +4); `npm run build`; `cargo check --locked`; `cargo test --locked` (34, +2 parity suites); targeted `rustfmt --check` clean on the new discovery_core files; e2e untouched (no UI/mock surface).

- [x] **RS-CORE-001** — TypeScript-reference parity harness foundation plus pure Rust candle/types and indicator parity.
  - Added an explicit deterministic fixture generator and committed `indicator-parity-v1` envelope with source SHA-256 hashes, exact warm-up positions, structural JSON rules, and the PR #66 default float tolerance. Synthetic sample candles are fixture input only.
  - Added a Tauri/SQLite-free Rust `discovery_core` library matching SMA/EMA/WMA/RSI/MACD/TR/ATR/Bollinger/stddev/extrema/ROC output against the same fixture; runner/DB/thread/event/UI work remains excluded.
  - Fixture SHA-256 is stable across regeneration with canonical `utf8-lf-v1` source hashing; 311 vitest + typecheck + build + rustfmt check + cargo check + 32 Rust tests + 25 Playwright e2e pass.

- [x] **IDENTITY-001** — durable identity prerequisite for discovery reuse.
  - Added SHA-256-only `strategy-v2` and `dataset-content-v2` identities over explicit cross-runtime binary encodings. Dataset identity includes the versioned field mapping, metadata, bounds/count, and every timestamp-sorted OHLCV value; FNV is explicitly `ephemeral-fnv1a` only.
  - Rust now recomputes strategy and dataset identities at the product write boundary. Dataset row + strict candle inserts commit in one transaction; identical re-imports reuse the row, while legacy/forged hashes, contradictory payloads, duplicate timestamps, non-finite values, and injected candle failures leave no partial write.
  - Committed one exact TypeScript/Rust fixture and the durable contract document. `npm run typecheck`, 308 Vitest, production build, `cargo check --locked`, 28 Rust tests, and all 25 Playwright e2e tests pass; no migration was required because versioned values fit the existing hash columns and legacy rows remain read-only/ineligible for discovery reuse.

- [x] PERSIST-001 — immutable validation-record persistence (implements the PR #64 handoff Resolution, revised Option C + DB-invariant addendum).
  - Migration `0002_validation_records.sql`: append-only audit table with FKs, the `gate_passed IN (0,1)` CHECK, and the addendum's D3 invariant CHECK (gate fail ⇒ score NULL, pass ⇒ NOT NULL); registered in the migration runner; fresh-apply, 0001→0002 upgrade-preserving-data, and idempotent re-run are test-locked. `discovery_runs` remains untouched (input-only batch config per the Resolution).
  - TS: shared `services/metricsCodec.ts` (single codec extracted from reportExport; `assertJsonSafe` recursively throws on any unencoded non-finite before serialization) + `services/validationRecord.ts` (`buildBenchmarkRecord` metrics-only snapshot with full Random Entry evidence, `encodeGateVerdict` with `valueStatus`, `AssessmentOutcome` discriminated union making "gate failed but scored" unrepresentable, `buildValidationRecord` self-contained `validation-record-v1` envelope with contract versions, `buildValidationBundle` producing D3-conformant Train/Validation summary rows + record row). Typed `db.saveValidationRecord` / `listValidationRecords` / `getValidationRecord` wrappers with mock-client parity (same pre-validation + append semantics).
  - Rust: `ValidationRecordRow` DTO, pre-transaction `validate_validation_bundle` (segments, identity, gate/score nullability incl. finite score, Train Phase-B nulls, envelope-version match), atomic `save_validation_bundle` (Train + Validation summaries/trades + record in ONE transaction; `write_backtest_result` extracted so the existing single-save keeps its own transaction), and newest-first list/get. Rollback-after-partial-write, append-on-rerun, CHECK/FK enforcement, and exact record JSON read-back are test-locked.
  - Documented in `docs/validation-record-contract.md`. Out of scope per the Resolution: UI wiring, lifecycle transitions, discovery runner, Test persistence.
  - PR #65 review fixes (3 blockers): (1) snapshots now deep-decouple from every caller input (`deepSnapshot`, cloned `monthlyReturns`, rebuilt strategy/plan/score/benchmark) and `toJsonSafeString` re-runs the recursive non-finite guard immediately at EVERY stringify boundary — post-build injected Infinity/NaN fails closed instead of silently persisting null, locked by mutation-isolation + injection regressions; (2) the Rust validator now requires the Validation summary score to be finite AND equal to the record row, and parses the envelope to enforce identity/gatePassed/score equality plus structural breakdown/benchmark snapshot equality with the summary's latest view (key-order differences tolerated), locked by a contradictions rejection test; (3) the mock runs the shared `assertValidBundle` (TS mirror of the Rust validator), returns detached rows from list/get, and gained `mockClient.test.ts` covering illegal bundles + append/read immutability.
  - PR #65 second-review fixes (2 residual P2): the TS validator now uses true JSON structural equality (`jsonStructuralEqual`, key order irrelevant / array order significant — matching `serde_json::Value` semantics, with a key-order-only mock pass regression), and BOTH validators lock a `bench-record-v1` shape (non-null object + exact version + benchmarks array + randomEntry object) so JSON null / `{}` / non-objects / wrong versions can never impersonate the required benchmark evidence, even when summary and envelope agree — rejection regressions on both sides.
  - Validation: `npm run typecheck`; `npm test` (300, +16); `npm run build`; `cargo check --locked`; `cargo test --locked` (21, +8); full `npm run e2e` (25) re-run after each mock-seam change.

- [x] SCORE-001 — §5.2 score-v1 ranking breakdown (pure service, no UI; implements the PR #61 handoff Resolution).
  - Added `alpha-factor-forge/src/services/score.ts`: `scoreCandidate` computes the unclamped weighted sum `Σ(components) − Σ(penalties)` over a Validation-segment result with fixed recorded normalization (D1), returning a fully JSON-safe breakdown — every entry `{ id, raw, rawStatus, normalized, weight, contribution, evidence? }` with `rawStatus ∈ finite | positive_infinity | insufficient | invalid | deferred`, plus `formulaVersion: score-v1`, `segment: validation`, the resolved config, and the lineage-final `testedCombinations` evidence (D5).
  - Components: CAGR/1.0, Sortino/5, Calmar/5, PF (pf−1)/2 (legitimate +Infinity → normalized 1), revised Consistency `1/(1+10σ)` with population σ and a 3-finite-month floor (D3); the regime component is a deferred placeholder whose non-zero weight throws until REGIME-001 (D2). Penalties: canonical cross-mode `complexityUnits` (decision nodes + distinct active indicator params + enabled SL/TP; MA-cross parity locked at 8 units across params/blocks/code), turnover proxy `closedTrades/totalBars@v1` with provenance evidence, and `clamp01(log10(N)/4)` data-mining with `lineage-final-unique` basis (D4).
  - Fail closed: invalid caps/weights/N throw `RangeError`; NaN/−Infinity → `invalid`; <3 months → `insufficient`; score always finite; only `ValidationRunResult.validation` is read (test-locked with throwing Train/Test getters). Documented in `docs/score-contract.md`.
  - 13 acceptance tests mirroring the reviewer checklist, including a hand-computed 2.65 baseline and a JSON round-trip lock.
  - Validation: `npm run typecheck`; `npm test` (284); `npm run build`; `e2e/` greps clean (module unwired, no UI surface).

- [x] METRIC-001 — core metrics correctness mandated by the SCORE-001 Resolution (Next → In Progress → Done in one session).
  - `core/metrics`: downside deviation is now `sqrt(mean(min(0, excess)^2))` over ALL bar returns, so a single downside observation yields a finite positive Sortino instead of 0; Sortino = `+Infinity` only when downside is 0 with positive mean excess; Calmar = `+Infinity` when drawdown is 0 with positive CAGR; every other zero-denominator case stays 0. Sharpe unchanged.
  - Explicit non-finite handling at JSON boundaries: new `services/nonFinite.ts` (`positive_infinity` / `negative_infinity` / `nan`, matching the Resolution's `rawStatus` vocabulary); the JSON report bumps to schema 2 with finite-or-null metric values plus a `metricsNonFinite` status map — never relying on `JSON.stringify`'s silent Infinity→null. The SQLite mapper's explicit finite→null narrowing is documented and test-locked (nullable REAL columns; DB status columns deferred to a future schema slice).
  - No golden updates were needed: goldens lock Sharpe (unchanged), not Sortino/Calmar; the sweep's existing Inf→99 guard already covers the now-reachable infinite Calmar.
  - +7 tests (6 metric locks incl. the four reviewer-mandated cases + report status round-trip; extended the mapper null-narrowing lock).
  - Validation: `npm run typecheck`; `npm test` (271); `npm run build`.

- [x] GATE-001 — §5.1 hard elimination gate (pure judgment, no UI).
  - Added `alpha-factor-forge/src/services/gate.ts`: `evaluateGate` judges a candidate segment result plus the complete §6 benchmark outputs against explicit thresholds and returns a full per-criterion verdict (fixed §5.1 order, observed value + threshold + pass, and the exact config) for reproducible recording. It runs no backtests.
  - §5.1 defaults recorded in `DEFAULT_GATE_CONFIG`: trades ≥ 30, cost-adjusted avg trade > 0 (strict), rolling 30-bar positive-window ratio ≥ 55% (window length is a recorded v1 convention), MaxDD ≤ 35%, UTC-monthly profit contribution ≤ 40%, single-trade contribution ≤ 25%, strictly beat all four deterministic benchmarks (ties lose), Random Entry percentile ≥ 95.
  - Missing evidence fails closed with `value: null` (equity shorter than one window; non-positive total profit makes concentration unverifiable); a missing benchmark or invalid config throws. Documented in `docs/gate-contract.md`; segment-length-adjusted minTrades deferred.
  - 7 focused tests: clean pass + fixed order, each criterion failing independently, strict benchmark ties, fail-closed evidence, threshold overrides, and structural throws.
  - Validation: `npm run typecheck`; `npm test` (264); `npm run build`.

- [x] BENCH-002 — Random Entry Monte Carlo benchmark (pure service, no UI).
  - Added `alpha-factor-forge/src/services/randomEntry.ts`: `runRandomEntryBenchmark` simulates N seeded runs that place the candidate's closed-trade count at random non-overlapping segment positions, holding periods sampled with replacement from the candidate's own `bars` (clamped ≥ 1), executed by the real engine long-only / 100% sizing / close fill with inherited costs; returns the per-run `netReturn` distribution + the candidate's strictly-beaten percentile. One `mulberry32(seed)` stream with a fixed consumption order; runs default 200, capped 1000; `planRandomTrades` exported for direct placement tests.
  - No pass/fail verdict — the §6 "≥ 95th percentile" threshold belongs to the Gate slice. Fail closed on empty series/segment, zero candidate trades, invalid runs/seed. Clip/drop conventions recorded in `docs/benchmark-suite-contract.md` (BENCH-002 section); `mulberry32` is now exported from `sampleData.ts` so the workspace keeps exactly one PRNG.
  - 9 focused tests: hand-verified placement (back-to-back, clip, drop), seeded invariants, determinism + seed sensitivity, default run count, percentile extremes, paired cost effect, fail-closed inputs, and an end-to-end real-candidate ranking. This completes the §6 benchmark set.
  - Validation: `npm run typecheck`; `npm test` (257); `npm run build`.

- [x] BENCH-001 — deterministic benchmark suite (pure service, no UI).
  - Added `alpha-factor-forge/src/services/benchmarks.ts`: `runDeterministicBenchmarks` runs the four `STRATEGY_DISCOVERY.md` §6 deterministic baselines — Buy & Hold (hand-built signals: enter first tested close, hold to the engine's EOD settlement), SMA 50/200 cross, RSI 14 30/70 reversion, Bollinger 20/2 mean reversion — over one candles × segment in a fixed order through the existing pipeline.
  - Fairness conventions recorded in `docs/benchmark-suite-contract.md`: benchmarks inherit the candidate's fee/slippage but always run long-only, 100% sizing, close fill, no SL/TP; segment restriction uses the engine's inclusive `from`/`to`; an empty series fails closed.
  - Random Entry Monte Carlo is deferred to BENCH-002 (needs matched holding-period distribution, run count, seed, and percentile conventions); Gate comparison rules are a later slice — this slice only produces per-benchmark metrics.
  - 8 focused tests: doc-definition lock, hand-calculated Buy & Hold (full range, sub-segment, cost effect), fixed suite order, pipeline parity for the signal benchmarks, determinism, and empty-input failure. Also restored the `## In Progress` board header dropped by the VAL-003 PR.
  - Validation: `npm run typecheck`; `npm test` (248); `npm run build`.

- [x] VAL-003 — usage-aware embargo derivation for the validation split.
  - Added pure `alpha-factor-forge/src/services/embargo.ts`: `deriveEmbargoBars(strat, holdingAllowanceBars)` returns `embargoBars = maxSignalLookbackBars + holdingAllowanceBars` with a recordable breakdown; `maxSignalLookbackBars` counts only the indicators the active mode's entry/exit signals actually reference (params signal ids, blocks rule operands, or the code expressions' interpreter-validated ASTs), using each core indicator's real warm-up (`sma`/`ema`/`bbands` → p, `rsi` → p+1, MACD signal/hist → max(fast, slow) + signal − 1) and adding one bar for `prev`/cross semantics.
  - Fail closed: unsupported `stoch*` signals, invalid code expressions, non-positive used periods, and a negative or non-integer allowance throw; unused configured periods never inflate the embargo or throw.
  - Documented the conventions in `docs/validation-split-contract.md` (new "Embargo derivation (VAL-003)" section). The module stays unwired until a later slice records the breakdown with a validation run; no UI, persistence, ranking, or Rust changes.
  - 14 focused tests across params/blocks/code modes, usage-awareness, fail-closed inputs, and `planValidationSplit` integration sanity.
  - Validation: `npm run typecheck`; `npm test` (240); `npm run build`.

- [x] VAL-002 — run the split plan through the existing backtest pipeline (Train + Validation only).
  - Added pure `alpha-factor-forge/src/services/validationRun.ts`: `runValidationBacktests` plans `planValidationSplit(candles.length, embargoBars)` and backtests the Train and Validation inclusive ranges via the existing `runParamsBacktest` `from`/`to` (full indicator history — the same causal pattern as Holdout; embargo bars are never evaluated).
  - Hidden-Test discipline preserved: the Test segment is planned but never backtested; `ValidationRunResult` exposes only `plan`/`train`/`validation`, and invalid input fails closed in the planner before any backtest runs.
  - 6 focused tests cover exact planned ranges, embargo-bar exclusion, parity with direct `runParamsBacktest` calls over the same ranges, determinism, the deliberately absent `test` field, and fail-closed inputs. No UI, persistence, ranking, or Rust changes; the module stays unused until a later slice wires it into discovery or UI.
  - Validation: `npm run typecheck`; `npm test` (226); `npm run build`.

- [x] VAL-001 — deterministic Train/Validation/Test + embargo split contract.
  - Added a pure `src/core/validation/split.ts` planner with inclusive `from`/`to` ranges compatible with the existing backtest boundary contract; no React, DOM, IO, persistence, or runner dependency.
  - V1 first excludes two equal caller-supplied embargo gaps, then allocates usable bars at fixed 60/20/20 ratios with exact integer largest-remainder math and deterministic Train→Validation→Test ties.
  - Invalid/insufficient input fails closed; 24 focused tests cover exact ranges, all five remainder residues, zero/nonzero embargo, total coverage, invalid inputs, and `Number.MAX_SAFE_INTEGER` precision.
  - Added `docs/validation-split-contract.md`; Test remains unwired from generation, tuning, ranking, prompts, UI, and hidden-Test reveal flows.
  - Validation: `npm run typecheck`; `npm test` (220); `npm run build`. Independent review closed one safe-integer precision blocker and approved the corrected implementation with blocker 0.

- [x] SEC-002 — Upgrade the development toolchain to the minimum patched Vite/Vitest lines.
  - Exact-pinned `vite@6.4.3` and `vitest@3.2.6`; the reviewed lockfile resolves `esbuild@0.25.12`, `@vitest/mocker@3.2.6`, and `vite-node@3.2.4` while retaining the compatible `@vitejs/plugin-react@4.7.0` peer.
  - Used an explicit targeted install only—no `npm audit fix`, forced upgrade, dependency override, or product source change.
  - Updated the audit report, README status in three languages, and CHANGELOG; full and production-only audits both report zero.
  - Validation: `npm run typecheck`; `npm test` (196); `npm run build`; `npm run e2e` (25); dependency tree has no invalid peers; a short-lived Vite smoke bound only to loopback (`::1`).

- [x] SEC-001 — Review npm audit findings without forced breaking upgrades.
  - Added `docs/security-audit-npm.md` with a reproducible lockfile/runtime snapshot, all five affected package-node paths, the five underlying advisories, exposure preconditions, patched floors, and an actionable classification.
  - All findings are dev-only; `npm audit --omit=dev --json` reports zero. Every affected node is `needs-window` because a complete fix crosses the current Vite 5 / Vitest 2 majors.
  - Recommended a separate minimum-patched upgrade (`vite@6.4.3` + `vitest@3.2.6`) and retained the ban on automatic/forced audit fixes. `package.json` and `package-lock.json` are unchanged.
  - Validation: full audit reproduced 3 moderate / 1 high / 1 critical; production-only audit 0; `npm run typecheck` passed; dependency-file diff is empty.

- [x] Reconcile the legacy prototype-issue carry-over before selecting new work.
  - Dataset changes already reload candles, so RSI refreshes from the new series; chart drawing already derives MA periods from current strategy state.
  - Replay and hover use one active-bar index across candle, signal, and position readouts, with unit and browser regressions covering boundaries and alignment.
  - Service Worker work remains intentionally deferred because Tauri is the target architecture; the legacy PWA is a read-only behavior reference.
  - This was a status reconciliation only; no product or test files changed.

- [x] FEAT-002 — Persist trade-detail rows with each saved backtest summary.
  - Added one typed `ClosedTrade` → `TradeRow` mapper and passed the rows through the existing Tauri client boundary.
  - Rust now upserts the summary, deletes prior child rows, and inserts the replacement trades in one transaction; regression tests cover replace, partial-insert rollback, and strategy-delete cascade semantics with foreign keys enabled.
  - Kept migration `0001_init.sql` unchanged: holding bars are not stored, per-trade fee/slippage remain `NULL`, and a trades-reading UI stays deferred to Results Explorer.
  - Validation: `npm run typecheck`; `npm test` (196); `npm run build`; `cargo check --locked`; `cargo test --locked` (13); `npm run e2e` (25).

- [x] Complete the planned browser E2E flows for BacktestPanel.
  - The dev-only `dataClient` mock seam now supports durable Chromium coverage without routing production around typed Tauri clients or storing product data in browser localStorage.
  - [x] Holdout stale-column reset — `e2e/holdout.spec.ts`.
  - [x] Invalid/valid code-mode feedback and Run guard — `e2e/code-validation.spec.ts`.
  - [x] Params/blocks/code tab-state preservation plus a blocks-mode run — `e2e/strategy-modes.spec.ts`.
  - [x] Save-result success banner through the mock persistence path — `e2e/save-message.spec.ts`.
  - Browser E2E validates frontend interaction only; Rust command wiring, SQLite persistence, migrations, AppData paths, and Tauri/WebView behavior remain owned by Rust integration tests and native smoke checks.
  - `npm run typecheck`, 193 vitest, production build, the focused save flow, and all 25 Playwright e2e tests pass. This test-only completion changes no product behavior, so no CHANGELOG or manual checklist is required.

- [x] Browser E2E flow — preserve params/blocks/code tab state across switches.
  - Added one browser/mock regression that verifies all three `aria-pressed` states, mode-exclusive controls, retained params/rule/code edits after repeated unmounts, dataset-aware Run availability, and one successful blocks-mode backtest.
  - `npm run typecheck`, 193 vitest, production build, the focused flow, and all 24 Playwright e2e tests pass. This test-only slice changes no product behavior, so no CHANGELOG or manual checklist is required.

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
