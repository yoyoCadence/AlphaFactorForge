# Handoff: P12e-2 cross-batch alpha allocation (declared schedule per trial family)

Date: 2026-10-01
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e2-alpha-allocation` (from merged PR #133, `3452033`)
PR: opened from this branch as a draft (its number is recorded by the next slice; no follow-up docs commit)
Status: Implemented and verified locally; P12e-3 (noise-data false-positive simulation) is next.

## Summary

P12e-2 adds the pure rule that says how much alpha a trial family's next
confirmation may use: `research-alpha-allocation-v1`
([contract](../docs/research-alpha-allocation-v1.md)). It has no runtime
caller, reserves and stores nothing, and cannot produce a confirmation `PASS`.

P12e-1 was published as PR #133 and merged as `3452033`.

## Decisions

- **Maintainer decisions of 2026-10-01:**
  1. The budget is a **declared schedule** — a total plus the alpha of the
     first, second, third… confirmation. An equal split is one schedule a
     helper generates, floored, never rounded up.
  2. **One budget per trial family (instrument)**, the same scope as the trial
     count, so a new workspace or campaign cannot reset it.
- **Design choices made in this slice — please confirm in review:**
  1. **A contradicted history is an error, not an allocation.** `reserved`
     must equal the schedule's prefix exactly; a different amount, another
     order, or more reservations than scheduled confirmations stops
     allocation. This also means two registries that each reserved
     "confirmation 1" for one family cannot be merged into a usable history;
     v1 fails closed and leaves the resolution rule to P13.
  2. **Nothing is recycled.** A confirmation that rejected nothing, failed or
     was abandoned has spent its share; an equal split's remainder is never
     usable. Conservative on purpose.
  3. **Exhaustion is `NOT_ELIGIBLE` + `alpha_budget_exhausted`**, the P12a /
     campaign-admission vocabulary, rather than an error.
  4. **`scope: "trial-family"` is a literal in the declaration** (like
     `correction: "holm"`), so the frozen document states its own scope and a
     per-campaign budget is not expressible in v1.
  5. **`allocationId`** uses the campaign declaration's construction
     (SHA-256 of version, `0x00`, canonical bytes); schedule order is part of
     it. P13 can use it to refuse a second, different declaration.
  6. **No cap on the number of confirmations** beyond what the total allows
     (every share is at least 1 ppm).

## What changed

- `discovery_core::alpha_allocation` (new, pure, in the library crate):
  `parse_alpha_allocation`, `allocate_confirmation_alpha`,
  `equal_alpha_schedule`, `alpha_allocation_id`; tests in
  `alpha_allocation/tests.rs`.
- Fixture `fixtures/rs-core/research-alpha-allocation-v1.json` (hand-written
  inputs and error messages; expected reports from the reference).
- Independent reference `src/parity/alphaAllocationFixture.ts`, its guarding
  Vitest, generator `scripts/generate-alpha-allocation-fixtures.ts` and
  `npm run fixtures:alpha-allocation`.
- Docs: the new contract; pointers from `research-confirmation-statistics-v1.md`
  and `research-precision-v1.md`; capability registry; task board; a
  Resolution on the P12e-1 handoff.

## Required Action / Decision

1. **P12e-3:** seeded noise-data false-positive simulation of the declared
   protocol, with predeclared size and tolerance.
2. **P13** (contract §9): keep one declaration per family with the trial
   registry and refuse a different one; reserve atomically before running and
   pass every reservation; require the confirmation's `alphaPpm` to equal the
   allocated share and re-run the P12a precheck at that alpha; carry
   declarations and reservations through export/import.
3. **Open for the maintainer (P13 design):**
   - A campaign's `sampling.alphaPpm` is what admission planned with. When it
     differs from the family's allocated share, should confirmation be
     refused, or re-checked at the allocated share?
   - What happens to a family whose merged history contradicts its schedule
     (design choice 1)?
   - Per-family budgets do not bound the error across instruments: with `N`
     instruments the chance of at least one false confirmation somewhere can
     approach `N × total`. If a portfolio-level bound is wanted, it needs its
     own decision.

## Verification

- `cargo test --locked`: **514 passed (121 library + 391 desktop + 2 service)**
  = 504 + 10 new library tests: fixture cases and equal splits reproduced exactly;
  strict parsing with its rejection order; integer-ppm boundaries (`1` /
  `999999` accepted, `0` / `1000000` / float literals refused; a sum equal to
  the total accepted, one ppm over refused); directly constructed
  declarations re-checked; a family spends exactly its schedule and every ppm
  is accounted for once; contradicted histories refused (and the undetectable
  hidden-history case documented); equal split floors over a 1..=60 sweep and
  refuses a zero share; identity pinned to the TypeScript reference and
  sensitive to every number and to order; no `PASS`; the allocated share is
  what P12e-1 judges against and what P12a must be run with.
- `npm test`: **1010 passed (58 files)**, including 17 new tests that the
  committed fixture equals the independent reference, with `allocationId`
  from the TypeScript canonical encoder.
- Mutation checks, each caught: budget bound off by one; equal split rounding
  up; history prefix unchecked; exhausted budget paying the last share again;
  identity ignoring schedule order; history length unchecked; zero share
  accepted; equal split flooring to zero.
- `npm run typecheck`, `npm run build`, `cargo check --locked --all-targets`:
  pass. Clippy: the five existing warnings only. rustfmt on the new Rust
  files; `git diff --check` pass.
- No UI change; Playwright not rerun locally.
