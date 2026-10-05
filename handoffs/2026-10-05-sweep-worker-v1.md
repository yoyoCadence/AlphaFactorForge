# Handoff: interactive parameter sweep worker

Date: 2026-10-05
Repo: yoyoCadence/AlphaFactorForge
Branch: `perf/sweep-in-worker`, rebased onto checked #160 merge `700b76e1`
Status: Resolved; PR #161 merged after all six final-head CI jobs passed.

## Scope / implementation plan

The maintainer authorized continued bounded tasks and checked PR merges. Use
the existing PERF-001 specification in docs/improvement-backlog.md; do not
create a second performance specification. REF-001 is already merged. The
current sweep still executes up to 256 backtests synchronously after a 20 ms
paint delay. Preserve its existing immutable input context, Holdout range and
generation/owner result fence.

1. Add runSweep/sweepResult to the existing worker protocol, using only job ID
   and structured-clone-safe data. The unchanged runParamSweep engine computes
   the grid; single interactive backtests remain unchanged.
2. Add a Vite module-worker client with an owned worker per invocation. Matching
   results/errors terminate that worker; cancellation terminates and rejects
   once. The next invocation creates a fresh worker, so no idle replacement or
   stale cross-job callback survives. Handle startup/postMessage/runtime errors.
3. Await the worker in SweepSection; remove the fixed paint delay, add the
   specified cancel-sweep button. Cancel an owned job on clear/reset/unmount,
   reject obsolete load completions before spawning, and retain the existing
   context/generation fence before accepting results or errors.
4. Test client cancellation/late and wrong-ID messages/errors and actual worker
   handler parity on a fixed seeded grid with structured-clone boundaries.
   Existing E2E assertions stay unchanged; full frontend checks and browser
   interactivity/cancel acceptance precede final-head six-green CI/merge.

No engine, DB, identity, dependency, discovery runner or confirmation change.
The root AGENTS.md and maintainer's autonomous merge authorization govern the
flow over the historical agent protocol's stop-for-review/maintainer-merge
steps. No separate independent-agent review is claimed.

## Initial implementation evidence

Worker/client/UI wiring is implemented. Focused 69 tests pass, including eight
owned-client error/cancel/late-message cases and three real handler tests with
structured-clone input/output boundaries (not a real browser thread). The
fixed seed-42, 500-bar 16×16 grid matches the synchronous engine exactly and the
declared 0–399 Holdout prefix is unchanged by a tenfold unseen suffix. Full
Vitest passes 1098 tests/67 files; typecheck/build pass. Vite emits the actual
17.83 kB module-worker chunk. No code under paramSweep/core or the existing
E2E specs was edited.

## Acceptance evidence (2026-10-05)

All 83 existing E2E tests pass unchanged in 1.2 minutes. The registered Chrome
control connection repeatedly disappeared, so the Playwright skill's separate
`aff-sweep-acceptance` browser session exercised the actual Vite module worker
through normal UI actions, with 20,000 synthetic hourly candles and 256 combos
(fastMA 5–20, slowMA 20–35, step 1, net). This is automated browser interaction,
not a manual operator or native campaign acceptance claim.

- A physical replay slider drag changed cursor 12041 → 7957 while aria-busy
  remained true. Cancel returned the UI to idle, removed its worker (close
  event observed), and left zero grid/apply actions. A first probe's fixed
  200 ms observer wait was too short; bounded polling confirmed removal. No
  product timeout or test assertion was weakened.
- Cancel followed immediately by another run created a distinct worker,
  completed exactly 256 cells, and left zero workers. Synthetic best display:
  fastMA=5, slowMA=20, net=57284.8%, 187 trades. These artificial prices only
  exercise behavior and are not market performance evidence.
- Changing strategy RSI period to 15 while a sweep was busy prevented that
  old context's result from appearing at completion. The strategy and chart
  both expose an RSI control, so the probe used the strategy section explicitly.
- Browser errors contained only the existing `/favicon.ico` 404, with no worker
  errors. The named CLI session was closed; Vite port 5203/PID 38832 was checked
  by TCP owner and command line, stopped, and verified absent. Temporary probes
  are archived under ignored `test-results/sweep-worker-acceptance-20261005/`;
  no E2E spec was changed.

Determinism: seed 42, 500 bars, startTime 1704067200000, same 256-cell grid.
The unchanged synchronous engine and actual worker handler (with structured
clone input/output) return strictly identical full DTOs. Both raw best cells:
`{ "x": 5, "y": 22, "metric": 0.17778163629629207, "trades": 11 }`.
This handler comparison is separate from the actual browser-thread acceptance
above; it does not claim a native runtime or all-input proof. Existing prefix
Holdout and stale-context regressions pass. No resampling acceptance seed or
calibration artifact was rerun or regenerated.

Remaining limits: single UI backtests still run synchronously; Discovery stays
in Rust. There is no progress percentage. Each cancelled worker is replaced
only when the next run starts. Native campaign/operator, numeric versioning
and other DB command groups remain their own tasks. Publish only after rebase
verification and all six checks for the submitted head; append merge evidence
in the next handoff update.

## Resolution (2026-10-05)

[PR #161](https://github.com/yoyoCadence/AlphaFactorForge/pull/161) merged as
`880b668000be53b3e02da1d3000d7ad6f7331541`. All six jobs in
[run 37302558882](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37302558882)
passed on final head `07db8db38312daf1818e121de502b71ce20e9c48`.
The continuation checked the clean matching worktree, current main, worker
ownership/context/error paths and empty review threads; 69 focused tests and
typecheck pass again. The unchanged test command needed escalation after
the sandbox denied esbuild child startup (`spawn EPERM`). No product change
was required to close the PR. The PR body now records the final CI evidence.
