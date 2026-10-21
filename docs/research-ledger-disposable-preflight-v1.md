# Disposable registry preflight snapshot

P13c-3b-2b-2a adds the internal `preflight::with_snapshot` substrate from merged
#204. It prepares a complete proposed-state check before the first destination
write. This section has no import/command/provider/UI caller or admission right.
The [implementation order](plans/research-confirmation-transport-implementation-v1.md)
keeps actual atomic v5 union in the next bounded section.

The caller validates original source/current evidence and borrows its actual
fixed SQLite transaction (immediate for final import). The helper streams local
schema and typed rows directly from that transaction into an owned private
temporary disk database. It does not re-export older primary history through
v3/v4 size limits or allocate the entire database in memory. Per-row buffers and
schema metadata are private; archive-supplied SQL is never executed.

All registry tables, row IDs, original document/blob/audit bytes, receipts,
checkpoints, restrictive rows and autoincrement high-water marks are copied.
Regular and WITHOUT ROWID tables use their actual column/primary-key metadata;
names are quoted and values bound. Indexes/views/triggers are recreated after
data, with the actual recursive-trigger setting. SQLite optimizer statistics
are local non-evidence and excluded. Foreign keys are enabled and checked before
the proposal callback, including cyclic confirmation dependencies.

The callback receives only the disposable transaction. The next section must
run its shared complete union and strict final graph/capacity checks there.
The helper checks deferred foreign keys again, rolls back the private proposal,
closes the database, and removes its owned directory before returning owned
results. A callback that prematurely commits is refused. The real transaction
is not modified, committed or released; independent writers remain fenced.
Caller-level trial/alpha/confirmation validity and quotas remain mandatory.

Temporary names use OS randomness and atomic creation under the canonical temp
directory. Unix directory/file modes are 0700/0600; Windows inherits the user's
temp ACL. Cleanup verifies the resolved absolute directory still equals the
owned path under that base before recursive removal. Callback/copy/FK errors
explicitly close and clean; unwinding has an owned Drop backstop. Cleanup failure
returns an error, so no real apply may follow. A process crash can leave private
temporary files; this slice adds no startup scavenger. SQLite still rolls back
the original transaction on process exit.

Verification includes actual full schema-7 witnessed graph/SQL reconstruction,
all-table/audit equality and original blobs/foreign history/stops, sparse row IDs,
sequence greater than current MAX(id), original WAL/uncommitted snapshot, writer
serialization, copied triggers/views, late private union failure, deferred FK
failure, callback error/panic/early commit and Windows locked-file cleanup failure.
No schema/dependency, statistical policy, reserved-seed or AI-context change.
