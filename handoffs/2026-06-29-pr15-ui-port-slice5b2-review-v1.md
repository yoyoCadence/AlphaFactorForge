# Handoff: PR #15 Slice 5b-2 Sweep UI Review

Date: 2026-06-29
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/ui-port-slice5b2-sweep-ui
PR: #15
Status: Needs one UI state fix before merge

## Summary

PR #15 adds the parameter-sweep UI and heatmap for Slice 5b-2. CI and local verification are green, but review found one stale UI state bug: after a sweep result is shown, changing sweep settings does not clear the old heatmap/apply-best action.

## Required Action

Fix stale sweep results in `alpha-factor-forge/src/components/BacktestPanel.tsx`.

Current behavior:

1. Run a parameter sweep.
2. Heatmap and `apply-best` appear.
3. Change any sweep config: X axis, Y axis, 2D toggle, or metric.
4. The old heatmap and `apply-best` button remain visible.
5. Clicking `apply-best` applies the old `sweepResult`, even though the visible controls now describe a different sweep config.

Relevant spots:

- `runSweep()` clears `sweepResult` only when a new run starts.
- The config controls call setters directly:
  - `AxisEditor title="X ..."` uses `onChange={setSweepX}`
  - `sweep-2d` checkbox uses `setSweepUse2d(...)`
  - `AxisEditor title="Y ..."` uses `onChange={setSweepY}`
  - metric select uses `setSweepMetric(...)`
- `sweepResult?.best` still renders `apply-best`.
- `sweepResult` still renders `SweepHeatmap`.

Suggested fix:

Clear stale result/error on every sweep config change:

```ts
setSweepResult(null);
setSweepErr(null);
```

Apply that to X axis, Y axis, 2D toggle, and metric changes. An equivalent fix is also acceptable: store the sweep config with the result and render/apply only when the current config still matches the result config.

Also add an E2E regression:

1. Load sample.
2. Open sweep.
3. Run sweep.
4. Confirm best cell and `apply-best` are visible.
5. Change metric or an axis range.
6. Confirm old best cell / `apply-best` disappears and the user must rerun.

## Review Notes

This is not a backtest-engine issue. The sweep engine and current happy-path UI are covered by tests. The gap is a React state consistency issue in the panel.

The existing E2E in `e2e/sweep.spec.ts` checks the happy path only:

- load sample
- open sweep
- combo count is 16
- run sweep
- best cell appears
- apply best shows confirmation

It does not cover changing config after a completed sweep.

## Verification

GitHub PR metadata:

- PR #15 title: `feat(ui-port): 參數掃描 UI + 熱力圖 (Slice 5b-2)`
- Base: `main`
- Head: `feat/ui-port-slice5b2-sweep-ui`
- Merge state: `CLEAN`
- CI checks: all successful at review time

Local commands run from `alpha-factor-forge/` unless noted:

- `npm run typecheck` - passed
- `npm run test` - passed, 105/105
- `npm run build` - passed
- `cargo check --locked` from `alpha-factor-forge/src-tauri/` - passed
- `cargo test --locked` from `alpha-factor-forge/src-tauri/` - passed, 2/2
- `npm run e2e` - passed, 2/2

Note: the first sandboxed `npm run e2e` attempt failed with `spawn EPERM` because Playwright could not spawn Chromium inside the sandbox. Re-running with external process permission passed.

## Resolution

Resolved on `feat/ui-port-slice5b2-sweep-ui` (after ef46d7e).

- Added `clearSweep()` (`setSweepResult(null)` + `setSweepErr(null)`) and call it on
  every sweep-config edit: X axis, 2-D toggle, Y axis, and metric `onChange`. The
  stale heatmap / 套用最佳 now disappear the moment any control changes, so the
  visible config can never describe a different sweep than the action acts on.
- Added `data-testid="sweep-metric"` to the metric select for the regression.
- New E2E in `e2e/sweep.spec.ts` ("changing sweep config clears the previous
  result"): run -> best cell + apply-best visible -> change metric -> both gone.

Verification: typecheck + `npm test` 105 green; `npx playwright test --workers=1`
3/3 green. Note: a parallel (2-worker) local run flaked with `page.goto` timeouts
(two Chromium instances racing on this Windows box, not a logic issue); serialized
and CI (Linux) runs are green.
