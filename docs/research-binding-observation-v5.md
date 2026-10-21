# Transactional whole-graph workspace binding v5

Date: 2026-10-21
Scope: P13c-3b-1b-3, following strict witnessed storage in #201.

The workspace observer now activates `trial-ledger-binding-v5`. The shared
[pure codec](research-binding-snapshot-v5.md) still checks only format. Actual
authority comes from the registry transaction described here; a binding grants
no execution, reveal, qualification or recovery permission.

## Observation and coverage

`TrialLedger::check_binding` uses one deferred SQLite transaction for the
existing verified event-prefix/replacement checks, all restrictive trial
evidence, checked alpha identities/provenance, and the complete
[strict witnessed graph](research-witnessed-confirmation-storage-v1.md).
Original witnesses independently reconstruct historical context; current
incorporation and all original bytes, SHA identities, scalar and interval
projections are checked before any confirmation identity is returned.

All nine sets are exact original identities: batches, selections, operations,
exposures, admissions, events, blobs, conflicts and unknown origins. The blobs
set includes original bindings, statistics and source-witness SHA IDs across
both physical tables. Counts, a matching last event, caller facts or raw SQL
IDs cannot replace the complete checked graph.

After the prefix is proven, every saved trial/alpha/provenance restriction
and each corresponding confirmation set must remain covered. Valid growth
is accepted; valid loss or equally sized substitution reports
`registry_evidence_rolled_back`. Invalid or incomplete graph/proof reports
`registry_state_invalid`. Prefix failure retains its existing blocked outcome.
Neither outcome updates the workspace binding.

Valid conflicted/manual/unknown histories remain readable and adoptable for
exploration. Their permanent stops remain represented; adoption is not admission.
Ordinary checks/adoption never append witnesses, complete conflicts, repair
projections or add uncertainty markers. Experimental managed schema-6 history
without original proof stays incomplete even if an unknown marker exists;
only the explicit verified proof-restoration path can make it readable.

## Workspace format and atomic adoption

Readers preserve exact v1–v4 shapes and their original 4 MiB bound.
Confirmation evidence is `None` (unknown) for every old format, never an empty
observation. New v5 input is bounded to 8 MiB including original whitespace,
then passed directly to the shared strict original-text codec. Duplicate fields
and dynamic family-map keys, missing/null fields, unordered/duplicate sets,
unsafe integers and over-capacity evidence are refused before normalization.
SQL type/byte-length preflight prevents an oversized setting from being hydrated.

The writer requires all observed evidence, including confirmation, and emits
only canonical v5 within 8 MiB. Existing history-blob text is never rewritten.
Adoption verifies the actual complete registry before entering the existing
owner-checked workspace write transaction. A stale epoch, refused graph,
lost evidence or late workspace failure retains the exact last-good JSON and
audit timestamp. Verified legacy adoption may write v5 with permanent unknowns;
it cannot silently substitute known-empty history for an incomplete graph.

Old schema-7 binaries reject v5 workspace bindings. The registry schema stays
at 7; #200 already demonstrated actual schema-6 binary refusal of schema 7.
The current slice additionally exercises the real preserved #201 service
against a newly adopted v5 workspace, followed by successful new-build reopen.

## Remaining work

Complete archive v5 transfer/atomic union is P13c-3b-2. Existing legacy exports
still refuse omission of any confirmation history. Private final admission
capture, authentic metadata/files and actual owner/current-count/share fences
remain P13c-4. Statistical policy and AI Train-context boundaries are unchanged;
no calibration or reserved seed is part of this slice.

[Implementation plan](plans/research-binding-observation-plan-v1.md),
[parent authority](plans/research-source-witness-storage-plan-v1.md).
