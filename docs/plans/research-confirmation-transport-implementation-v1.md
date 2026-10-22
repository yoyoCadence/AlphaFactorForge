# Complete witnessed archive v5 implementation order

Date: 2026-10-21
Baseline: merged #202 (`f4b0cd7`).
Authority: [storage/transport §4/7](research-confirmation-storage-transfer-plan-v1.md)
and [witness amendment §7/8](research-source-witness-storage-plan-v1.md).

Keep P13c-3b-2 in two ordered stages, delivered in bounded PRs before the next begins:

1. **3b-2a: complete canonical export.** Add a backend-only v5 exporter with
   one deferred snapshot for actual v4 trial/alpha projection and the complete
   strict witnessed graph. Exact schema-7 header/nine counts, prescribed section
   ordering, all three original blob kinds/text/SHA, permanent unknowns and final
   newline. Bound encoded output while serializing: 256 MiB file, 128 MiB embedded
   v4 text, 4096-byte header, 16 MiB + 4096 other lines; escaping counts. Keep
   v3/v4 public omission guards. No importer, command/provider/UI activation.
   Verify independent Node byte/hash oracles, real managed/manual/witness/unknown
   exports, original bytes/audits, corrupt/incomplete graph refusal, concurrent
   independent snapshot/reopen and exact/one-over encoded bounds. Deliberate
   mutants must fail assertions, then be restored before the complete suite.
2. **3b-2b: strict input and atomic union.** Strict canonical original v5 frame
   parsing, nested v4 source validation and independently reconstructed complete
   source graph precede destination mutation. Reuse caller-owned trial/alpha
   helpers under one immediate transaction, preflight current/proposed state and
   capacities, retain all independent claims/stops/original blobs, derive required
   monotonic conflicts/markers, strictly reload and commit once. Test two hops,
   replacements, duplicate replay, qualification provenance, every resource/file
   guard and late all-table/receipt rollback. No execution/admission capability.

The first export API does not imply a released end-user import/export workflow.

## 2b implementation refinement (2026-10-21)

After merged #203 (`1958e3d`), deliver 2b in two bounded sections:

- **2b-1 strict source validation:** closed canonical original v5 decoding,
  inclusive encoded/decoded limits, exact source/count/hash/order checks, and
  full nested v4 trial/alpha validation. Materialize the exact parsed source
  chain, protocols, receipts, restrictive evidence and alpha rows in a private
  disposable in-memory SQLite database with the existing seven migrations.
  Stage the supplied confirmation inventory without completing conflicts or
  inventing unknown markers; independently reconstruct witnesses and strictly
  reload the complete graph/projections there. No destination is opened or
  locked. A backend-only validation API grants no import/execution rights.
- **2b-2 atomic destination union:** consume the independently validated source,
  preflight actual and complete proposed union before the first destination
  write, then one immediate transaction for trial/alpha/confirmation and final
  strict reload. Preserve the frozen two-hop/replacement/replay, provenance,
  capacity and late rollback requirements above. Record the precise disposable
  proposed-state staging mechanism before implementing it; no destination
  savepoint writes may stand in for preflight.

Each section gets its own verification, draft PR, six exact-head CI checks and
guarded merge before proceeding. This refines granularity, not wire/policy.

## Destination preflight mechanism (2026-10-21, after #204)

Deliver **2b-2a disposable destination snapshot** before **2b-2b atomic union**.
2b-2a adds only an internal transaction-scoped substrate; no import API or live
caller. 2b-2b will use it to run the same complete trial/alpha/confirmation apply
and strict final checks privately before the first real destination write, then
apply and commit once under the original immediate transaction.

Clone directly from the caller's borrowed, fixed SQLite transaction into an
owned private temporary disk database. Stream typed cells row by row; preserve
schema, all registry tables, explicit row IDs, original audits/import receipts,
checkpoints/stops and SQLite's autoincrement sequence. Recreate indexes/views/
triggers after data copy; check foreign keys before invoking the private proposal
closure. SQLite optimizer statistics are local non-evidence and need not travel.
The original transaction stays open and untouched. A temporary copy avoids adding
v3/v4 export limits to already larger primary destination history and avoids
allocating a copy of the entire database in memory. Only actual local schema SQL,
never archive-supplied SQL, is executed in the private database.

Use a random atomically created owned temp directory (0700 on Unix, inherited
user temp ACL on Windows) and private file (0600 on Unix). Explicitly roll back
the disposable proposal, close the connection and remove the owned directory
before returning a value. Failure/panic must also drop owned resources; cleanup
failure returns an error before any real write. Do not use destination savepoints
as preflight. No dependency/schema version change is needed. All source/union
capacity checks and final strict graph reloads remain the caller's responsibility.

Tests must prove complete original row/blob/audit/sequence/schema preservation,
real strict graph loading, independent writer serialization and WAL snapshots,
private trigger enforcement, callback/FK/clone failures, panic/normal cleanup and
unchanged source transaction/changes. Deliberate mutants and full checks precede
the usual exact-head PR/CI/merge, then continue 2b-2b immediately.

## Atomic union implementation (2026-10-22, after #205)

Use new backend-only archival and provenance-checked v5 import methods. Parse and
independently reconstruct the complete source before opening the receiver mutex.
Under one real IMMEDIATE transaction, strictly read actual current witnessed/
trial/alpha state, then run shared caller-owned trial/alpha/confirmation apply
and final checks through the disposable snapshot. Return its expected owned
summary/graph only after private rollback/close/cleanup. Apply the same logic to
the original transaction, compare the final summary/graph, trial head/restrictive
evidence and alpha identities/provenance, and commit once.
Record the full original outer-v5 SHA in the existing trial import audit.

Extract only the existing alpha transaction prepare/apply logic needed for reuse;
preserve every legacy v3/v4 omission/qualification/provenance behavior. v5 never
adds legacy unknown markers merely because it contains an internal projection.
Its witnessed union still derives required permanent gap markers/conflicts and
retains all independent original claims/proofs/audits/stops. Current readers keep
their actual local-history semantics; do not reinterpret legitimate local
non-effective benchmarks as portable archives or impose an export-size bound on
pre-existing primary history.

The qualification-named method proves transport coverage only. Before any real
write, source, actual destination and complete proposed union must each have
known trial families and no unincorporated original-witness trial coverage,
including gaps already blocked by actual conflict/quarantine evidence; no alpha
unknown origins; and no confirmation unknown origins. Add the focused
`trial_history_unknown` reason alongside existing alpha/confirmation reasons.
Do not use `is_restricted` as an unknown-provenance test: valid fully accounted
conflicts/stops remain transportable and never authorize actual eligibility or
execution. Workspace legacy exposure/trial uncertainty, owner/count/share/file
authentication and final admission remain P13c-4 and later barriers.

Verify original managed/manual/empty/unknown/conflicting histories, two hops,
replacement/replay, exact original blobs and first audits, old API guards,
qualification source/current/proposed refusal, source-before-lock validation,
independent writer serialization, all joint capacity guards and late actual-only
all-table/checkpoint/import-audit rollback. No source proof may borrow missing
dependencies from the destination. Mutants/full checks and the normal guarded
PR/CI/merge delivery precede the next task.

Final admission/actual owner-count-share/file authentication remains P13c-4.
Do not change statistical policy, run new calibration/reserved-seed experiments
or expose witnesses to AI context. Existing committed-report reproductions stay.
