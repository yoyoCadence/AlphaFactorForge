# Handoff: P13c-3a-2b checked confirmation storage

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3a2b-confirmation-storage
Status: Merged as PR #194; binding codec follow-up in progress

## Summary

Complete P13c-3a-2 storage from merged #193 (`44870ef`): bounded original-byte
graph snapshots, every scalar/interval projection, original-origin lookup and
monotonic immediate-transaction append with derived conflicts and final reload.
[Contract](../docs/research-confirmation-storage-v1.md),
[parent plan](../docs/plans/research-confirmation-storage-transfer-plan-v1.md).

## What Changed / Responsibilities

- `trial_ledger_confirmation/storage.rs`: aggregate/individual type/byte limits
  before hydration, original blob SHA/kind/length, materialized graph validation,
  actual current local trial-chain and trial/alpha identity/provenance/target
  matching, consistent read transactions and indexed original-origin lookup.
- `projection.rs`: exact non-audit columns from checked documents; complete
  interval geometry/key projection. Missing/extra/equal-count substituted
  metadata/index rows cannot be treated as valid empty history.
- `write.rs`: verify current materialized evidence, bound/overlay every incoming
  representation before deduplication, retain first stored text/audit time,
  preserve all existing rows/unknowns, complete deterministic witnesses and
  preflight the deduplicated union. Write only additions; fully reload/check
  inside the transaction before the standalone wrapper commits.
- `append_in`/`load_in` are internal transaction-scoped extension points for
  final admission/complete import. The caller owns its surrounding commit and
  must propagate failures/roll back. Standalone append owns one immediate
  transaction; no helper grants execution/recovery/qualification rights.
- Reuse existing pure limits and trial/alpha observation algorithms; expose
  their existing constants/current-evidence helper without changing arithmetic,
  fields, persisted versions, schema or statistical policy. Unknown-origin
  hydration now also preflights text type/length before reading values.

## Observation Boundary

The typed GraphContext still supplies complete original-source metadata and
independent historical prefix/count observations, as merged in 3a-1b. They are
not today's larger counts or historical audit receipts. The receiver can retain
an independently valid conflicting source branch outside its primary trial
rows; requiring that branch to become primary would drop legitimate evidence.

The backend independently checks current registry restrictions/alpha identities
and the actual local chain. It does not authenticate publishers, prove artifact
presence/source completeness, or turn supplied historical observations into a
fresh live count. Bindings/transport must retain/build the proper host context;
do not substitute a receipt's reported counts for independent source evidence.
P13c-4 still owns actual current counts/source/owner observations inside the
private final transaction. These archive APIs have no runtime caller/permit.

## Verification

**782 Rust** (259 library + 521 desktop + 2 service; one ignored),
**1234 Vitest / 77 files**, typecheck, production build, all-target cargo check
and Clippy pass; only the five existing warnings remain.

Twenty new real-SQLite tests cover a full Validation/selection/Test chain built
with real registered batches/alpha shares and actual original-prefix counts;
independent connections/reopen, same-file concurrent writers/replay, original
origin after receiver import/current family growth, exact stored text/audit
replay, malformed duplicate inputs, current evidence substitution, actual
contradictory witnesses/manual barriers and preserved non-representative stops.

Corruption columns are independently enumerated from the storage contract,
not the production projection descriptor. Every non-audit document column and
interval column, blob kind/SHA/length/original display whitespace, malformed
JSON/audit/unknown types and missing stored witnesses are exercised without
relying on counts alone. Injected late index failure and postwrite damage
rollback all new rows and existing-row damage; caller-owned rollback remains
uncommitted by the helper.

The public writer succeeds at exact 64 MiB and 10,000 records with all production
SQL capacity/append guards active, replays at capacity and rejects proposed
overflow before SQL mutation. Oversized corrupted original text is rejected by
preflight. The focused 20-case suite took 49.49 s locally; aggregate SQL guards
scan existing history on inserts, so future bulk transport/runtime callers must
keep large unions in the backend workflow and avoid a brittle wall-time test.
No bootstrap, calibration/final-seed run or protected execution was performed.

## Next Step

After final-head six-job CI and merge, P13c-3b-1 adds binding v5: one consistent
checked graph observation, exact identity coverage/equal-count substitution,
last-good binding preservation, legacy adoption and real downgrade refusal.
Then 3b-2 completes v5 archive transport before any live admission work.
Production qualification remains behind statistical policy/manual mediation.

## Resolution (2026-10-17 — merged storage delivery)

PR [#194](https://github.com/yoyoCadence/AlphaFactorForge/pull/194) verified head
`8520ecc29da3d57fd299ec9aadb226e5f05a49f2` and merged as
`bb16fcf41e8446da35155226319fe2a37a6c345f`. Run `38990251837` passed all six
jobs: 782 Rust (one ignored), 1234 Vitest and 98 first-pass E2E. Native rendering,
SQLite invokes, metrics windows and event round trip passed; exact runner
browser policy was restored and owned processes/temp directories removed.
No review submissions or unresolved threads remained. Local main was fast-forwarded
and its tree matched the verified head.

The next binding task is split into a pure codec/coverage foundation (3b-1a)
and transactional observation/adoption (3b-1b). The latter must address durable
historical context: a workspace reopening has no caller-supplied GraphContext,
and trial import can retain conflicting claims without their primary source
rows. Copying admission counts into a context is not independent verification.
No runtime v5 writer is activated by the foundation.
