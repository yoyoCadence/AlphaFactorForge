# Complete witnessed v5 atomic import

Date: 2026-10-22. Baseline: merged #205 (`1e6e4e7`).
Scope: P13c-3b-2b-2b; backend-only archival transport.

`TrialLedger::import_witnessed_json_lines` accepts the complete original v5
archive. `import_witnessed_qualification_json_lines` adds a transport provenance
gate. Neither method adopts a workspace binding, reveals data, grants admission,
executes a strategy or recovers an operation. There is no command/provider/UI
caller. P13c-4 and later retain those separate authority barriers.

The [source validator](research-confirmation-source-validation-v5.md) parses and
independently reconstructs the entire original source before the receiver mutex.
Missing source dependencies cannot borrow receiver evidence. The receiver then
holds one IMMEDIATE transaction, strictly observes actual current history and
runs the complete proposal in the [private snapshot](research-ledger-disposable-preflight-v1.md).
Only after private rollback, close and owned cleanup does the identical apply
run on the receiver. Final strict graph/projection/FK checks and equality with
the private expected summary/inventory, trial head/restrictive evidence and
alpha identities/provenance precede the sole commit. Any late error
rolls back trial events, alpha, confirmation, checkpoints and import audits.
Preflight rejection leaves even the receiver's SQLite `total_changes()` intact;
a late rolled-back write can increase that connection counter without changing
persistent evidence.

Trial/alpha union reuses caller-owned v4 merge logic. The import audit records
the SHA-256 of the complete original outer v5 bytes. Internal projection reuse
does not invent legacy alpha/confirmation uncertainty. Existing public v3/v4
omission and qualification guards remain. Actual local non-effective benchmark
history does not have to become a portable benchmark attestation merely to
receive unrelated complete evidence.

Confirmation union retains independent claims, exact original three-kind blobs,
first-observed document bytes/audits, permanent stops and all alternative claims.
It derives only the required monotonic conflicts and coverage-gap markers.
Replay deduplicates identities while appending the normal import audit. Two-hop
and replacement transfer preserve original origin/prefix facts and saved whole
graph binding evidence; importing does not write workspace files.

The qualification-named method checks **source, actual current receiver and
complete proposed state**, before real writes, in this order:

1. No unknown trial family or raw unincorporated original-witness trial coverage:
   `trial_history_unknown`. A conflict already blocking that gap does not make
   its trial provenance known.
2. No alpha unknown origin: `alpha_history_unknown`.
3. No confirmation unknown origin: `confirmation_history_unknown`.

Fully accounted known conflicts remain transferable, restrictive evidence. A
successful provenance gate is not an eligibility decision. Workspace legacy
uncertainty, authenticated owner/count/share/file observations and final
admission are still later checks. The summary exposes trial-history coverage,
alpha and confirmation additions, the resulting confirmation unknown count and
`restricted` (including unknown trial history).

Existing limits apply jointly to the full proposed union: 10,000 document/blob
identities, 64 MiB exact original document/blob bytes, 128,000 interval rows and
10,000 confirmation unknown origins, alongside alpha/trial/source bounds.
Valid primary history is not subjected to a new whole-destination export limit.
The archive wire remains the [canonical v5 format](research-confirmation-export-v5.md).
No schema/dependency/statistical-policy change or new calibration is included.

Implementation order and remaining barriers:
[transport plan](plans/research-confirmation-transport-implementation-v1.md),
[handoff](../handoffs/2026-10-22-p13c3b2b2b-atomic-witnessed-import-v1.md).
