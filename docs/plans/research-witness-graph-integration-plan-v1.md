# Source-witness graph integration implementation plan

Date: 2026-10-17
Scope: P13c-3b-1b-2a-2, from merged #198 (`f15853d`).
Parent: [frozen durable-witness plan](research-source-witness-storage-plan-v1.md).

Add strict pure `complete_witnessed_confirmation_graph` and
`validate_witnessed_confirmation_graph` entry points with explicit immutable
source-witness host observations. Keep the existing schema-6 compatibility
entry points until schema 7/context/restoration can activate strict reads in
the next PR. Compatibility entry points and the schema-6 writer refuse the
new source-witness kind; they cannot persist a proof into the old two-kind table.
No migration, transport, runtime reader/writer or execution switch in this PR.

Each observation ties exact witness bytes/SHA to its self-contained anchor and
generic original trial facts. Its constructor checks domains/consistency, not
the opaque archive; the backend must first run original-source reconstruction.
All supplied witness blobs need matching checked host observations, including
standalone roots. No borrowed same-prefix proof or unused host observation.
Identical observations can coalesce; unequal facts for the same source/anchor
fail. Metadata is immutable for the original key `(workspaceId, runId,
trialBatchId)`; changes to campaign/engine/candidate artifact fields cannot
create an alternative key. Such fields remain claims rather than authenticated
files. Existing registered-source context must agree when keys overlap.

Every Validation source needs a full-source witness. Every admission adds
one deterministic material dependency to a witness matching the full source
and exact original binding trial anchor, alongside its old binding/exposure/
operation dependencies. Independently validate every supplied witness before
choosing the smallest matching SHA, avoiding quadratic audit-proof fan-out.
Test retains its original full Validation source and counts. Generic witness
facts may be noneligible; only original two-test/all-effective facts can enter
the existing unchanged admission/report checks. Legacy eligible host trial
observations, when supplied for the matching key, must agree.

Witnesses have no binding dependency and may be roots. Existing stats/binding
blobs must remain directly used by an admission/event. Binding pre-state edges,
iterative Kahn checks and existing joint row/byte quotas include the third
logical blob kind without widening limits. Strict root validation requires
explicit per-witness-SHA whole-snapshot coverage (including foreign-origin
event/protocol facts), or actual permanent stops for every declared
unincorporated origin. An equal local prefix cannot substitute for that
coverage. The backend, in the next slice, establishes that
accounting from actual rows in its transaction; a witness never ingests trials.
No unknown flag excuses a missing/bad witness or lost restrictive dependencies.

Align current protocol coverage with existing `LedgerEvidence::covers`: only
an actual permanently quarantined family may cover its saved higher HWM. Keep
the actual current map and restricted state. Original witness/binding anchor
matching stays exact. Retain the synthetic legacy graph fixture for explicit
schema-6 compatibility; add an independent strict fixture built on #198's real
four-event source. Cover missing/mismatched proof, roots/orphans, same-head
upgrades, metadata/fact conflicts, inherited Test counts, current stops,
unchanged P12/report rules and assertion-caught mutants before delivery.
