# Alpha registry storage v1 (P13b-1)

Status: Host-agnostic storage only. No production caller, confirmation
admission permit, workspace alpha binding, complete alpha transfer or reveal.
Implements the first slice of the
[persistence plan](plans/research-reservation-registry-plan-v1.md).

## 1. Authority and schema

Registry migration `0004_alpha_reservations` appends three tables to the same
external local SQLite trial registry. It changes no workspace migration,
trial identity/chain, old result, statistic or accepted calibration set.

- `alpha_family_budgets`: one known family, canonical existing allocation
  document and its content ID. Identical replay preserves the row; a changed
  declaration keeps the original and both declarations as conflict evidence.
- `alpha_reservations`: checked P13a record ID, bound family, canonical full
  record and first-recorded UTC time. No unique number/operation constraint
  may discard independent conflicting evidence.
- `alpha_conflicts`: canonical content-addressed conflict body and one row
  per affected family. Cross-family operation reassignment stops both.

All three tables refuse UPDATE and DELETE. Timestamps are local audit metadata,
not identities. Reads reproduce canonical JSON and content IDs, check family
keys/budget/number/share bindings, and verify conflict documents/kind/affected
family copies. A malformed row fails alpha reads; it never becomes empty history.

Alpha conflicts do not insert ordinary trial-family quarantines. Exploration
registration remains available even if alpha evidence is blocked or corrupt.

## 2. Storage APIs

`TrialLedger::declare_alpha_budget(family, declaration)` returns Stored,
Replayed or Blocked. The family must already exist in the trial registry;
unknown families and invalid declarations cause no write. Changed budgets
commit permanent conflict evidence rather than overwrite/restart the schedule.

`read_alpha_family(family)` reads a consistent SQLite transaction and the
complete bounded observed alpha evidence. No declaration returns None,
which is not a new budget or an admission permit. A configured family returns
its original declaration, every checked reservation, persistent/derived
conflicts and the existing allocator's report only when unblocked. Exhaustion
retains that allocator's NOT_ELIGIBLE result; conflicts have no allocation.

`reserve_alpha_evidence(family, operation_key, batch_id)` is an unwired backend
storage primitive. The key and batch ID are canonical 32/64 lowercase hex;
the source is this registry's permanent ID. It holds the connection mutex and
one IMMEDIATE transaction while loading all evidence and writing the next
number/share, so independent connections serialize. There is no status filter
to remove failed or never-started reservations.

An exact source/key/family/batch retry returns the original record, including
after later reservations exhaust the schedule. A changed operation keeps its
attempted body as conflict evidence and permanently blocks the affected
families. Within the same family it retains the original confirmation number,
even after exhaustion; it is not renumbered as a successful next reservation.
Cross-family reassignment uses the destination's next slot, or its first slot
if exhausted, solely as the preserved conflicting claim. Both source-operation
claims remain; a Blocked outcome grants no record/permit to execute.

Storage errors roll back the whole operation, including a proposed record
inserted before a failed conflict write. No enqueue, prompt, backtest,
Validation/Test read or statistics run happens here. This API does **not**
check campaign admission, a final trial fence or precision; P13c must integrate
transaction-scoped helpers with those checks before any production caller.

## 3. Bounds and integrity

Initial explicit resource bounds: budget JSON 262144 bytes, record JSON 4096
bytes, conflict JSON 1048576 bytes; at most 10000 total alpha table rows and
16777216 total stored JSON bytes per registry. SQL counts byte lengths before
loading JSON; writes check their added rows/bytes before mutation. Exceeding
a bound fails closed with no partial write or truncated history. These are
storage limits, not new statistical support or bootstrap restrictions.

Conflict identity is SHA-256 of UTF-8 `alpha-registry-conflict-v1`, a zero
byte and canonical JSON of exactly `contractVersion`, `kind`, `documents`.
The two distinct documents are sorted by their canonical JSON. Kind is
`budget`, `origin_key` or `confirmation_number`; bodies must demonstrate the
declared relation and affected families. IDs do not authenticate an origin.

## 4. Legacy transfer and downgrade

The complete alpha-aware transfer remains P13b-2. Until then existing v3
export **refuses any nonempty alpha table**, including a pinned budget, so
it cannot drop alpha evidence and claim a complete backup.

With empty alpha tables, v3's unchanged trial-only projection declares schema
`0003_family_protocol_upgrades`, independently of the physical schema-4 DB.
Existing v3 files remain readable and the pre-alpha trial transfer is preserved.
They never become P13 qualification/alpha-history proof; P13b-2 must enforce
the new complete format and extend the workspace binding before qualification.

Older builds reject migration 0004 through the existing unknown-schema guard.
No downgrade rewrites, deletes or clears evidence. Joint rollback of all local
evidence remains the ledger's stated offline limitation.

## 5. Acceptance and remaining work

Tests cover independent connections, exact replay after intervening/exhausted
history and reopen, changed budget, same/cross-family operation conflict,
ordinary exploration after alpha stop, all append-only triggers, invalid and
oversized inputs, aggregate row/byte limits, corrupted evidence after reopen,
rollback on injected storage failure and the legacy-export omission guard.

P13b-2 still owns binding identity sets and complete transfer union. P13c owns
frozen batch/selection, final trial/precision checks atomically with reservation,
exposure identity/overlap, crash/execution recovery, prompt boundaries and
runtime/native acceptance. Storage data never substitutes for those proofs.

P13b-2a follow-up: [alpha-aware workspace binding](research-alpha-workspace-binding-v1.md)
now observes checked budget/reservation/conflict identities in the same registry
read transaction and refuses loss while preserving the last good workspace
watermark. Complete alpha-aware transfer remains P13b-2b.

P13b-2b follow-up: [complete transfer](research-alpha-transfer-v1.md) adds
schema-5 legacy provenance, identity union and binding v4. Alternative budget
declarations retained in conflicts now validate their own reservations on
read; the original primary row is unchanged and a budget conflict has no
allocation. Unknown legacy provenance also blocks alpha allocation, while
ordinary trial exploration remains available. This supersedes the transfer
boundary above; storage writes still grant no production reveal permission.
