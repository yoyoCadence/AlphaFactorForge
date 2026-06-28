# Handoff: PR #7 UI Port Slice 4a Review

Date: 2026-06-28
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/phase-a-ui-port-slice4a
PR: #7
Status: Needs one small fix before merge

## Summary

PR #7 implements Slice 4a: blocks/rules strategy mode. The architecture is aligned with the current decision: Slice 4a should only ship safe blocks mode, while code mode stays split into Slice 4b and must use a safe whitelist-AST interpreter with no `new Function`, `eval`, or dynamic execution.

The PR is mostly good, but there is one small strategy-semantics bug in right-operand parsing that should be fixed before merge.

## Required Action

Fix `alpha-factor-forge/src/services/strategySignals.ts`.

Current code:

```ts
const operand = (name: string): Operand => {
  if (known.has(name)) return series[name as OperandId];
  const num = Number(name);
  return Number.isFinite(num) ? num : Number.NaN;
};
```

Problem:

- The blocks UI right operand is a free text input.
- `Number('') === 0`, so if the user clears the right-side operand, the rule becomes a comparison against `0`.
- This conflicts with the intended/legacy semantics documented in the PR: unknown non-numeric operands should never be true.
- It can create unexpected signals, especially for rules like `price > ''`, which becomes `price > 0`.

Recommended fix:

```ts
const operand = (name: string): Operand => {
  const key = name.trim();
  if (known.has(key)) return series[key as OperandId];
  if (!key) return Number.NaN;
  const num = Number(key);
  return Number.isFinite(num) ? num : Number.NaN;
};
```

Also add a focused test in `alpha-factor-forge/src/services/services.test.ts`:

- `r: ''` should never fire.
- `r: '   '` should never fire.

## Review Notes

The rest of the PR looks acceptable:

- No `new Function`, `eval`, or dynamic execution found in the Slice 4a implementation.
- Blocks mode uses AND semantics across rule rows.
- Empty rule lists never fire.
- `buildSignals` dispatches by `strat.mode`.
- `strategy_def.type` is set from `strat.mode`, so saved blocks strategies should persist as `blocks`.
- Operand list is intentionally limited to currently supported core indicators; STOCH/ATR/VolMA can stay deferred.

Non-blocking naming note:

- `ParamsStrategy` now also represents `mode: 'blocks'`. This is slightly inaccurate naming, but not worth blocking Slice 4a. It can be cleaned up later if the strategy model grows.

## Verification

Local checks run by reviewer on 2026-06-28:

```text
npm run typecheck       PASS
npm run test            PASS, 50/50
npm run build           PASS
cargo check --locked    PASS
```

GitHub PR state observed by reviewer:

```text
PR #7 mergeStateStatus: CLEAN
typecheck:   SUCCESS
test:        SUCCESS
build:       SUCCESS
cargo-check: SUCCESS
```

`rg -n "new Function|eval\\s*\\(|Function\\s*\\(" alpha-factor-forge/src tasks.md` only found documentation/comments/tests that forbid or mention eval, not an implementation path in Slice 4a.

## Merge Decision

Do not merge yet. Ask the PR author to fix blank/whitespace right-operand parsing and add the small regression test. After that, if CI stays green and the manual `cargo tauri dev` smoke check is acceptable, PR #7 can be merged.

## Manual Smoke Checklist

After the fix, run `cargo tauri dev` or `npm run tauri -- dev` and check:

1. Load sample data.
2. Switch strategy tab to blocks.
3. Add a rule such as `RSI < 30`.
4. Run backtest and confirm metrics update.
5. Switch back to params and confirm params still work.
6. Save result and confirm `strategy_def.type` is `blocks` for a blocks-mode save.

## Resolution (addressed — commit on feat/phase-a-ui-port-slice4a)

Fixed in `strategySignals.ts` `buildBlocksSignals` operand resolver: trim the
operand string; a blank/whitespace operand now returns `NaN` (never compares
true) instead of `Number('') === 0`. Known series ids and numeric constants
still resolve as before (trim also lets `" 70 "` / `" maFast "` resolve).

Added regression tests in `services.test.ts`: `r: ''` and `r: '   '` never
fire. Reran: `npm run typecheck` PASS, `npm test` PASS 51/51, `npm run build`
PASS; PR CI re-run green. Scope unchanged — no code mode (Slice 4b).
