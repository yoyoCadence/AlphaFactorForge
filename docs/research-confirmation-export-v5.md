# Complete witnessed registry export v5

Date: 2026-10-21
Scope: P13c-3b-2a, following merged #202.

`TrialLedger::export_witnessed_json_lines` is a backend-only complete snapshot.
It does not expose a command, an importer, source authentication, admission or
execution rights. The strict importer/atomic union is the next bounded slice.

## Snapshot and format

One deferred SQLite transaction first reads the complete strict witnessed graph
and actual trial/alpha/provenance context, validating original proofs, SHA/bytes,
all scalar/interval projections and graph dependencies. The same transaction
builds the existing internal v4 trial/alpha projection. An independent writer
cannot split the trial, alpha and confirmation observations into different times.
Corrupt/incomplete history is refused; an unknown marker cannot replace a proof.
Export never appends witnesses/stops, repairs rows or changes original audits.

The canonical UTF-8 JSON Lines header has exactly `version`, `registryId`,
`schemaVersion`, `exportedAt`, `counts`, `bodySha256`. Version is
`trial-ledger-export-v5`, schema is `0007_confirmation_source_witness`; counts
are the nine existing batches/selections/operations/exposures/admissions/events/
blobs/conflicts/unknownOrigins totals. The digest covers exact body bytes,
including every newline, and the archive has a final newline.

Body order is one `alphaLedger` containing complete canonical v4 text, then
`confirmationBatch`, `confirmationSelection`, `confirmationOperation`,
`confirmationExposure`, `confirmationAdmission`, `confirmationEvent`,
`confirmationConflict`, `confirmationBlob`, `confirmationUnknownOrigin`.
IDs are strictly sorted within each section. Records use `{type,record}`;
blobs use `{type,sha256,kind,text}` and unknowns `{type,originRegistryId}`.
Blob kinds are `binding`, `statistics`, `source_witness`. Both physical blob
tables participate in the one logical blob section/count, preserving exact
original UTF-8 text and digests. Record wire objects are canonical semantic
documents; source database document text/first representation/audits stay intact.
No derived interval indexes or local SQL audits are transferred.

## Bounds and compatibility

The full encoded archive is at most 256 MiB; embedded v4 text at most 128 MiB;
header at most 4096 bytes; each body line after the projection at most
16 MiB + 4096 bytes, excluding its newline. The first projection line is instead
bounded by its decoded v4 limit and the full encoded file limit. The serializer
enforces byte ceilings while writing, so JSON escaping/nesting also counts.
Header/newline capacity is checked before publishing the final Vec; failures
drop private partial output, never truncate or publish a partial archive.
Strict storage retains its existing joint 10,000-row/64 MiB, 128,000 interval
and separate 10,000-unknown limits. No public caller can relax these bounds.

Valid restricted/manual/unknown histories remain exportable. Legacy public v3
and v4 exports still refuse any confirmation history rather than omit evidence;
the internal projection is not a known-empty-history assertion. No legacy
import/qualification guard changes, schema migration, policy expansion or AI
context change is part of this slice.

## Verification and next slice

An independent Node oracle freezes exact v5 bytes/digests using a real complete
four-event original-source fixture, with standalone proof/unknown and trial-only
cases. Rust verifies the original source chain and exact oracle outputs. Real
SQLite cases cover all nine nonempty sections/all three blob kinds, old guards,
original text/audits, corruption/incomplete refusal, independent concurrent
snapshots and reopening. Small private test ceilings exercise the same production
serializer at exact/one-over file/header/projection/line bounds and escaping.

Next: strict original input/source validation and one-commit trial/alpha/
confirmation union, including all contradictory claims/stops, two-hop/replay,
qualification provenance and late rollback. [Bounded implementation order](plans/research-confirmation-transport-implementation-v1.md),
[parent transport](plans/research-confirmation-storage-transfer-plan-v1.md),
[witness amendment](plans/research-source-witness-storage-plan-v1.md).
