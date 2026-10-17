# Checked confirmation storage and original-origin lookup

P13c-3a-2b completes the [schema-6 storage contract](plans/research-confirmation-storage-transfer-plan-v1.md)
on merged #193. It reuses the [complete pure graph](research-confirmation-graph-v1.md)
and [immutable schema/provenance guards](research-confirmation-schema-v6.md).
No persisted field/version, runtime command, provider, bootstrap, production
policy or admission/recovery permit changes.

## Snapshot and supplied observations

Read all confirmation sections in one deferred transaction. Before fetching
JSON/BLOB values, check joint row/byte/interval/unknown aggregates and every
individual original byte length/type. Preserve statistics/binding UTF-8 bytes,
verify original SHA/kind/length, then validate the materialized typed graph.
Recompute every non-audit column and the entire interval projection from the
checked documents: wrong/missing/extra/substituted rows are invalid.

The caller supplies the pure graph's typed `GraphContext`, including complete
original registered-source metadata and independent historical prefix/count
observations, as in the merged pure contract. Check the actual local trial
chain and require current trial/alpha restrictive identities and alpha unknown
provenance to match independently checked registry observations.

Historical counts/prefixes are checked against the supplied independent
original-source observations, never replaced with the receiver's current
larger counts, primary branch or historical audit receipts. A receiver may
retain a conflicting source branch as restrictive evidence rather than primary
trial rows; requiring those historical rows to be the current primary branch
would discard independently valid contradictory confirmation history. Source
metadata/artifact authenticity and availability remain the host's responsibility; storage checks
do not authenticate publishers or establish source completeness by themselves.

Unknown/conflicted valid history remains readable and restrictive. A snapshot
is archival consistency against those observations, not a freshness guarantee,
unseen-data proof or live transaction capability.

## Monotonic append and replay

The standalone evidence writer owns one immediate transaction; its internal
helper can join the final fence/complete-import caller's existing transaction.
Validate the existing materialized graph before composing a union, preserving
all existing records/blobs/unknown origins. A missing existing restrictive row
is corruption rather than an opportunity to repair it silently.

Check incoming section uniqueness, bounds and every supplied original record,
even for IDs already stored. An overlay verifies incoming representations
against real dependencies before deduplication can hide malformed duplicate
JSON or an altered document. Equal semantic record bodies keep first-observed
stored text/audit time; equal blob SHA requires identical kind and raw bytes.
Different bodies under one ID fail. Unknown-origin union is sorted/permanent.

Complete the proposed pure graph with required deterministic conflict witnesses,
apply deduplicated joint capacity before the first mutation, then write only
new rows with local first-observed audit time. Derive exact interval rows once
from their immutable exposure. Re-read/revalidate all stored content and
projections inside that transaction before returning to the final commit.
All late failures roll back the entire standalone transaction; the internal
helper's caller retains responsibility for its final commit/rollback.

## Original-origin lookup

Lookup uses the original 32-digit registry/key, never a receiver identity.
Validate the complete snapshot in the same read transaction, then use the
schema's origin/key index to return the sorted actual operation IDs alongside
that checked history. Multiple contradictory operations are retained. A lookup
does not permit re-execution, refund, seal recovery or artifact disclosure.

## Verification

Use real registered trial batches/alpha shares and a full Validation/selection/
Test chain grounded in actual registry prefixes. Test both independent
connections and reopen; semantic duplicate replay, malformed duplicates,
original blob changes, all column/interval corruption with equal row counts,
real contradictions/manual barriers, legacy markers, exact joint capacities
and failures after staged document/index/conflict writes. Retain the existing
SQL trigger and pure contract regressions. Final-head six-job CI and merge
precede binding v5 work.

Implementation: `trial_ledger_confirmation/storage.rs` owns bounded snapshots,
context matching and lookup; `projection.rs` owns exact scalar/interval
derivation; `write.rs` owns checked overlay/replay/union and staged append.
The 20 storage regressions use actual registered trial counts/shares, independent
connections/reopen, contract-defined corruption columns and all SQL backstops
for a real 10,000-record valid union. Full local verification: 782 Rust (one
ignored), 1234 Vitest / 77 files, typecheck/build/all-target check and Clippy
with only the five existing warnings.
