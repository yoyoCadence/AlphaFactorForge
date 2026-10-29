# P13c-4b-2c-4b original provenance/snapshot producer projections v1

Date: 2026-10-29 (UTC+8)
Baseline: verified #235 (`fd1660f`). Design/data only.

This refines the next ordered market evidence scope after the [registry
contract](research-original-market-registry-projections-v1.md) and actual4a
controls. Provenance producer controls4b follow this verified design; snapshot
producer controls follow4b in their own PR. Only then refine bounded whole-market
consumer APIs/work and implement them before trial. No source constructor or
runtime activation is specified here.

## 1. Original identity and retained projections

| Original producer | Identity bytes | Retained projection and exclusions |
| --- | --- | --- |
| [provenance::record_raw / record_hash](../../alpha-factor-forge/src-tauri/src/market/provenance.rs) | SHA256 of research canonical JSON containing exactly version=market-provenance-v1, instrumentId, interval, source, role, requestScope, rawSha256, rawByteLen, mediaType, retrievedAt, availableAt, accepted, rejectionReason, revisionOf. No domain prefix. | All original scalar/null columns, exact request_scope_json, raw checksum/length/path/media. ID/created_at/path are not identity fields. The identity JSON itself is reconstructed, not stored as a content_json column. Keep exact RFC3339 strings/offset spelling; instant comparison for validation does not normalize identity text. |
| [snapshot::build_snapshot](../../alpha-factor-forge/src-tauri/src/market/snapshot.rs) | SHA256 of research canonical JSON containing exactly version=market-snapshot-v1, instrumentId, instrumentContentHash, interval, datasetHash, priceBasis, calendarId, corporateActionVersion, costProfileVersion, kind, sorted sourceRecordHashes. No prefix. | content_json and all scalar/null columns, exact coverage_json, complete source relations. asOf, row IDs, status and coverage are outside identity. The original admitting as_of/coverage remain stored when a later-cut retry returns Existing. |
| [ArtifactStore::put_with_extension](../../alpha-factor-forge/src-tauri/src/research/artifacts.rs) | SHA256 of exact raw bytes. Kind=market-raw-v1, length in bytes; relative path is first two checksum hex characters + / + checksum + .extension. | JSON/csv/zip/plain use json/csv/zip/txt. Extension is a path property, not a checksum input. A matching filename is verified before reuse. Exact retry still verifies/recovers its artifact. |
| [dataset_content_hash / repository import](../../alpha-factor-forge/src-tauri/src/identity.rs) | Existing dataset-content-v2 codec: version UTF-8 + zero, then u32be-length strings for the original field mapping/exchange/symbol/interval, i64be start/end, u32be count, then each i64be timestamp and five f64be fields. | Sorted unique candles, exact count/bounds and finite normalized numbers. Repository imports must verify frozen identity, rather than generate their own expected value. This is setup evidence for actual snapshot producers, not raw-body parsing or source/candle authentication. |

Research canonical JSON recursively sorts object keys and preserves array order,
nulls and original values under the locked legacy numeric policy. It is distinct
from identity::canonical_bytes. Frozen dataset values are small dyadic numbers;
there is no float parser change, repair, dual hash, silent rehash or new numerical
execution proof. The [numeric audit](../numeric-json-identity-audit-v1.md) remains
binding.

## 2. Existing behavior, including characterization boundaries

record_raw validates the existing instrument-id/interval rules, nonblank source,
accepted/rejectionReason consistency, parsed RFC3339 instants with available<=
retrieved, and the four supported media types. requestScope is an arbitrary Value,
not necessarily an object. Rejected and comparison retrievals are retained.
The writer does not parse/validate the raw body's claimed media contents. Every
fixture body here is synthetic, including the zip-tagged identity bytes.

Exact record-hash retries are resolved before fork checks, even after a revision
has a child. New revisions require an existing target in the same instrument/
interval/source and a single child per target. Earlier records/artifacts remain
unchanged. Row IDs in revisionOf are original identity inputs; never renumber
them or substitute a current head. No new chronology rule is introduced.

Snapshot building verifies current stored dataset candles/identity/admissibility
before a transaction or Existing return. It selects the latest instrument at
BUILD time, then freezes that exact content hash/row. A later historical consumer
must resolve the snapshot's captured revision, never replace it with latest.
Complete components must be accepted, current primary records of the same series.
Comparison/rejected/superseded/missing/mixed-publisher components block, including
a late invalid member after a valid one; do not filter the family into success.
Binance archive/rest endpoints share an origin under the existing foundation rule.

Forward-observed requires known component availability <=cut. Binance archive
availability additionally requires availabilityBasis=observed-at-retrieval and
availableAt/retrievedAt equal as instants. An old period guess remains unknown.
The positive offset-spelling control deliberately compares equal instants while
preserving distinct original identity strings. All observed bars must be closed
by the cut. Unknown cost version degrades any snapshot; unknown action version
degrades ETFs. Demo never qualifies. Declaring a version can make the EXISTING
writer report ok; that is not immutable authenticated action/cost evidence.

Blocked returns append their complete quality events atomically and create no
snapshot/source relations. Missing row errors occur before writes; duplicate
component IDs currently reach the relation primary-key constraint and roll back
the whole transaction. Do not mischaracterize that as a producer validation rule
or silently deduplicate the identity. Existing immutable rows stay unchanged.

## 3. Independent frozen fixture and exact setup

[original-market-source-projections-v1.json](../../alpha-factor-forge/fixtures/research/original-market-source-projections-v1.json)
is independently authored using Python json/hashlib/struct without importing or
executing project code. Separate Node built-ins reconstruct every fixed JSON
identity, scalar/raw projection, signed integer/double dataset frame, revision
sequence, snapshot/source/status/coverage projection and refusal phase. These
finite artifact checkers are not shipped replacement parsers or market engines.

Outer UTF-8 packaging SHA256 after physical CRLF→LF normalization:
`7e762b0c2493fc4a78b6608cb04a78af6ef668c4b28b9e83341056e1927bc0ef`.
Never normalize decoded inputJson/requestJson/contentJson or rawHex bytes. CSV
CRLF, Unicode text and opaque binary bytes are deliberate. The registry fixture
has its own immutable packaging pin; eight original source files pin #235.

Nine retrieval projections cover all four media types, primary/comparison,
accepted/rejected, null availability, observed/legacy archive availability,
arbitrary array scope and revision chain1→5→7. Twelve actual registration calls
include exact root retry after revision and retry of an already-revised child.
Thirteen refusals distinguish typed decode from record validation/revision checks.

Two datasets freeze complete synthetic crypto hourly and ETF daily candles,
metadata/hash and binary payload hex. Use the referenced registry literals for
calendar0/1, instrument0/1/2. In a fresh migrated in-memory DB, register calendars,
instrument0, import datasets as rows1/2 and complete provenanceSequence as rows1..9.
The writer permits recording ETF provenance before registering its instrument.

Execute eight snapshot calls in fixture order. The first historical snapshot
and its reordered-source/later-cut Existing retry bind instrument row1. Register
instrument1 as row2 ONLY before call3; it is a new BTC revision. Register ETF
instrument2 as row3 ONLY before call7. Compare seven unique content identities,
raw stored scalars/content/coverage, exact admitting as_of and complete relations.
The existing retry must leave all stored evidence unchanged. All5 crypto snapshots
have two relations, both ETF snapshots one:7 rows/12 relations, with2 degraded
events. Eleven subsequent snapshot refusals pin before/after counts: blocked
calls add one event each, ending at11; both error cases change no count. Also
compare complete pre-existing rows/relations/events, not counts alone.

These bodies do not encode a verified lineage from downloaded bytes to candles.
The repository imports/provenance writers and snapshot checks are mechanical
producer evidence only. There is no network, credential or protected Test input.

## 4. First code after delivery: actual provenance controls4b

Add a private cfg(test)-only child of market/provenance.rs, e.g.
provenance_projection_controls.rs; only that declaration may change the existing
file. Use fresh migrated in-memory SQLite and an exclusively allocated owned
artifact root. Compare actual record_hash to every frozen identity; call actual
record_raw for all twelve sequence entries. Read raw SQL to compare every retained
scalar/null, request_scope_json and raw reference; read the actual stored file
to assert exact bytes/checksum/length/extension/kind. Avoid lossy display defaults.
Repeated input bytes/typed Values, old rows/created_at, all related tables and
sqlite_sequence must remain unchanged on retries/refusals, except expected file
recovery. Check original chain1/5/7 and both directions of revision lookup.

For every refusal require the frozen decode/record phase and message fragment;
compare complete DB/artifact inventories before/after. Explicitly remove one
owned raw file and prove exact retry recovers it without a new DB row; then alter
owned bytes and prove retry refuses the checksum contradiction without rewriting
the row/file. This is controlled disposable IO, not future4b-3 authentication.
UPDATE/DELETE must fail and preserve rows. Drop all handles before owned cleanup.

Six actual-source mutants must assertion-fail: omit availableAt; ignore accepted;
erase revisionOf; replace requestScope with {}; move exact retry after fork check;
bypass artifact verification on exact retry. Target behavioral tests, excluding
source-pin tests; compiler failures are not evidence. Restore exact source bytes
after every run and keep failure logs before the full suite. No fixture changes.

## 5. Separate next code: actual snapshot controls

Only after verified4b delivery, add a private cfg(test)-only child of snapshot.rs.
Use the same frozen setup, actual repository import with FROZEN dataset hashes,
actual registry/provenance calls and actual build_snapshot. Do not extract a
production hash helper, synthesize SQL success rows or record current output as
expected. Compare all eight calls and eleven refusals, raw SQL/JSON, exact complete
source sets/hash sorting, old revision persistence, Existing as_of/coverage, kind/
status/qualification characterization and persisted events with correct dataset/
snapshot/provenance links. Blocked evidence must remain complete; error rollback
preserves all state and original inputs. Include immutable snapshot/relation
UPDATE/DELETE controls and a post-import candle tamper refusal before Existing.

Seven behavioral mutants must assertion-fail: omit instrumentContentHash; include
asOf in identity; omit source hash sorting; drop one source; filter rejected
components before validation; ignore superseded records; trust archive period
availability. Exclude source-pin tests and restore bytes/log every case. The
late-rejected family must catch filtering into a successful subset. No raw output
or version-only action/cost claim can become source admission authority.

Both code slices require full fresh exclusive GUID TEMP/TMP locked Rust BEFORE
Node-heavy checks, locked all-target check/Clippy, scoped format and frontend
checks. Allocate test-owned roots exclusively, never reuse/delete stale PID roots;
verify exact root/parent and every descendant's containment/no-reparse status
before cleanup. No dependency/helper/production refactor or manual server needed.

## 6. Delivery and remaining gates

This design changes only docs/unconsumed fixture/lifecycle receipts. Local checks
cover strict outer/nested JSON, every independent projection/frame/sequence,
source catalogs/pins, links/scope, all older fixtures/runtime bytes and append-only
#235 receipt. Do not rerun local runtime suites solely for these artifacts.
All six completed exact-head/base CI jobs still check unchanged #235 runtime:
1121 Rust/one existing ignored,1242 Vitest/80 files,98 E2E and native smoke.
Green design CI does not imply either future producer test/mutant suite ran.

Fresh reviews/main, normal expected-head merge, GitHub readback and clean-main/
full-tree equality complete delivery. Stop afterward. On a later explicit resume,
actual provenance4b controls are next, then separate snapshot controls. A bounded
whole-market consumer design must compose all parse/hash/copy/node/output costs
and feasible whole-operation boundaries before code; no per-row budget reset or
unchecked public constructor follows from this fixture. Trial, file/candle/
availability4b-3, separately supported action/cost originals and final actual-owner/
registry4d admission stay open. Market4 and all source/live parents remain incomplete;
RegisteredSource/numericCostsHash remain unavailable. No manual user test needed.
