# P13c-4b workspace and input observation

Date: 2026-10-22. Baseline: merged #208 (`6db0674`).
Status: bounded implementation contract; no admission/execution activation.
Parent: [final-admission order](research-final-admission-implementation-v1.md#3-remaining-bounded-slices)
and [workspace preparation/final guard](research-confirmation-exposure-recovery-plan-v1.md#51-preparation-and-workspace-observation).

## 1. Existing boundaries and bounded order

| Source | Existing behavior and consequence |
| --- | --- |
| [Runtime workspace](../../alpha-factor-forge/src-tauri/src/runtime/mod.rs), [OS lease](../../alpha-factor-forge/src-tauri/src/runtime/lease.rs) | Startup holds an actual OS lock before opening/migrating/acquiring an epoch. OwnershipHandle contains a private lock, but workspace fields and numeric epoch are public; loose copies of them are not physical ownership. The lock path accessor currently exists only in tests. |
| [Ownership transactions](../../alpha-factor-forge/src-tauri/src/db/ownership.rs) | Quiet writes reserve IMMEDIATE before checking an optional epoch; domain writes also increment mutation_seq. The future protected path must require a positive actual epoch and compare its observation before that increment. Existing optional-epoch helpers remain unchanged. |
| [Snapshot resolver](../../alpha-factor-forge/src-tauri/src/research/campaign_snapshot.rs) | Revalidates actual dataset/candles and returns public reference rows. Those references and P06 non-demo/OK status do not prove unseen acquisition or retain an owner guard. |
| [Research history](../../alpha-factor-forge/src-tauri/src/research/history.rs) | AttemptRow is a display/history read and converts some invalid stored JSON to Null. An authentic source reader must instead reject malformed original JSON and prove complete membership. |
| [Artifact store](../../alpha-factor-forge/src-tauri/src/research/artifacts.rs) | Existing read verifies checksum/length after an unbounded allocation/read_to_end. Preparation needs an explicitly bounded, immutable pinning contract before reuse, without changing unrelated exploration reads. |
| [RegisteredSource](../../alpha-factor-forge/src-tauri/src/discovery_core/confirmation_contracts.rs), [original trial reconstruction](../../alpha-factor-forge/src-tauri/src/research/trial_ledger_transfer/historical_observation.rs) | Public source fields validate domains; original trial bytes prove trial membership/protocol. Campaign/run/artifact/engine metadata remain claims until independently derived from the actual backend inputs. |

Implement **4b-1 transaction-owned workspace observations** first, as specified
below. Then freeze and implement 4b-2 complete source/reference observations and
4b-3 bounded immutable input materialization. Each contract/code slice follows
its predecessor's verified PR/merge; only 4b-1 enters Next after this design.
Parent 4b remains incomplete until all source/input and mutation tests pass.

## 2. First code slice: private workspace observations

Add an internal module `research::confirmation_inputs` with visibility confined
to `research`. Keep the first slice limited to metadata observation: no runtime
caller, source/candidate/artifact loading, registry access or migration.

Freeze these APIs (lifetimes elided below):

```rust
ExpectedWorkspace::new(
    workspace_id: &str,
    epoch: i64,
    holder_kind: HolderKind,
    holder_instance_id: &str,
) -> AppResult<ExpectedWorkspace>

capture_workspace_in(
    tx: &rusqlite::Transaction,
    expected: &ExpectedWorkspace,
) -> AppResult<WorkspaceObservation>

recheck_workspace_in(
    tx: &rusqlite::Transaction,
    expected: &ExpectedWorkspace,
    saved: &WorkspaceObservation,
) -> AppResult<()>
```

Both types have private fields and no Serialize/Deserialize/Default or unchecked
constructor. ExpectedWorkspace owns validated String identities, i64 epoch and
HolderKind; WorkspaceObservation owns the same fields plus i64 mutation_seq and
is constructed only by an actual SQL read. A clone of a
valid observation remains historical comparison evidence. The expectation
constructor checks domains only and cannot authenticate its caller.

The observed fields are exactly:

| Field | Actual source and domain |
| --- | --- |
| workspace_id | JSON string in app_settings key workspace_id; exactly 32 ASCII hex characters, matching existing runtime identity rules. Compare case exactly, without normalization. |
| epoch | workspace_ownership singleton id=1; SQLite integer, positive i64. No None or zero acquisition. |
| holder_kind | That ownership row; exactly desktop-embedded or service, matching HolderKind. |
| holder_instance_id | That ownership row; exactly 32 ASCII hex characters, compared exactly. |
| mutation_seq | runtime_state singleton id=1; SQLite integer, nonnegative i64. Observation never increments it. |

Do not include heartbeat_seq, PID, timestamps or run display fields. Heartbeats
and diagnostic-only changes must not invalidate a domain observation.
The source tables are [0004 ownership](../../alpha-factor-forge/src-tauri/migrations/0004_workspace_ownership.sql),
[0005 identity](../../alpha-factor-forge/src-tauri/migrations/0005_runtime_ledger.sql)
and [0006 mutation state](../../alpha-factor-forge/src-tauri/migrations/0006_request_effects.sql).

### 2.1 Bounds and refusal order

The workspace-ID JSON has an inclusive **256 UTF-8-byte** cap. This fits even a
fully escaped 32-character ASCII hex ID (194 bytes including quotes); excess
whitespace is still counted. Holder kind and instance ID have inclusive 16/32
byte caps and then their exact domain checks. These are resource limits for the
new dormant reader, not a change to existing workspace identity/public helpers.

Check SQLite typeof and byte length before copying text into Rust. Refuse BLOB,
NULL, oversized text, missing singleton rows, non-string/malformed JSON,
non-integer/negative version values, zero/unowned acquisition and unsupported
holder kinds. Do not coerce, truncate, invent defaults or read unrelated large
row payloads. Missing tables/SQL failures propagate as AppError::Db.

ExpectedWorkspace::new validates the supplied identity/positive acquisition
domains without IO. Each reader first requires an active transaction, then
reads and validates all three actual metadata records in the same snapshot.
Compare actual workspace identity to the expectation, then actual owner tuple.
For recheck, also compare the saved workspace, owner tuple and mutation_seq in
that order. Any sequence change, including a decrease, refuses; do not replace
the saved observation with a new one and continue the same prepared operation.

Use static AppError::Other messages `workspace_observation_invalid` for invalid
expectations/transaction/metadata and `workspace_observation_changed` for a
workspace or mutation comparison mismatch. A valid positive acquisition whose
epoch/kind/instance differs uses existing AppError::StaleOwner with a static
detail. Do not add global error variants or include original JSON/IDs in errors.

### 2.2 Transaction and authority boundaries

Readers borrow the caller's active transaction and perform reads only. No
receiver/global mutex, nested transaction, commit/rollback, DML, workspace
mutation increment, registry/control lock, callback, file IO, event or worker.
Success/refusal/error preserves transaction activity, all data and total_changes.
The caller owns its reservation and its eventual commit/rollback.

These are **consistency observations only**. An independent matching copy or a
deferred historical snapshot can pass these comparisons. Neither grants a live
owner/admission capability, verifies the physical OS lease, establishes unseen
history or authorizes work after the transaction ends. Do not name the result
an authenticated guard or use a serialized expected identity as one.

The future integrated path must construct a separate private guard from the
actual runtime-owned workspace/connection/lease, reserve workspace IMMEDIATE,
recheck the captured metadata/references/pinned bytes before domain increment,
then take registry mutex -> registry IMMEDIATE. This first reader does not
verify IMMEDIATE by inspecting a Transaction wrapper or claim cross-file
atomicity. No runtime ownership fields or production authority are relaxed.

## 3. Required 4b-1 evidence

Use real file-backed migrated SQLite and independent connections; test fixtures
may hold the actual isolated OS lease before acquiring their test epoch.

1. Capture actual fields and recheck unchanged state; preparation reads do not
   change mutation_seq, ownership, events, requests or any table.
2. Caller-owned pending mutation is visible inside its transaction only; an
   independent reader sees committed state. Caller rollback restores all rows.
3. An independent zero-timeout writer stays Busy throughout caller IMMEDIATE,
   including after success/refusal/error; only caller finish releases it.
4. Deferred observation remains internally old across another connection's
   committed mutation. A fresh read detects it; old equality is no live permit.
5. Independently change workspace ID, epoch, holder kind, instance ID and
   mutation_seq; each comparison refuses. Test sequence growth and rollback.
   Heartbeat/PID/timestamp-only changes preserve equality.
6. Inactive transactions, a different workspace, absent rows, unowned/invalid
   metadata, SQL type substitutions, actual SQL read errors and malformed
   expectations refuse without repair. An identity-matching copy is explicitly
   consistency-only, not a positive physical-ownership test.
7. Exact 256-byte ID JSON passes; 257 bytes refuses. Cover escaped IDs, supported
   case behavior and exact holder domains/limits without truncation or defaults.
8. All outcomes retain caller activity, total_changes and complete pending
   state. No nested BEGIN or hidden transaction finish; old public ownership and
   identity helpers keep their existing behavior.

Catch deliberate omitted-owner, omitted-version, inactive-transaction and
premature-finish mutants through explicit assertions, restore exact bytes, then
run full checks and six-job exact-head CI before guarded merge.

## 4. Remaining source and input contracts

Before 4b-2 code, freeze the complete candidate/reference derivation: stored
campaign re-freeze, original run config/engine, the entire registered candidate
set and trial batch, strict original fingerprints/lineage, exact immutable
attempt/artifact references, snapshot/instrument/calendar/cost/provenance and
their canonical hashes. Do not trust a limited UI list, selected subset, latest
row or fallback-to-Null history DTO. Define precise bounded SQL projections,
contradiction/error order and per-field digest derivation; public claimed hashes
or original trial witnesses alone cannot fill missing backend evidence.

Before 4b-3 code, freeze per-input and joint resource bounds, checked byte/count
arithmetic, path/handle policy, original JSON/source parsing, candle identity and
occupied support, and immutable backend-owned buffers. Materialize expensive
data outside registry locks; final execution must consume pinned verified bytes,
not reread a filename. Explicitly define file mutation/replacement during prepare
and final recheck behavior, including what a pinned handle guarantees on each
supported platform. Do not claim a filesystem-wide atomic snapshot from hashes
or a SQLite transaction. Retain the parent requirement to refuse stale
preparation when a source/ref/artifact/snapshot/owner changes before the final
guard. Define how those changes are detected and how the final pinned inputs
stay immutable through later consumption. Mutate each boundary and verify
refusal without changing the frozen original request; pinned bytes cannot excuse
a mismatched authenticated reference or a claim that the current file is intact.

These later rows are ordering obligations, not frozen unreviewed schema or APIs.
Record concrete contracts and move through Next/In Progress before each code
slice. No generic artifact-store rewrite or new workspace migration belongs in
4b-1. Original immutable intent persistence remains 4e's separate contract.

## 5. Production authorization remains unavailable

All source integrity and pinning checks remain distinct from managed protected
acquisition, complete manual exposure mediation and statistical-policy authority.
Historical P06/legacy data cannot be promoted to known-unseen by this reader,
by a hash, by matching IDs or by a user boolean. Preserve the
[parent refusal gates](research-final-admission-implementation-v1.md#5-activation-remains-unavailable).
Test-only positive fixtures must not introduce a release constructor/bypass.

No alpha reservation/spend, confirmation admission/execution, ordinary runner
activation, command/provider/UI, policy/threshold/B change, calibration or
reserved seed 20261117. Parent P13c-4 stays incomplete through integrated
contention/crash/recovery acceptance. Delivery evidence belongs in the
[handoff](../../handoffs/2026-10-22-p13c4b-workspace-input-contract-v1.md).

4b-1 implementation evidence: [transaction-owned workspace observations](../../handoffs/2026-10-23-p13c4b1-workspace-observations-v1.md).
