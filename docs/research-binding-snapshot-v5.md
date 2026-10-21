# Pure binding v5 snapshots and confirmation coverage

P13c-3b-1a implements the format foundation of
[binding v5](plans/research-confirmation-storage-transfer-plan-v1.md#6-binding-v5).
It reuses the original-text parser already used by confirmation binding blobs.
It does not switch the workspace reader/writer, observe a database or complete
P13c-3b-1. The runtime still reads/writes its existing v1-v4 contract.

## Immutable format checks

`parse_binding_snapshot` returns `BindingSnapshot` with private fields and
read-only getters. Original JSON goes directly through the shared strict
decoder: duplicate fields, unknown/missing/null fields, duplicate or unordered
dynamic family-map keys and sets are refused before normalization. Prefixes,
genesis, lowercase IDs, family identities and safe integers retain the existing
graph-blob checks. Diagnostic errors are static and do not disclose inputs.

The complete original UTF-8 input is bounded to 8 MiB before deserialization.
Trailing whitespace consumes capacity. Legacy workspace formats retain their
4 MiB bound; this pure v5 API accepts no legacy/unknown versions. It cannot
turn unknown legacy confirmation evidence into empty observed sets.

Confirmation has nine required sorted unique arrays:

| Section | Identity | Capacity |
| --- | --- | --- |
| batches, selections, operations, exposures, admissions, events, blobs, conflicts | Original 64-digit record/blob ID, separately per section | 10,000 jointly |
| unknownOrigins | Original 32-digit registry ID | 10,000 separately |

Alpha budgets/reservations/conflicts retain their joint 30,000-identity limit
and alpha unknown origins their separate 10,000-origin limit. Trial evidence
and alpha fields remain mandatory and are exposed without mutable access.

## Serialization and coverage

`to_canonical_json` sorts all object keys recursively and writes compact JSON
with no trailing newline, checking the encoded limit. It serializes a new
binding observation only. Existing `HistoryBlob` text, digest and whitespace
remain original evidence and must not be replaced by this representation.
The Node generator independently freezes original/canonical bytes and SHA-256;
Rust and TypeScript verify the same fixture.

`covers_confirmation(saved)` checks each of the nine saved identity sets as a
subset of the corresponding current set. Growth is allowed; loss, equal-sized
substitution and moving an ID to another section fail. Sorted merge traversal
does not require a second allocated set. The result covers confirmation IDs
only: different registry IDs can legitimately retain the same original IDs.
Prefix/replacement, trial/alpha evidence and provenance acceptance remain
separate host checks.

A format-valid ID need not exist. A snapshot does not verify a graph, publisher,
artifact, historical count, source completeness, freshness or ownership. It
grants no admission, qualification, recovery or execution rights. Before using
coverage, 3b-1b must observe the complete checked current registry in one
transaction; raw SQL IDs and a valid snapshot cannot replace that observation.

## Integration prerequisite

The [storage reader](research-confirmation-storage-v1.md) requires independently
supplied historical GraphContext observations. Workspace reopen does not retain
that caller context. Trial import also skips incoming conflicting-family rows
and withholds incomplete/conflicted origin checkpoints. A valid contradictory
source branch can therefore exist outside the receiver's primary trial rows.

For example, a receipt at source prefix N can correctly record four family
trials while the receiver now has five. Using five rewrites history; copying
four from the receipt into a context does not independently verify it. Requiring
the receiver's primary branch also discards valid conflicting source history.
Full source metadata/artifact claims need the same distinction.

3b-1b must specify how independent historical context remains available across
reopen and complete transfer before activating observation/adoption. It must
preserve readable valid unknown/conflicted archival history, fail invalid state
without changing the last good binding and keep live actual-count authority
inside P13c-4's final transaction. This foundation does not silently choose a
receipt-derived context, introduce a new witness format/schema, or activate a
partial observer.

## Verification

Tests cover every closed object level including budget entries, original-text
duplicates and ordered dynamic maps, all identity domains, safe integers,
genesis, exact/one-over original bytes, joint confirmation/alpha capacities and
separate provenance capacities. All nine sections match an exhaustive 32-by-32
independent bit-subset oracle (9,216 comparisons), plus cross-section and
replacement-boundary cases. A real workspace-SQL test verifies that the dormant
codec does not enable v5 in the runtime reader/writer or enlarge legacy bounds.
No calibration, bootstrap or protected execution is involved.

## Runtime integration (2026-10-21)

P13c-3b-1b-3 activates the workspace v5 reader/writer and complete transactional
observer after durable independent historical context was delivered in #201.
The format-only boundaries above remain; see the
[actual observation/adoption contract](research-binding-observation-v5.md) for
strict graph/context checks, legacy unknown handling and last-good preservation.
Complete archive v5 transfer and final admission capture remain later work.
