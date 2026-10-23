# P13c-4b-2b-2 complete graph adapter contract v1

Date: 2026-10-24
Baseline: merged #214 (`07cf11d`). Documentation only.

The [source closure](research-source-reference-observation-v1.md#3-complete-workspace-row-closure)
and [133-column root/graph manifest](research-source-sql-projection-v1.md)
remain fixed. The delivered roots are incomplete consistency evidence. This
refinement defines the necessary selector/enumeration adapter and remaining
typed graph reads; neither can construct RegisteredSource or authenticate files.

## 1. Grounding and ordered code slices

| Existing implementation | Consequence |
| --- | --- |
| [Root reader](../../alpha-factor-forge/src-tauri/src/research/confirmation_inputs/source_rows.rs) | Reuse immutable roots, guarded cells, fixed locator and retained-resource usage. Do not reload through display DTOs or reset the budget. |
| [Campaign codec](../../alpha-factor-forge/src-tauri/src/discovery_core/campaign.rs), [store](../../alpha-factor-forge/src-tauri/src/db/campaign.rs) | Supported declaration v1 contains bounded integers/strings/nulls. Re-freeze original bytes through the existing codec and require the stored/requested ID. |
| [Config](../../alpha-factor-forge/src-tauri/src/discovery_core/config.rs), [enumerator](../../alpha-factor-forge/src-tauri/src/discovery_core/enumerate.rs) | Existing v1/v2/v3 parsing, prune/deduplicate/hash-sort/index rules remain authoritative. The enumerator clones strategies/base IDs across the raw product; candidate count alone does not bound those copies. |
| [Runner](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs), [numeric audit](../numeric-json-identity-audit-v1.md) | Stored config is serialized from the original runner Value. Resume uses current locked legacy serde_json semantics. Selection reconstruction is consistency evidence, not proof of original floating-point execution. |
| [History schema](../../alpha-factor-forge/src-tauri/migrations/0007_research_history.sql), [market schema](../../alpha-factor-forge/src-tauri/migrations/0008_market_foundation.sql) | Preserve raw optionality, exact references and all original JSON. Do not obtain a complete family by inner joins or completed-only filters. |

One PR/verified merge per slice:

1. **4b-2b-2a: bounded root selection.** Implement only section 2, its scratch
   preflight and independent selector tests. No graph SQL or source assembly.
2. **4b-2b-2b: complete typed row graph.** Implement sections 3-5 against the
   first slice's immutable selection. All eleven tables ship together so a
   partial graph cannot be reported as the complete declared closure.
3. **4b-2c: original semantic source assembly.** Still requires separately
   frozen original strategy/engine/lineage/trial/digest adapters and fixtures.
   File/candle evidence remains 4b-3; final actual-owner fence remains 4d.

Only 2a becomes Next after this design's verified delivery. Parent 4b-2b/4b-2/
4b/P13c-4 remain incomplete. No execution, policy, calibration or reserved-seed
activity is authorized by these consistency objects.

## 2. First code slice: bounded root selection

Add a private `source_rows::selection` child module. Its `derive_selection(
roots: &RootRowsObservation) -> AppResult<SourceSelection>` and immutable types
have only source_rows visibility. The sole constructor derives from captured
roots: no caller-provided row list, candidate subset, selector factory, mutable
accessor, Default/Debug/serde, callback, runtime caller or research re-export.

SourceSelection owns these private fields in order:

| Field | Type / domain |
| --- | --- |
| snapshot_id | String; exact 64 lowercase hex from the matching declaration binding |
| dataset_id | i64; existing config's positive JS-safe dataset ID |
| dataset_hash | String; existing dataset-content-v2 identity, equal to declaration |
| interval | String; exact supported declaration interval, nonempty <=4096 bytes |
| candidates | Vec<CandidateKey>, nonempty <=4096, in final ascending index order |

CandidateKey owns `index: i64, strategy_hash: String`; index is the existing
enumerator's consecutive 0..N-1, hash is its exact `strategy-v2:` plus 64
lowercase hex. Preserve this stored prefix; pure-source suffix conversion is
a later semantic adapter, not a row/selection operation. Parent-visible
read-only getters return borrowed strings/slices and copied integers. No source
or registry capability, costs, declaration/config Value or strategy tree escapes.

Derivation order is fixed:

1. Require stored campaign version `research-campaign-declaration-v1`. Parse
   its already-bounded OriginalJson with the existing locked serde_json policy,
   call freeze_campaign and require its ID equals both captured campaign IDs
   and the saved locator. Select exactly one instrument binding whose ID equals
   admission.instrument_id, never first/latest/default. Validate with the
   existing declaration/instrument/interval rules, without market/DB resolution.
2. Parse original run config with the unchanged locked default serde_json Value
   reader and parse_discovery_config. Accept only its existing v1/v2/v3 versions
   and exact contract set. Missing/duplicate JSON keys are already refused by
   OriginalJson; do not enable float_roundtrip/arbitrary_precision, roundtrip
   parsed JSON or switch identity policies. Unsupported numeric reconstruction
   cannot become an original-execution claim.
3. Parsing must be independent of the current computer. If maxConcurrency is
   null, use logical_cores=1; otherwise require its existing finite positive
   JS-safe integer domain and use that same value as logical_cores. Existing
   resolve_concurrency checks remain in force. This reproduces candidate inputs,
   not historical worker count, resource permission or run liveness. Prove that
   valid different core choices leave candidate identities/order unchanged.
4. Require config dataset hash equals the declaration binding; retain exact
   config dataset ID, declaration snapshot ID and interval. Do not read candles,
   rehash datasets, interpret eligibility/status/costs as permission or synthesize
   missing cost/action/source documents.
5. Validate scratch expansion below **before** enumerate_candidates. Then call
   that unchanged pure enumerator once. Require its nonempty bounded manifest,
   valid exact indexes/hashes and counts. Retain only CandidateKeys; discard
   parsed trees, strategies, axes, seeds and concurrency output.

Every selection refusal uses static `source_rows_invalid`. No root mutation,
hash/token rewriting, SQL, IO, registry, clock, hardware probe, mutex or event.
The original root bytes remain unchanged even when legacy number reconstruction
differs. This is not a new numeric compatibility policy or source identity.

### 2.1 Scratch expansion preflight

Root JSON is bounded, but directly calling the existing Cartesian enumerator
can multiply a large base ID/strategy by up to 4096 before any row budget sees
it. Add a private checked preflight; do not change the shared enumerator.

For each parsed base compute p, the checked product of its existing axis_values
lengths (one for no axes). Check the sum of p against both config.caps.candidates
and 4096 before Cartesian allocation, even if pruning/deduplication would later
shrink it. axis_values retains its existing 64-values and numeric rules.

Measure serialized byte counts with the current serializer into a bounded
counting sink that retains **no bytes**. This is in-memory accounting, not file
IO, hashing, an identity encoding or a replacement for original JSON. Count
Value nodes without cloning a tree. For a base let:

- S = serialized bytes of base.strategy, T = its value nodes;
- I = serialized bytes of base.id as a JSON string;
- A = sum, across axes, of K + V + 3, where K is serialized axis-key bytes and
  V is the largest serialized finite numeric value from that axis_values set;
- B = S + 2*A + 2 + I; Q = T + 2*axis_count + 2.

All products/additions/counting are checked. The extra axis terms conservatively
cover patched strategy members plus the applied-axis map and base-ID scalar;
do not subtract overwritten values or use a guessed maximum f64 text length.
Require sum(p*B) <=33,554,432 and sum(p*Q) <=262,144 **before** the enumerator
can clone a Cartesian combination. Refuse the first excess, never truncate,
refund, sample, retry or lower a declared candidate cap. Counter overflow refuses.

These are inclusive scratch logical-payload ceilings for this new dormant
adapter. They do not reset or consume the final graph's retained-row budget;
scratch trees/selection are dropped during graph capture and are not retained
observation fields. Existing parsing/intermediate copies remain bounded by the
input/axis/product limits; neither ceiling claims a 32 MiB whole-process peak.
The delivered root-only API and existing exploratory runner limits are unchanged.

### 2.2 Required first-slice evidence

Use roots actually captured from migrated file-backed fixtures with isolated
leases. Reuse #214's test helpers only under cfg(test); no production unchecked
root constructor. Existing root/JSON/workspace tests must remain unchanged in
behavior. Tests need no graph records, files, ledger or execution.

- Use the committed [campaign declaration](../../alpha-factor-forge/fixtures/rs-core/research-campaign-declaration-v1.json)
  and [runner parity fixture](../../alpha-factor-forge/fixtures/rs-core/runner-config-v1.json).
  The original declaration binds dataset hash suffix `b*64`, while these runner
  cases bind `a*64`. For these tests clone the declaration and change only its
  datasetHash to the runner's exact value; independently freeze that test variant
  before asserting the adapter. Never edit the committed fixtures or weaken the
  dataset equality check to make unrelated originals join.
  Independent binary-encoding controls (all other declaration values unchanged):
  original ID `2ffaed4e40e8cd81c1d8631f802c8b744876f83cfef29e2d7db01db5e5f26c45`;
  dataset-a variant ID `3d64d27d196dcb275b30a1d59a50ea3481199331cab4a04a2dda2b4fb0a47b9e`.
  Both canonical payloads are 1151 bytes; the domain prefix/zero is additional.
  Enumeration oracles are the fixture's saved expected indexes/hashes/counts,
  never expected values generated by the adapter under test.
- Preserve the six independent enumeration cases: single-axis 3, multi-base 9,
  cross-field prune 1, cross-base duplicates 4, disjoint bases 4 and reversed
  disjoint bases 4. Candidate count alone is insufficient: assert every ordered
  hash/index and dataset binding. Preserve all supplied configs/expected values.
- Add authored v2 fixed-DSL and v3 walk-forward cases with independently frozen
  expected manifests before testing the adapter. Assert selection is independent
  of maxConcurrency's valid performance-only choices and current hardware.
- Missing/wrong/duplicate instrument, wrong stored/frozen campaign IDs, unsupported
  versions/contracts, malformed original config, hash mismatch and all-pruned/
  over-cap enumeration refuse statically without altering any roots/state.
- Exact/one-over nonempty scratch byte/node/product counters, checked overflow,
  pruning/deduplication not excusing raw over-cap expansion, and large repeated
  base/strategy payloads must refuse before entering the Cartesian enumerator.
  A test-only thread-local entry counter/tripwire may prove ordering without
  allowing a deliberately broken mutant to allocate an oversized product.
- Long-float audit tokens retain original bytes and characterized legacy parsed
  bits; no frontend/current/legacy dual-hash acceptance or silent rehash. Re-freeze
  integer/string/null campaign identities with independently recorded expected IDs.
- Explicit mutants: first-instrument selection, omitted dataset equality, omitted
  scratch preflight and selected/incomplete candidate manifest. Each must produce
  an assertion failure; restore exact source before full locked Rust, frontend,
  scoped format/check/Clippy and six-job expected-head delivery.

## 3. Complete graph API and exact record types

The later private `source_rows::graph` module exposes only to confirmation_inputs:

```rust
capture_source_rows_in(tx: &Transaction, expected: &ExpectedWorkspace,
                      locator: &SourceLocator) -> AppResult<SourceRowsObservation>
recheck_source_rows_in(tx: &Transaction, expected: &ExpectedWorkspace,
                      saved: &SourceRowsObservation) -> AppResult<()>
```

No replacement locator/selection argument on recheck. All fields are private;
only Clone/Eq/PartialEq are allowed. No Debug/serde/Default, mutable accessors,
unchecked factories, RegisteredSource conversion or runtime/public re-export.
Types may use ancestor visibility solely for these private functions.

Observation field order: roots, jobs, attempts, strategies, hypotheses, artifacts,
snapshot, instrument, calendar, dataset, sources, provenance, usage. Roots is the
existing RootRowsObservation; usage is final ResourceUsage. The five plural
candidate tables and sources/provenance are Vecs; the four market/dataset roots
are exact singleton records. Selection is transient and not stored in this object.

Each record's exact field order is the existing [117-column manifest](research-source-sql-projection-v1.md#4-complete-graph-projection-manifest-retained-for-later-code).
Use those column names (Rust `r#type` for strategy.type), with `_bits` suffix on
the three nullable instrument REAL fields. This type map is exhaustive:

| Record | INTEGER / special fields | Remaining manifest fields |
| --- | --- | --- |
| JobRow | id/run/strategy/dataset IDs positive i64; candidate_index nonnegative i64; result_id Option<positive i64> | segment/status String |
| AttemptRow | id/hypothesis/strategy/dataset IDs positive i64; discovery_run_id Option<positive i64>; candidate_index Option<nonnegative i64>; result_artifact_id Option<positive i64> | input/engine fingerprints OriginalJson; outcome_json Option<OriginalJson>; trial_event_id Option<String>; other cells String |
| StrategyRow | id positive i64; parent_strategy_id Option<positive i64> | original_definition_json OriginalJson; dsl_json Option<OriginalJson>; other cells String |
| HypothesisRow | id positive i64; strategy_id/parent_strategy_id Option<positive i64> | applicability_json/content_json OriginalJson; variation_kind Option<String>; other cells String |
| ArtifactRow | id positive i64; byte_len nonnegative i64 | String |
| SnapshotRow | id/instrument_row_id/dataset_id positive i64; as_of signed i64 | coverage_json/content_json OriginalJson; corporate_action_version/cost_profile_version Option<String>; other cells String |
| InstrumentRow | id/revision positive i64; lot_size_bits/price_step_bits/min_notional_bits Option<u64>; listed_from/delisted_at Option<signed i64> | suspensions/source_capabilities/content JSON OriginalJson; other cells String |
| CalendarRow | id positive i64 | definition_json OriginalJson; other cells String |
| DatasetRow | id positive i64; start_time/end_time signed i64; candle_count nonnegative i64 | String |
| SnapshotSourceRow | snapshot_row_id/provenance_id positive i64 | none |
| ProvenanceRow | id positive i64; raw_byte_len nonnegative i64; accepted i64 in 0/1; revision_of Option<positive i64> | request_scope_json OriginalJson; available_at/rejection_reason Option<String>; other cells String |

String/OriginalJson bounds and original numeric opacity remain unchanged. Hash
columns sha256/raw_sha256/record_hash/content_hash/hypothesis_hash/snapshot_id
use their existing 64-lowercase-hex domain; strategy_hash uses exact strategy-v2
and dataset_hash exact dataset-content-v2 prefixes plus 64 lowercase hex. Keep
the prefixes, never introduce a new digest or suffix-only row identity. Identity/version
strings must be nonempty; ordinary descriptive text and optional text may be
empty if the schema permits. Empty is not NULL and supplies no missing evidence.

Closed vocabularies are exact existing schema values: job segment train/validation
and status queued/running/done/failed/skipped; attempt submitted/running/completed/
failed/skipped; hypothesis source manual/discovery/ai; calendar kind continuous/
trading-days; instrument market crypto/us-etf/tw-etf and asset_type spot-crypto/etf;
snapshot price_basis raw/adjusted-split/adjusted-total-return, kind demo/historical/
forward-observed and status ok/degraded; provenance role primary/comparison.
Other schema-unconstrained kind/type/source/version strings retain bounded raw
values; supported semantic interpretation is later, never a default mapping.

Nullable cells emit marker=1/value=NULL only for actual SQL NULL. A nonnull cell
must satisfy its exact typed guard; invalid emits marker=0/value=NULL and refuses
before any typed/nullable read. Nullable REAL is finite actual REAL or NULL;
INTEGER/TEXT never coerces. JSON TEXT `null` is Some(OriginalJson), not None.

## 4. Queries, membership and recheck

Capture calls the existing root capture, derives selection, then reads the graph
in the manifest's table order. Continue ResourceBudget from the fresh roots'
usage, with no reset or configurable bypass. Every retained text/JSON copy
passes the same pre-copy joint checks; count deduplicated row storage once,
while candidate membership/counts still include every candidate.

Freeze fixed literal query shapes: jobs/attempts/source links use their manifest
WHERE and ORDER BY; each distinct reference uses WHERE id=?1 ORDER BY id LIMIT 2,
snapshot uses exact snapshot_id and calendar exact calendar_id with LIMIT 2.
Every logical cell emits the same (valid,value) pair. Bind ?1 selector, ?2=4096
only when ordinary text appears and ?3=4194304 only when JSON appears. Bind only
slots actually present: jobs/artifacts need no phantom ?3; source links need no
text/JSON cap bindings. No caller SQL, identifiers, IN-list factory or inner join.

Jobs use LIMIT 8193, attempts 4097 and snapshot sources 1025. A row beyond the
allowed cap is a cardinality sentinel refused before its payload is retained.
Distinct strategy/hypothesis/artifact ID sets are checked <=4096 and provenance
IDs <=1024 before insertion/copy/query; all are read in ascending ID order.
Missing or duplicate referenced singletons refuse, never disappear in a join.

After typed row validation, check exact membership:

1. Jobs contain exactly Train/Validation for every independently enumerated index
   and no extra/duplicate index/segment. Attempts contain exactly one matching
   nonnull run/index for each index; no completed-only or mutual-index fallback.
   All three use selection.dataset_id, and each pair/attempt shares strategy_id.
2. Attempt keys equal `run:<saved run ID>:candidate:<index>`; trial_event_id is
   nonnull existing `trial-event-v1:<64 lowercase hex>`, unique across this
   candidate set. Completed attempts have
   a nonnull result artifact. All retained artifact/hypothesis/strategy references
   resolve exactly; their candidate strategy hashes agree with selection.
3. Hypothesis.strategy_id must match the attempt's strategy for this supported
   runner graph. For nullable strategy/hypothesis parent IDs, the first adapter
   supports only NULL or references already in the direct captured strategy set.
   An external parent refuses as unsupported closure; no recursive history scan,
   unbounded expansion or claim that omitted parent evidence was authenticated.
   Richer lineage requires a separate bounded adapter, not a selected subset.
4. Snapshot matches exact declared ID/instrument/interval and selection dataset
   ID/hash; instrument matches its exact row ID and instrument ID; calendar ID
   matches both snapshot and instrument; dataset ID/hash/interval match selection.
   Every source link points at this snapshot and exactly one captured provenance
   row, with no duplicate pair or omitted source. Provenance series identity agrees;
   accepted/rejection_reason pairing matches schema. No comparison-source promotion.

This is the declared row closure, not every foreign-key target in the database:
job result_id points at mutable latest summaries and provenance.revision_of is
retained historical metadata; neither expands the fixed manifest or becomes
original result/current revision authority. Failed/unfinished members and raw
optional cost/action/availability evidence are retained or refused as prescribed,
never dropped to manufacture a complete eligible family. Later semantic assembly
must refuse unsuitable members and unresolved evidence before any source exists.

Recheck first calls recheck_workspace_in with the saved root stamp, preserving
old-owner/version refusal priority. Then capture the entire fresh closure using
only the saved locator, including its normal workspace/root/selection validation.
The small repeated metadata read is in the same caller snapshot; it adds no
retained observation fields. Validate the full fresh closure before comparing
every field/member/usage to saved; a valid difference is source_rows_changed.
Do not call root recheck to compare roots prematurely ahead of graph validation.
Invalid/missing/duplicate/unsupported/budget data uses source_rows_invalid;
workspace errors and real AppError::Db are preserved, with no original content
in diagnostic strings. No mutation or replacement of saved preparation.

Every outcome preserves caller transaction/activity/reservation, rows/schema,
total_changes and mutation_seq. No commit/rollback, DML, registry, mutex,
workspace callback, file/candle loading, event, worker or nested transaction.
Deferred/copy equality remains historical consistency; final live-owner and
registry fences are separate.

## 5. Complete graph fixture and evidence contract

Start from section 2's independently expected three-candidate single-axis case.
Create one real migrated file-backed workspace and isolated actual lease. Insert
the exact declaration/root config, one dataset, one calendar, one instrument
revision, one exact snapshot, two source links/provenance rows, three strategies,
three hypotheses, three artifacts, six jobs and three completed attempts.
Freeze base fixture IDs as follows (i is the saved expected index 0..2):

| Row | Fixture ID / exact reference |
| --- | --- |
| campaign / admission / run | dataset-a declaration ID above / admission 1 / run 1 |
| dataset | 7, matching the unchanged enumeration input |
| calendar / instrument / snapshot | 11 / 12 / 13; exact declaration snapshot ID, instrument ID and interval |
| provenance / source links | 14 and 15, both linked to snapshot row 13 |
| strategy / hypothesis / artifact | 100+i / 200+i / 300+i |
| attempt | 400+i; run 1, index i, matching 100+i/200+i/300+i and dataset 7 |
| Train / Validation job | 500+2*i / 501+2*i; run 1, index i, strategy 100+i, dataset 7 |

Bind every FK/attempt key/trial ID to those same literals. The base raw fixture
may keep nullable job result_id NULL: latest summary rows are outside this
closure and this fixture is not a semantically admitted/executed source.
Artifact paths/hashes are claims; do not create or authenticate
files or a registry. Independently record the 16+117 ordered cell manifests and
expected retained byte/node totals before assertions; no production reader or
serializer may generate its own expected observation.

Required graph regressions:

- Complete capture/recheck and every retained field/member change; excluded
  diagnostic changes do not stale it. Shared references deduplicate row storage
  while candidate counts stay unchanged. Unrelated rows/runs remain outside scope.
- Delete the same candidate from both jobs and attempts, or all jobs/attempts:
  the original config manifest still refuses. Extra/duplicate/Test jobs, missing
  or null candidate/trial links, cross-run/strategy/dataset links and orphaned
  artifact/hypothesis/market/source rows refuse. Exercise external-parent refusal.
- Real damaged-schema duplicate singletons, each nullable marker/type/NULL/JSON
  distinction, invalid UTF-8/BLOB/coercions/nonfinite REAL and all vocabularies.
  Exact/one-over per-field, whole-observation and candidate/job/source caps must
  run through actual rows; internal budget tests alone are no longer sufficient.
- Pending complete graph writes visible only inside caller, full typed row/schema
  rollback and no observation writes; independent zero-timeout writer Busy across
  success/refusal/SQL error until caller finish; deferred old equality versus a
  fresh workspace/graph change; matching copies grant no physical-owner authority.
- Root-valid change plus graph corruption yields invalid before changed; saved
  workspace/owner changes retain first priority. No new locator/adoption on recheck.
- Explicit mutants: mutual-jobs/attempts-only membership, inner-join omission,
  per-table budget reset and premature transaction finish. Assertion-catch and
  restore exact source before full checks and six-job expected-head delivery.

This design changes no source/schema/dependency/workflow and runs no calibration,
simulation, protected acquisition or execution. Source evidence, original numeric
execution, missing immutable cost/action documents and actual files remain
unsupported until their own adapters. Record delivery in the
[handoff](../../handoffs/2026-10-24-p13c4b2b2-graph-adapters-v1.md).
