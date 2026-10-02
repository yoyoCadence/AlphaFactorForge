# Handoff: P12e-6a draft candidate statistics S1 and S2 (no diagnostic run)

Date: 2026-10-02
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e6a-candidate-statistics` (from merged PR #137, `50b20e5`)
PR: opened from this branch as a draft (its number is recorded by the next slice; no follow-up docs commit)
Status: Implemented and verified locally. **No diagnostic or acceptance cell of the recalibration plan was run**; nothing is known yet about either candidate's calibration. P12e-FINDING-1 stays open; P13 stays blocked.

## Summary

The two experimental statistics of
[`confirmation-recalibration-plan-v1`](../docs/plans/confirmation-recalibration-plan-v1.md)
§4 are written out as draft contracts, implemented, and selectable in the
simulation engine:
[`research-confirmation-candidates-draft-v1`](../docs/research-confirmation-candidates-draft-v1.md).

PR #137 (the v2 engine) was merged as `50b20e5`.

**P12e-6 is split.** This is P12e-6a: contracts, implementations, references
and measured cost — everything that can be reviewed before a result exists.
P12e-6b runs the diagnostic grid and records the selection. After the first
diagnostic run a change to a candidate is a plan amendment; before it, it is
an edit of the draft. The plan's content and order are unchanged; the split
is recorded in the plan.

## What the candidates are

Both keep v1's inputs, resampling (the same block draws), exact p-values,
Holm and report, and change only what makes a resample extreme.

- **S1** `research-confirmation-candidate-s1-v1` — block-variance studentized
  bootstrap: `(S* − S)/√v* ≥ S/√B(L)`, `v*` from the blocks that were drawn.
- **S2** `research-confirmation-candidate-s2-v1` — centred bootstrap stretched
  by `c = √max(1, (2·B(2L) − B(L))/B(L))`; requires `(2L)² ≤ n`.

Neither is a product contract. Only the simulation engine can select them.

## Decisions

- No new maintainer decision; the definitions are the plan's.
- **Design choices made in this slice — please confirm in review, before
  P12e-6b runs anything:**
  1. **No square root and no division in a decision.** Both rules are
     evaluated as `a·√p ≥ s·√q` through `(a·a)·p` against `(s·s)·q` with four
     sign cases. It matches the square-root form on a grid of values (tested)
     and keeps the candidates free of functions whose rounding could differ
     between Rust and TypeScript.
  2. **Summation order is fixed**: `B(ℓ)` sums each circular block from zero
     (no rolling update); `v*` uses per-block sums from zero while `S*` is
     accumulated continuously as in v1.
  3. **Degenerate cases follow from the comparison, with no special
     branches.** Consequence to know: under S1 a constant series has every
     resample extreme, so `rawP = 1` and it is never rejected, where v1
     rejects a constant positive series outright.
  4. **S2 falls back to `c = 1`** when `B(L)` is zero or the flat-top
     estimate is not above it; it never shrinks.
  5. **S2 with `c = 1` is not bit-identical to v1** (`S* − S ≥ S` against
     `S* ≥ 2·S`); it is its own contract.
  6. **The engine checks S2's wider block up front** (`(2L)² ≤ bars`).
  7. **v1's scaffolding was extracted, not copied**: `evaluate_with` in
     `confirmation.rs` holds the input checks, p-values and Holm, and v1 calls
     it with its own bootstrap. v1's fixture still reproduces bit for bit.

## Measured cost (release build, timing-only, seed 1, outcomes not printed)

| Bars | V1 | S1 | S2 |
| --- | --- | --- | --- |
| 256 | 0.373 s | 0.599 s | 0.393 s |
| 512 | 0.797 s | 1.069 s | 0.844 s |
| 1024 | 1.491 s | 1.845 s | 1.589 s |

Per 500 simulations. The whole size grid of plan §6 is about five minutes of
CPU. Recorded in the plan.

## What changed

- New `discovery_core::confirmation_candidates` with tests.
- `discovery_core::confirmation`: `evaluate_with` extracted; `Observed`,
  `ordered_sum`, `fail` and two `SplitMix64` methods widened to `pub(super)`.
  No behaviour change.
- `discovery_core::noise_simulation_v2`: `SimulatedStatistic::CandidateS1` /
  `CandidateS2`, the per-statistic block rule, the statistic error message.
- Fixture `fixtures/rs-core/research-confirmation-candidates-draft-v1.json`
  (eight cases and five hand-worked block variances); two more cases in
  `research-noise-simulation-v2.json`.
- References: `src/parity/confirmationCandidatesFixture.ts` with its Vitest
  and `npm run fixtures:confirmation-candidates`; the v1 reference exports
  `candidateStream`, `below`, `orderedSum` and `referenceHolm`; the v2
  simulation reference dispatches on the statistic.
- Docs: the draft contracts; engine contract §2/§3; a record appended to the
  plan; pointers; task board.

## Required Action / Decision

1. **Review the draft contracts now.** They are cheap to change until
   P12e-6b runs.
2. **P12e-6b**: declare the diagnostic grid (seed 20261005, 4,000 simulations,
   {V1, S1, S2} × {R3, R4} × six cells, one checkpoint at `4096 / bars`),
   commit the declarations, run them with the release runner, commit every
   report, and record the selection and every failed pair.
3. **Separate observation, not acted on — a maintainer call.** `serde_json`
   is built without its `float_roundtrip` feature, so it reads a long decimal
   literal only to within one unit in the last place. Seen here: the fixture
   literal `0.0036944444444444438` (a displayed mean) is read by Rust as the
   neighbouring double, while Node reads it exactly. No decision is affected —
   every count, p-value and rejection matched — and short decimals such as
   prices are read exactly. Whether any product path reads long float
   literals where a one-ulp difference matters (stored metrics read back, or
   an identity hashed from re-read floats) was **not** investigated. The
   existing v1 statistics test compares such floats bit for bit and passes
   only because its literals happen to parse exactly.

## Verification

- `cargo test --locked`: **553 passed (155 library + 396 desktop + 2
  service), 1 ignored** = 545 + 7 candidate tests + 1 engine test: the eight
  fixture cases reproduce the TypeScript reference on every decision field
  (extreme counts, p-values, Holm ranks, rejections), with at least ten tests
  strictly between "never" and "always" extreme; `B(ℓ)` equals the
  hand-worked values and the Bartlett lag-window sum, and `2·B(2L) − B(L)`
  the flat-top one; `v*` uses the drawn blocks (the review's example gives 1
  and 2); the comparison matches its square-root form; the documented edge
  cases; S2 counts at least as many extreme resamples as v1 for a positive
  sum; doubling every return moves no decision; refused inputs including
  `(2L)² > n` and a non-finite block variance; both candidates selectable in
  the engine, with the same noise and a different digest.
- `npm test`: **1046 passed (61 files)**, 19 new.
- The three display floats of a test result are compared to one unit in the
  last place in Rust (see Required Action 3) and exactly in the Vitest.
- Mutation checks, all twelve caught by failing tests: `B(ℓ)` not centred on
  the block mean; divided by `n` only; the sign of the deviation ignored; the
  both-negative case reversed; a non-negative deviation against a negative
  sum not extreme; `v*` ignoring a partial block's length; `v*` divided by
  the block count; S1 using the resampled scale on both sides; S2's flat-top
  replaced by `B(2L)`; S2 allowed to shrink; S2's wide block equal to `L`; S2
  accepting `L² ≤ n`.
- v1 unchanged: its fixture and the two declared acceptance runs reproduce.
- `npm run typecheck`, `npm run build`, `cargo check --locked --all-targets`:
  pass. Clippy: the five existing warnings only. rustfmt on the new Rust
  files; `git diff --check` pass.
- No UI change; Playwright not rerun locally.
