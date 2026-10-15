# Alpha reservation evidence and history v1

Status: P13a pure integrity/conflict boundary. No production caller, registry
storage, alpha reservation, data exposure or confirmation PASS.

## 1. Purpose and authority

This implements the adopted [allocation §9.1 decisions](research-alpha-allocation-v1.md#91-maintainer-decisions-for-p13-2026-10-01):
verify family/declaration, reservation identity and confirmation numbering
before projecting history to the existing amount-only allocator. Exact
replay is deduplicated; independent reservations of the same confirmation
number stop the family and retain both records. Equal adjacent shares must
not hide that conflict (PR #134's counterexample).

`discovery_core::alpha_reservations` is pure Rust. A consistent history is
neither a reservation nor an admission permit. The caller must prove it read
the entire authoritative history, including failed and unfinished work, and
synchronize its last ledger/precision checks with an atomic reservation.
A caller that omits a valid suffix cannot be detected here.

## 2. Record

Stored evidence is an object with exactly `reservationId` and `body`:

| Body field | Domain and meaning |
| --- | --- |
| `contractVersion` | Exactly `research-alpha-reservation-v1` |
| `familyId` | Existing `trial-family-v1:` plus 64 lowercase hex digits |
| `allocationId` | Existing alpha declaration identity, 64 lowercase hex digits |
| `originRegistryId` | Permanent source trial-registry ID, 32 lowercase hex digits |
| `reservationKey` | Stable operation identity within that registry, 32 lowercase hex digits |
| `confirmationBatchId` | Content identity of a future frozen confirmation batch, 64 lowercase hex digits |
| `confirmationNumber` | Integer in [1, 999999], 1-based within this family's schedule |
| `alphaPpm` | Integer in [1, 999999]; must equal the schedule entry at that number |

`reservationId` is lowercase SHA-256 of the UTF-8 contract version, one zero
byte, then the existing type-tagged `canonical_bytes(body)` encoding. All
numbers are bounded below the exact IEEE-754 integer limit. Object key order
does not affect identity. Every body field is covered, including source and
operation identity: otherwise two independent reservations of the same
batch/number could be mistaken for an exact replay.

`freeze_alpha_reservation(body_text)` validates and freezes a proposed body's
identity; it generates no random value and writes nothing. The future
registry assigns/persists the stable operation key and verifies the actual
batch. `parse_alpha_reservation(record_text)` validates the stored shape and
recomputes the ID. Both parse text directly with strict structs: duplicate,
unknown and missing fields, floats, out-of-domain integers, unsupported
versions and noncanonical identifiers are errors. Checked records have
private fields and cannot be deserialized through a second unchecked API.

Neither a content hash nor a correctly shaped origin/batch ID authenticates
its source or proves a reservation happened. Mapping a family to its actual
instrument, verifying frozen batch bytes and trusting an imported registry
remain storage/admission responsibilities. This boundary never mints such
authority.

## 3. History review

`review_alpha_reservations(family_id, allocation_declaration, history)`:

1. Validate the selected family and the existing allocation declaration.
2. Require every checked record to bind to that family and allocation ID.
   Reject a number past the schedule or a different share; do not filter out
   mismatches or silently redeclare the budget.
3. Deduplicate only identical content IDs. Sort retained records by
   `(confirmationNumber, reservationId)`, independent of arrival order.
4. Detect multiple distinct IDs at one confirmation number, and multiple
   distinct IDs for the same `(originRegistryId, reservationKey)`. An operation
   cannot be reassigned to another batch, share or number. The same key in
   different registries is not itself an operation-key collision; a shared
   confirmation number still conflicts.
5. On any conflict return `BLOCKED`, all distinct checked evidence, stable
   conflict groups and **no allocation report**. Never pick a winner, move
   records to later numbers, delete evidence or recycle a failed share.
6. Otherwise require numbers to be exactly the contiguous prefix 1..k.
   Project their amounts in that order into `allocate_confirmation_alpha`.
   A used-up schedule is a `CONSISTENT` history with the allocator's
   `NOT_ELIGIBLE` / `alpha_budget_exhausted` result, not a conflict.

The report has version `research-alpha-reservation-history-v1`, family and
allocation IDs, status, duplicate replay count, retained evidence, conflict
groups and optional allocation report. Conflict groups are ordered first by
kind (`confirmation_number`, then `origin_key`), then by the sorted group
key; their ID lists are sorted. Binding/domain/gap errors produce an error
and no report. The caller retains the input evidence on errors too; an
error is never permission to discard or clear a stored reservation.

## 4. Remaining P13 work

P13b-1 adds [unwired registry storage](research-alpha-registry-storage-v1.md),
including permanent conflicts and serialized local writes. Identity-set
binding/complete transfer and final-fence/runtime admission are still pending.

The external append-only registry must persist one immutable budget per
family; reserve atomically; stop the family permanently on conflicts; carry
all reservation/conflict evidence through export/import; and extend workspace
rollback protection to that evidence. A truncated/restored history cannot
be trusted merely because this pure review says `CONSISTENT`.

Before any reveal, use the allocated alpha and the frozen sample count/cap
for P12a, and synchronize the current trial-ledger fence with reservation.
No samples, schedule or alpha may be changed after results are seen.
Batch/selection freezing, Validation/Test exposure identity and period
overlap, crash/retry recovery, allowed prompt assembly, execution, command
and UI surfaces remain separate slices. The calibration policy questions
in recalibration-plan §11 stay open; this module runs no statistic or
simulation and makes no new calibration claim.

## 5. Acceptance cases

- Empty, ordered/unordered prefix, exact replay and exhausted budget.
- Two independent confirmation-1 reservations with equal shares: amounts
  alone appear valid, but history is blocked with both records retained.
- Reused origin operation across different numbers/batches; registry scope.
- Arrival-order independent evidence/conflicts and no renumbering.
- Wrong family/declaration/share, missing prefix, invalid typed declaration.
- Every identity field covered; stored checksum tampering refused; strict
  duplicate/unknown/missing fields and integer domains.
- A deliberately truncated valid suffix still looks consistent, demonstrating
  why persistence, watermark and atomicity are mandatory future work.
