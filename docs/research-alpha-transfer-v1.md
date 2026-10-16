# Complete alpha transfer v1 (P13b-2b)

This implements the transfer slice of the [persistence plan](plans/research-reservation-registry-plan-v1.md).
No production confirmation, statistical policy or data-reveal permission is added.

## Wire contract

`trial-ledger-export-v4` is canonical UTF-8 JSON Lines with a final newline.
The header contains exactly `version`, `registryId`, `schemaVersion`,
`exportedAt`, `budgetCount`, `reservationCount`, `conflictCount`,
`unknownOriginCount`, and `bodySha256` (SHA-256 of every exact body byte).
Schema is `0005_alpha_history_provenance`. Body sections, in order:

1. One `trialLedger` record, with `export` holding the complete canonical v3
   trial projection as a string. Its source ID must equal the outer ID.
   It is an internal projection, not evidence that alpha was empty. Reusing
   the validated trial format preserves its identity/checkpoint contracts.
2. `alphaBudget`: `familyId`, `allocationId`, `declaration`, in family order.
   These are immutable primary rows; all alternative declarations remain
   embedded in permanent budget-conflict evidence as well.
3. `alphaReservation`: `record`, in reservation-ID order.
4. `alphaConflict`: `conflictId`, `familyId`, `body`, ordered by that pair;
   every affected-family copy must be present.
5. `alphaUnknownOrigin`: `originRegistryId`, in origin order.

Every record has the `type` tag and exactly the fields above. Sections and
their identities are strictly sorted and duplicate-free. Canonical equality,
counts, hashes, families, declarations/shares, all trial evidence, complete
conflict copies and every conflict's exact reservation-row references are
validated before touching the destination database.
Unknown fields, duplicate fields, missing final newline and unsupported
formats are errors. A valid unseen suffix omission with recomputed hashes
cannot be detected without prior evidence: files do not authenticate origins
or prove a publisher disclosed all of its offline history.

Bounds: total file 128 MiB, embedded trial projection 64 MiB, header 4096
bytes, alpha record line 1 MiB + 2048 bytes. Existing per-document and total
alpha storage limits remain 262144/4096/1048576 bytes, 10000 rows and 16 MiB
JSON. Unknown origins have a separate 10000-entry bound. The output is refused
if any bound would be exceeded; history is never truncated.

## Union and snapshot authority

Export reads trial, alpha and unknown provenance in one SQLite transaction.
Import validates the full input first, then acquires the connection mutex and
one IMMEDIATE transaction, validates current state and unions trial plus
alpha evidence before committing once. Trial helpers do not acquire nested
locks or commit on behalf of their caller. Any later failure rolls back trial
events/checkpoints/import receipts, alpha rows and provenance together.

Exact identities deduplicate. Source origin/key/number/share/body never change.
An existing primary budget is never overwritten; another valid declaration
is retained in a permanent budget conflict. Reservations bound to either
validated declaration remain readable, but a budget conflict has no allocation.
New number/operation collisions preserve all records and add permanent stops;
repeated/two-hop imports preserve existing conflict IDs. Conflict generation
uses a deterministic representative per group, retaining every distinct claim
without requiring quadratic pairwise evidence. No alpha stop quarantines
ordinary trial exploration.

## Unknown legacy provenance and bindings

Migration 0005 adds append-only `alpha_unknown_origins`, backfilled from
existing import receipts and known imported origins. Every successful legacy
v3 trial-only import records its source and carried origins as unknown in
the same transaction. v4 archives carry this set transitively; neither a
later v4 export nor a complete import clears it. Old builds refuse schema 5.

Binding v4 preserves the complete v3 evidence plus mandatory sorted unique
`alphaUnknownOrigins`. v1/v2/v3 bindings have unknown provenance until verified
adoption; only v4 can be written. The 4 MiB total binding bound remains.
Missing saved origin markers is evidence rollback and leaves the last good
binding untouched. This provenance is distinct from legacy manual exposure,
which remains a P13c barrier.

`export_complete_json_lines` and `import_complete_json_lines` archive all
observed evidence, including unknown provenance. `import_qualification_json_lines`
requires v4 and refuses unknown source or destination provenance before any
write. `require_known_alpha_history` and alpha-family reads/reservation guards
also refuse to allocate against unknown provenance; a legacy import cannot
bypass the barrier by calling the earlier storage primitive. Ordinary trial
registration/adoption remain available. Passing these checks proves transport
history coverage only, never statistical eligibility or permission to reveal.

The existing `export_json_lines`/`import_json_lines` APIs stay explicitly
trial-only v3. Export still refuses nonempty alpha tables; every import through
that compatibility path records unknown provenance even when the file is empty.
Joint rollback of all registry/workspace evidence remains the offline limit.
