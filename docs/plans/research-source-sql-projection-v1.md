# P13c-4b-2b bounded source SQL projection v1

Date: 2026-10-23
Baseline: merged #212 (`be699bc`). Documentation only.

The [complete source contract](research-source-reference-observation-v1.md#3-complete-workspace-row-closure)
remains authoritative. This document fixes concrete projection order, shared
resource accounting and a small first code slice. Root rows alone are incomplete
consistency evidence; no complete candidate/source/registry/file proof is implied.

## 1. First slice and private APIs

Implement **4b-2b-1 root row observations and shared budget** first, inside a
private `confirmation_inputs::source_rows` module. Types/functions have only
parent-module visibility. No research/public re-export or runtime caller yet.

```rust
SourceLocator::new(campaign_id: &str, run_id: i64) -> AppResult<SourceLocator>
capture_root_rows_in(
    tx: &rusqlite::Transaction,
    expected: &ExpectedWorkspace,
    locator: &SourceLocator,
) -> AppResult<RootRowsObservation>
recheck_root_rows_in(
    tx: &rusqlite::Transaction,
    expected: &ExpectedWorkspace,
    saved: &RootRowsObservation,
) -> AppResult<()>
```

SourceLocator owns a 64-lowercase-hex campaign ID and positive i64 run ID.
Construction validates domains only. JS-safe conversion belongs to later source
assembly. Recheck uses the immutable saved locator, with no replacement request
argument. RootRowsObservation owns the actual WorkspaceObservation, locator,
the three exact root records below and ResourceUsage counters.

All fields are private. Clone/Eq/PartialEq are allowed; no Debug/serde/Default,
mutable accessor, unchecked constructor, public generic row/SQL factory or
RegisteredSource conversion. Raw JSON is OriginalJson; ordinary text is String;
integer cells are i64; finite REAL observations retain `f64::to_bits()` as u64.
No f64/Value reconstruction of JSON or new numeric identity enters the reader.

| Private record | Exact Rust field order |
| --- | --- |
| CampaignRoot | campaign_id, version, document_json |
| AdmissionRoot | id, discovery_run_id, campaign_id, instrument_id, batch_id, status, report_version, report_json, fee_pct_bits, slip_pct_bits |
| RunRoot | id, status, config_json |
| ResourceUsage | text_bytes: usize, json_nodes: usize |

Record statuses retain exact String values checked against existing schema
vocabulary: admission ELIGIBLE/NOT_ELIGIBLE; run idle/running/paused/completed/
failed/cancelled. IDs are positive SQLite integers, campaign IDs 64 lowercase
hex and batch ID exactly `trial-batch-v1:` plus 64 lowercase hex. Instrument ID
is nonempty printable ASCII (0x21..0x7e) <=256 bytes, without trimming; canonical identity proof is
later. Version/report-version are nonempty UTF-8 <=4096 bytes; supported-version
interpretation is later. Costs must be SQLite REAL and finite; negative values
can be observed as historical claims but cannot pass later execution suitability.

Root JSON is syntax-checked, not interpreted as a campaign/config/report schema.
Even TEXT `null` can be observed; it is not SQL NULL or a valid declaration.
Unknown versions/status suitability, campaign re-freeze, config enumeration,
complete job/attempt/market graph and authenticated digests remain subsequent
steps. A root observation is never sufficient input to confirmation freezing.

## 2. Exact root SQL and transaction order

Capture first calls capture_workspace_in in the caller's active transaction,
then reads campaign, admission, run in that order. Recheck first calls
recheck_workspace_in with the saved stamp, then repeats those same root reads
and compares every retained field exactly. No nested transaction, mutex,
registry, finish, write, mutation increment, callback, file IO, event or worker.
Success/refusal/SQL error leaves all rows/schema/total_changes/activity and the
caller's writer reservation unchanged. Deferred equality is historical only.

The fixed query selectors and logical columns are:

```sql
-- campaign: locator campaign_id parameter
SELECT campaign_id, version, document_json
FROM research_campaigns WHERE campaign_id = ?1
ORDER BY campaign_id LIMIT 2;

-- admission: locator run_id parameter
SELECT id, discovery_run_id, campaign_id, instrument_id, batch_id,
       status, report_version, report_json, fee_pct, slip_pct
FROM campaign_run_admissions WHERE discovery_run_id = ?1
ORDER BY id LIMIT 2;

-- run: locator run_id parameter
SELECT id, status, config_json FROM discovery_runs WHERE id = ?1
ORDER BY id LIMIT 2;
```

These are column manifests, **not permission to select unguarded payloads**.
For logical column j the actual query emits exactly two physical columns:
`valid_j, value_j`, preserving manifest order (2*j, 2*j+1). For required TEXT:

```sql
CASE WHEN typeof(c)='text' AND length(CAST(c AS BLOB)) <= ?2
     THEN 1 ELSE 0 END,
CASE WHEN typeof(c)='text' AND length(CAST(c AS BLOB)) <= ?2
     THEN c ELSE NULL END
```

The selector is ?1, ?2 binds 4096 for ordinary TEXT, and the equivalent JSON
pair uses ?3 bound to 4194304. Use fixed literal column names/query constants and bound integer caps, never
caller SQL or identifiers. INTEGER gates require typeof integer and the field's
positive/nonnegative domain; REAL gates require typeof real, followed by Rust
finite validation before retaining bits. Do not coerce INTEGER/TEXT to REAL.
Check markers before typed reads; guarded NULL after invalid data is a refusal,
not an absent optional. UTF-8/domain/OriginalJson checks follow before retention.

Require exactly one row for each root. Read a second row as a cardinality
sentinel and refuse it without retaining its payload; query_row alone would
miss damaged-schema duplicates. Missing roots refuse. After all three valid
singletons are read, require campaign/admission campaign IDs equal locator and
admission/run IDs equal the requested run. A collation/coercion match in SQL
cannot replace exact Rust comparisons. No latest/default/repair lookup.

Order within each table: validate the first row's marker/type/raw syntax/domain
in logical column order, then its uniqueness; finally cross-root links. Earlier
workspace errors retain their priority. Row invalidity/missing/duplicate/link/
budget refusal uses static `source_rows_invalid`; a fresh valid but different
root observation uses static `source_rows_changed`. Real SQL failures remain
AppError::Db. Do not include original content/paths/IDs in diagnostic strings.

## 3. Shared pre-copy budget

Preserve the parent **32 MiB retained TEXT / 262,144 JSON-node** joint caps,
4 MiB/65,536 per JSON, depth 64, decoded-key 1024 bytes, non-JSON TEXT 4096.
Use a private local ResourceBudget with checked used counters. It is created
once for the entire observation, not reset per table or JSON. RootRowsObservation
stores resulting ResourceUsage so later complete-graph construction continues
the same accounting. No public configurable-cap/bypass constructor.

Account every owned text copy in the returned observation: 64 bytes for the
locator campaign ID, 32 each for actual workspace/holder instance IDs (128
fixed bytes total), then every retained row TEXT/JSON cell in manifest order.
HolderKind enum, integers and REAL bits add no TEXT bytes. Do not count borrowed
caller expectations, transient decoded JSON strings/keys, diagnostic columns or
discarded sentinel payloads as retained text. Per-document structural limits
still bound transient parsing. A cloned observation is another bounded object,
not a claim that the whole process has a 32 MiB memory ceiling.

Reserve the fixed 128 bytes before capture/locator clone. Check remaining joint
bytes against guarded ValueRef text length before any owned cell copy. Validate
UTF-8 before String ownership. OriginalJson::parse currently discovers its node
count and then owns the bytes; checking the joint node cap afterward is too late.
Add only this parent-visible bounded constructor to its private module:

```rust
OriginalJson::parse_with_node_limit(raw: &[u8], remaining_nodes: usize)
    -> AppResult<OriginalJson>
```

Existing parse delegates with 65,536. The bounded constructor clamps its effective
node limit to min(65,536, remaining_nodes), rejects zero, and walks under that
limit **before** its sole original byte copy. Existing size/depth/key/syntax/
opaque-number behavior and static source_json_invalid stay unchanged. The SQL
layer maps that refusal to source_rows_invalid; do not expose parsing details.
After successful parse, charge its actual node count and raw byte length with
checked arithmetic. No parse-copy-then-check-joint-nodes path, refund, truncation
or retry under another cap. Any failure aborts the local observation/budget.

The three root JSON columns cannot alone reach the whole 32 MiB/262,144 caps.
Test ResourceBudget internally at exact and one-over joint limits (including
nonempty prior usage); do not inflate root fixtures or lower production caps.
Test-only remaining-budget states exercise resources, not live authorization.
Later graph reads must exercise whole-observation caps through actual rows.

## 4. Complete graph projection manifest retained for later code

After roots, retain these logical columns in this order. Every column uses the
same marker/value encoding, shared budget and exact type/null rules. T?, I?, J?
mean nullable text/integer/original JSON only where schema permits; INTEGER ID
references are positive when nonnull. JSON null remains distinct from SQL NULL.

| Table / selector and row order | Logical column manifest |
| --- | --- |
| discovery_jobs WHERE discovery_run_id; candidate_index, segment, id | id, discovery_run_id, candidate_index, strategy_id, dataset_id, segment, status, result_id(I?) |
| research_attempts WHERE discovery_run_id; candidate_index, id | id, attempt_key, hypothesis_id, strategy_id, dataset_id, discovery_run_id(I?), candidate_index(I?), status, input_fingerprint_json, engine_fingerprint_json, outcome_json(J?), result_artifact_id(I?), trial_event_id(T?) |
| strategy_def by distinct referenced id; id | id, type, original_definition_json, dsl_json(J?), strategy_hash, parent_strategy_id(I?) |
| hypotheses by distinct referenced id; id | id, hypothesis_hash, version, source, mechanism, applicability_json, failure_modes, strategy_hash, strategy_id(I?), parent_strategy_id(I?), variation_kind(T?), content_json |
| research_artifacts by distinct referenced id; id | id, sha256, kind, byte_len, relative_path |
| market_snapshots by exact supported declaration's snapshot_id; id | id, snapshot_id, version, instrument_row_id, instrument_id, interval, dataset_id, dataset_hash, price_basis, calendar_id, corporate_action_version(T?), cost_profile_version(T?), kind, status, as_of, coverage_json, content_json |
| market_instruments by snapshot instrument_row_id; id | id, instrument_id, revision, content_hash, version, market, venue, symbol, base, quote, asset_type, session_calendar_id, timezone, lot_size(REAL?), price_step(REAL?), min_notional(REAL?), listed_from(I?), delisted_at(I?), suspensions_json, source_capabilities_json, content_json |
| market_calendars by snapshot calendar_id; id | id, calendar_id, version, kind, timezone, content_hash, definition_json |
| datasets by snapshot dataset_id; id | id, exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash |
| market_snapshot_sources WHERE snapshot_row_id; provenance_id | snapshot_row_id, provenance_id |
| market_provenance by every referenced provenance_id; id | id, record_hash, version, instrument_id, interval, source, role, request_scope_json, raw_sha256, raw_byte_len, raw_artifact_path, raw_media_type, retrieved_at, available_at(T?), accepted, rejection_reason(T?), revision_of(I?) |

Nullable INTEGER declarations retain raw schema optionality, but complete source
membership must later require nonnull run/index/trial links and completed-attempt
artifacts. Semantic timestamps may be signed market-range integers, not positive
row IDs. accepted is exact integer 0/1; byte_len/raw_byte_len/count/index are
nonnegative. Do not apply an ID rule indiscriminately to all numeric cells.
Nullable REAL is either actual NULL or finite REAL; text/integers do not coerce.

Root slice reads none of these rows. Later typed graph adapters must preserve
this manifest and the parent's candidate/job/source caps, refuse cap+1 sentinels,
and check every referenced singleton without inner-join omission. Full frozen
enumeration must detect consistent deletion from both jobs and attempts;
comparing only their mutual indexes is insufficient. No selected subset or
successful-only filter. Supported campaign/config interpretation, missing parent
or market references, exact original trial membership and semantic digests need
their own adapter/fixture refinement before graph assembly code. Raw SQL closure
and logical registration completeness must not be conflated.

## 5. Required first-slice evidence and delivery

Use real migrated file-backed SQLite and isolated actual OS leases as in
[4b-1 tests](../../alpha-factor-forge/src-tauri/src/research/confirmation_inputs/tests.rs).
Tests may deliberately replace constraints/triggers in their owned fixture to
exercise corruption; production readers never repair them.

1. Capture all exact root bytes/types/bits; unchanged recheck, all old metadata
   helpers and diagnostic-only historical admission epoch/created_at/progress/
   name/error changes. Actual current workspace owner epoch still must match.
2. Active pending root writes visible only inside the caller, full raw typed
   row/schema snapshot restored by rollback, no mutation_seq/total_changes delta.
3. Independent zero-timeout writer remains Busy across success/refusal/SQL error
   until caller finish. Deferred old equality versus fresh root/workspace change;
   identity-matching copies stay consistency-only.
4. Missing each root, damaged-schema duplicate rows, substituted run/campaign,
   valid changed retained fields/status/cost bits/JSON and every owner/version
   change. Refuse without replacing saved locator or prepared request.
5. Invalid BLOB/NULL/UTF-8/types/positive IDs/hash domains, nonfinite REAL and
   integer-to-REAL substitution; marker-invalid cannot become nullable absence.
6. Exact/one-over text/JSON byte/node/depth/key limits, original long-float tokens,
   shared nonempty budget remainder and node limit zero/large-clamping. Compare
   budget limits before OriginalJson's byte ownership copy; static review must
   confirm it. Preserve all 15 existing original JSON tests.
7. Static no-runtime-caller/wire/default/Debug/unchecked constructor, generic SQL,
   registry/lock/finish/write/IO boundary. Explicitly catch omitted uniqueness,
   ignored shared node remainder, omitted recheck comparison and premature
   caller finish mutants; restore exact bytes before full verification.

Only 4b-2b-1 enters Next after this design's verified delivery. Complete graph,
semantic source assembly, pinned bytes and final actual-owner/IMMEDIATE/registry
fence remain separate. No schema, dependencies, existing public reader change,
spend, execution/command/provider/UI, policy/calibration/reserved-seed activity.
Production authorization remains [unavailable](research-final-admission-implementation-v1.md#5-activation-remains-unavailable).
Parent 4b-2b/4b-2/4b/P13c-4 stay incomplete.

Baseline #212 has 978 Rust (one ignored), 1242 Vitest / 80 files, verified native
smoke/restoration/cleanup and six successful final CI jobs. Its first E2E had
97 pass + one font-settled timeout/internal-retry flake; an unchanged-head
E2E-job-only repeat passed 98 with no retries. Do not rewrite that history as
six first-attempt clean jobs. Delivery evidence belongs in the
[handoff](../../handoffs/2026-10-23-p13c4b2b-source-sql-contract-v1.md).

Root/budget implementation evidence: [4b-2b-1 handoff](../../handoffs/2026-10-24-p13c4b2b1-root-observations-v1.md).
