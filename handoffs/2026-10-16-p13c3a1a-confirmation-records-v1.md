# Handoff: P13c-3a-1a pure exposure/event/conflict records

Date: 2026-10-16
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3a1a-confirmation-records
Base: merged design #190 (`f1e0e63602e0694c6ef4203bc4056dbd06deb2bc`)
Status: Implemented and verified locally; delivery PR records final-head CI and merge status.

## Summary

Implements the [single-record codec contract](../docs/research-confirmation-records-v1.md)
under the merged [storage design](../docs/plans/research-confirmation-storage-transfer-plan-v1.md).
Strict immutable records retain the frozen exposure/event/conflict versions
and identity convention. Existing P13c-1 wire shapes/IDs are unchanged; three
read-only getters expose already-checked operation/batch bindings.

Managed exposures require the exact original operation/alpha origin and key,
batch/family/allocation, instrument and full frozen stage footprint. Manual
and legacy-unknown claims have a separate constructor; they cannot become
managed confirmation or known-unseen claims. Events bind their operation and
batch, started workspace and sealed engine, with closed variants/counts/IDs.
Conflict declarations check structure, sorted pair/target limits and canonical
family derivation from instruments. They expose no stop or admission API.

No IO, migration, storage, provider, execution or runtime caller is included.
No reserved-seed/statistical-policy/calibration or legacy float parser change.

## Verification

Independent Node fixture generation uses no product encoder. Sixteen record
identities plus two existing-format alpha reservations are reproduced in
TypeScript and Rust; regeneration is byte-identical. Fifteen Rust regressions
cover both stages, all closed manual/legacy/terminal codes, every supplied
binding, exact geometry, changed operations, malformed original nested JSON,
missing nullable fields, safe integers, market endpoints, 128/129 intervals,
4 MiB/exact-one-over bodies and wrappers, original IDs and static diagnostics.

Local `cargo test --locked`: 723 passed (235 library + 486 desktop + 2 real
service smoke), one existing ignored test. `npm test`: 1215 passed / 76 files;
typecheck and production build pass. All-target check and Clippy pass with
five existing warnings, none in the new module. Assertions caught four
executable mutants: omitted alpha key, exact support, sealed engine and derived-family
target checks. All mutated source was restored byte-for-byte. Targeted
rustfmt, five relative file links and fixture regeneration pass. Final-head CI
is recorded with the delivery PR. User has authorized continuing the
task/PR/CI/merge loop.

## Deliberate partial-reference boundary

These types check single records and supplied references. They do **not**
establish the existence of actual admission/start/report/artifact/claim rows,
complete transition consistency, after-seal containment, conflict relation or
exact complete derived targets. Explicit regressions preserve that distinction.
A `ConflictDeclaration` is never validated restrictive evidence by itself;
no caller may use it to stop research or authorize execution. No current
runtime caller exists, and full graph validation must precede storage authority.

P13c-3a-1b is next after this slice merges: admission/P12a integer proof,
original statistics/binding blobs, full acyclic dependency checks and actual
conflict derivation. P13c-3a-2 storage follows only after both pure slices.
Production remains behind the existing policy/manual-mediation gates.
