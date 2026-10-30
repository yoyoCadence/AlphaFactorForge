# P13c-4b-2c-4c bounded original market projection consumer v1

Date: 2026-10-29 (UTC+8). Baseline: verified #238 (`8e99fcf`).
Design/data only; implementation follows verified delivery.

Seed erratum (2026-10-31): the additive
[v2 correction](research-original-market-consumer-seed-correction-v2.md) supersedes
the v1 dataset-rebound input fingerprints and affected preparation byte counts.
The v1 artifact remains immutable historical evidence; all other rules below
remain binding. New market controls must use the corrected v2 vectors.

The [original registry](research-original-market-registry-projections-v1.md),
[provenance/snapshot](research-original-market-source-projections-v1.md),
[lineage](research-original-lineage-semantics-v1.md) and
[warmup](research-original-warmup-consumer-v1.md) contracts remain binding.

## 1. A single historical projection observation

Freeze `observe_original_market(rows: &SourceRowsObservation) ->
AppResult<OriginalMarketProjectionObservation>` inside the existing private
`research::confirmation_inputs` graph boundary. Its sole input is the sealed,
immutable complete captured graph. No DB, filesystem, clock, latest-row resolver,
prepared-parts constructor or caller resource policy enters this function.

The result owns one existing `OriginalWarmupObservation`, three original hashes
(`snapshot_id`, `instrument_revision_hash`, `calendar_hash`) and a parallel
`Vec<OriginalMarketSourceBinding>` sorted by ascending original provenance ID.
Each binding owns exactly its original positive i64 ID and 64-character record
hash. Fields/constructors stay private; Clone/Eq/PartialEq and ancestor-only
borrowed getters are sufficient. No Debug/serde/Default/mutable getters,
RegisteredSource conversion, runtime export or separately supplied evidence.
The warmup retains every candidate, including failed members, in original index
order; all hashes/bindings are withheld until the entire operation succeeds.
Every failure at this entry maps to static `original_market_invalid`.

This is original row/document consistency. The graph has no raw files/candles,
immutable action/cost originals or complete global provenance-successor closure.
It cannot authenticate dataset payload/coverage, first observability or whether
another record now supersedes a selected component. Do not claim current-head
status by scanning only selected components. Preserve known revisionOf as an
original identity input; future4b-3/final admission must provide their complete
authenticated/current observations. Declared action/cost version strings are
only compared to original content. `sourceEvidenceHash`, `numericCostsHash` and
RegisteredSource remain unavailable. No Test/Validation generation, execution,
statistical sufficiency, protected acquisition, registry spend or activation.

## 2. One preparation, one complete reservation

Add a private market child at the graph/original-lineage boundary. Refine only
the narrow private preparation and warmup stages needed to compose this entry.
Old lineage/warmup entries retain their existing ceilings, outputs, ordering and
errors. Do not call the old observer and prepare/enumerate a second time.
There is no generic callback, arbitrary document factory or caller-selectable cap.

The existing graph capture already validates the original campaign declaration
and selects its instrument binding. Retain that binding's exact `fromMs`/`toMs`
as private immutable i64 metadata in the selection and sealed graph. The market
observer requires `dataset.start_time == fromMs` and `dataset.end_time == toMs`
during post-reservation verification; it must not reparse
the campaign or repeat selection/enumeration. No SQL columns, new identity fields
or caller-supplied bounds are added. These two derived integers add no text/Value
work to the frozen formulas; existing selector results and work stay unchanged.
Extract them with checked integer access from the selected frozen binding, never
a float cast or the first binding by position. Retaining the scalars must not add
a new refusal to graph capture or the old lineage/warmup observers: exact dataset
range agreement is a new market semantic check after the joint reservation.

Before any new Value materialization, add every market original document below
once to the existing original-lineage parse P bytes/nodes. Reserve joint P
against 33554432 bytes/262144 nodes. Then preserve existing config parsing and
raw expansion E; check joint P+E under those same ceilings before enumeration.
Keep the old P/E charges as named components; no refund/reset/deduplication.
Materialize originals once, not a cloned source graph or display DTO.

Market document order D is snapshot coverage_json/content_json; instrument
suspensions_json/source_capabilities_json/content_json; calendar definition_json;
then each selected provenance request_scope_json in ascending original ID.
Arrays, arbitrary scope Values, original strings and SQL REAL bits are retained.
OriginalJson syntax limits still apply to each document; no parser changes.

After that bounded preparation, create the existing borrowed lineage context
once and reserve all lineage/warmup/market work BEFORE the first post-preparation
hash, typed document conversion, payload clone, validator/date parser, source
sort or output allocation. Borrowed structural/counting walks may measure sizes;
never serialize/hash/validate merely to estimate a charge. Use fixed private
reserve/verify/finish stages, keeping constructors private.

After reservation: verify original complete lineage; verify all market
projections; call the unchanged actual executor bridge for every candidate's
warmup using the original holding allowance; finish one lineage/warmup and one
market output. No invalid/failed member is filtered into a successful subset.
Discard all temporary results on any late refusal.

## 3. Supported original schemas and field agreement

Use original JSON and raw typed cells from
[the graph](../../alpha-factor-forge/src-tauri/src/research/confirmation_inputs/source_rows/graph.rs)
and [its fixed rows](../../alpha-factor-forge/src-tauri/src/research/confirmation_inputs/source_rows/graph/rows.rs).
Resolve references by actual IDs, never zip unrelated vectors by position.
The captured instrument revision remains authoritative even after a later
registry revision exists. Every selected relation/provenance must participate.

- Calendar: session-calendar-v1, exact supported SessionCalendar keys plus
  version; validate the existing calendar rules. Reproduce the original research
  canonical JSON hash; compare every calendar scalar and complete definition.
- Instrument: market-instrument-v1, exact original InstrumentDraft keys plus
  version; reproduce content hash and compare all original scalar/null fields,
  parsed market/venue/symbol, suspension/capabilities projections and optional
  finite positive lot/step/notional f64 bits. Apply the existing ID/asset,
  trimmed-field, listing/delisting, sorted nonoverlap and calendar-kind rules.
  Revision number/row ID remain captured scalars, not new identity fields.
- Provenance: market-provenance-v1, complete original fourteen-field identity
  described by the producer contract. Validate ID/interval/nonblank source,
  role/acceptance/reason, RFC3339 instants and available<=retrieved. Keep original
  timestamp spelling, scope Value, revisionOf, raw SHA/length/media/path; all four
  media suffixes follow the original checksum-prefix path rule. Hash the exact
  reconstructed original identity, not raw files or a new source-evidence frame.
- Snapshot: market-snapshot-v1, exactly its eleven original content keys.
  Bind instrumentContentHash to the captured instrument; datasetHash/interval/
  instrument/calendar/price basis/kind/action/cost versions to all stored fields
  and root selection. Reproduce sorted complete sourceRecordHashes and the
  original snapshot hash. Preserve as_of separately; it remains outside identity.

The first consumer supports raw price basis, status=ok, historical or
forward-observed kind, a declared cost version and, for ETF, a declared action
version. Demo/degraded/unsupported price basis or schema/version refuses.
These declarations never establish immutable action/cost support. Components
must all be accepted primary records of the exact series, with consistent
publisher origin under the existing Binance archive/rest origin rule. All
claimed record identities/scalars must agree before a result can escape.
Forward metadata additionally requires known availability<=captured as_of;
Binance archive requires observed-at-retrieval and equal available/retrieved
instants. A legacy period guess stays unknown, even after coherent rehashing.

Coverage supports exactly the original market-foundation-v1 report keys, empty
events, blocking=false, issue=null, notDueCount=0 and equal positive expected/
matched/observed/dataset candle counts <=1000000. Dataset exchange compares to
venue case-insensitively; symbol/interval/bounds/count/hash retain their original
rules and selected campaign range. These are consistency checks only. Do not
generate expected timestamp grids or claim actual bars closed/complete without
authenticated candles. Captured as_of must be a representable instant.

Use the locked legacy numeric policy and existing canonical/hash implementations
after reservation. Exact optional REAL bits/nulls and JSON projections matter.
No float_roundtrip/arbitrary_precision change, token repair, dual hash, silent
rehash or numerical-execution compatibility claim. Unsupported reconstruction
refuses atomically; the [numeric audit](../numeric-json-identity-audit-v1.md) stays open.

## 4. Complete composed work

Let Ww/Cw/Aw/Qw/Ow be the unchanged complete warmup reservation, including its
lineage context, all candidates and output. Original P/E phases stay separately
bounded as above. Define exact market quantities from borrowed prepared values:

- T: summed UTF8 bytes of EVERY captured market text cell, including original
  JSON bytes, in snapshot/instrument/calendar/dataset/selected provenance rows.
  IDs/integers/REAL bits add no text. Repeated cells are charged each occurrence.
- Jd/Nd: summed compact serde JSON lengths/value-node counts of D, including
  containers, excluding object keys from node counts. D original bytes/nodes
  separately determine the new P charge.
- I: original calendar, instrument, snapshot identity documents, followed by
  every reconstructed provenance identity. Ji/Ni sum their canonical compact
  UTF8 lengths/value nodes. Length is measured without creating I/canonical
  buffers; include fixed keys, tags, scalar spellings, punctuation and all hashes.
- m: number of selected components, 1..=1024. Original relation/provenance
  membership is exact; source-hash sorting never drops/replaces a member.
- Om=192+72*m: three 64-byte hashes and, per binding, one i64 plus 64 hash bytes.
  Vec/String headers and bounded indexes keep their existing metadata bounds;
  this logical output size is not JSON text or an OS allocation measurement.

Freeze Wm=Ji, Cm=32*(T+Jd+Ji)+4096*(Nd+Ni)+8192*(m+4)+Om,
Am=128*(T+Jd+Ji)+4096*(Nd+Ni)+8192*(m+4), Qm=4*Nd+2*Ni.
The conservative text/node charges cover typed copies, fixed-schema identity
construction, canonical strings/escaping/key sorting, complete source sorting/
origin comparisons, field/date/rule validation, internal bounded diagnostics
and final output copies. OriginalJson's depth64/key1024/node65536 limits bound
these structural walks; no per-source full-family rescans or Cartesian source
comparison is allowed. Compare publisher origin against one fixed reference
under the existing equivalence rule; validate each member's series separately.

| Whole post-preparation stage | Inclusive ceiling |
| --- | --- |
| Ww+Wm | 33554432 hash bytes |
| Cw+Cm | 67108864 copy units |
| Aw+Am | 268435456 analysis units |
| Qw+Qm | 262144 Value clone/construction nodes |
| Ow+Om | 4194304 output units |

The old warmup Qw<=old prepared P+E node check remains independently required;
the new Q ceiling cannot relax it. Audit actual market clone/construction sites
against Qm; counters/tripwires may be cfg(test)-only. Qm conservatively reserves
up to four copies per D node and two constructions/copies per I node. Do not
invent an unreachable exact-Q full-positive case: retain P/E boundaries and
audit composed Q on every real capture. Checked conversion/add/multiply/count
overflow and any whole-stage overage refuse before payload. No second allowance,
per-row reset, truncation, memoized different preimage or partial output.

## 5. Independent fixture and complete boundaries

[original-market-consumer-v1.json](../../alpha-factor-forge/fixtures/research/original-market-consumer-v1.json)
is independently authored with Python json/hashlib/struct; separate Node built-ins
verify catalogs, campaign binary identities, original hashes, projections and
work. Neither checker imports/executes project code. Product original strings
are never normalized; LF packaging alone has a SHA pin recorded in the handoff.
Six immutable source packages are referenced. Six private consumer-source pins
record pre-code grounding; legitimate scoped code refinement will change those
consumer files, while all original market producers/old fixtures stay unchanged.

Four complete family references use actual frozen crypto/ETF datasets and the
registry/provenance/snapshot sequence. Rebind original config dataset ID/hash,
canonical configHash, every original input fingerprint and hypothesis content/
applicability hash together. Hash widths, two-byte intervals and one-digit
dataset IDs preserve prior work sizes. Fresh actual history IDs are1/2, not the
old synthetic graph's200/201. Config declaration order can differ from candidate
order. Freeze campaign declarations with the exact snapshot/range and original
binary campaign codec, not SHA of JSON. Sampling declarations are domain-valid;
2/4 bars and large lookbacks do not imply executable/sufficient samples or an
eligible admission. Retain NOT_ELIGIBLE historical admission status.

| Complete positive | W | C | A | Q | O |
| --- | --- | --- | --- | --- | --- |
| captured-old-revision | 7435 | 768059 | 1304279 | 478 | 925 |
| new-revision-mixed | 10005 | 945189 | 1529431 | 577 | 1098 |
| forward-offset-availability | 7478 | 811739 | 1368407 | 508 | 925 |
| etf-declared-references | 9562 | 871101 | 1380823 | 561 | 1026 |

Five coherent refusal vectors rebuild EVERY snapshot/source/campaign hash while
retaining rejected/comparison/mixed-publisher/legacy-archive/late-availability
semantics. They are controlled graph claims, never successful producer rows.
The [7,4] late-invalid family must refuse even when a subset would be valid.
Additional scalar/schema/REAL-bit/cut/coverage/member contradictions remain in
the negative catalog; no hash mismatch may substitute for these semantic tests.
Include a coherently rehashed campaign with only the selected range changed,
and a multi-instrument campaign whose selected binding is not first. The former
must refuse after reservation; the latter must use the selected original range.

Four exact/+1 recipes attach the valid captured-old-revision market to the
existing complete resource captures. All joint preparation/other limits fit.
Hash shortens the first stored entryCode by1931; analysis uses DSL name2085699
and params suffix99; copy uses DSL name706963, twelve 0-to10 replacements and
wrapper counter0/10. These three accept exact reservation then refuse original
identity; they are explicitly NOT successful semantic observations. The output
recipe creates1101 actual original candidates, main base ID3648 bytes and other
ID540 bytes, so Ow+336=4194304. Exact MUST succeed for the whole observation;
other ID+1 MUST refuse before payload/analysis. No scalar-only boundary evidence.

## 6. Implementation and delivery gates

After this design's verified delivery, actual4c code uses real migrated,
file-backed isolated workspace captures. Register original calendars/instruments,
import frozen dataset hashes and create provenance/snapshots with actual writers
BEFORE installing the remaining historical family closure. Reuse original
strategy/history producers with the new frozen config/input/hypothesis strings;
compare them to independent literals. No SQL-synthesized market success rows,
current-output expectations, protected job execution or files-as-authority.
Preserve full original inputs/graph, DB rows/schema/total_changes/mutation_seq,
caller savepoint/transaction/reservation and physical workspace lease across
success/refusal. Keep all existing lineage/warmup tests and bridge parity.

Require whole joint P/E gates before materialization/enumeration, all W/C/A/Q/O
preflight tripwires, all four positive families/five coherent refusals, original
row/request/hash ordering and captured revision checks. Seven actual-source
mutants must assertion-fail: omit market parse from joint P; omit market W/C/A/O
from whole reserve; omit instrument scalar/REAL-bit agreement; select one source
only; skip accepted/primary validation on the late invalid family; trust archive
period availability; allocate final bindings before preflight. Exclude checksum
tests and restore exact source bytes/log every mutation. No compiler failure
counts as behavioral evidence.

Actual code requires fresh exclusive GUID TEMP/TMP full locked Rust BEFORE
Node-heavy checks; locked all-target check/Clippy/scoped format and frontend
checks. Exclusive owned roots skip existing names, close DB handles and verify
canonical parent/leaf/every descendant containment/no-reparse before cleanup.
No manual server/user test is needed. Only code delivery closes actual4c;
trial follows market-row semantics. File/candle/availability4b-3, separately
supported action/cost originals and actual-owner/registry4d stay mandatory.

This design runs finite artifact checks, not local runtime suites solely for
Markdown/unconsumed data. Six completed exact-head/base CI jobs still validate
unchanged #238 runtime:1133 Rust/one ignored,1242 Vitest/80 files,98 E2E and native
smoke. Fresh reviews/main, expected-head normal merge, GitHub readback and clean
main/full-tree equality complete delivery. Under the updated continuous-cycle
authorization, append the verified receipt in the next slice and proceed to
actual4c without another confirmation. Market/source/live parents remain open.
