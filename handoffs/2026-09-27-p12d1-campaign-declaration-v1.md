# Handoff: P12d-1 frozen campaign declaration

Date: 2026-09-27
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12d1-campaign-declaration` (from merged PR #123, `f747131`)
Status: Implementation and local verification complete; P12d-2 admission remains open.

## Summary

The empty Next queue leaves the existing P12 In Progress ordering authoritative.
P12a–c are merged, so this task completes the first independently reviewable
P12d slice: a pure, strict `research-campaign-declaration-v1` constructor with
an immutable document and content identity. The user authorized selection,
implementation, validation, commit, push and a Chinese ready-for-review PR;
merging remains maintainer-owned.

## Decisions and scope

- Bind the exact instrument set, listing period, data range, snapshot ID and
  versioned dataset hash; reject duplicate instruments even across intervals.
  The 128-instrument limit is a bounded v1 declaration limit, not a statistical
  threshold. Preserve P06's exclusive delisting semantics and canonical IDs.
- Freeze explicit sampling and per-instrument history/fold policies with
  rationale. Reuse P12a/P12c input domains without fabricating admission counts
  or returning an eligibility status. All thresholds are declared; the fixture
  is synthetic and does not establish a product default.
- Reuse the existing canonical binary identity encoding with a versioned
  domain separator. Object/universe order is irrelevant; all declared policy
  and binding changes alter the ID. A frozen value cannot be mutated through
  the API, and restoring a document requires revalidation.
- No runtime caller, persisted campaign row/artifact, command, frontend,
  migration or dependency. Existing runner/Gate/Score behavior is unchanged.
  This is an input declaration, not proof of pre-exposure freezing, trustworthy
  provenance, sufficient samples or complete autonomous protocol configuration.

## Required Action / Decision

1. P12d-2 must resolve exact authoritative snapshots and verify dataset content,
   source status, listing/range/interval and cost semantics. Caller declarations
   must never substitute for these checks. Reject legacy/demo/degraded data.
2. Persist campaign identity and bind strategy/cost/execution inputs before
   enqueue. Connect total history/derived embargo and P12c evidence, unknown
   history blocking, P12b current counts, P12a and the ledger freshness fence.
3. The later complete admission protocol must also freeze trade/cost stress/
   parameter stability rules before results; do not treat this declaration as
   that complete protocol. P12e/P13 own confirmation and one-time reveal.
4. Keep P12d/P12 In Progress; the pure declaration is the only completed item.

## Verification

- `cargo test --locked --quiet`: **451 pass** (101 library, 348 desktop,
  2 service smoke). Seven new tests exercise nested required/unknown fields,
  every contract pin, invalid identities/listing ranges, bounded unique
  universes, policy boundaries, immutable round-trips and identity sensitivity.
- The fixture ID was independently calculated with Node crypto and a separate
  binary encoder, then pinned in the Rust test and contract documentation.
- `cargo check --locked --all-targets`: pass.
- `cargo clippy --locked --all-targets`: pass; only the five pre-existing
  warnings in backtest, score and file_commands.
- `npm test`: **975 pass**. `npm run typecheck`, `npm run build`: pass.
  The first sandboxed test/build failed before execution with esbuild
  `spawn EPERM`; the escalated rerun passed.
- Targeted rustfmt and `git diff --check`: pass.
- Local Playwright, packaged desktop UI and a live campaign were not rerun.
  Remote CI is pending publication; the PR checks are the current CI authority.
