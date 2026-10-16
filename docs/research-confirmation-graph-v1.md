# Pure confirmation admission/blob/graph validation (P13c-3a-1b)

This completes the pure half of the
[schema-6 contract](plans/research-confirmation-storage-transfer-plan-v1.md),
using the [single-record codecs](research-confirmation-records-v1.md).
It adds no IO, database schema, engine execution, provider or runtime permit.
The persistent wire fields/versions remain those of the merged contract.

## Inputs and authority

The pure inventory contains original stored batch, selection, operation,
exposure, admission, event and conflict texts, original statistics/binding
blobs and unknown origins. IDs are unique within each typed section. References
resolve against their required record kind, never a generic hash dictionary.
Ordinary document/blob/interval/unknown bounds are exactly the storage design.

Separate typed host observations supply complete registered sources, alpha
declarations/reservations/restrictive identities and historical trial count/
prefix observations. Revalidate their domains and compare receipts against
them. Historical observations correspond to the receipt's registry prefix;
they are not replaced with today's larger family count on archive read.
P13c-4 still must derive live admission from the **current** database inside
the final transaction. A caller-invented or consistently truncated observation
cannot be authenticated by a pure function; checked graph values grant no
freshness, publisher authenticity or execution rights.

## Admission and original blobs

Admission binds operation, alpha, exact exposure and the original pre-spend
binding-v5 bytes. Reproduce its existing nine-field P12a plan from actual share,
frozen samples/cap/error and supplied whole-family/original registered-batch
counts. Compare every integer/enum proof field with unchanged P12a output and
require ELIGIBLE. Selected Test membership never reduces the registered batch
or family m. No existing float report is normalized into a new identity.

Binding blobs are at most 8 MiB and statistics blobs 4 MiB, hashed from exact
UTF-8 bytes. Strictly parse binding-v5 fields, sorted unique arrays and dynamic
family-map keys, including duplicate-key refusal. Preserve all trial/alpha/
confirmation evidence fields. Pre-spend binding identities must be covered
by the supplied graph/host history and may not reference the new exposure,
admission or future events. Existing local/imported prefix proof remains a
host observation; seq-zero genesis can also be checked directly.

Seals require actual matching admission/start/report records and the frozen
engine. Reuse P13c-1's integer report verification for both Validation and Test,
choosing each stage's original settings/bar count and requiring the admission's
exact whole-family m. Selection requires the actual Validation seal, original
report/reservation and exact frozen rule; Test requires that real selection.
Original report bytes, including display floats and whitespace, are retained.
Opaque result files are not read or claimed present by this module.

## Closure, contradictions and conflicts

Resolve the typed dependency graph iteratively. Missing/wrong-kind references,
cycles, unmatched confirmation exposure/admission halves, false claim relations,
incorrect target sets and unused original blobs are invalid. A saved binding's
coverage list alone cannot make an orphan blob used: an actual admission or
seal must directly reference each binding/statistics blob. Pre-admission
batches/operations and manual/unknown occupancy may stand independently.

Independently valid contradictory history is retained with actual conflict
witnesses, rather than dropped as an invalid union. Group operation-key reuse,
multiple admissions/starts/seals and conflicting terminal claims by their
actual operation/key. Use a deterministic representative and preserve all
previous valid witnesses. A no-start terminal claim conflicting with an actual
start, or terminal versus seal, creates a terminal conflict; it never refunds
spend or permits restart.

For time overlap, interval intersection is not an equivalence relation: a
chain A/B/C may have A disjoint from C. Use a deterministic interval sweep
and choose actual overlapping witnesses, without a quadratic all-pairs list
or invalid pairs with a non-overlapping representative. Confirmation claims
of the same logical operation are not independent overlap claims. A checked
manual-after-seal claim is exempt only for the exact containing exposure;
unmediated/legacy intersections remain restrictive. Manual/manual overlap
does not create a confirmation conflict.

Derive targets solely from referenced claims. Manual/legacy claims have no
family reference; their instrument target remains restrictive even if a family
is registered later. Do not reinterpret old immutable conflict targets using
newly created families. Every supplied conflict declaration must prove its
kind and exact targets. Preserve valid older witnesses even if the current
representative changes.

Completion returns a bounded canonical inventory including required missing
conflict witnesses for a proposed union. Materialized validation additionally
requires those witnesses already stored; a reader cannot silently overlook
missing restrictive rows. Both paths validate all supplied declarations first
and apply proposed-union capacity checks before returning. The future writer
must persist the completed result in its one transaction and validate it again.

Unknown provenance and valid conflicts are readable archival restrictions,
not corrupt empty history. Returning a checked graph with restrictive status
does not authorize qualification or recovery execution. Train context remains
its separate closed allowlist; none of these protected records may be passed
to it as generic reports or hashes.

`ValidatedGraph::is_restricted()` reports global archival conflict/provenance
flags only. False does not clear any manual/legacy/confirmation time occupancy
or grant admission. The complete returned inventory retains that geometry.

## Verification

Use an independent byte-preserved Validation-to-seal-to-selection-to-Test
fixture, integer P12a/report calculations and actual conflict witnesses.
Test forged/missing/reordered records and dependencies, stale/wrong counts,
all proof/settings/alpha/selection bindings, raw-byte changes, binding coverage,
dynamic duplicate keys, cyclic toy dependencies, contradictory histories,
same-key and temporal witnesses, legacy markers, exact capacities and static
diagnostics. No bootstrap or reserved final-seed simulation. Storage is next
only after this complete pure slice merges.
