# Additive source-witness schema and provenance foundation

Date: 2026-10-17
Scope: P13c-3b-1b-2b-1, from merged #199 (`7dde2ef`).
Parent: [durable witness plan](research-source-witness-storage-plan-v1.md).

Split the parent's storage phase into a reviewable migration/provenance PR and
the following actual-context/strict-IO/restoration PR. This first slice does not
expose witness writes or activate runtime binding/transport readers.

1. Append registry migration `0007_confirmation_source_witness`. Create only
   the planned eleventh table with exact SHA, original 1-8 MiB BLOB/byte length
   and first audit timestamp. Keep the old stats/binding table and birth meaning.
   Protect updates/deletes and REPLACE changes (including audit); enforce the
   cross-table logical SHA namespace in both insertion directions.
2. Replace only the eight old joint row/byte capacity triggers and add the
   witness trigger, counting all seven document tables and both blob tables.
   Keep 10,000 rows/64 MiB, individual limits, 128,000 intervals and 10,000
   unknown origins. Old and new insertion paths must enforce the same budget.
3. Before recording migration 0007, run bounded original legacy inventory/blob
   preflight and exact scalar/interval projection checks. Decode original
   binding bytes with strict v5/SHA/kind/length checks. For pre-witness managed
   history, append permanent uncertainty for local, actual operation/exposure
   and original-binding registry origins. Existing markers remain immutable;
   empty/manual-only history needs no new managed assertion. Never derive
   historical counts, proof authority or known-empty state from receipts.
4. If needed, add narrowly scoped pure original-text operation/exposure
   provenance getters that verify closed shape/domains/reproduced identity
   and return only record/origin IDs. They do not validate full historical
   graph references or create executable records. Migration does not pretend
   missing original source proofs have become a complete checked graph.
5. Apply DDL, trigger changes, bounded provenance additions and migration receipt
   in the same immediate transaction. Capacity, original-byte/projection
   corruption or injected late failure rolls everything back. Preserve old
   bytes/audits, foreign keys and last-good workspace data.
6. Extend legacy export omission guards to all eleven tables, including orphan
   witness rows. Until 2b-2 supplies complete current context, old experimental
   graph reads/writes refuse any witness row before hydration. No partial read,
   nested trial import or newly exposed write API. Strict pure APIs stay dormant.
7. Verify exact/one-over domains/capacities, every old/new insertion path,
   immutable/namespace guards with recursive triggers off, bounded managed
   provenance and rollback, fresh/manual-only behavior, independent reopen and
   a real schema-6 binary's refusal of schema 7. Preserve shipped v3/v4 projections.

Then 2b-2 loads both physical blob tables, reconstructs each original witness,
computes COMPLETE actual current event/protocol accounting (including foreign
origins), invokes strict graph/projection validation and implements explicit
materialized witness-only restoration/final reload. No caller facts/receipts
may fill missing original proofs; lost restrictive witnesses still fail.
