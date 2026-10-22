# P13c-4 final-admission implementation order

Date: 2026-10-22. Baseline: merged #206 (`9f36cbb`).
Status: implementation refinement; no admission/execution activation.
Authority: [parent ordering/crash contract](research-confirmation-exposure-recovery-plan-v1.md#5-final-admission-ordering-and-linearization)
and [original witness capture](research-source-witness-storage-plan-v1.md#7-binding-complete-transport-and-final-capture).

Complete [v5 archival import](../research-confirmation-import-v5.md) now preserves
trial/alpha/confirmation evidence atomically. Transport coverage remains separate
from authenticated owner/source/count/share/file authority. Finish P13c-4 in the
bounded order below; each slice starts after its predecessor's PR/CI/merge.

## 1. Existing boundaries

| Current implementation | Consequence for final admission |
| --- | --- |
| [Binding/fence methods](../../alpha-factor-forge/src-tauri/src/research/trial_ledger_binding.rs) own deferred transactions and return after ending them. | Their observations cannot bridge a later reservation transaction. Reuse the logic within the final caller's transaction. |
| [Alpha reservation](../../alpha-factor-forge/src-tauri/src/research/trial_ledger_alpha.rs) owns an immediate transaction, chooses a share and commits independently. | Never spend through this public helper before the integrated final fence. Preparation and persistence need a narrowly shared transaction contract. |
| [Workspace ownership](../../alpha-factor-forge/src-tauri/src/db/ownership.rs) reserves a write transaction before checking epoch; its quiet helper leaves mutation_seq unchanged. | Require an actual production epoch, compare the captured observation before the domain increment, then retain that guard across registry admission and workspace projection. |
| [Snapshot resolution](../../alpha-factor-forge/src-tauri/src/research/campaign_snapshot.rs) returns public row references from the current read. | These references, hashes and caller-constructed structs are not source authority or proof of unseen protected acquisition. |
| [Pure admission records](../../alpha-factor-forge/src-tauri/src/discovery_core/confirmation_history/admission.rs) check frozen observations and recompute P12a. | Valid serialized evidence cannot construct a private live admission capability. |
| [Exploratory runner](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs) has its own control, registration and enqueue flow. | A dormant protected intent/receipt must not accidentally start its ordinary coordinator or inherit its lock order. |

## 2. First code slice: P13c-4a transaction-owned observations

Extract internal `TrialLedger::check_binding_in(tx, saved)` and
`TrialLedger::fence_admission_in(tx, batch_id, earlier)` accepting a borrowed
`rusqlite::Transaction`. Visibility stays inside `research::trial_ledger`.
They perform reads only: no receiver mutex, nested transaction, commit/rollback,
workspace/control lock, file IO, callbacks, reservation or admission capability.
The caller owns the snapshot and its lifetime.

Reject an inactive transaction or mismatched registry-meta identity. These are
consistency guards, not physical-registry ownership authentication; a private
copy with the same ID still grants no live authority. Final admission will need
its own private guard created from the actual locked receiver connection.

Preserve the existing public wrappers and their deferred transaction behavior,
including pre-mutex saved-binding/prefix validation, check order, return/error
codes and existing finish/early-refusal behavior. Helpers validate their supplied
evidence too. Do not globally reformat the legacy binding module.

4a organization: public wrappers and borrowed entry points share private SQL
observation bodies. The new active/metadata guards belong only to the borrowed
entry points, preserving old public error precedence in damaged combinations.
An unproven public count-fence prefix still returns through transaction drop;
other successful public observations commit their own deferred read. See the
[4a implementation receipt](../../handoffs/2026-10-22-p13c4a-transaction-observations-v1.md).

Binding observation retains the actual chain, complete restrictive identities,
alpha/provenance and strict whole witnessed v5 graph/coverage checks. The count
fence retains its narrower prefix/family/batch/count semantics; it does not
silently become a complete binding/admission check. `AdmissionSnapshot` remains
comparison evidence. Only actual SQL constructs the private-field
`AdmissionCount`; no deserialized report becomes a count.

A deferred caller gets a consistent historical snapshot only. Future final
admission must use the actual IMMEDIATE registry transaction under the verified
workspace guard, recompute eligibility there and persist before releasing it.
Returning a count or `Unchanged` does not authorize an operation after that
transaction ends. No runtime caller switches in 4a.

Required 4a evidence:

1. Public/helper parity for current/unchanged/grown/inconsistent, replaced or
   rolled-back prefixes and missing/substituted restrictive identities. Existing
   whole-graph, legacy and local non-effective benchmark rules stay intact.
2. Helpers can read the caller's uncommitted internal import and observe its
   actual count/identities. An independent reader sees no committed change;
   caller rollback preserves every row/head/checkpoint/audit.
3. The receiver mutex may already be held. Use an independent matching-registry
   connection in a worker and a bounded channel, always releasing the held mutex
   before joining, so a nested-lock mutant fails without hanging the suite.
4. An independent connection with zero busy timeout cannot acquire a write
   reservation while the caller owns IMMEDIATE, including after the helpers
   return. Only the caller's commit/rollback releases it.
5. Success/refusal/error leaves the caller transaction active and unchanged,
   including `total_changes()`. Inactive/wrong-registry transactions, malformed
   prefixes and a chain damaged after open refuse without repair.
6. A deferred snapshot stays internally consistent across another connection's
   commit; a subsequent fresh public observation sees growth. This proves
   snapshot semantics without presenting the old observation as a final permit.

Catch deliberate mutants by assertions, restore exact source bytes, run the
required full checks and preserve all old cases before the guarded PR merge.

## 3. Remaining bounded slices

| Order | Scope | Gate before completion |
| --- | --- | --- |
| 4b: backend workspace/input observation | Actual epoch and mutation_seq, exact immutable references/digests, complete registered candidates and backend-owned verified input bytes. Materialize expensive artifact/source data before the registry lock. | Mutate snapshot/artifact/reference/owner between preparation and final guard; refuse stale observations without changing the frozen request. Freeze the exact private types/constructors and input bounds before implementation. |
| 4c: actual-share transaction preparation | Narrowly extract alpha proposal/persistence logic after recording its exact API and contradiction semantics. Review complete current history and use the actual next share with all-family P12a/count/protocol. | Failed eligibility creates no new spend; retries keep original origin/key/slot; contradictions preserve permanent evidence without renumbering. Existing public archival reservation behavior remains. |
| 4d: private integrated registry admission | Under the verified workspace guard, acquire registry mutex then IMMEDIATE. Check complete binding/provenance and original operation, capture/reuse an actual matching original witness, recompute overlap/actual-share P12a, append reservation/exposure/admission and commit once. | Cross-process registration, complete import, manual-overlap and competing admission cannot interleave across final check/spend. Late SQL errors roll everything back. Untrusted serialization cannot mint the private guard/capability. |
| 4e: registry-first workspace projection | Persist original immutable intent before expensive prepare. After registry commit, enqueue/project its exact receipt in the still-owner-checked workspace transaction, update binding/mutation_seq, release all DB guards, then schedule only the separately gated future path. | Freeze the exact workspace schema/IDs/bounds before any additive migration. Every commit-boundary crash preserves registry spend and original operation; no duplicate share/job or automatic execution. Parent P13c-4 remains incomplete until this and contention evidence pass. |

This table fixes ordering and obligations, not unreviewed future schema fields.
Each later slice must record its concrete API/schema/resource contract and move
through Next/In Progress before touching implementation. Refine a row further
when it cannot fit one reviewable session; do not mix migrations or activation
into the read-helper PR.

## 4. Integrated lock and crash obligations

The parent order remains workspace DB guard/owner-checked IMMEDIATE -> registry
mutex -> registry IMMEDIATE. Recheck workspace identity, epoch, mutation_seq,
immutable references and pinned bytes before the domain state increment.
Registry helpers never call back into workspace/control locks. No artifact IO,
download, bootstrap, UI/event emission or runner-control lock occurs while the
registry write lock is held.

An exact original-operation retry returns its immutable receipt or a conservative
spent/blocked outcome. Import/replacement does not assign a new origin/key/share.
Capture actual original trial bytes/source metadata within the final count fence
before writes; an older historical witness cannot substitute for actual current
count/share/owner checks or reconstruct an unavailable old high-water mark.

Registry commit is the linearization point. Before it, a late failure leaves no
partial reservation/exposure/admission. After it, workspace failure or process
death keeps spend and occupancy even if no workspace job exists. Workspace
projection commits before workers/events and cannot refund the registry.
No cross-database atomicity or automatic protected re-execution is claimed.

## 5. Activation remains unavailable

Current P06/manual/legacy data cannot become known-unseen merely because its hash
or row references verify. Managed protected acquisition and complete manual
exposure mediation are still missing prerequisite authority. Statistical policy
for real-market/unaccepted configurations also remains an explicit refusal under
the [parent policy contract](research-confirmation-exposure-recovery-plan-v1.md#9-statistical-policy-remains-an-explicit-gate).

Do not add serialized booleans, environment switches or user checkboxes that mint
these authorizations. Bounded `cfg(test)`-only fixtures may exercise dormant
positive paths; they must be absent from release authority construction.
P13c-4 neither enables the ordinary coordinator nor adds commands/providers/UI,
execution/sealing/recovery (P13c-5) or production mediation/activation (P13c-6).
No calibration, reserved seed 20261117, threshold/B/alpha change or protected
witness/selection/report content in Train generation context is authorized.

Delivery evidence: [handoff](../../handoffs/2026-10-22-p13c4-admission-order-v1.md).

## 6. Workspace/input refinement

After 4a's verified #208 delivery, [the 4b contract](research-workspace-input-observation-v1.md)
grounds current workspace/ownership, snapshot, history/source and artifact
boundaries. Only 4b-1 transaction-owned workspace consistency observations are
specified first; complete source/reference derivation and bounded immutable
materialization require their own concrete contracts before code. This does not
complete parent 4b, create physical owner/unseen authority or activate execution.
