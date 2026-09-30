# Handoff: P12e-0 upward-only trial test count

Date: 2026-10-01
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e0-protocol-upgrade` (from merged PR #131, `b3f05c3`)
PR: pending
Status: Implemented locally; P12e-1 (circular block bootstrap / Holm) is next.

## Summary

Before P12e-1 can test two hypotheses per trial, the trial ledger must count
them. Discovery had pinned every family at `testsPerTrial = 1`, and the spec
refused any different protocol. This slice lets a family's test count rise —
never fall — so existing families move to two tests without opening a new,
zero-count family. Spec: [`docs/trial-ledger-v1.md`](../docs/trial-ledger-v1.md) §22.

## Maintainer decisions (2026-10-01)

1. **Two tests per trial**: net return above zero, and per-bar excess over
   buy-and-hold above zero, reported separately (not combined into one
   p-value as AlphaBTC did). The ledger upgrades upward-only.
2. **Circular block bootstrap** with a fixed block length (P12e-1).
3. **Block length is a declared integer**, frozen with a rationale before any
   result is read; no data-driven selection (P12e-1).

## What changed

- Registry migration `0003_family_protocol_upgrades` (append-only).
- `register_batch`: fewer tests than the family's effective count is
  `family_protocol_mismatch`; more appends an upgrade in the same transaction.
  `admissionCount.testsPerTrial` is the effective (maximum) count.
- Export `trial-ledger-export-v3` with `protocolUpgrade` records; import
  unions protocols to the maximum instead of quarantining; v1/v2 refused.
- `fence_admission`: a raised test count is `Grew`; a fallen one is
  `Inconsistent`.
- Runner, legacy backfill and pre-P05 recovery register
  `DISCOVERY_TESTS_PER_TRIAL = 2`.
- Spec revisions (§3, §8.1, §8.2, §10, A17, §19 note) and §22; campaign
  contract note.

## Compatibility

- Registry schema becomes `0003`; older builds refuse it (`registry_schema_newer`).
- A decision made before a family's upgrade is re-evaluated at its next fence
  with the doubled `m`; stored decisions themselves are not rewritten.
- The former acceptance rule "a different protocol quarantines the family"
  (§8.2, old A15 protocol half) and "any different `testsPerTrial` is refused"
  (A17) are superseded; both directions of the old tests were rewritten.

## Required Action / Decision for P12e-1

- Pure Rust `research-confirmation-v1` (name to confirm): circular block
  bootstrap of per-bar returns with a declared block length, sample count and
  seeded, specified PRNG; one-sided `(1 + #extreme)/(B + 1)` p-values for the
  two tests; Holm over the family size `m` from the ledger's admission count;
  authored fixtures computed independently; no confirmation `PASS`.

## Verification

- `cargo test --locked`: **494 passed (101 + 391 + 2)** = 490 + 4 new tests;
  `a17` and the `a15` receipt case rewritten for §22.
- Mutation checks, each caught: allowing a downgrade; dropping raised counts
  on import; a fence that ignores a raised count; the runner still
  registering one test.
- `cargo check --locked --all-targets` pass; clippy shows only the five
  existing warnings; `git diff --check` pass.
- No TypeScript or UI change; npm and Playwright not rerun.
