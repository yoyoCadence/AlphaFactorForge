# Handoff: P12e-1 confirmation statistics (circular block bootstrap + Holm)

Date: 2026-10-01
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e1-confirmation-statistics` (from merged PR #132, `cd244ef`)
PR: pending
Status: Implemented locally; P12e-2 (cross-batch alpha allocation) is next.

## Summary

P12e-1 adds the pure statistical calculation a confirmation will use:
`research-confirmation-statistics-v1`
([contract](../docs/research-confirmation-statistics-v1.md)). It has no
runtime caller and cannot produce a confirmation `PASS`.

## Decisions

- Maintainer decisions of 2026-10-01 (recorded in the P12e-0 handoff): two
  tests per trial reported separately; circular block bootstrap; a declared,
  frozen block length.
- **Design choices made in this slice — please confirm in review:**
  1. **`blockLength² ≤ n`.** A block as long as the series only rotates it, so
     every resample has the observed sum and any positive mean gets the
     smallest possible p-value. The floor of `√n` blocks is the usual upper
     end of block-length growth, not a tuned threshold; a stricter or looser
     rule is a maintainer call.
  2. **SplitMix64** with one stream per candidate keyed by candidate index.
     The existing `mulberry32` has a 2^32 period, too short for independent
     per-candidate streams of bootstrap size.
  3. **Holm uses the whole family's `m`**, not the batch size — the same
     quantity P12a plans with. Ties break by `(extremeCount, candidateIndex,
     test)`.
  4. **Centered one-sided test**: a sample is extreme when `sum* ≥ 2·sum`.
  5. The benchmark is buy-and-hold **supplied by the caller**; this module
     does not derive it.

## What changed

- `discovery_core::confirmation` (new, pure, in the library crate) with its
  tests in `confirmation/tests.rs`.
- Fixture `fixtures/rs-core/research-confirmation-statistics-v1.json`
  (hand-written inputs; derived values from the reference).
- Independent reference `src/parity/confirmationStatisticsFixture.ts`, its
  guarding Vitest, generator `scripts/generate-confirmation-statistics-fixtures.ts`
  and `npm run fixtures:confirmation-statistics`.
- Docs: the new contract; pointers from `research-precision-v1.md` and the
  capability registry; task board.

## Required Action / Decision

1. **P12e-2:** the pre-declared cross-batch alpha allocation that supplies
   `alphaPpm` (per-confirmation allocation, total-budget bound, exhaustion and
   rounding rejection).
2. **P12e-3:** seeded noise-data false-positive simulation of this exact
   protocol, with predeclared size and tolerance — the empirical check on
   design choice 1 and on the bootstrap's size.
3. **P13:** build per-bar net and buy-and-hold return series from the frozen
   confirmation segment; pass `familyTests` from the synchronized ledger
   fence; decide `PASS` there. Nothing here checks where its inputs came from.

## Verification

- `cargo test --locked`: **504 passed (111 library + 391 desktop + 2 service)**
  = 494 + 10 new library tests: published SplitMix64 vectors; unbiased index
  draws; five fixture cases reproduced bit for bit (including `f64` means);
  strict declaration parsing with its rejection order and boundaries; refused
  inputs (block-length boundary 8/9/10 bars at `L = 3`, non-finite values,
  overflowing sums, duplicates, `familyTests` matrix); batch-order and
  batch-composition independence; Holm over the whole family, monotone in
  family size and equal to P12a's `bestAdjustedP` in its best case; exact
  alpha boundary (10/200 at 50,000 vs 49,999 ppm); no `PASS` in the report.
- `npm test`: **993 passed (57 files)**, including 7 new tests that the
  committed fixture equals the independent reference. The TypeScript reference
  reproduced an earlier standalone Node implementation byte for byte.
- Mutation checks, each caught: uncentered statistic; Holm over the batch
  only; no running maximum; one stream for all candidates; no block-length
  guard.
- `npm run typecheck`, `npm run build`, `cargo check --locked --all-targets`:
  pass. Clippy: the five existing warnings only. rustfmt on the new Rust
  files; `git diff --check` pass.
- No UI change; Playwright not rerun locally.
