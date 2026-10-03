# Handoff: P12e-6b recalibration diagnostics — grid run, S2-R3 selected

Date: 2026-10-03
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e6b-diagnostics` (from merged PR #138, `13100ce`)
PR: draft, opened with this slice
Status: The diagnostic grid of
[`confirmation-recalibration-plan-v1`](../docs/plans/confirmation-recalibration-plan-v1.md)
§6 has been run on seed 20261005 and the selection recorded: **S2 with block
rule R3**. This is a selection, not an acceptance — nothing is shown to be
calibrated until P12e-7 passes §7 on seed 20261117, which has not been used.
P12e-FINDING-1 stays open; P13 stays blocked.

## Summary

PR #138 (draft candidates S1/S2) was merged as `13100ce`; the reviewer's
re-review records were committed unchanged as `2593af2`.

1. **Declarations committed before running** (`41d68eb`): the 72
   declarations of §6 (36 size, 36 power), generated from the plan's tables
   by `alpha-factor-forge/scripts/declare-recalibration-diagnostics.ts`, in
   `alpha-factor-forge/fixtures/research/recalibration-plan-v1-diagnostics.json`.
2. **Size runs**: all 36, release runner, 395.6 s wall on 16 threads.
3. **Power runs**: only the 18 that §6 requires (eligible pairs, at and above
   their lowest supported tested length), computed from the recorded size
   reports by the committed rule, then run: 304.9 s wall. The other 18 power
   declarations stay declared and unrun.
4. **Reports and selection** added to the same file by
   `scripts/record-recalibration-diagnostics.ts`; declarations untouched.

Full tables: the plan's "Record — 2026-10-03, P12e-6b".

## Results

Size (rejecting simulations of confirmation 1 / confirmation 2 / family, out
of 4,000; screen 120 / 120 / 240):

- **Ten failed cells**: every pair at 256 bars with φ = 0.3; V1-R3 at 256
  bars with φ = 0 (121 on confirmation 1, one over); V1-R3, V1-R4 and S1-R4
  at 512 bars with φ = 0.3. Confirmation 1 fails in all ten; the family check
  also fails in two (V1-R3 and V1-R4 at 256 bars, φ = 0.3: 253 and 268).
- **Lowest supported tested length**: V1-R3 1024, V1-R4 1024, S1-R3 512,
  S1-R4 1024, S2-R3 512, S2-R4 512. No pair supports 256 bars.

Power at 512 bars (confirmation 1, net-return test, smaller of the two noise
models): S2-R3 3067, S1-R3 3066, S2-R4 3061 → **S2-R3**.

## What this does and does not show

- It shows what the declared rule picks on the declared grid. The three
  power counts differ by 1 and 6 out of 4,000 on the same simulated data,
  and no interval for the differences was computed: it does **not** show
  that S2-R3 is more powerful than S1-R3 or S2-R4.
- The screen is a point estimate on 4,000 simulations. A passing cell is not
  a pass of §7, and S2-R3 failed the screen at 256 bars with φ = 0.3 (127 on
  confirmation 1). Which lengths S2-R3 supports is for §7 alone.
- V1 with R3 at 256 bars, φ = 0.3 — the configuration of the P12e-3
  acceptance run (129 of 2,000, 6.45%) — counted 253 family rejections of
  4,000 (6.325%), failing the screen too, on a different seed and sample
  count. Nothing here changes P12e-FINDING-1.

## Decisions

- No new maintainer decision; every rule applied is the plan's.
- **Prefix cost above the plan's estimate — please confirm in review.** The
  54 prefixes (`4096 / bars` simulations each, declared in P12e-5) take
  16.6 s of debug time on one thread on the development machine, against
  §8's "about ten seconds"; on 16 threads 1.25 s. The test spreads the
  reports over the available cores. Dropping reports from the re-check would
  break §8 ("every committed report"), and the prefix is part of the
  declarations that were run, so neither was changed. Read this PR's
  cargo-check job time; P12e-7 adds six acceptance reports.

## What changed

- Fixture: reports (36 size, 18 power), `requiredPowerRuns`, `pairs` (every
  pair with every cell, failed ones included) and `selection` added; the
  declarations, screen and description are byte-identical to `41d68eb`.
- `scripts/record-recalibration-diagnostics.ts`: adds runner output (refuses
  undeclared ids and power runs the rule does not require), writes the
  runner input for the required power runs, records the selection.
- `src/parity/recalibrationDiagnostics.ts`: the §6 rule (screen from counts,
  lowest supported length, required power runs, selection), shared by the
  script and the Vitest. Test support only.
- Rust test `every_committed_diagnostic_report_reproduces_its_declared_prefix`
  in `noise_simulation_v2/tests.rs`.
- Vitest `src/parity/recalibrationDiagnostics.test.ts`.
- Docs: plan record; status notes on the draft-candidate and engine
  contracts; capability registry; task board.

## Required Action / Decision

1. Review the selection record and the prefix cost above.
2. **P12e-7** (next, after this merges): freeze S2 with R3 as
   `research-confirmation-statistics-v2` (a product contract, from the draft
   S2), declare and commit the six final-acceptance declarations, run them
   once on seed 20261117 with 20,000 simulations per cell, record the
   result and the supported tested configurations, and close or keep
   P12e-FINDING-1. Record this PR's number, merge commit and CI time there.
3. NUMERIC-JSON-001 stays separate and untouched.

## Verification

- The committed reports equal the runner's output (54 / 54); the runner's
  inputs equal the committed declarations; the record script is idempotent.
- `cargo test --locked`: **557 passed (159 library + 396 desktop + 2
  service), 1 ignored** = 556 + the new test, which re-runs all 54 prefixes
  and compares the counts and the echoed fields (library suite 30.8 s locally
  in debug).
- `npm test`: **1060 passed (62 files)**, 8 new: the grid is declared on the
  diagnostic seed only; every size report's status is the screen on its
  counts; exactly the required power runs are recorded; the recorded pairs
  and selection are the rule's; the rule at its boundaries (120/121,
  240/241, a pass below a failure, both noise models, power only at and
  above the lowest length, smaller-of-two power, tie order independent of
  input order).
- Mutation checks on the rule, all 13 caught by the rule tests alone:
  exclusive limits (two), only confirmation 1 screened, one noise model
  sufficing, a failure not stopping the scan, power only at or only above
  the lowest length, larger-of-two power, smaller power preferred, longer
  length preferred, tie order reversed, tie order taken from input order,
  power read from confirmation 2.
- Fixture tamper checks, all 12 caught by the Rust test, the Vitest or both:
  a checkpoint count or family count, a declaration's block length or seed,
  the echoed statistic, a power report removed or an unrequired one added, a
  size status flipped, a size count moved across the screen, the selection,
  a failed cell dropped, a power count. A count after the checkpoint that
  changes no status, required run or selection is caught by neither — only
  re-running the documented command checks it.
- `npm run typecheck`, `npm run build`, `cargo check --locked --all-targets`:
  pass. Clippy: the five existing warnings only. rustfmt on the changed Rust
  file; `git diff --check` pass.
- No UI change; Playwright not rerun locally.
