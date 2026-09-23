# Handoff: P12b-1a trial-ledger registry core

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12b1-trial-ledger-core` (from main `69c1033`, PR #116 spec merged)
PR: [#117](https://github.com/yoyoCadence/AlphaFactorForge/pull/117) (ready for review; merge is maintainer-owned)
Status: implementation and local verification complete; review pending. P12b-1b, P12b-2 and P12 stay open.

## Summary

First implementation slice of the accepted P12b spec (`docs/trial-ledger-v1.md`). P12b-1 was too large
for one session, so it is split into **P12b-1a** (this handoff: the registry core) and **P12b-1b**
(export/import, event union, origin checkpoints, conflict quarantine writes). The new module
`research::trial_ledger` and its own migration `registry_migrations/0001_trial_ledger.sql` implement
opening, registration, receipts, the current admission count, and the only ledger-to-P12a path.
Nothing in the runtime calls it yet.

## Scope and decisions

- **Opening (§2):** creates/canonicalizes the registry directory; refuses a registry inside the workspace,
  the same directory, or a workspace inside the registry; refuses UNC/network paths and paths under the
  `OneDrive*` environment roots; its own `registry_migrations` table (append-only) with newer-schema refusal;
  a random 16-byte `registry_id` written once with migration 0001.
- **Verification at open (§7.1):** recomputes every event id from its payload, checks columns against the
  payload, the contiguous `seq`, the whole chain, and each batch's members. Any mismatch → `ChainBroken`:
  registration still appends, every admission is blocked with `registry_chain_broken`.
- **Registration (§4, §6.1):** pure `prepare_batch` validates kinds and required fields and computes the
  canonical payload, event id and content-bound batch id; the `BEGIN IMMEDIATE` transaction checks
  quarantine, compares every existing key (content first, then batch membership), checks the pinned
  protocol (also on replay), verifies reproductions against the stored original, writes batch → events
  (chain) → protocol pin → receipt, and reads the current `admissionCount` before commit.
- **Outputs (§6.3, §6.4, §11):** `RegistrationReceipt` (renamed audit fields) and `Admission` =
  `Count(AdmissionCount)` with private fields, or `Blocked(family_unknown | family_quarantined |
  registry_chain_broken | no_effective_trials)`. `precision_plan_from_count` maps
  `prior = familyEffective − batchEffective`, `planned = batchEffective`.
- **Decisions the spec left open** are recorded in spec §16: conflict precedence, chain text encoding,
  per-kind required fields, frozen benchmark identity (strategy parameters only; costs excluded, tied to
  contract versions by a test), the volume detection rule, broken-chain behavior, `no_effective_trials`,
  and creating all v1 tables (incl. those P12b-1b fills) in migration 0001.
- The module is `#![allow(dead_code)]` with a rationale, following `market/mod.rs`, until P12b-2/P12d add
  callers; it is added to the host-agnostic source scan.

Files: `alpha-factor-forge/src-tauri/src/research/trial_ledger.rs` (new),
`alpha-factor-forge/src-tauri/registry_migrations/0001_trial_ledger.sql` (new), `research/mod.rs` (+1),
`runtime/boundary_tests.rs` (+1), plus `docs/trial-ledger-v1.md` (status + §16), `tasks.md`, `CHANGELOG.md`,
`docs/autonomous-research-capability-registry.md`.

## Required Action / Decision

1. Review the §16 implementation decisions, especially conflict precedence (`idempotency_conflict` before
   `batch_conflict`) and "broken chain still accepts registrations but blocks every admission".
2. Next slice: P12b-1b (export/import/union/checkpoints; A13–A16, A21, A25, A26, A29), then P12b-2.

## Review Notes

- A27 initially failed because a sequential per-key check reported `batch_conflict` for an unchanged
  sibling key before reaching the changed one; the two-pass check makes A27 and A28 both hold. This is
  recorded rather than silently chosen.
- Mutation checks, each restored afterwards: skipping the chain comparison fails A30; counting benchmarks
  as effective fails A5 and A6; capping the family count at the batch (a receipt-like count) fails A31,
  the read-after-later-batches test and the AlphaBTC mapping.
- A8 runs two ledgers on one registry file from two threads; receipts tile the count exactly (38 trials)
  and the reopened chain is intact.

## Verification

- `cargo test --locked` (CARGO_TARGET_DIR outside OneDrive): **410 passed** (88 lib + 320 bin + 2 service
  smoke); 24 new tests in `research::trial_ledger`.
- `cargo check --locked --all-targets` pass; `cargo clippy --locked --all-targets` only the 5 pre-existing
  warnings; `rustfmt --check` on the new file passes.
- `npm run typecheck` pass, `npm test` **973/973**, `npm run build` pass. Playwright not rerun locally
  (no frontend change).
