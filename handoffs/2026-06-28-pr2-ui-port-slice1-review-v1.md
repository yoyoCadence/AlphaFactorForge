# Handoff: PR #2 UI Port Slice 1 Review

Date: 2026-06-28
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/phase-a-ui-port
PR: #2
Status: Needs one small fix before merge

## Summary

PR #2 is a reasonable incremental UI-port slice. It adds a pure TypeScript params-mode backtest pipeline under `alpha-factor-forge/src/services/` and does not attempt a large UI port in one PR.

The overall direction is good:

- `strategy.ts` defines the params-mode strategy shape and legacy defaults.
- `strategySignals.ts` maps strategy signal ids to boolean entry/exit arrays using canonical `core/indicators`.
- `backtestRunner.ts` bridges legacy UI percent units into `core/backtest`.
- `metricsMapper.ts` centralizes camelCase `Metrics` to snake_case `BacktestSummary`.
- `services.test.ts` adds focused coverage.
- `tasks.md` now records the UI port as a sliced workflow.

## Required Fix

Before merge, please preserve the legacy execution-model clamping in `backtestRunner.ts`.

Legacy behavior in `AlphaFactorForge.dc.html`:

```js
fee = Math.max(0, feePct) / 100
slip = Math.max(0, slipPct) / 100
sizeFrac = Math.min(1, Math.max(0.01, (sizePct || 100) / 100))
```

Current PR code directly divides by 100:

```ts
sizingPct: strat.sizePct / 100
feePct: strat.feePct / 100
slippagePct: strat.slipPct / 100
```

This can change behavior for edge inputs:

- negative `feePct` or `slipPct` becomes a rebate instead of clamping to zero
- `sizePct = 0` no longer follows the legacy fallback/minimum behavior
- values above 100 rely on `core/backtest` clamping size, but the service layer should still document and mirror legacy conversion

Suggested fix:

```ts
const feePct = Math.max(0, strat.feePct || 0) / 100;
const slippagePct = Math.max(0, strat.slipPct || 0) / 100;
const sizingPct = Math.min(1, Math.max(0.01, (strat.sizePct || 100) / 100));
```

Then pass those constants into `BacktestConfig`.

Please add one small unit test covering this conversion behavior.

## Review Notes

The signal semantics look aligned with legacy `makeSig()` for the implemented params-mode signals:

- `maCrossUp` / `maCrossDown`
- `emaCrossUp` / `emaCrossDown`
- `priceAboveSlow` / `priceBelowSlow`
- `rsiOversold` / `rsiOverbought`
- `macdCrossUp` / `macdCrossDown`
- `bbLowerTouch` / `bbUpperTouch`

The RSI signals are correctly cross-based, matching legacy:

```js
rsiOversold: cond({ l: 'rsi', op: 'crossUp', r: String(strat.rsiBuy) })
rsiOverbought: cond({ l: 'rsi', op: 'crossDown', r: String(strat.rsiSell) })
```

Keeping `stochOversold` / `stochOverbought` in the type while throwing a clear runtime error is acceptable for this slice because `core/indicators` does not yet implement STOCH.

Using `core/indicators` as canonical is acceptable even if values are not bit-for-bit identical to the legacy prototype.

## Verification Already Run

GitHub PR checks were green:

- `typecheck`
- `test`
- `build`
- `cargo-check`

Local verification also passed:

```powershell
cd alpha-factor-forge
npm run typecheck
npm run test      # 33/33
npm run build

cd src-tauri
cargo check --locked
```

## Notes For Next Agent

Please keep the PR narrowly scoped. Do not start Slice 2 UI work in the same PR.

After the clamp fix and test are added, rerun:

```powershell
cd alpha-factor-forge
npm run typecheck
npm run test
npm run build

cd src-tauri
cargo check --locked
```

If all pass, this PR is OK to merge from the review perspective.

---

## Resolution (addressed — commit `fc11abf`)

Required fix applied:

- `backtestRunner.ts`: conversion extracted into a pure, exported
  `toExecCostFractions()` mirroring the legacy clamp:
  - `fee  = Math.max(0, feePct||0) / 100`
  - `slip = Math.max(0, slipPct||0) / 100`
  - `size = Math.min(1, Math.max(0.01, (sizePct||100) / 100))`
- `services.test.ts`: +4 unit tests (normal conversion, negative-clamp,
  `sizePct` 0 fallback / >100 cap, 0.01 floor).

Reran (all green): `npm run typecheck`, `npm test` 37/37, `npm run build`,
and PR CI `cargo-check`. Scope unchanged — no Slice 2 UI work in this PR.
