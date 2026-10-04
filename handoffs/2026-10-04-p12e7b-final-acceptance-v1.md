# Handoff: P12e-7b final acceptance

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e7b-final-acceptance-results` (from merged PR #152, `f0459b0`)
Status: Merged as PR #153, `d1bf8e2`, after all six final-head CI jobs passed.

## Scope and authorization

The maintainer requested continued task-board work with autonomous PR creation
and merges after FU-9. P12e-7b is the next unfinished implementation slice in
In Progress. PR #140 (`b9b63a3`, reviewed head `759ecde`) already froze S2-R3
as v2 and the six final declarations. Their current Git blob matches that head.
This is the separately authorized product task after the acceptance follow-up
work order; that work order's prohibition on running final acceptance during
FU-1–FU-9 is preserved as historical scope.

## Plan

1. Build the unchanged release example, then run exactly the six committed
   declarations once on seed 20261117. An exclusive marker and separate raw
   stdout artifact prevent an accidental second full run. Keep the reports
   unedited and record input/output hashes and timings.
2. Add the reports under the declaration fixture's preplanned `reports` field.
   Recompute only the predeclared checkpoints in ordinary Rust tests. Keep
   declarations and all earlier v1/diagnostic reports unchanged.
3. Independently check each confirmation and family count against plan §7,
   including point estimates, Wilson intervals and the lowest supported tested
   length. Update contract status and finding only within that tested set.
4. Verify, publish one PR, wait for its final-head CI, merge and continue the
   task board. P13 remains blocked if no supported tested set exists.

## Limits

No new statistic, cell, threshold, seed or simulation count; no rerun to tune
an outcome. Failures stay on record. This is simulation calibration, not a
claim about arbitrary markets/lengths/distributions or exact nominal alpha.
O1–O4 and P09 authenticated external acceptance remain separate.

## Verification / results

The full matrix ran **once**, with no changed declaration, method, threshold,
sample count or seed. An exclusive pre-launch marker was created before the
hidden release process started at 2026-10-04 14:46:01 UTC. Raw stdout is
committed separately from the fixture that attaches its reports; the recorder
refuses to overwrite existing reports and never runs a simulation.

- Frozen input Git blob: `ae3f1e9d73fe2baac83e5ed64090f21c6a10c1aa`, identical
  to reviewed PR #140 head `759ecde` before the run.
- Raw input SHA-256: `9436c5b8986cc93efac0bc6a12450d58a6142c55228a2b022733d8eb1bc44bd0`.
- Raw stdout SHA-256: `347c857286c41e608a52415ccea07a1cb25f0092f7114491857d34810e05e5e6`.
- Six release cells on six threads: **66.217 s wall**; incremental build 33.59 s.
- Every cell passes both confirmation checks and its family check: **18/18**.
  Counts, point estimates, Wilson intervals and per-cell timings are in the
  [dated plan record](../docs/plans/confirmation-recalibration-plan-v1.md).
- Lowest supported tested length **256**; exact supported set is lengths
  **{256, 512, 1024} × coefficients {0, 0.3}**, `ar1-uniform-sum`, two
  confirmations of one candidate, no prior trials, B=799, frozen S2-R3.
  This does not support any other length, coefficient, distribution or family,
  or prove exact nominal-alpha control. The original v1 129/2000 (6.45%) failure
  stays unchanged. P12e-FINDING-1 closes only within this v2 tested set.

Local verification: **1076 Vitest / 64 files; 579 Rust (170 library + 407
desktop + 2 service), 1 ignored**. Typecheck, build, all-target check, targeted
rustfmt and diff checks pass; clippy has only the five existing warnings.
Library suite 9.31 s, desktop 19.38 s, service 5.60 s. The six new Rust
prefixes replay only 16 / 8 / 4 simulations by length and verify all counts
and extreme-total digests; the full acceptance never runs in tests. Vitest
independently checks all Wilson decisions with BigInt and interval bisection,
declarations, raw report equality, seed isolation and supported-set selection.
No rendered UI changed; local Playwright was not rerun. The PR's CI runs the
existing complete e2e suite.

## Remaining work

P13's statistical-calibration prerequisite is satisfied for the exact tested
set. Its reveal registry, alpha reservation/consumption and recovery are not
implemented here. P12 remains In Progress for native campaign acceptance;
P09 authenticated acceptance and O1–O4 stay separate. Follow-up task work
must not rerun this full acceptance or revise results to fit a new method.

## PR / CI

[PR #153](https://github.com/yoyoCadence/AlphaFactorForge/pull/153), base main,
head `feat/p12e7b-final-acceptance-results`; implementation commit `00b909e`.
The PR records final-head CI, job timing and merge status before merging.
Merge requires all six jobs to succeed on the unchanged final head.

## Resolution — 2026-10-04, CI and merge

Final head `4344519e80571f3015e465081e6b41dd3b578213` passed typecheck, test,
build, cargo-check, e2e and native-smoke in
[CI run 37211472263](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37211472263).
CI Rust: 170 library + 407 desktop + 2 service pass, 1 ignored; suite timings
20.40 / 53.13 / 5.66 s respectively. The PR was marked ready and merged on
that verified head as `d1bf8e2e1cd4677ea8ee32a17ad88e5801301b7c`.
No full acceptance was rerun. Continuation moved to the separately recorded
NUMERIC-JSON-001 audit while operator acceptance remains pending.
