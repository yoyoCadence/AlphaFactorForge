# P13b reservation registry persistence plan v1

Status: Design and implementation order only. No migration, runtime
reservation, reveal or new qualification authority is implemented here.

## 1. Scope and existing decisions

P12 is accepted; P13a provides checked
[`research-alpha-reservation-v1`](../research-alpha-reservations-v1.md)
records and pure history review. The existing
[`research-alpha-allocation-v1` §9](../research-alpha-allocation-v1.md#9-what-p13-must-do-this-module-cannot)
already fixes one budget per family, actual allocated alpha, exact replay
deduplication, permanent family stop on conflicting confirmation numbers,
and no overall cross-family false-positive guarantee. This plan supplies
the persistence/atomicity design needed to enforce those decisions.

Use the **same external trial-registry SQLite database**, with its existing
local-volume/containment rules and permanent registry ID. A separate alpha
database would create another cross-database gap between the final trial
count and reservation. No credentials or market-return arrays belong here.

The new tables, binding/export versions and runtime integration are separate
implementation slices. No existing migration, event ID, trial chain,
historical result or statistical computation is rewritten.

## 2. Append-only evidence

The next registry migration adds these responsibilities (exact SQL is owned
by P13b-1):

| Evidence | Key and invariants |
| --- | --- |
| Family budget | One family → one canonical allocation document and allocation ID. The family must already be a known trial family. Idempotent identical declaration; a different declaration preserves both documents as conflict evidence, never overwrites or starts a fresh budget. |
| Reservations | Content-keyed P13a records, validated on write/read. Exact duplicate is a no-op. Keep distinct records even when their confirmation numbers or source operation keys conflict. |
| Alpha conflicts | Content-keyed canonical evidence naming the family, conflicting budget documents or reservation IDs, and reason. Permanent; no release/delete/renumber path. A family remains stopped after export/import even if a later caller supplies only a consistent subset. |
| Binding observations | A versioned set of the budget identities, reservation IDs and conflict IDs already observed by the workspace, alongside existing trial-ledger evidence. Counts alone are insufficient. |

All new evidence tables have UPDATE/DELETE refusal triggers. In particular,
do **not** use a unique `(family, confirmation_number)` constraint that loses
the second imported reservation, or `INSERT OR REPLACE` to select a winner.
Indexes may accelerate conflict lookup; they are not the authority.
The storage boundary also checks `(originRegistryId, reservationKey)` across
families: the pure per-family reviewer cannot see a conflicting reassignment
hidden in another family's history. Preserve both bodies and stop every
affected family instead of filtering the other record away.

The normal local reservation API serializes writers so it never allocates an
already occupied number. Import must still retain independently allocated
duplicates as conflicts. Malformed or hash-invalid input is refused before
any mutation; an integrity-valid but conflicting union is durably recorded
and stops that family. Storage errors roll the entire transaction back.

No lifecycle field can release a reservation. Cancellation, failure,
missing artifacts and never-started work still consume the share.
Alpha-family stops and alpha-integrity refusal apply to confirmation; they
must not be implemented by inserting an ordinary trial-family quarantine
that also prevents exploration. Existing trial-only registration/counting
behavior stays unchanged. Corrupt alpha evidence must fail every affected
alpha read/admission, including after reopen, without inventing empty history.
Storage/transfer inputs must have explicit tested byte/record bounds and
refuse oversized inputs before mutation. Never truncate history to meet a
transport or binding limit; exact bounds belong to each slice's contract.

## 3. Atomicity boundary

The existing `fence_admission` ends its read transaction before returning.
Calling it, then opening another transaction to reserve alpha is unsafe.
The future integrated operation must instead:

1. Acquire the existing registry connection mutex and a SQLite **IMMEDIATE**
   transaction. The SQLite lock is required across independent processes;
   an in-process mutex alone cannot serialize another workspace's writer.
2. Reverify the saved event prefix and restrictive binding evidence, current
   family protocol/count and quarantine state in that same transaction.
   Reuse transaction-scoped helpers; do not call methods that try to reacquire
   the mutex or open nested transactions.
3. Load the family's immutable budget and complete reservation/conflict set;
   review P13a history and allocate the actual next share. Re-run P12a with
   that share, the current family count and the already frozen sample
   count/cap. The original campaign remains unchanged as audit evidence.
4. Require the verified immutable confirmation batch/selection and its bound
   snapshot/candidate evidence. Read any costly artifact data before holding
   the registry write lock; recheck the stable observation at admission.
   No caller-provided count or stale ELIGIBLE report substitutes for this.
   P13c must define the workspace/registry lock order and the immutable
   observation or owner/epoch/mutation guard used for that recheck. Merely
   holding a workspace lease does not prove a previously read artifact or
   dataset still matches its frozen identity.
5. Persist the reservation plus the exact final admission/fence/precision
   evidence and commit. This commit is confirmation admission's linearization
   point. Registrations/imports cannot interleave between its final count
   and reservation. Freeze the count and versions used for audit.

The final integrated API is deferred to P13c. P13b-1's persistence primitives
must not be exposed as a production permission to execute confirmation.
No plugin/Tauri command can synthesize a trusted admission token from a
serialized pure-history report.

## 4. Workspace crash and retry

There is no atomic transaction spanning the external registry and workspace
database. Use **registry commit first**, then owner/epoch-checked workspace
enqueue referencing that reservation ID. Failure of the second commit never
rolls back or refunds the first. No data is revealed before reservation.

Before reservation, freeze the complete request and its stable operation key.
A retry must recover the same committed record by origin/key and verify the
exact bound body; it must not request the allocator's next number. Reusing a
key with a different body retains conflict evidence and stops the family.

| Crash point | Required recovery |
| --- | --- |
| Before registry commit | No spend exists; an exact retry may attempt admission afresh using the same frozen request/key. |
| After registry commit, before workspace enqueue | Share remains spent. Exact retry returns the original reservation and may reconstruct the missing owner-checked enqueue. |
| After enqueue, before response | Workspace request idempotency returns the existing job; no new reservation or job. |
| During execution / result write | Reservation remains spent. Recover the same job/evidence or record failure; never rerun as a new unseen confirmation. |
| Bound artifacts/engine unavailable | Keep the spend and failure; do not invent replacement evidence or silently change the frozen request. |

P13c's batch/exposure contract must decide which interrupted computations may
resume without a second reveal; this plan authorizes no automatic re-execution.

## 5. Rollback and transfer

Extend the versioned workspace binding (new `trial-ledger-binding-v3`) to
record sorted, unique **identity sets** for observed budgets, reservations
and conflicts. Preserve existing event-prefix and §23 restrictive evidence.
A current registry must contain every saved identity with valid matching
content; a different equally sized set is not coverage. Missing evidence
blocks confirmation and leaves the last good workspace binding untouched.

Legacy bindings carry **unknown alpha evidence**, not a silently empty set
or a confirmation permit. A verified adoption writes a complete new binding;
old builds must refuse the unsupported binding/registry version. This does
not prove old manual data was unseen: `legacy_exposure_unknown` remains a
separate P13 exposure barrier.

Introduce a complete alpha-aware export version (planned
`trial-ledger-export-v4`) carrying budgets, all reservation bodies/IDs and
conflict evidence together with the existing complete trial export. Validate
the full input before mutation; union by identity and recompute conflicts in
one transaction. Preserve source origin/key/number; do not rebase or renumber
reservations, max counts, sum duplicate records, or choose a later budget.

For **P13 qualification transfer**, v1–v3 exports cannot prove alpha history
and must be refused before writes. Historical trial-only import behavior is
not silently converted into alpha authority; integration must prevent that
legacy path from bypassing P13 coverage checks. An old-format export must not
drop existing alpha evidence and then advertise itself as complete.

Binding/proof transfer must work through replacement registries and repeated
imports. Missing registry files, rollback, conflicted origins and incomplete
replacement checkpoints remain fail-closed. As in the existing ledger threat
model, jointly restoring all machine/workspace evidence to a prior state
cannot be detected locally; no tamper-proof claim is added.

## 6. Implementation order and acceptance

| Slice | Scope | Required executable evidence |
| --- | --- | --- |
| P13b-1 | External append-only budget/reservation/conflict storage, checked reads, permanent stop and idempotent exact retry; no runtime caller | Two concurrent connections cannot double-allocate; same body replay, changed declaration/key conflicts, equal-share duplicate numbers, failed/unfinished spend, append-only triggers, reopen and corrupted-body refusal. |
| P13b-2 | Versioned binding evidence and alpha-aware transfer union | Restored equal-count/different-ID history refused; lost conflicts/budgets refused; duplicate/two-hop imports preserve IDs and stop; conflicting budgets/confirmation numbers retain both; malformed or incomplete inputs cause no partial writes; unknown legacy alpha evidence never authorizes confirmation. |
| P13c design | Frozen batch/selection, final-fence reservation, exposure/overlap and recovery contract | Declare authority and ordering before adding production reserve/execute calls; include gap/interleaving and all crash points in §3–4. |
| P13c implementation slices | Integrate only the agreed batch/exposure/admission contract | Real cross-process contention and deterministic crash injection, followed by typed service/Tauri/native acceptance. |

P13b-1 implementation must be reviewed separately from binding/transfer and
from runtime reveal. The API must use the registry's whole observed history,
not a caller's subset. The full existing suite remains required, including
trial-ledger rollback and campaign admission; no full final-seed rerun.

## 7. Decisions deliberately left for runtime confirmation

P12e calibration remains limited to its exact recorded tested set. Whether
real-market confirmations or other lengths/family shapes/bootstrap counts
may execute, and whether each declaration needs its own predeclared simulation,
remain the recalibration plan §11's explicit P13 policy decisions. Budget
storage does not settle or bypass them. No statistical PASS, new alpha
schedule, extra bootstrap samples or relaxed threshold is authorized here.

Validation/Test exposure identity, standardized-instrument period overlap,
legacy manual exposure and prompt filtering also need their P13c contract.
They cannot be inferred from reservation IDs or dataset hashes alone.

## P13c follow-up (2026-10-16)

After complete alpha transfer merged as PR #185, the
[confirmation/exposure/recovery contract](research-confirmation-exposure-recovery-plan-v1.md)
defines those remaining authorities and separates the pure-document, Train-only
context, exposure storage/transfer, final admission and dormant execution/UI
slices. Registry spend still commits before workspace enqueue. Production
activation additionally requires complete manual-exposure mediation and an
explicit statistical-policy decision; P13b evidence alone grants neither.
