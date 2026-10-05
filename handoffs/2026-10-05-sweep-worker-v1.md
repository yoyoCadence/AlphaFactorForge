# Handoff: interactive parameter sweep worker

Date: 2026-10-05
Repo: yoyoCadence/AlphaFactorForge
Branch: `perf/sweep-in-worker`, from checked #158 merge `2c2362bb`
Status: In Progress; existing PERF-001 plan, publish after layout/doc PRs.

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
17.83 kB module-worker chunk. Full E2E and browser interactivity/cancellation
are still pending; no acceptance claim yet. No code under paramSweep/core or
the existing E2E specs was edited.
