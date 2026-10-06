# Handoff: memoized chart series and frame-coalesced pointer moves

Date: 2026-10-06
Repo: yoyoCadence/AlphaFactorForge
Branch: `perf/chart-compute-memo`, from merged PR #164 (`616f34c`)
PR: opened from this branch
Status: Complete locally; final-head six-green CI gates merge.

## Scope / implementation plan

Continuation under the user's standing instruction to complete tasks, open
PRs and merge checked ones; the user then asked to stop after this task's PR
is merged. PR #164 (DB-ASYNC-001f, closing DB-ASYNC-001) was verified and
merged first; its Resolution is appended to
[its handoff](2026-10-06-discovery-progress-read-async-v1.md).

PERF-CHART-COMPUTE-001 (P2, from the
[PR #76 audit](2026-07-31-pr76-post-merge-audit-v1.md)): every hover, replay,
zoom or pan repaint of `CandleChart` recomputed the full-series SMA×2, EMA,
Bollinger and RSI plus a full time→index map for the trade markers, and every
`pointermove` triggered its own React update and repaint. Task: memoize the
full-series data independently of the repaint, throttle pointer rendering with
`requestAnimationFrame`, and benchmark a large dataset before and after.

1. `src/charts/chartSeries.ts` (pure): `computeChartSeries` (same indicator
   calls over the full close series, `null` when an overlay is off) and
   `tradeMarkers` (same time→index map + `tradeLegs`). `CandleChart` memoizes
   them on the candles, the six primitive periods and the toggles / the trades
   — not on `strat` / `show` object identity; `draw()` only paints.
2. `src/charts/frameThrottle.ts` (pure, injectable frame source): at most one
   run per frame with the latest value; `flush()` runs it synchronously,
   `cancel()` drops it.
3. `CandleChart` pointer handling: `pointermove` calls `preventDefault`
   synchronously when a drag is past its 4 px threshold (as before) and
   schedules the position; the frame applies the existing hover/pan logic
   inside `flushSync`, so the update renders and paints in that frame.
   Pointer up/cancel flushes the last move before ending the gesture; leaving
   the canvas (not dragging) and unmount cancel it. A `hoverRef` mirrors the
   last hover index so a flushed move and the following up/leave never compare
   against a stale render's value.

No change to the indicator code, scale/paint helpers, visible output, testids,
the native chart-window bridge (`PERF-CHART-BRIDGE-001` stays separate),
dependencies or any backend.

## Verification

- New unit tests: `chartSeries.test.ts` (full-series equality with the
  indicator functions, switched-off overlays, marker legs incl. off-grid
  times and empty/undefined trades) and `frameThrottle.test.ts` (latest value
  once per frame, synchronous flush, cancel, reuse after cancel).
- `npm run typecheck`, **1105 Vitest / 69 files** (1098 + 7) and
  `npm run build` pass.
- Full E2E, `--workers=1` on its own port 5212: 82 passed; the first spec
  (`campaign.spec.ts`) hit the known cold dev-server `page.goto` timeout and
  then passed 5/5 on rerun, so all **83** pass. No E2E spec was edited.

### Benchmark (ignored `alpha-factor-forge/test-results/chart-compute-benchmark-20261006/`)

`bench.mjs` loads 20,000 synthetic hourly bars in `?mock=1` through the paste
JSON import, enables MA/EMA/BB/RSI/volume/trades, runs a backtest so trade
markers exist, and measures main-thread work with CDP `Performance.getMetrics`
(headless Chromium, 1400×1000). Google Fonts requests are aborted in every run
so both sides use the same fallback font. One dedicated Vite server
(127.0.0.1:5211, PID 40596) served both code states; it was stopped by PID
afterwards and the port verified free.

- *Paced*: 120 hover moves across the canvas, each followed by two animation
  frames; per-move delta. *Burst*: one `mouse.move` with 600 steps, as fast as
  CDP delivers. Five rounds each; medians reported. A warm-up run preceded
  each measured run.

| Run | Paced task ms/move | Paced script ms/move | Burst task ms | Burst script ms | Burst wall ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| before (main `616f34c` frontend), warm-up | 16.31 | 10.58 | 7,436 | 5,233 | 15,063 |
| before, measured | 18.30 | 10.78 | 8,467 | 5,513 | 16,588 |
| after, warm-up | 13.54 | 3.19 | 4,396 | 959 | 10,200 |
| after, measured | 6.14 | 1.14 | 2,926 | 589 | 10,032 |

Script time per hover move falls about 10× (10.8 → 1.1 ms) and burst script
time about 9× (5.5 s → 0.59 s). The task/wall figures include CDP and frame
waiting overhead and vary between runs (see the two "after" rows); they are
evidence for this machine only, not a portable performance claim. The probe's
bar-info readout stayed visible, the window stayed at 500 bars and no page
error was recorded in any run.

Not claimed: native-window (Tauri WebView2) measurements or the chart-window
bridge; manual operator acceptance.

## Next recommended step

Per the user's instruction, work stops after this PR merges. For the next
session: `PERF-CHART-BRIDGE-001` (P2) is the natural follow-up; the P3 tooling
items (`DB-MIGRATION-DIAGNOSTIC-001`, `CI-RUSTFMT-001`, `TOOLCHAIN-001`,
`SEC-RUST-001`) remain open.
