# Handoff: PR #14 UI Port Slice 5b-1 Review

Date: 2026-06-29
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/ui-port-slice5b1-param-sweep
PR: #14
Status: Needs one config guard before merge

## Summary

PR #14 adds Slice 5b-1: a pure parameter-sweep engine in
`alpha-factor-forge/src/services/paramSweep.ts`, plus unit tests. The direction
is good: no UI, no AI, no eval/dynamic execution; it reuses `runParamsBacktest`,
supports 1-D/2-D sweeps, caps axis/combo counts, and keeps single-combo failures
as null cells instead of failing the whole sweep.

One engine-level guard should be added before merge so the upcoming Slice 5b-2 UI
cannot accidentally create misleading 2-D heatmaps.

## Required Action

Fix `alpha-factor-forge/src/services/paramSweep.ts`.

Current behavior:

```ts
const xKey = sweep.x.key;
const yKey = sweep.y ? sweep.y.key : null;

for (const yv of ys) {
  const row: SweepCell[] = [];
  for (const xv of xs) {
    const variant: ParamsStrategy = { ...strat, [xKey]: xv };
    if (yKey != null && yv != null) variant[yKey] = yv;
```

Bug / risk:

- A 2-D sweep can specify the same parameter for both axes, e.g.
  `x.key = 'fastMA'` and `y.key = 'fastMA'`.
- In that case the `y` assignment overwrites the `x` assignment.
- The returned grid still shows distinct `(x, y)` coordinates, but the actual
  backtest only used the final overwritten value.
- This would make the next heatmap / "apply best" UI misleading.

Recommended fix:

```ts
if (sweep.y && sweep.y.key === sweep.x.key) {
  throw new RangeError('sweep x/y params must be different');
}
```

Add a unit test in `alpha-factor-forge/src/services/paramSweep.test.ts`:

- A config with `x.key === y.key` throws `RangeError`.

Optional but useful:

- If the y-axis value list is empty, throw a `RangeError` instead of returning an
  empty grid. The x-axis already has an empty-axis guard.

## Review Notes

Everything else reviewed looks acceptable:

- PR #14 is `CLEAN` and CI is green.
- Changed files are limited to `paramSweep.ts`, `paramSweep.test.ts`, and
  `tasks.md`.
- `paramSweep.ts` is pure/deterministic and does not touch UI, AI, storage, or
  dynamic execution.
- `SWEEP_PARAM_KEYS` is scoped to currently supported numeric strategy params.
- `buildAxisValues()` handles inclusive ranges, max < min, zero/negative step,
  float drift, and the 64-value cap.
- Total combos are capped at 256.
- `best` requires `trades > 0`.
- `pf` / `calmar` non-finite values are guarded.
- `from` / `to` are passed through for sub-range sweeps.
- Single failing combos become null cells.
- `tasks.md` correctly splits Slice 5b into 5b-1 engine done and 5b-2 UI current.

## Verification

Local checks run by reviewer on 2026-06-29:

```text
npm run typecheck       PASS
npm run test            PASS, 103/103
npm run build           PASS
cargo check --locked    PASS
cargo test --locked     PASS, 2/2
npm run e2e             PASS, 1/1
```

GitHub PR state observed by reviewer:

```text
PR #14 mergeStateStatus: CLEAN
typecheck:   SUCCESS
test:        SUCCESS
build:       SUCCESS
cargo-check: SUCCESS
e2e:         SUCCESS
```

## Merge Decision

Do not merge yet. Ask the PR author to reject duplicate x/y sweep params and add
the regression test. After that, if CI remains green, PR #14 can be merged.

## Resolution

Pending.
