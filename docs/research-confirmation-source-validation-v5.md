# Complete witnessed v5 source validation

P13c-3b-2b-1 extends the [frozen export contract](research-confirmation-export-v5.md)
from merged #203. `TrialLedger::validate_witnessed_archive(bytes)` is a backend-only
validation entry point with no receiver argument, command, UI or execution right.
Successful validation does not import, qualify, adopt or authenticate a source.
Atomic destination union remains the next implementation section.

The decoder compares the original header and each original JSONL record with
their closed canonical representation before normalization. Duplicate fields,
including fields inside dynamic record bodies, are refused. It checks exact
version/schema/source identity, the nine closed counts, body-byte SHA, final LF,
section/ID ordering and uniqueness. The first record is exactly one embedded
complete v4 projection with the same source ID; its unchanged nested trial v3
and alpha data are independently validated. Legacy frames gain no v5 authority.

Inclusive limits remain 256 MiB for the encoded file, 4096 bytes for the header,
128 MiB for decoded embedded v4 (including its existing 64 MiB nested v3 limit),
and 16 MiB + 4096 for subsequent encoded lines, excluding LF. The first line has
its own projection/file bounds. Original binding/statistics/source-witness blobs
retain exact UTF-8 text and SHA; no reserialization repairs them. Decode checks
joint row/byte/unknown limits incrementally; strict graph/SQL validation applies
all existing per-document/blob, 10,000 row/64 MiB and 128,000 interval bounds.

Source reconstruction runs only in a fresh private in-memory SQLite database
using the existing seven migrations. Exact source sequences/chains, protocols,
families, batches, events, receipts, checkpoints, genesis and restrictive rows
are staged directly. Ordinary import cannot serve this role: it rebases chains
and can skip conflicting primary rows. Staging refuses an occupied registry.
Exact alpha rows/stops are reloaded and compared before confirmation validation.

The supplied confirmation inventory is strictly validated against this actual
reconstructed source before insertion, without deriving missing conflicts or
unknown markers. The complete graph is strictly reloaded after deterministic
SQL projections/indexes are staged; foreign keys and exact inventory equality
are checked. Every original witness is independently reconstructed and its
entire historical source is accounted for, including foreign events and
event-less upgrades. Missing original restrictive evidence is not excused by
unknown markers. Valid unknown/conflicted archival sources remain readable.
The disposable transaction is dropped without a destination write or receipt.
Errors use static diagnostics and do not echo supplied source contents.

Validation proves internal consistency of supplied complete evidence. A fresh
hash cannot reveal a valid omitted suffix that no retained binding witnessed;
publisher/file authentication and offline joint-rollback limits remain as
specified by the [parent plan](plans/research-source-witness-storage-plan-v1.md).
No statistical policy, reserved-seed experiment or AI context changes here.

[Implementation order](plans/research-confirmation-transport-implementation-v1.md).
