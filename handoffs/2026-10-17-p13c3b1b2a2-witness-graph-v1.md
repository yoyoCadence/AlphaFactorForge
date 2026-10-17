# Handoff: strict source-witness graph integration

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b1b2a2-witness-graph
Status: Implemented and locally verified; final-head CI/merge pending.

## Summary

Extend the pure graph with explicit strict witness entry points, immutable
original-source host observations, per-SHA WHOLE-snapshot current accounting,
exact original admission/source dependencies and checked proof roots. Keep
schema-6 compatibility separate until the next additive storage/context slice.

## Decisions and boundaries

- Each witness blob matches exact bytes/SHA of a host observation. All roots
  are checked, including unused/freeze-only sources. Metadata must agree for
  `(workspaceId, runId, trialBatchId)`; identical original-anchor facts coalesce,
  unequal facts fail in either order. Artifact/campaign/engine fields remain
  supplied claims, without file/publisher authentication.
- Every Validation source has complete proof; Test retains full original
  source/counts. Admissions require exact original trial anchors and unchanged
  two-test/all-effective eligibility/report/P12 checks. Select the smallest
  matching SHA for a material edge after validating ALL supplied witnesses,
  avoiding quadratic dependency fan-out from audit-only duplicate proofs.
- An equal local chain prefix does not account for foreign-origin rows.
  Per-witness host coverage must independently inspect the complete original
  snapshot against actual current rows, or identify every unincorporated
  origin with an actual permanent stop. The next backend slice performs this
  transactional accounting. No nested trial import or receipt-derived repair.
- Existing stats/binding orphan, binding pre-state, Kahn, restrictive-witness
  and joint capacity rules stay strict. Actual current quarantine covers saved
  protocol HWM as established by `LedgerEvidence::covers`, without inflating
  the current map or relaxing original anchor matching/live stops.
- New strict APIs are not wired to runtime/storage. Schema-6 compatibility
  refuses witness blobs; a real-SQLite test verifies refusal preserves all
  existing rows and head. No migration/table/transport/activation in this PR.

## Verification

Fourteen new pure graph regressions pass, including exact shared 10,000-row
capacity. Independent strict Node fixture rebuilds every dependent identity
from the actual four-event source; old legacy fixture regeneration retains
the exact Git object. Three new TypeScript cases cover all identities, three
blob kinds and complete source/Test counts. Source reconstruction now feeds
the immutable host observation constructor, including generic protocol 3.

One initial full Vitest run during Rust compilation/build timed out the
unchanged R3 1..50,000 scan (1241 passed). The isolated full rerun passes
**1242 / 80 files**, retaining the original 5-second timeout. Production build
passes. The restored final full Rust suite passes **833** (294 library, 537
desktop, 2 service; one ignored). All-target locked cargo check/Clippy and scoped
rustfmt pass, retaining only five existing warnings. The large existing desktop
capacity suite took 177.70 s while all-target tooling also ran; the new fourteen
pure graph cases take about 2.3 s. No timeout was changed.

Five executable mutants were assertion-caught: exact protocol anchor, existing
blob liveness, whole-source current coverage, quarantine/HWM parity and immutable
original-source key. Every source was restored before final checks. Strict
fixture regeneration retains SHA-256
`ab69556629fc37da2cce566fd73475f21eadfd0e598fb21f007300b13ef7d58f`
(Git object `29020d6c87eecf1489bac235c0eed6c7f5dca107`); the old fixture is
unchanged. Five new local document links and diff checks pass; no owned
historical-source temp directory remains. Final-head six-job CI/review/base
checks, exact merge receipt and clean main tree equality remain delivery gates.

## Next action

After final-head CI/review/base checks and merge, implement 2b additive schema
7, eleventh witness table, namespace/immutable/joint capacity guards, actual
original-source/current-coverage context and explicit atomic witness-only
restoration. Activate strict graph reads there; preserve original birth metadata,
legacy uncertainty and last-good data. Binding observation/transport/runtime
remain later slices. No bootstrap/calibration/reserved seed was run; no witness
data enters AI context, generation, tuning or ranking.

[Implementation plan](../docs/plans/research-witness-graph-integration-plan-v1.md),
[contract](../docs/research-witnessed-confirmation-graph-v1.md),
[parent design](../docs/plans/research-source-witness-storage-plan-v1.md).
