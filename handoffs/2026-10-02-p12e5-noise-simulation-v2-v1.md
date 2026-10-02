# Handoff: P12e-5 noise simulation engine v2 (no new statistic, no plan run)

Date: 2026-10-02
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e5-noise-simulation-v2` (from merged PR #136, `556a397`)
PR: opened from this branch as a draft (its number is recorded by the next slice; no follow-up docs commit)
Status: Implemented and verified locally. **No candidate statistic exists yet and no diagnostic or acceptance cell of the recalibration plan was run.** P12e-FINDING-1 stays open; P13 stays blocked.

## Summary

P12e-5 is the first slice of
[`confirmation-recalibration-plan-v1`](../docs/plans/confirmation-recalibration-plan-v1.md):
the engine the plan's diagnostics and final acceptance will run on,
`research-noise-simulation-v2`
([contract](../docs/research-noise-simulation-v2.md)), with a release-build
runner and a measured cost. v1 and its two declared acceptance runs are
untouched.

PR #136 (the plan and its review fix) was merged as `556a397`.

## What the engine adds to v1

- **Selectable statistic.** Declared by contract name; only the baseline
  `research-confirmation-statistics-v1` exists. Any other name is refused
  until P12e-6 adds S1 and S2.
- **Checks on every confirmation and on the family**, each against its own
  nominal rate, with limit `nominal × limitMultiplierPpm / 1e6`:
  `point-screen` (plan §6) or `wilson-upper-bound` (plan §7, in 128-bit
  integers). `WITHIN_LIMITS` requires every check; a family within its limit
  does not excuse a confirmation above its share.
- **Shifted scenario** for power: an integer shift in millionths added to the
  strategy's returns only; a rule is refused there.
- **Checkpoints**: running counts at declared prefixes. A run cut at a
  checkpoint reproduces it exactly, which is how the normal suite will
  re-check long committed reports.
- **Wilson interval** reported for every check in whole ppm, rounded
  outwards, in exact integers.
- **Runner**: `cargo run --release --locked --example noise_simulation_v2 --
  [--timing-only] <runs.json>`; parallel across runs; a Cargo example, so no
  product build includes it.

## Decisions

- No new maintainer decision was needed; this slice implements the merged
  plan.
- **Design choices made in this slice — please confirm in review:**
  1. **`z = 49/25` is fixed in the contract**, not declarable (plan §7: the
     convention is fixed).
  2. **A limit must be a whole number of ppm below 100%**, otherwise the
     declaration is refused rather than rounded (`16666 × 1.2` is refused).
     The plan's limits (30000, 60000) are whole.
  3. **Neutral count names**: `rejectingSimulations`, not "false positives",
     because the same field is a detection count in the shifted scenario.
  4. **Extreme-count totals in every count block**, so a checkpoint is a
     meaningful digest even when no simulation rejects.
  5. **Declared prefix** for the plan's reports: one checkpoint at
     `4096 / bars` simulations (16 / 8 / 4), recorded in the plan.
  6. **Bounds tighter than v1**: `bootstrapSamples ≤ 1,000,000`, at most 16
     checkpoints, `effectMillionths ≤ 1e9`, multiplier in `[1.0, 10.0]`.
  7. **The runner is an example**, with a `--timing-only` mode that prints no
     outcome, so cost can be measured without looking at results.
  8. **v1 helpers widened to `pub(super)`** (noise generator, streams,
     domain checks) so v2 reuses them; `stream` and `noiseSeries` exported
     from the v1 TypeScript reference. No behaviour change.

## Measured cost (release build, baseline statistic only)

Timing-only run, seed 1, AR(1) 0.3, the plan's family shape, block rule R3,
500 simulations per cell; outcomes not printed or stored:

| Bars | 500 simulations | 4,000 (diagnostic cell) | 20,000 (acceptance cell) |
| --- | --- | --- | --- |
| 256 | 0.366 s | 2.9 s | 14.6 s |
| 512 | 0.797 s | 6.4 s | 31.9 s |
| 1024 | 1.496 s | 12.0 s | 59.8 s |

So for V1 one candidate/rule pair's size grid is about 43 s of CPU and the
whole final acceptance about 3.5 minutes, before parallelism. The first
release build took 5 min 37 s; a debug build was 5 to 13 times slower.
**S1 and S2 are unmeasured** — they do not exist; P12e-6 measures them before
running their grids. The numbers are appended to the plan as a record.

## What changed

- `discovery_core::noise_simulation_v2` (new, pure) with tests;
  `examples/noise_simulation_v2.rs`.
- `discovery_core::noise_simulation`: visibility only.
- Fixture `fixtures/rs-core/research-noise-simulation-v2.json` (five small
  cases, none a plan cell); reference
  `src/parity/noiseSimulationV2Fixture.ts`, guarding Vitest, generator
  `scripts/generate-noise-simulation-v2-fixtures.ts`,
  `npm run fixtures:noise-simulation-v2`.
- Docs: the new contract; a record appended to the plan; pointers from the
  v1 contract and the capability registry; task board.

## Required Action / Decision

1. **Review** the eight design choices above.
2. **P12e-6**: draft contracts, implementations and references for S1 and S2
   (settling zero or negative variance estimates, partial blocks, signs,
   summation order and comparison arithmetic before any run); measure their
   release cost; run the diagnostic grid of plan §6 on seed 20261005 with the
   declared checkpoint; record the selection and every failed pair.
3. **P12e-7** after that. P13 stays blocked.

## Verification

- `cargo test --locked`: **545 passed (147 library + 396 desktop + 2
  service), 1 ignored** = 535 + 10 new library tests: the five fixture cases
  reproduce the TypeScript reference exactly; the null scenario counts what
  v1 counts on every v1 fixture case; a run cut at a checkpoint reproduces
  it; the Wilson rule gives the plan's 552 / 1134 limits, agrees with the
  reported bound for every count at seven sizes and with the floating-point
  formula to one ppm, and treats an exactly equal bound (4902 of 10,000 at
  50%) as within; each rule's exact boundary (120 / 121 of 4,000 for the
  screen, 98 / 99 for the bound); a confirmation above its share fails a run
  whose family is within its limit; the shifted scenario; strict parsing and
  rejection order; domains and whole-ppm limits; direct construction
  re-checked; no `PASS`.
- `npm test`: **1027 passed (60 files)**, 8 new: fixture equals reference;
  the reference's bisection Wilson bounds match the plan, the PR #135
  review's intervals and the floating-point formula.
- The runner's reports for the fixture declarations are deep-equal to the
  fixture in both a debug and a release build.
- Mutation checks: 13 caught by failing tests (`>` for `>=` in the Wilson
  rule; one-sided `z`; bound rounded down; `<` for `<=` in the screen; Wilson
  replaced by the screen; status from the family only; shift on the
  benchmark; checkpoint one simulation late; fractional limits rounded; a
  rule allowed under a shift; extreme totals overwritten; a checkpoint equal
  to the simulation count; excess-only rejections dropped). **One mutant is
  equivalent and cannot be caught:** adding the shift in the null scenario
  too, because the shift is then `0.0` and the generator never produces a
  negative zero.
- A hand-written expectation in the first test draft was wrong (76,127 for
  the upper bound of 129/2000; the engine's 76,123 is confirmed by bisection
  and by the floating-point formula). The expectations now come from those
  two independent computations.
- `npm run typecheck`, `npm run build`, `cargo check --locked --all-targets`:
  pass. Clippy: the five existing warnings only. rustfmt on the new Rust
  files; `git diff --check` pass.
- No UI change; Playwright not rerun locally.
