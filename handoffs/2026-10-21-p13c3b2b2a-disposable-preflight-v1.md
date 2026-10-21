# Handoff: disposable destination preflight snapshot

Date: 2026-10-21
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b2b2a-disposable-preflight
Status: Implemented and verified; authoritative delivery receipt is in the containing PR.

## Scope

From merged #204 (`7058106`), implement the tracked 2b-2a internal staging
substrate before 2b-2b actual v5 atomic union. Clone actual local schema/typed rows
directly from a borrowed fixed transaction to a private temporary disk database.
Preserve original rows/blobs/audits/checkpoints/stops/row IDs/autoincrement sequence
and indexes/views/triggers. Check source and proposed FKs; roll back, close and
clean the private proposal before returning a value. No destination write/commit,
public importer, command, new dependency/schema, qualification or execution right.

## Decisions and limits

A disposable Python SQLite probe confirmed same-writer backup is unavailable
while BEGIN IMMEDIATE holds the writer lock; a second reader could back up safely
in the simple probe. The adopted direct row-stream clone uses the borrowed actual
snapshot itself, including uncommitted caller rows, with no second-connection
path race/fingerprint or backup feature. The owned probe root was removed.

The clone avoids imposing v3/v4 export bounds on pre-existing primary destination
history and holds only per-row/schema buffers. SQLite optimizer statistics are
not evidence. The caller still validates complete trial/alpha/confirmation state
and capacities and must use the same apply/final checks privately and for the real
transaction. This substrate alone validates no union or provenance claim.

Private random owned temp paths have canonical cleanup fencing, Unix 0700/0600
and inherited Windows user temp ACL. Cleanup failure refuses return; panic Drop
is a backstop. Process crashes may leave owned private temp data; no startup
scavenger is added. No API keys belong in this registry or its copy.

## Verification

Focused cases cover real complete witnessed history, unchanged all-table/audit
snapshots, sparse row IDs and retained sequence HWM, foreign events/checkpoints/
receipts/stops, WAL/independent writers, copied views/triggers, late private union
failure, missing source/proposed deferred FKs, callback error/panic/early commit,
inactive source/failed clone and Windows locked-file cleanup refusal.

Full restored checks, deliberate mutants and final-head delivery follow in
Resolution/the containing PR. Initial invalid-FK fixture setup was corrected to
explicitly disable FKs before corruption; production guards were not weakened.
No new calibration/reserved-seed experiment; normal committed-report tests remain.

## Next action

Finish full verification, fresh-main rebase/rechecks, draft PR, six exact-head
CI checks and guarded merge. Immediately continue 2b-2b: strict source validation
before receiver lock, complete private preflight via this substrate, shared
trial/alpha/confirmation apply, all original branches/stops, qualification
provenance, two-hop/replacement/replay, capacities and late real rollback, then
exactly one real commit. No destination savepoint as preflight or live admission.

[Contract](../docs/research-ledger-disposable-preflight-v1.md),
[plan](../docs/plans/research-confirmation-transport-implementation-v1.md),
[previous delivered section](2026-10-21-p13c3b2b1-strict-source-validation-v1.md).

## Resolution (2026-10-21 - implemented and verified)

The complete restored locked suite passes **917 Rust** (297 library, 618 desktop,
2 service; one existing ignored), **1242 Vitest / 80 files**, typecheck/build,
all-target check/Clippy and scoped rustfmt. Only five existing warnings remain.
Local library/desktop/service times: 33.61 / 139.68 / 5.73 s; the large existing
capacity cases completed without timeout changes. Twelve new regressions pass.

Four deliberate mutants were assertion-caught: omitted original sequence,
omitted triggers, skipped final proposed FK check and ignored cleanup error.
The last panic message was split by PowerShell line wrapping; independent raw-log
audit confirmed the assertion failure. Exact source bytes were restored before
full verification. The ignored-cleanup mutant intentionally left a locked-probe
directory; after the test exited, its exact owned name/parent/artifacts were
verified and removed. Other roots were untouched. No product guard was relaxed.

The containing PR records owned verification-temp cleanup, document/diff checks,
fresh-main rebase/rechecks, six exact-head CI jobs, fresh base/reviews, guarded
merge and clean-main receipt. Continue 2b-2b immediately after delivery; this
substrate still has no live import/admission caller or source/union authority.
