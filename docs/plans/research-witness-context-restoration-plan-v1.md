# Actual witness context, strict IO and explicit restoration

Date: 2026-10-18
Scope: P13c-3b-1b-2b-2, from merged #200 (`3d83d57`).
Authority: [durable witness plan](research-source-witness-storage-plan-v1.md).

1. Hydrate both physical blob tables only after combined bounded preflight.
   Verify original SHA/length/UTF-8/kind/strict codec, cross-table namespace,
   all document scalar/interval projections and unknown origins. Preserve the
   explicit schema-6 compatibility boundary; strict APIs accept no caller facts.
2. Reconstruct every self-contained original witness. In the same transaction,
   verify actual current trial chain/indexes, restrictive evidence and protocol
   rows. Compare ALL original event identities/payloads/batch membership and
   original protocol high-water marks against actual current rows. A matching
   local prefix never accounts for missing foreign-origin trials. Report every
   unincorporated original event origin and the source origin for protocol gaps.
   Original lost quarantines/conflicted origins always fail.
3. Derive pure host observations from original proofs, historical prefixes from
   those original anchors, and complete current alpha/trial/provenance context
   from actual checked rows. Historical prefixes/counts are archival facts,
   never a current workspace-prefix/admission permit. Keep generic protocols
   out of eligible legacy trial facts; the strict graph retains its two-test
   checks. Read-only strict observation refuses gaps lacking actual permanent
   confirmation-unknown or origin-conflict stops.
4. Add strict witnessed read/lookup and monotonic union, keeping original bytes,
   first audits and every restrictive row. Verify incoming representations
   before replay, complete only ordinary union-derived conflicts, check the
   materialized graph/projections and reload before the caller's one commit.
   Missing proofs are never replaced with receipt/current-primary counts.
5. Add explicit witness-only restoration. Accept only original witnesses;
   validate the MATERIALIZED old-plus-witness graph without completing old
   conflicts/events/blobs/indexes. Derive only required permanent uncertainty
   from actual gaps; insert only new witnesses/markers and reload strictly in
   one immediate transaction. Duplicate replay preserves first audit. Every
   late failure rolls back all additions. No nested trial import or deletion.
6. Test actual source snapshots/current growth/reopen, foreign-origin gaps with
   equal local prefixes, same-head upgrades, complete historical counts,
   invalid/unused original proofs, namespace/projection corruption, independent
   connections, replay/capacity, lost restrictive evidence and late rollback.
   Catch deliberate source/current-accounting/restore mutants by assertions.

Binding observer/adoption, archive v5 transport and private final admission
capture remain later PRs. Do not activate runtime execution, change policy,
run calibration/reserved seeds or expose witness data to AI context.
