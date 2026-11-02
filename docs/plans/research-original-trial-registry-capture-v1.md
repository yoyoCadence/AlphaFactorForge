# P13c-4b-2c-5b-design-1 original trial registry rows capture v1

Date: 2026-11-02 (UTC+8). Baseline: verified #244 (`394dd10`).
This is the bounded first child of5b-design, not the completed trial consumer.

The [original producer contract](research-original-trial-projections-v1.md)
and delivered [5a controls](../../handoffs/2026-10-31-p13c4b2c5a-trial-producer-controls-v1.md)
remain binding. The new [capture catalog](../../alpha-factor-forge/fixtures/research/original-trial-registry-capture-v1.json)
freezes original SQL shape, fixed limits and finite recipes. It is unconsumed
conformance data; the application must not load it as a policy or source factory.

## 1. Missing boundary and ordered delivery

The sealed workspace graph contains an original admission batch_id and all
attempt trial_event_id links, but no registry payload/member/scalar closure.
Existing market controls intentionally use declared dummy batch/event IDs.
Preserve those immutable fixtures; a later coupled corpus must add genuine
original ledger links instead of relabeling them as existing proof.

ID-only ledger queries cannot supply this closure. export_trial_in invokes
global verify_chain before its output cap, re-reads/parses/hashes all events and
batches, and omits original scalar/audit mirrors. It is not a bounded raw reader.
check_binding_in additionally reads restrictive/alpha/unknown-origin/v5
confirmation graphs; it belongs to the later current observation/fence scope.
An active foreign transaction with the same registry ID proves no physical
custody. open/open_existing perform setup/migration and are not capture steps.

Delivery order is capture contract, separately scoped capture implementation
and actual SQL/producer controls, then coupled original vectors and the full
trial consumer's joint work contract before consumer code. Parent5b-design and
trial/source/live tasks remain open. No RegisteredSource construction or runtime
activation follows from this child.

## 2. Sole capture entry and immutable result

The future private research API is a zero-argument method on an already open
TrialLedger, named capture_original_trial_rows. It returns an immutable
OriginalTrialRegistryRowsCapture through the existing LedgerError result type.
No path/Connection/Transaction/selector/callback/rows/parts/count/cap/protocol
argument, importer/exporter, arbitrary JSON constructor or test-only runtime
escape hatch is accepted. No frontend command or research/runtime re-export.

Acquire that handle's actual connection mutex. Require its connection to be in
autocommit before beginning one owned DEFERRED transaction; refuse an already
active caller transaction without ending it. Pin the snapshot at its first
schema/count read. Complete all schema/count/data reads inside that same
transaction, finish only that read transaction and release the mutex on every
path. Never trust cached ledger.integrity as fresh row validation. A caller's
separate workspace transaction/lease remains untouched. Do not reenter the
method while holding the same mutex.

The result owns the fixed typed table rows and structural usage only. Fields
and row constructors stay private; ancestor research getters borrow immutable
slices/str/OriginalJson and copy integer/null values. No Debug/serde/Default,
whole-capture Clone, mutation, unchecked/from-parts constructor or authority
conversion. The registry ID is borrowed from the retained unique metadata row,
not allocated again; no unseen binding/count/empty-history flag is invented.

Reuse the existing [OriginalJson](../../alpha-factor-forge/src-tauri/src/research/confirmation_inputs/original_json.rs)
syntax carrier through only the necessary narrow pub(in crate::research)
visibility on its module/type/existing accessors. Preserve all implementation,
limits/errors, opaque numeric tokens and original callers/tests. No duplicated
parser, numeric-policy flip, source graph visibility widening or broad refactor.
The carrier's existing Clone is syntax-only; cloning bytes cannot construct the
private registry capture.

## 3. Exact original SQL projection

Capture all12 named main-schema trial tables and all56 columns in the catalog.
Do not SELECT *, project away audit fields or limit to the target/family. Other
alpha/confirmation tables may contain real data; leave their data unread and
never interpret that as known-empty or current eligibility.

Qualify every schema/count/data lookup with main, including table/index pragma
lookups. TEMP or attached objects with the same names cannot shadow these rows;
they are not inputs. Do not depend on unqualified SQLite name resolution.

| Table | Complete ordered unique key |
| --- | --- |
| registry_migrations | version COLLATE BINARY |
| registry_meta | key COLLATE BINARY |
| trial_families | family_id COLLATE BINARY |
| trial_batches | batch_id COLLATE BINARY |
| trial_events | seq numerically, then event_id COLLATE BINARY |
| batch_receipts | batch_id COLLATE BINARY, receipt_registry_id COLLATE BINARY |
| family_protocol_upgrades | family_id COLLATE BINARY, tests_per_trial numerically |
| origin_genesis | origin_registry_id COLLATE BINARY |
| origin_checkpoints | origin_registry_id COLLATE BINARY, origin_seq numerically |
| registry_imports | import_id numerically |
| family_conflicts | conflict_id numerically |
| registry_conflicts | conflict_id numerically |

Retain all timestamps/origins, NULL family/protocol, raw members/payload/family/
upgrade/conflict JSON, receipt counts and imports counters. SQL NULL is distinct
from JSON text null; do not replace either with absence/default. TEXT is exact
valid UTF-8, with no trimming, prefix rewriting, timestamp conversion or JSON
canonicalization. INTEGER stays exact signed64-bit; never pass through f64 or
JavaScript Number for SQL values beyond its safe range.

The shape gate checks the original column nullability and integer domains:
positive IDs/seq/origin_seq; effective exactly0/1; tests_per_trial at least2;
nonnegative receipt/import counters and head_seq. Upper bound is native i64,
not a newly invented precision-count policy. Event kind has exactly the six
original SQL values. Other TEXT, including hash-shaped claims, remains bounded
uninterpreted text; hash/key/family/protocol/FK semantic consistency is later work.
The exact registry_meta registry_id row must equal the handle's observed ID.
Other metadata/birth markers remain opaque and cannot certify empty history.

## 4. Fixed schema gate and structural limits

Before DataRow construction, require all seven known migration versions exactly
once and all12 fixed names to be ordinary real tables in main, not views,
virtual/shadow tables or attached substitutes. Check bounded table_list flags:
main/table, expected ncol, without-rowid=0 and strict=0. Use bounded table_xinfo, including
hidden/generated columns, to check exact column count/order/name/declared type/
notnull/default/PK ordinal and hidden=0. Compare the actual catalog, not an
assumption that every TEXT primary key has notnull=1. Verify complete primary
keys and the additional original seq/idempotency_key unique keys with BINARY
text key collation. No rowid/insertion/default-collation dependency.

Schema inspection has fixed ceilings:1024 metadata rows,1 MiB metadata UTF-8,
4096 bytes per metadata text and32 indexes per selected table. Check borrowed
metadata before owning it; drop it after validation. Do not scan unrelated data.
CHECK/FK/trigger SQL definitions are not authenticated by this shape gate and
must never be advertised as source immutability/custody proof. Enforce the
declared value domains directly. Tests preserve complete preexisting schema,
triggers and pragmas across capture; capture never changes them.

| Fixed private limit | Inclusive maximum |
| --- | ---: |
| R, total data records across12 tables, including migrations/meta | 10000 |
| Single non-JSON TEXT UTF-8 bytes | 4096 |
| Single JSON original UTF-8 bytes | 4194304 |
| Single JSON value/container nodes, excluding object keys | 65536 |
| Single JSON value depth under the original walk rule | 64 |
| Decoded JSON object key UTF-8 bytes | 1024 |
| B, sum of every retained non-NULL TEXT/JSON cell's UTF-8 bytes | 33554432 |
| J, sum of retained JSON value/container nodes | 262144 |

Integers/SQL NULL contribute zero to B/J; R bounds their fixed typed overhead.
Repeated identical strings in different cells are charged every time. No shared-
row deduplication/refund, caller cap, truncation or retry under another policy.
Schema metadata has its separate bounded transient counters, not retained B/J.
This bounds the Rust capture, not whole-process/SQLite-internal memory.

Complete the global R pass using fixed named tables and remaining+1 row probes
before allocating a DataRow vector or copying data cells. Do not run an unbounded
COUNT/SUM/export/verifier first. Thereafter SQL CASE/type/length guards expose
bounded borrowed fields. Check exact borrowed UTF-8 byte lengths and remaining B
before every owned text copy. OriginalJson::parse_with_node_limit receives the
remaining global J; its unchanged per-document clamp/depth/decoded-key/duplicate-
key checks precede that document's sole owned copy. Charge with checked arithmetic
before counters change. A late failure drops partial internal rows and returns
one static StateInvalid diagnostic, never a partial observation or raw payload.

Capture performs no SHA, canonical serialization, domain materialization, sort
of reconstructed members, chain verification, kind-proof validation, current
binding/admission read, migration, PRAGMA update or file/worker/clock/network IO.
Its fixed SQL ordering follows already bounded tables after R/schema preflight.

## 5. Meaning of success and required later composition

Success means bounded original SQL shape/bytes observed through that open handle
in one historical snapshot. It does not mean those claims have correct hashes,
form a complete chain, match any workspace, have valid execution/protocol proof,
or describe the current authorised registry. Empty captured trial rows are not
known-empty alpha/confirmation/history. A same-ID copy can be historically
consistent; this API does not detect it as an illegal current replacement.

The later sole semantic entry must consume this sealed capture together with
the sealed workspace graph. Select the batch only by the admission's original
batch_id; require all enumerated candidates/attempt IDs and every complete member,
including failed/null split members, and no extra batch events. First target
support can be native-origin normal request Variant only; reject unsupported
target kinds/import origins as a whole. Unrelated history cannot be filtered to
lower accounting. Preserve family/base/upgrade/receipt/origin/conflict originals;
protocol is absent from event/batch identities and receipt counts are audit-only.

Full global event/batch validation is still mandatory: old verify_chain reads
every event and every batch, clones/canonicalizes/hashes/sorts members, and is
neither a shape gate nor complete kind/protocol/FK validation. Do not invoke it
inside capture. Before any later verifier/semantic payload/hash/copy/output,
freeze and reserve the combined old P/P+E and market/warmup W/C/A/Q/O plus all
global trial parse/clone/sort/hash and target reconstruction work. Reuse the
existing single prepared enumeration; do not stack old observer calls or call
candidate_split_hashes for a second enumeration. That full contract/coupled
corpus follows actual capture conformance and is not claimed complete here.

The graph's candle_count remains declared historical input only. Authenticated
candle/count/availability4b-3, supported action/cost originals, actual owner/
saved-binding/current-registry/restrictive evidence and final IMMEDIATE fence4d
remain mandatory before any RegisteredSource or protected operation.

## 6. Independent evidence and implementation gates

Run finite artifact checks from repository root:

```powershell
py alpha-factor-forge/scripts/generate-original-trial-registry-capture-v1.py
node alpha-factor-forge/scripts/verify-original-trial-registry-capture-v1.cjs
```

The finite catalog pins original schema/opener/parser/reader sources at #244.
Python stdlib derives schema metadata in an isolated in-memory SQLite database
from exact original migrations; it never opens a product database. A separately
written Node built-ins verifier derives the schema from pinned DDL and recomputes
producer references/resource recipes without importing product code. Published
v1 stays tied to that baseline, including after narrow parser visibility changes.

The22 recipes prescribe all17 frozen ledger batches, known-family2-to3 and an
unrelated mixed Diagnostic/Variant batch, plus separate original import, receipt-
conflict and origin-divergence cases to populate origins/imports/conflicts through
unchanged original producers. They preserve all old audit values. These are
source-grounded recipes to execute during capture conformance, not22 newly run
Rust controls or a new coupled workspace/registry proof.
Conflicting export controls re-seal the original body hash as existing tests do;
they are declared negative-history carriers, not genuine source authority.

Actual implementation must verify all56 cells, ordering, usage and raw JSON
against these originals and independently inspected SQL values. Use fresh owned
file-backed sibling registries/workspaces; no protected worker or Test seed.
Add exact/+1 R/B/J/per-cell bytes/nodes/depth/key tests with all other limits fit;
non-target oversized/invalid records must refuse before semantic verifier calls.
Cover BLOB/REAL/null/vocabulary domains, views/hidden/generated/extra columns,
unknown/missing migrations, malformed UTF-8/decoded duplicates and opaque extreme
number tokens. Prove bounded shape-only hashes/imported history are captured,
not silently promoted into semantic success.

An actual second writer commits between count/data passes to prove the pinned
old snapshot; a later capture sees its growth. Refuse an already active registry
transaction without ending it. All successes/refusals preserve every workspace/
registry table/schema/storage type/REAL bit/trigger/pragma, actual writer
total_changes, artifacts and caller lease/transaction. Alpha/confirmation may be
nonempty and remain unread/unchanged. Lock/connection remains reusable after
errors. Close owned handles and check exact containment/no-reparse before cleanup.

Exact artifact reproduction, deliberate invalid-artifact assertions, source/
fixture/link/scope/lifecycle checks and independent review precede this design's
PR. Six exact-head/base CI jobs, normal expected-head merge, GitHub readback and
clean-main/full-tree equality complete only5b-design-1; capture code and the
whole consumer contract remain separate ordered tasks.

Finite catalog pin:107230 LF bytes, SHA256
`985d4ed5f81ce867a168f25463b3fc940ec97bfbf7c0422bb6f6b1a7ef1586dd`.
Exact Python reproduction and independent Node DDL/recipe/syntax checks pass.
Eight coherently altered artifacts fail assertions; source/code checksum changes
are not used in place of those schema/order/audit/budget/syntax/scope controls.
