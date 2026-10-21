# Durable original source witnesses before binding v5 observation

Date: 2026-10-17
Status: Design only, from merged reconstruction #196 (`ea8387a`). No schema,
wire implementation, runtime writer, execution or statistical-policy change.
Task status: [tasks.md](../../tasks.md).

This refines [P13c-3 §3-8](research-confirmation-storage-transfer-plan-v1.md)
before the observer activates. Existing implemented schema 6 stays historical
baseline. The next migration is additive; no old migration or stored document
is rewritten. Binding v5 keeps its already frozen shape.

## 1. Why a durable original snapshot is required

The [verified reconstruction primitive](../research-historical-source-observations-v1.md)
derives source counts/protocol/membership independently of receipt numbers.
Reopen loses caller context; imported source branches can differ from receiver
primary rows. Four original trials must stay four when the receiver has five.
Protocol high-water marks can also change with the same event head. Capture
the actual original source snapshot, not an invented later reconstruction.

A source witness proves facts against its supplied complete trial snapshot.
Artifact/campaign/run/engine metadata absent from trial rows remain supplied
claims. Publisher authentication, unseen history, actual files, current owner,
current counts and an actual alpha share still belong to P13c-4's final fence.
No witness, binding or import authorizes qualification/execution/recovery.

## 2. Exact self-contained source-witness format

Add logical blob kind `source_witness`. Its original UTF-8 text is canonical
compact JSON, object keys sorted recursively, without a trailing newline,
and has exactly:

| Field | Value |
| --- | --- |
| contractVersion | `research-confirmation-source-witness-v1` |
| trialExportText | Original complete trial v3 JSONL text, including its final newline |
| registeredSource | Exactly `source` (existing SourceBinding) and `candidates` (complete original CandidateBinding vector) |

The typed nested source structures retain existing closed fields, safe integer,
identity/domain, ordered candidate and 1-4096 candidate checks. Decode original
JSON before any normalization: duplicate/unknown/missing/null fields fail.
Require exact canonical outer bytes after decoding. Never normalize the nested
original archive. SHA-256 of exact outer bytes is the logical blob ID; no
additional `{id,body}` or declared authoritative count field is added.

The witness is self-contained. It does not refer to a full binding SHA: an
original trial snapshot can support several bindings with different alpha or
confirmation state. An unadmitted frozen source can also have a proof without
fabricating empty historical alpha/confirmation fields.

Whole witness text is at most **8 MiB**, including escaped nested text and
metadata. Nested trial input independently retains its 8 MiB reconstruction
limit. A nested archive fitting alone can fail the outer limit. Never truncate.
Statistics remain 4 MiB; bindings remain 8 MiB; legacy workspace formats remain
4 MiB. All existing joint limits remain: **10,000 documents/logical blobs,
64 MiB original bytes, 128,000 intervals, 10,000 unknown origins**. New witnesses
consume those same logical-blob row/byte budgets, not another quota.

## 3. Source anchors and host facts

Separate the reconstruction primitive internally into a self-contained closed
original-source parser and its existing anchored wrapper. Derive an immutable
trial anchor from the original archive: registry ID, exact sequence/chain and
complete restrictive trial evidence (quarantines, conflicted origins and all
pinned/upgraded family tests). The current `reconstruct_historical_source`
continues comparing that anchor exactly with the supplied v5 binding. Preserve
its static errors, original bounds, closed reproduction resolver and existing
nonportable exempt-benchmark refusal.

Every witness, including an independent root, must be fully decoded and passed
through that original-source reconstruction by the backend. Recompute actual
family/batch effective trials, target protocol and complete candidate membership;
never use receipt family-before totals, receiver counts or receiver references.
The nested v3 text is a read-only proof projection, **never a nested import** or
a known-empty alpha/confirmation assertion.

Give the pure graph explicit typed host witness observations tied to each
witness's exact SHA, source claims, derived anchor and facts. The pure outer
codec alone does not validate the opaque trial archive or create these facts.
An unchecked witness cannot borrow another witness's valid same-prefix context.
Host observations remain subject to the existing pure trust boundary; no type
authenticates a lying publisher/host or grants a current permit.

Aggregate source metadata only when equal for the same original source key;
unequal supposedly immutable source metadata is invalid, not last-writer-wins.
Coalesce only identical derived observations. Independently valid divergent
prefixes remain separate. Current trial/alpha/provenance observations are still
read and checked from actual registry rows in the same transaction.

A witness is not ingestion into the current trial ledger. Before treating
current history as known, check that its original event facts and protocol
evidence are accounted for by validated current trial history, or covered by
actual permanent restrictive evidence that blocks confirmation on the
unincorporated history. Never allow a foreign proof root to hide extra trials
while a smaller current count remains eligible. Read-only observation refuses
unmarked gaps. Archival union/restoration may add permanent confirmation-unknown
origins for such gaps in its write transaction; qualification import must reject
them before writes. Neither unknowns nor conflicts excuse invalid proof bytes
or a missing witness. Do not import the nested projection merely to derive facts.

## 4. Graph dependencies and unchanged confirmation policy

An admission requires at least one fully checked witness matching its complete
original registered source and **exact trial anchor of its original binding**.
Add material admission-to-witness dependencies alongside its existing binding,
operation and exposure dependencies. All matching supplied witnesses must be
valid; do not choose a good one to hide a bad one. Source witnesses are roots,
with no witness-to-binding dependency. Existing binding-to-prestate edges and
iterative Kahn cycle checks remain.

Each stored Validation source, even before an admission exists, needs a checked
complete-source witness for metadata/membership; it need not have an admission
or fabricated historical binding. Test inherits its full original Validation
source; a Test subset is not a new original source. Permit fully checked
source-witness roots for this purpose. Keep the existing requirement that every
statistics/binding blob is directly used by an admission/event: the new root
rule must not legalize orphan blobs of those existing kinds.

The core's eligible historical admission context still requires **two tests
per trial** and its existing complete-batch/count conditions. Generic archived
witness facts can have other valid source protocol values; do not blindly copy
them into eligible admission context or widen that policy. For a same-head
protocol upgrade, select witness facts by the admission's exact original binding
anchor, retaining older facts. Identical eligible same-key observations can be
coalesced; unequal ones fail. No new report/statistical configuration follows
from retaining an otherwise noneligible original source proof.

Align pure binding coverage with the already established
`LedgerEvidence::covers` rule: an actual permanently quarantined family covers
its saved protocol high-water mark. Keep the actual current protocol map rather
than inflating it from an incoming branch. Independently validate the original
anchor against its witness, and keep the quarantine blocking live admission.
This lets a valid contradictory imported source survive without becoming the
receiver's primary branch or weakening a live gate.

## 5. Additive registry migration 0007

Create `confirmation_source_witnesses` with exactly:

| Column | Contract |
| --- | --- |
| witness_sha256 | 64 lowercase hex, text, primary key |
| byte_length | Integer, 1-8 MiB, exact original content length |
| content_bytes | BLOB, exact original witness UTF-8 |
| recorded_at | Text, first local observation audit only |

Kind is implicit. There is no FK to receiver trial rows or a binding: original
branches are self-contained. Keep the existing `confirmation_blobs` stats/
binding table, contents, foreign keys and limits unchanged. Load both physical
tables into the existing logical blob vector/namespace. Enforce cross-table SHA
namespace uniqueness, immutable rows and anti-REPLACE guards even with recursive
triggers off. Protect audit columns too. Add no removal/overwrite API.

Replace only joint-capacity triggers as needed to include witness rows/bytes
on **every** old/new insertion path. The other guards remain. Include all eleven
tables in legacy omission checks, even for corrupted orphan cases. Preflight
combined/individual types and byte/row limits before hydration, then check every
original SHA/length/UTF-8/codec/proof and all existing scalar/interval projections.
No checksum, count or raw SQL ID selection replaces full validation.

Keep immutable `confirmation_history_birth = 0006_confirmation_history` meaning
unchanged. For pre-0007 managed confirmation history, add permanent confirmation
uncertainty for the local registry, actual stored operation/exposure origins and
original binding registry IDs. Enumerate binding IDs only through bounded
original SHA/kind/length/strict v5 parsing, not unbounded SQL JSON extraction or
claimed counts. Existing unknown markers stay permanent. Empty/manual-only
history needs no source witness or new managed-history assertion.

Apply table/guard changes and bounded provenance backfill with the migration
receipt in one transaction. Capacity, corruption or injected late failure rolls
back all DDL/trigger/marker changes and leaves 0007 unapplied. Test existing exact
capacities and original bytes/audit values before and after migration. No old
blob table rebuild/drop or global FK bypass is needed.

## 6. Legacy evidence and explicit restoration

Unknown markers never excuse bad original bytes, kind, projection, index,
dependency, cycle or missing required source witness. There is no receipt-derived
unknown-count fallback. Valid unknown/conflicted histories with complete proofs
remain readable/adoptable for exploration, while their stops block admission.

Existing v3/v4 imports contain no managed confirmation records and retain their
unknown markers. Ordinary existing empty/manual-only histories remain available.
Schema-6 managed records came only through the experimental supplied-context
APIs; no runtime writer had activated. If such records lack original proofs,
they are incomplete under the new observer: preserve all rows and the last good
workspace binding, refuse adoption and require actual proof restoration. Never
invent proof from the current primary, silently adopt empty or erase data.

Provide a narrowly scoped explicit witness-only restoration helper before the
observer activates. Accept only fully verified self-contained witnesses; no new
old-kind documents/events/conflicts/bytes are accepted through that helper.
Preflight/read original rows and exact projections. Validate the proposed
**materialized** old-plus-witness union without auto-completing old conflicts.
A witness cannot change old exposure relations, so missing restrictive conflict/
event/blob/index evidence still fails. Insert only new witnesses and perform a
strict final reload in the same immediate transaction before its one commit.
The helper may additionally append only required permanent provenance markers
derived from the restored source/gaps, never caller-supplied old-kind rows or
removals. Late failures roll back every witness/marker addition. Permanent
unknown markers cannot clear.
If the actual original snapshot is unavailable, the helper cannot fabricate it.

## 7. Binding, complete transport and final capture

The observer reads all current trial, alpha, provenance and complete witness/
confirmation evidence in one deferred transaction. It derives historical context
from witnesses, checks the graph/projections, then returns exact ID sets.
Witness SHA IDs join binding v5's existing `confirmation.blobs`; no field/set is
added. Loss or equally sized valid substitution still fails coverage; invalid
proof/graph is invalid state. Neither overwrites the last good workspace binding.
The observer never appends witnesses on ordinary checks/adoption.

The complete archive v5 writer is **not implemented/released**. Revise its planned
schemaVersion to `0007_confirmation_source_witness` and allow the third logical
`confirmationBlob.kind = source_witness` through the existing exact blob wrapper.
Its nine counts stay unchanged; `blobs` counts all three kinds/two physical tables.
Retain original text/digest, deterministic section order, all current file/line/
joint bounds and one-transaction trial/alpha/confirmation union. New table and
all dependency/provenance additions participate in the same final commit.
Legacy v3/v4 omission/uncertainty guards remain active. Old schema-6 builds refuse
schema 7; old workspace readers refuse binding v5. Prove both with real binaries.

P13c-4 captures an actual original trial snapshot and authenticated source metadata
inside its private owner/current-count fence before admission/reveal. Reuse a
checked witness only when the actual current trial anchor and complete source
claims still match; alpha-only changes need not duplicate a trial snapshot.
Otherwise capture new original bytes before writes. A later registry cannot
retroactively recreate an old high-water mark. Historical facts never replace
the final actual `AdmissionCount`, actual share or owner check. Witnesses/notes/
hashes never enter AI Train context, prompts, generation, tuning or ranking.

## 8. Ordered PRs and required evidence

Implementation refinement after merged graph #199: split 2b into 2b-1 additive
schema/provenance/old-entry guards, then 2b-2 actual source/current-coverage
context, strict combined IO and explicit restoration. Before 2b-2 activates
strict reads, old experimental readers/writers must refuse any witness rows
instead of ignoring the new physical table. Both remain before binding observer
activation. [Schema implementation plan](research-source-witness-schema-plan-v1.md).

| Slice | Scope and verification |
| --- | --- |
| 3b-1b-2a-1 | Pure canonical witness codec and self-contained source anchors, retaining the anchored wrapper. Independent Node/Rust byte/hash fixtures, closed fields, exact bounds, metadata-claim separation and protocol/head changes. No graph/SQL switch. |
| 3b-1b-2a-2 | Checked witness host facts, source/admission dependencies and new root liveness. Original/mismatched/missing/bad witness, no laundering, same-head eligible/noneligible proof distinction, existing quarantine coverage parity and unchanged report/P12 policy; executable mutants. No IO/schema. |
| 3b-1b-2b | Additive schema 7, combined snapshots/context, exact rows/projections and witness-only legacy restoration. Independent connections/reopen, all old/new capacity/replacement paths, namespace substitution, unincorporated-event/protocol stops, permanent markers, preserved bytes/audit, every late rollback. No binding writer switch. |
| 3b-1b-3 | Original whole-graph binding v5 observation/adoption, exact identity coverage and last-good preservation, valid unknown/conflicted exploration, legacy adoption and real old-build schema/binding refusal. |
| 3b-2 | Complete v5 transport/atomic union, original branches/witnesses/stops, two-hop/replacement/replay, all file/resource limits and late rollback. |

Each begins from merged main after its predecessor's six exact-head CI jobs and
merge. Tests must retain all old graph/provenance guards, use actual registered
source snapshots, compare actual source facts rather than audit totals and catch
deliberate mutants by assertions. No bootstrap, calibration, reserved seed
20261117, production activation or statistical-policy expansion.

New proof fixtures must contain actual complete source events and their derived
prefix/membership. Older pure graph fixtures used supplied synthetic observations,
including nonempty counts at a genesis prefix; those are not durable source
proofs and must not be made valid by relaxing archive/anchor checks.

## Implementation refinement (2026-10-21)

Whole-graph binding observation/adoption is delivered in #202. Split the
remaining P13c-3b-2 into complete backend export (2a), then strict original
input/source validation and atomic union (2b), each merged after its six
exact-head CI jobs before the next begins. The schema-7 v5 shape, three blob
kinds, original proof/stop semantics and all bounds above stay fixed. Export
alone is not a released end-user transfer workflow or import/admission permit.
[Detailed bounded order](research-confirmation-transport-implementation-v1.md).
