# P13c-4b-2 source/reference observation contract v1

Date: 2026-10-23
Baseline: merged #210 (`443940d`). Documentation only.

This refines [4b's remaining obligations](research-workspace-input-observation-v1.md#4-remaining-source-and-input-contracts).
The output is an observation of original workspace rows, not authenticated
files, complete current registry history, physical ownership or permission to
execute. Parent 4b remains incomplete through 4b-3 and final stale-input tests.

## 1. Grounded boundaries and implementation order

| Existing source | Consequence |
| --- | --- |
| [Campaign storage](../../alpha-factor-forge/src-tauri/src/db/campaign.rs), [schema 0010](../../alpha-factor-forge/src-tauri/migrations/0010_campaign_admission.sql) | Read the exact run admission and exact campaign; re-freeze the original declaration. ELIGIBLE is historical evidence, not a current permit. P06 cost-profile references have no numeric document. |
| [Run/job store](../../alpha-factor-forge/src-tauri/src/db/discovery.rs), [config](../../alpha-factor-forge/src-tauri/src/discovery_core/config.rs) | Complete enumeration comes from the stored config, not jobs selected by UI. Jobs must contain exactly its Train/Validation pairs. |
| [History](../../alpha-factor-forge/src-tauri/src/research/history.rs), [schema 0007](../../alpha-factor-forge/src-tauri/migrations/0007_research_history.sql), [trial links](../../alpha-factor-forge/src-tauri/migrations/0009_trial_ledger_binding.sql) | Display DTOs replace some bad JSON with Null. Read original content/fingerprint/outcome strings and exact attempt/trial/artifact links instead. |
| [Strategy rows](../../alpha-factor-forge/src-tauri/src/db/repositories.rs), [numeric policy](../manual-strategy-numeric-policy-v1.md) | original_definition_json already exists. Preserve it; dispatch its declared policy when interpreting it. Do not reserialize parsed legacy floats to establish a new identity. |
| [Runner lineage/artifacts](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs), [engine fingerprint](../../alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs) | Lineage binds config, candidate, seeds, dataset and engine. candidate-result-v1/v2 contains observed Train/Validation results; it is not protected unseen input. |
| [Market schema](../../alpha-factor-forge/src-tauri/migrations/0008_market_foundation.sql), [campaign resolution](../../alpha-factor-forge/src-tauri/src/research/campaign_snapshot.rs) | Follow exact snapshot revision/source references, never latest/unique guesses. Existing resolution loads candles and is not a bounded row-only helper. |
| [Original trial reconstruction](../../alpha-factor-forge/src-tauri/src/research/trial_ledger_transfer/historical_observation.rs), [pure source](../research-confirmation-contracts-v1.md#2-batch-body) | Original trial witnesses prove original membership/protocol, not workspace files or current counts. Public RegisteredSource is a supplied claim. |
| [Artifact store](../../alpha-factor-forge/src-tauri/src/research/artifacts.rs) | Existing read allocates/reads before checking length. Do not call it from the new bounded source reader or change unrelated exploratory reads. |

Implement in this order, with one verified PR/merge before the next:

1. **4b-2a: bounded original JSON observations**, the exact small API in section 2.
2. **4b-2b: complete bounded workspace row projection**, sections 3-4. Before
   its code, record the concrete per-table Rust tuple/SQL column order and test
   fixture joins in a focused refinement; the membership/bounds below stay fixed.
3. **4b-2c: semantic source assembly**, section 5. Before code, freeze supported
   original format adapters and independent digest fixtures. Refuse unsupported
   legacy interpretation; do not silently invent missing cost/action evidence.
4. **4b-3: immutable input pinning**, a separate path/handle/candle/resource
   contract. Only verified bytes can upgrade row hash claims to file evidence.

Only 4b-2a enters Next after this design's verified delivery. These steps do not
complete 4b-2 or authorize an all-at-once source builder.

## 2. First code slice: bounded original JSON

Add private `research::confirmation_inputs::original_json`; no public module or
runtime caller. The parent may re-export to research only when an actual row
reader needs it. Freeze this API, confined to confirmation_inputs:

```rust
OriginalJson::parse(raw: &[u8]) -> AppResult<OriginalJson>
OriginalJson::as_bytes(&self) -> &[u8]
OriginalJson::node_count(&self) -> usize
```

OriginalJson owns private original bytes plus checked node count. It has no
Deserialize/Serialize/Default, unchecked constructor, mutable accessor or public
field. Clone/Eq/PartialEq are allowed; do not derive Debug that prints source
content. Construction validates JSON syntax/structure only, not a source schema,
version, numeric policy, original strategy identity or authenticity.

| Inclusive bound | Value |
| --- | --- |
| Original input, including leading/trailing JSON whitespace | 4 MiB UTF-8 bytes |
| All JSON value nodes, including root, containers and scalar values | 65,536 |
| Value depth; root is 1, every member/element value adds 1 | 64 |
| Each decoded object key | 1,024 UTF-8 bytes |

An object member's key is not an extra node. Count empty arrays/objects as one
node. Count all values, including null and booleans. Refuse the first excess;
never truncate. Root may be any JSON value, including null; SQL NULL remains a
different absence and later source schemas may require an object.

Check raw size and UTF-8 before allocation/parsing. Accept exactly one JSON
value plus JSON whitespace; reject empty input, trailing values/garbage, BOM,
comments, malformed escapes/numbers and invalid Unicode scalar strings. Reject
duplicate **decoded** keys at every depth, including `a` versus `\u0061`;
do not perform Unicode normalization. The same key in different objects is legal.

Use the already-enabled serde_json raw_value support and bounded structural
walking. Preserve all number tokens without converting them to f64, i64 or
serde_json::Value. Valid JSON numbers such as -0, 1.00, large integers and 1e999
are opaque tokens here; downstream version/numeric-policy adapters must reject
unsupported/non-finite interpretations. This lexical acceptance is not approval
of those values for execution or existing domain codecs.

Decode keys and string values only as needed to validate Unicode/duplicates.
Do not retain a second parsed tree, normalize whitespace/escapes, canonicalize,
hash, rewrite tokens or emit original content in errors. Bound structural
recursion before descending; the library recursion limit is not the contract.
Use one static AppError::Other detail `source_json_invalid` for every refusal.
No SQL, file IO, locks, callbacks, registry, dependency/feature changes or edits
to existing history/manual/config parsers belong in this code slice.

Required evidence: exact byte preservation across long-float counterexamples,
exponents, negative zero, escaped strings and whitespace; all scalar roots;
escaped/nested duplicate keys; valid equal keys in different objects; malformed
syntax/UTF-8/surrogates/trailing values; exact and one-over byte/node/depth/key
caps, including multibyte keys and branching trees. Independent expected byte
and node counts must not call the implementation. Catch omitted duplicate,
node/depth limit and original-byte-preservation mutants through explicit
assertions; restore exact source before full checks and six-job delivery.

## 3. Complete workspace row closure

The later reader takes a domain-checked exact campaign ID and positive run ID,
the existing ExpectedWorkspace, and a borrowed active workspace transaction.
The request carries no candidate subset, arbitrary path, claimed hash map,
latest selector or caller-provided row list. A private immutable observation
contains the actual workspace stamp and ordered original typed rows. No wire
constructor or conversion to RegisteredSource is permitted in the row layer.

Capture/recheck call the 4b-1 metadata reader first. Read all rows below in the
same snapshot, preserving original text and integer/finite-REAL types. Recheck
repeats the exact closure and compares every retained value, including membership;
it never updates the saved observation and continues. Matching copies/deferred
snapshots remain consistency-only. Final IMMEDIATE/actual runtime lease and
registry lock ordering stay in [4d](research-final-admission-implementation-v1.md#4-integrated-lock-and-crash-obligations).

| Exact scope and order | Retained projection and membership obligation |
| --- | --- |
| research_campaigns by requested campaign_id | campaign_id, version, original document_json; exactly one. |
| campaign_run_admissions by requested discovery_run_id | id, run/campaign/instrument/batch IDs, status, report_version, original report_json, fee_pct, slip_pct; exactly one and same requested campaign. Epoch is historical diagnostic evidence, not the current owner tuple. |
| discovery_runs by requested ID | id, status, original config_json; exactly one. Exclude name/progress/best/error/timestamps. |
| All discovery_jobs WHERE discovery_run_id, ORDER BY candidate_index, segment, id | id, run/index/strategy/dataset IDs, segment, status, result_id. Exactly one Train and one Validation per enumerated candidate; no extra or duplicate index/segment, even a Test row inserted in a damaged schema. |
| All research_attempts WHERE discovery_run_id, ORDER BY candidate_index, id | id, attempt_key, hypothesis/strategy/dataset/run/index IDs, status, original input/engine/outcome JSON, result_artifact_id, trial_event_id. Exactly one per enumerated candidate; matching jobs/attempt key and no missing/null candidate/trial link. |
| Distinct strategy_def IDs referenced by these jobs/attempts, ORDER BY id | id, type, original_definition_json, nullable original dsl_json, strategy_hash, parent_strategy_id. Exclude editable name/lifecycle/source/prompt/schema/display timestamps. |
| Distinct hypotheses referenced by these attempts, ORDER BY id | id, hypothesis_hash, version, source, mechanism, original applicability_json, failure_modes, strategy_hash, nullable strategy/parent IDs, variation_kind and original content_json. Keep all semantic scalar/document projections for contradiction checks. |
| Distinct research_artifacts referenced by these attempts, ORDER BY id | id, sha256, kind, byte_len, relative_path. Require a nonnull artifact on every completed attempt; do not use latest result summaries as replacement. |
| Exact declared market_snapshots ID for the admission's instrument | id, snapshot_id, version, instrument_row_id/instrument_id, interval, dataset_id/hash, price_basis, calendar_id, corporate_action_version, cost_profile_version, kind/status/as_of, original coverage_json/content_json. Exactly one. |
| Its exact market_instruments revision and market_calendars calendar_id | All non-created_at columns from schema 0008, including raw content/definition, numeric lot/step/notional, listing/suspensions/capabilities and existing content hashes. No latest-revision fallback. |
| Exact referenced datasets row | id, exchange/symbol/interval, bounds, candle_count, source, dataset_hash. Candle bytes/identity are verified later; this row is only a claim. |
| All market_snapshot_sources for this snapshot, then distinct market_provenance by id | Both join IDs; every non-created_at provenance column (including acceptance/revision/scope/raw-file metadata). No selected source subset or independent comparison-source promotion. |

Require complete query membership before filtering for suitability. A failed,
unfinished or unsupported member refuses the source; do not shrink the family
to its completed winners. Repeated strategy/hypothesis/artifact references are
deduplicated only in their row storage, not in candidate membership/counts.
Unrelated runs and unreferenced records are outside this exact closure.

Trial registry rows are not in this SQLite database. Retain all attempt event
IDs and admission batch ID as claims; section 5 joins the complete actual batch
through separately validated original/current ledger evidence. No nested registry
lock or workspace callback is allowed from this row reader.

## 4. SQL resource, type, refusal and recheck contract

These limits apply to the new dormant preparation, not existing exploratory
readers. All arithmetic is checked before allocation/copy/reserve.

| Bound | Value |
| --- | --- |
| Complete candidates / attempts / distinct strategies / hypotheses / artifacts | 4,096 each |
| Job rows | 8,192 |
| Snapshot source links / distinct provenance rows | 1,024 each |
| Each JSON field | Section 2's 4 MiB / 65,536 nodes / depth 64 |
| Joint retained TEXT bytes, including IDs/paths/JSON | 32 MiB |
| Joint JSON value nodes across the entire observation | 262,144 |
| Each non-JSON text cell | 4,096 UTF-8 bytes; domains impose tighter bounds |

Stream fixed per-table queries in the listed order. Use a cap+1 sentinel row
or a bounded preliminary count, **refusing** excess; never return the first cap
as complete. Do not inner-join away missing references or read oversized row
payloads into a Vec and check afterward. Validate typeof and
length(CAST(column AS BLOB)) in SQL before exposing a variable-width value;
return a separate validity marker so invalid TEXT/BLOB is not confused with a
legitimate nullable column. Copy bounded ValueRef bytes only after checking the
remaining joint budget. Validate UTF-8 and strict original JSON before retention.
No SQLite numeric/text coercions, defaults, fallback Null DTOs or repair writes.

IDs/counts/indexes retain exact SQLite INTEGER values; row IDs are positive,
indexes nonnegative and source-wire integers must later fit JS safe range.
REAL cells require finite f64; preserve their returned bits (including signed
zero if present), never decimal-reserialize them for row equality. Accept NULL
only for explicitly nullable schema columns; JSON `null` is separate. Do not
interpret timestamps/PIDs/heartbeat as source liveness or hash inputs.

Refusal order: active transaction + existing workspace checks; bounded type/raw
syntax validation in table/order/column order; exact cardinality/reference
closure; semantic suitability later. Recheck validates the fresh closure before
comparing it, then refuses any retained field/member change. Use static
`source_rows_invalid` / `source_rows_changed` for row invalidity/change; preserve
4b-1 errors and real SQL AppError::Db. No original IDs/JSON/paths in diagnostics.
Success/refusal/SQL error must preserve active caller transaction, reservation,
all rows/schema, total_changes and mutation_seq. No commit/rollback, DML, nested
transaction, registry, file loading, mutex, event or worker.

Before 4b-2b completion, prove real migrated SQLite complete/subset/extra-member
cases, missing referenced rows without join omission, SQL type/UTF-8/JSON/NULL
corruption, each exact/one-over field/joint/member bound, every source/ref change,
unchanged caller state and independent writer contention. Old DTO behavior is
unchanged. Final stale-file/lease tests remain 4b-3/4d obligations.

## 5. Original-byte digests and semantic assembly

Keep original identities separate from new P13 host binding digests. Existing
strategy-v2, dataset-content-v2, campaign/family/snapshot/calendar/provenance
algorithms and numeric policies are unchanged. A row claim that looks like a
hash is never proof that its original bytes or semantic inputs match.

For new host digests only define `H(domain, parts)` as SHA-256 of ASCII domain,
one zero byte, u64 big-endian part count, then for each part u64 big-endian byte
length followed by the exact bytes. No JSON serialization enters this framing.
Indexes/IDs included as parts use u64 big-endian, not decimal text. Domains and
ordered inputs are frozen here; independent byte/hash fixtures precede assembly.

| P13 field | Derivation and required evidence |
| --- | --- |
| campaignId | Re-freeze the exact bounded original declaration with its existing version and require stored/requested identity agreement. |
| campaignDocumentHash | SHA-256 of exact stored document_json UTF-8, including whitespace; separately verify its re-freeze. |
| workspaceId / runId / instrumentId / familyId | Actual captured identity/requested run, exact declaration/admission instrument, existing canonical family derivation. Pure source requires lowercase workspace ID; upper-case runtime identity must refuse conversion, never normalize silently. |
| trialBatchId | Actual admission/trial batch has `trial-batch-v1:<64 lowercase hex>`; the pure source field is exactly its 64-hex suffix. Restore exactly one prefix for ledger lookup, with complete batch/attempt/index/hash/workspace/family membership verification. |
| strategyHash | Existing saved and enumerated strategy-v2 identity must agree under its declared original numeric policy. No legacy float rehash under a new parser; unsupported original reconstruction refuses. |
| definitionHash | H(`p13-definition-original-v1`, [original_definition_json bytes]). Parse/verify its original supported definition separately; raw hashing alone cannot certify semantics. |
| executionHash | H(`p13-execution-original-v1`, [original run config_json, original attempt engine_fingerprint_json]). Bind candidate index via the enclosing candidate record and original input lineage; this digest does not replace strategy identity or numeric-cost checks. |
| resultArtifactHash | Exact original candidate-result-v1/v2 file SHA-256, equal to immutable artifact row hash/length/kind and parsed run/index/attempt/strategy/dataset lineage. A matching row claim is insufficient until 4b-3 bounded file authentication. |
| lineageHash | H(`p13-lineage-original-v1`, [original hypothesis content_json, original attempt input_fingerprint_json, original attempt engine_fingerprint_json, exact trial_event_id UTF-8]). Compare every corresponding scalar projection and exact original batch event. |
| engineManifestHash | H(`p13-engine-manifest-original-v1`, [index u64be, original engine fingerprint] repeated for every complete candidate in ascending index). Includes all engines; no selected/first/latest representative. |
| minimumWarmupBars / minimumEmbargoBars | Derive from the exact supported original strategy/DSL and execution/split contracts; never accept caller integers. Freeze a supported adapter and independent fixtures before semantic code. Unsupported analysis refuses. |
| snapshotId / instrumentRevisionHash / datasetHash / interval / calendarId / calendarHash | Exact referenced supported original market objects; reproduce existing identities with their original policy and later verified actual candles/calendar. Stored scalar hashes alone remain claims. |
| corporateActionsHash / costProfileHash | Require actual immutable supported original action/cost documents and version/hash reproduction. P06's version/identifier-only rows do not supply them. Missing originals refuse; do not hash a version string or synthesize a document. |
| numericCostsHash | H(`p13-numeric-costs-bits-v1`, [fee_pct f64 bits u64be, slip_pct f64 bits u64be]). Require finite nonnegative exact values consistent with original run config's declared parser and the authenticated cost profile. No new float serialization or P06 numeric-authenticity claim. |
| sourceEvidenceHash | Remains unavailable until 4b-3 freezes complete original provenance/raw-file framing and independent fixtures. No row-only placeholder/hash, selected source or public caller value may fill it. |

Strict original JSON is necessary, but not a complete semantic adapter. Re-freeze
campaign; parse original config with its declared legacy semantics; enumerate the
whole candidate set; verify jobs/attempts/hypotheses/engine/input projections and
supported result artifacts; join the complete original trial batch. Never derive
generation/ranking/prompts from Test or protected Validation reports. Current
registry counts/share/provenance/overlap and physical workspace guard remain the
later final fence, not an old source witness.

No RegisteredSource builder ships until every required digest and input has a
supported authentic derivation. A missing sourceEvidenceHash/cost/action/warmup
adapter is an explicit unsupported refusal, not an empty string, None, boolean,
known-unseen claim or invitation to change the pure wire format.

## 6. Delivery and authority limits

No source/runtime/schema/dependency/workflow edits in this design PR. Baseline
is #210's 963 Rust (one ignored), 1242 Vitest / 80 files, 98 first-attempt E2E,
six successful CI jobs and verified native restoration/cleanup. Each code slice
runs its relevant local checks, full locked Rust before Node-heavy checks, and
six exact-head CI jobs before expected-head merge/clean-main verification.

Managed protected acquisition, complete manual mediation and statistical policy
remain [unavailable](research-final-admission-implementation-v1.md#5-activation-remains-unavailable).
No permission from rows, matching copies, private data wrappers, hashes, imported
witnesses or test fixtures. No migration, reservation/spend, confirmation job,
ordinary runner activation, command/provider/UI, B/alpha/threshold/policy change,
calibration or reserved seed 20261117. Record delivery in the
[handoff](../../handoffs/2026-10-23-p13c4b2-source-reference-contract-v1.md).
