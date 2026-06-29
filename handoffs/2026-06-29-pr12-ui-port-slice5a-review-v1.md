# Handoff: PR #12 UI Port Slice 5a Review

Date: 2026-06-29
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/phase-a-ui-port-slice5a
PR: #12
Status: Needs one small UI state fix before merge

## Summary

PR #12 implements Slice 5a: holdout / out-of-sample comparison in `BacktestPanel`.
The core split approach is sound: run the full-period result, then optionally run
in-sample `[0, split - 1]` and out-of-sample `[split, n - 1]` over the same candle
array so indicators keep their full history while trading is restricted by
`from` / `to`.

One UI state bug should be fixed before merge because it can leave stale holdout
columns visible after the user disables holdout.

## Required Action

Fix `alpha-factor-forge/src/components/BacktestPanel.tsx`.

Current behavior:

- `holdoutResult` is cleared at the start of `run()`.
- When holdout is enabled and the run completes, `holdoutResult` is set.
- The metrics table decides whether to show three columns solely from
  `holdoutResult`.
- The checkbox `onChange` only updates `holdout`; it does not clear
  `holdoutResult`.

Relevant code:

```ts
const metricCols = result
  ? holdoutResult
    ? [
        { label: '全期', metrics: result.metrics },
        { label: '樣本內', metrics: holdoutResult.inSample.metrics },
        { label: '樣本外', metrics: holdoutResult.outSample.metrics },
      ]
    : [{ label: '', metrics: result.metrics }]
  : [];
```

```tsx
<input type="checkbox" checked={holdout} onChange={(e) => setHoldout(e.target.checked)} />
```

Bug:

1. Enable Holdout.
2. Run backtest.
3. Metrics table shows `全期 / 樣本內 / 樣本外`.
4. Disable Holdout.
5. The table can still show the old three-column holdout result because
   `holdoutResult` remains populated.

This conflicts with the manual acceptance item: disabling holdout should return
the result table to the normal single-column view.

## Suggested Fix

Either clear stale holdout results when disabling holdout:

```tsx
<input
  type="checkbox"
  checked={holdout}
  onChange={(e) => {
    const checked = e.target.checked;
    setHoldout(checked);
    if (!checked) setHoldoutResult(null);
  }}
/>
```

Or gate the table columns by both states:

```ts
const showHoldoutCols = holdout && holdoutResult;
const metricCols = result
  ? showHoldoutCols
    ? [
        { label: '全期', metrics: result.metrics },
        { label: '樣本內', metrics: holdoutResult.inSample.metrics },
        { label: '樣本外', metrics: holdoutResult.outSample.metrics },
      ]
    : [{ label: '', metrics: result.metrics }]
  : [];
```

Best result: do both. Also clear `holdoutResult` when `holdoutPct` changes so the
screen does not show stale metrics for an old split percentage.

## Review Notes

Everything else reviewed looks acceptable:

- PR #12 is `CLEAN` and CI is green.
- Changed files are limited to `BacktestPanel.tsx` and `tasks.md`.
- Holdout split range appears correct:
  - in-sample: `[0, split - 1]`
  - out-of-sample: `[split, n - 1]`
- Reusing the same candles with `from` / `to` preserves full indicator history.
- Save still persists only the full-period result, which matches the PR scope.
- `tasks.md` correctly moves Slice 5a to done and Slice 5b to current.

## Verification

Local checks run by reviewer on 2026-06-29:

```text
npm run typecheck       PASS
npm run test            PASS, 87/87
npm run build           PASS
cargo check --locked    PASS
cargo test --locked     PASS, 2/2
```

GitHub PR state observed by reviewer:

```text
PR #12 mergeStateStatus: CLEAN
typecheck:   SUCCESS
test:        SUCCESS
build:       SUCCESS
cargo-check: SUCCESS
```

## Manual Smoke Checklist

After the fix, run `cargo tauri dev` or `npm run tauri -- dev` and check:

1. Load sample data.
2. Enable `Holdout 樣本外驗證`.
3. Run backtest.
4. Confirm the metrics table shows `全期 / 樣本內 / 樣本外`.
5. Change the holdout percentage and confirm stale split results are cleared or
   recomputed only after running again.
6. Disable holdout and confirm the metrics table returns to the single-column
   full-period view.
7. Run again with holdout disabled and confirm the single-column view remains.

## Merge Decision

Do not merge yet. Ask the PR author to clear/gate stale holdout results as above.
After that, if CI remains green and the manual smoke checklist passes, PR #12 can
be merged.

## Resolution (addressed — commit on feat/phase-a-ui-port-slice5a)

Did all three guards in `BacktestPanel.tsx`:

- Checkbox `onChange` clears `holdoutResult` when holdout is disabled.
- `holdoutPct` `onChange` clears `holdoutResult` (stale split no longer matches the new %).
- `metricCols` gates the three columns on `holdout && holdoutResult` (so even a
  lingering `holdoutResult` can't show 3 columns while the toggle is off); the
  table header now shows only when `metricCols.length > 1`.

Net: disabling holdout (or changing the %) returns the table to the single
full-period column until the next run. Reran: `npm run typecheck` PASS,
`npm test` 87/87, `npm run build` PASS; PR CI re-run green.
