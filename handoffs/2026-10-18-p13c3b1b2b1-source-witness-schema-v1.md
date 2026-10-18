# Handoff: additive source-witness schema and legacy provenance

Date: 2026-10-18
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b1b2b1-source-witness-schema
Status: locally verified; final-head CI/review/merge delivery pending

## Summary

P13c-3b-1b-2b-1 starts from merged #199
(`7dde2ef51d0af6cf2683340fc84976756688fde5`). Registry migration 0007 adds
the eleventh append-only witness table, shared logical namespace/capacity
guards and bounded permanent legacy uncertainty in one transaction. This
preserves original rows and refuses partial legacy reads until actual
source/current-history context and strict IO/restoration arrive in 2b-2.

## Review notes

- Keep old migrations, two-kind blob bytes/FKs and schema-6 birth meaning.
  New witnesses have exact original-byte length, lowercase digest and first
  audit fields; changed REPLACE, including audit-only replacement with recursive
  triggers disabled, is refused. Both cross-table namespace directions are guarded.
- Every old/new insertion path shares the original 10,000-row/64 MiB budget.
  Individual document/blob, interval and permanent unknown-origin caps remain.
  The checked-in migration has a reproducible Node generator.
- Bounded original inventory/blob/projection checks precede provenance extraction.
  Strict original operation/exposure getters verify closed fields and reproduced
  identity, returning only record/origin IDs. Original v5 binding bytes provide
  their registry ID. Managed history retains local and all actual original origins
  as permanent uncertainty; empty/manual-only histories gain no managed assertion.
- DDL, capacity triggers, markers and receipt roll back together on corrupt bytes,
  invalid projections, capacity or late failure. No source/count/known-empty
  authority is reconstructed from receipts. Old rows/audit timestamps stay exact.
- All eleven tables participate in public v3/v4 omission guards. Until 2b-2,
  old graph IO refuses even an orphan witness before hydration. There is no new
  witness-write API, observer activation, nested import or downgrade path.

## Verification

Fourteen new regressions cover strict pure provenance (3), schema/domain/all-path
capacity/omission guards (8), and actual managed legacy migration/projection/
capacity/late-receipt rollback (3). The confirmation storage area passes all 47
tests. Six executable mutants were assertion-caught: permanent managed provenance,
shared row budget, shared byte budget, old-reader omission, audit replacement
and cross-table namespace. Each mutation was restored before continuing; the
restored migration generator reproduces exact bytes. Scoped rustfmt, TypeScript
typecheck and seven new local document links pass. The restored full suite
passes **847 Rust** (297 library, 548 desktop, 2 service; one ignored) and
**1242 Vitest / 80 files**. Desktop capacity tests take 167.51 seconds locally;
no timeout was changed. Production build and locked all-target cargo check/
Clippy pass with only the five existing warnings. Exact-head six-job CI,
review/base checks, merge receipt and clean main tree equality remain delivery
gates.

An actual service binary built from schema-6 baseline #199 has SHA-256
`41cd9a7fe5ee078c2df6f901e4ed943e291afb54ea86350aa2fd3ebdc09cf350`.
The new service opened an isolated schema-7 workspace and stopped normally.
That old binary refused the same workspace with exit code 1 naming migration
0007 and created no endpoint. Hidden new-service owners/ports were independently
verified and stopped (PIDs 34612/35980, ports 60629/55377); the task-owned temporary
directory was checked for live owners, verified beneath TEMP and removed.

## Next action

After exact-head CI/review/base checks and merge, implement 2b-2 actual original
source/current event/protocol accounting, combined strict IO and explicit atomic
witness-only restoration. Missing proof stays incomplete even with permanent
uncertainty; restoration cannot heal lost old restrictive witnesses. Runtime
binding observation, complete transfer and final admission capture stay later.
No statistical calibration, reserved seed or AI-context policy was changed.

[Contract](../docs/research-source-witness-schema-v7.md),
[implementation order](../docs/plans/research-source-witness-schema-plan-v1.md),
[parent plan](../docs/plans/research-source-witness-storage-plan-v1.md).
