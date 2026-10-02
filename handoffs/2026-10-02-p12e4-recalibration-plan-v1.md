# Handoff: P12e-4 confirmation recalibration plan (plan only)

Date: 2026-10-02
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p12e4-recalibration-plan` (from merged PR #135, `1764c48`)
PR: opened from this branch as a draft (its number is recorded by the next slice; no follow-up docs commit)
Status: Plan written; awaiting review. **No code, fixture or test changed and no simulation was run.** P12e-FINDING-1 stays open; P13 stays blocked.

## Summary

[`confirmation-recalibration-plan-v1`](../docs/plans/confirmation-recalibration-plan-v1.md)
fixes, before any implementation or run, how a successor to the P12e-1
confirmation statistic is chosen and accepted. It answers the open finding of
P12e-3 (the declared AR(1) 0.3 run: 129/2000 = 6.45% against a 6% limit),
whose record stays as it is.

PR #135 (P12e-3, the host-test startup wait and its review fixes) was merged
as `1764c48`.

## Decisions

- **Maintainer decisions of 2026-10-02 for this plan:**
  1. **Confidence-bound acceptance.** Each confirmation and the whole family
     pass only when the 95% Wilson upper bound of the observed false-positive
     rate is at most 1.2 × nominal; 20,000 simulations per cell, run once with
     a release build and committed, not in every CI run. Point estimates are
     recorded too.
  2. **Scope:** 256, 512 and 1,024 bars × independent and AR(1) 0.3 noise.
     Fat tails and volatility clustering are a later version.
  3. **Power** is reported and used only to choose between candidates.
- Carried from the PR #135 review: revise the statistic (studentized
  bootstrap and a serial-correlation variance correction); check each
  confirmation and the family; keep the 6.45% failure on record.
- **Design choices made in this plan — please challenge them in review. After
  the first diagnostic run, changing one is an amendment on record.**
  1. **Candidate S1** — studentized circular block bootstrap: a resample is
     extreme when `(S* − S)/√v* ≥ S/√B(L)`, each side scaled by its own
     block-based variance.
  2. **Candidate S2** — v1's centered comparison with the resampled deviation
     stretched by `c`, `c² = max(1, (2·B(2L) − B(L))/B(L))`: the flat-top lag
     window in place of the Bartlett window the block bootstrap reproduces.
     This is my reading of "serial-correlation variance correction"; a
     different correction would be a different candidate.
  3. **V1 is eligible.** If the unchanged statistic is supported from some
     length upward and no new one does better, the outcome is "v1 with a
     minimum length".
  4. **Two block rules for every candidate**, `round(n^(1/3))` and
     `round(n^(1/4))`, written as integers (6/8/10 and 4/5/6), instead of
     assuming the cause the exploratory runs hinted at.
  5. **One family shape:** two confirmations of 2.5% each, one candidate per
     confirmation, no prior trials, 799 samples — the failed run's shape.
  6. **Diagnostics use a point screen** (at most 3.0% per confirmation and
     6.0% for the family, 4,000 simulations); the Wilson bound is kept for
     the final acceptance, where the sample size supports it.
  7. **Selection order:** smallest supported minimum length, then larger
     power, then V1 < S1 < S2 and R3 < R4.
  8. **Seeds:** diagnostics 20261005; final acceptance 20261117, used once.
  9. **Supported range:** the smallest declared length that passes together
     with every larger declared length becomes the new contract's minimum.
  10. **Power scenario:** a shift of three long-run standard errors on the
      strategy's returns, declared as integer millionths per cell.
  11. **Cost is capped by the matrix**, not by wall time; neither phase joins
      the normal suite, which only re-computes a declared prefix (about ten
      seconds of debug time in total).

## What changed

- New: `docs/plans/confirmation-recalibration-plan-v1.md`.
- Pointers from `docs/research-noise-simulation-v1.md` §10,
  `docs/research-confirmation-statistics-v1.md` §10 and the capability
  registry.
- Timing wording corrected in `docs/research-noise-simulation-v1.md` §0 and
  on the task board: the library suite took 8.17 s and 19.17 s in two CI
  runs; these are suite totals that vary with the runner.
- Task board: P12e-4 done; P12e-5, P12e-6 and P12e-7 added.
- The PR #135 re-review records were committed unchanged first (`c20b52f`);
  Resolutions appended to the P12e-3 handoff and the PR #135 review handoff.

## Required Action / Decision

1. **Review the plan**, above all the eleven design choices and the two
   candidate definitions. Nothing has been run, so everything is still free
   to change.
2. After it merges: **P12e-5** (the `research-noise-simulation-v2` engine and
   a measured release-build cost), then **P12e-6** (candidates and
   diagnostics), then **P12e-7** (freeze and the single final acceptance).
3. P13 stays blocked until P12e-7 records a supported range.

## Verification

- Documentation only. `git diff --check` passes; no source, fixture or test
  file is in the diff, so the suites were not rerun (the merged head's CI run
  [37005074144](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37005074144)
  passed all six jobs: 535 Rust, 1 ignored; 1019 Vitest).
- The plan's derived numbers were computed by a script, not by hand: the
  block lengths; the power shifts (108253 / 76547 / 54127 and 154647 /
  109352 / 77324 millionths); and the integer acceptance rule, which admits
  at most 552 of 20,000 simulations for a confirmation (Wilson upper bound
  2.996%, and 3.002% at 553) and 1134 for the family (5.999%, and 6.004% at
  1135). A method exactly at nominal passes one confirmation check with
  probability about 0.991; one whose true rate equals the limit, about 0.025
  (normal approximation).
- Not verified, because nothing is implemented: that S1 or S2 improves the
  calibration, and what a release build costs. The plan says so (§12, §8).
