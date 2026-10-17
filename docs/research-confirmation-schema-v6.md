# Confirmation registry schema 6: first storage slice

P13c-3a-2a implements the unchanged table/field/version/bound contract in
[the storage plan §3–5](plans/research-confirmation-storage-transfer-plan-v1.md).
The checked graph reader, projections, append/union writer and original-origin
lookup remain 3a-2b. No production caller or admission capability is introduced.

## Migration and append-only invariants

Append migration `0006_confirmation_history`; never rewrite 0001–0005. All ten
tables reject UPDATE/DELETE, and inserts cannot use SQLite REPLACE semantics
to change an existing row, including its first-observed audit time. Exact
idempotent duplicates do not increase capacity. Identity/graph verification
will additionally run in the 3a-2b reader/writer; SQL shapes are not authority.

Use the exact merged columns, deferred foreign keys, enums, safe integers,
lowercase ID domains, original UTF-8/BLOB lengths and interval time domains.
Apply combined 10,000 document/blob rows and 64 MiB; 128,000 interval rows,
128 per exposure; 10,000 unknown origins. Individual stored documents and
statistics blobs remain 4 MiB; binding blobs 8 MiB. Capacity failure rolls
back migration or the caller's entire transaction; no restrictive row is dropped.

The initializer writes immutable `confirmation_history_birth =
0006_confirmation_history` only when creating migration 0001 and registry ID.
Existing databases never receive the birth key. Migration 0006 marks an old
database's own origin and all known import/genesis/checkpoint origins unknown.
Fresh creation is distinct from unseen manual-history proof.

## Existing transport boundaries

Every public legacy v3/v4 archival import retains source and carried origins as
confirmation-unknown in its existing transaction, including empty archives.
Complete alpha v4 additionally carries alpha reservation/conflict/unknown origins.
The marker has no clearing API. A late failure rolls back trial, alpha and
confirmation provenance together.

Public v3/v4 exporters refuse any confirmation record/blob/interval/conflict/
unknown table contents. Internal old-format projections remain available for
the future complete v5 writer in the caller's snapshot. Tests of old transport
internals use an explicit test-only projection while new public-boundary tests
assert refusal; production callers cannot select a bypass flag.

Qualification of a legacy archive cannot establish known confirmation history.
Existing more-specific alpha-provenance failures retain their priority, then
legacy qualification refuses before writes. Ordinary archival import remains
available and preserves restrictive history. Full v5 qualification/transport
remains 3b-2 and this transport check grants no execution permission.

## Verification

Use real independent SQLite connections/reopen for every append trigger,
deferred FK, field/domain/capacity/index definition, first-observed replacement
refusal, fresh/old creation, old origin backfill and migration rollback.
Test legacy empty/nonempty/duplicate/two-hop archival import markers, all public
omission guards and late-failure atomicity. Keep old trial/alpha regression
coverage while updating assertions for the deliberately narrowed public API.
No bootstrap, calibration or supported statistical-policy change is involved.
