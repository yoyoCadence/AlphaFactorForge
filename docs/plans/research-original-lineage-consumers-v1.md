# P13c-4b-2c-2b bounded original lineage consumers v1

Date: 2026-10-25
Baseline: merged #222 (`754b08a`). Design/data only.

The [producer contract](research-original-producer-projections-v1.md) and
[original adapter matrix](research-original-semantic-adapters-v1.md) remain
authoritative. #222 proves nine pre-code literal candidates against actual
writers, including the shared-base correction and preserved contradictory
library claims. It does not establish semantic source admission. This refinement
fixes bounded preparation first, before candidate/engine/hypothesis comparison.
No current runner/parser/identity, schema, dependency or workflow changes here.

## 1. Supported comparison boundary and API order

Only the existing discovery-config-v1 params, v2 fixed DSL and v3 walk-forward
formats in the literal catalog are contemplated. The original winning base,
axis order, deduplication, candidate order/identities and seed-v1 remain exact.
Manual, AI, code/blocks and unknown original formats have no discovery adapter.
Existing graph capture remains the sole source of the complete historical rows.

The eventual private entry is:

```rust
observe_original_lineage(rows: &SourceRowsObservation)
    -> AppResult<OriginalLineageObservation>
```

It must validate the entire family before returning any observation. Proposed
immutable output is config_hash plus ordered candidate records (index,
strategy_id, strategy_hash, hypothesis_id, hypothesis_hash, base_id). No source,
file, market, warmup, cost, registry, owner or admission field is implied. The
exact output-copy/hash-work budget and executable semantic comparison tests
must be refined before this later entry is implemented. No partially verified
record or public per-candidate constructor may escape.

**First code 4b-2c-2b-1 is only bounded preparation**, sections 2-5. It does not
implement that eventual entry or claim candidate/engine/hypothesis agreement.
Add private graph::original_lineage and the sole ancestor-visible preparation:

```rust
prepare_original_lineage(rows: &SourceRowsObservation)
    -> AppResult<PreparedOriginalLineage<'_>>
```

PreparedOriginalLineage borrows the exact sealed graph, owns the bounded parsed
run plan and parsed document vectors, and records immutable work totals. No
Clone/Debug/serde/Default, unchecked constructor, mutable getter, row-list input,
caller locator/budget/callback, runtime/research re-export or RegisteredSource
conversion. Borrowed getters stay within confirmation_inputs. Its lifetime
cannot outlive the graph. No complete graph/original-byte clone is retained.
Parsed values and a validated enumeration are preparation, not source proof.

## 2. Whole parse manifest before materialization

Before the first new serde_json::Value materialization, use only captured
OriginalJson byte lengths/node_count and checked arithmetic. Walk every retained
row in its saved order, once; sharing references does not duplicate stored rows.
Reserve the following logical payload work:

| Original document | Weight |
| --- | --- |
| run config_json | 2 |
| every strategy original_definition_json | 1 |
| every present strategy dsl_json | 1 |
| every attempt input_fingerprint_json and engine_fingerprint_json | 1 each |
| every hypothesis content_json and applicability_json | 1 each |

P_bytes/P_nodes are the weighted sums. The second config reservation covers the
resolved-config payload alongside the parsed original config; the config is
parsed from bytes only once. These are explicit serialized-payload/node work
units, not a claim about allocator bytes or whole-process peak memory. Fixed
struct/map/vector/index overhead is separately bounded by the graph's <=4096
candidates/strategies/hypotheses/attempts, <=8192 jobs and existing row/text caps.
OriginalJson already enforces <=4 MiB/document, <=65536 nodes/document, depth 64,
1024-byte decoded keys, unique decoded keys and syntax validity.

Require P_bytes<=33554432 and P_nodes<=262144, inclusive, before any parse/copy.
All additions/multiplications/conversions are checked. Refuse the whole operation
on overflow/one-over; do not reset per candidate/table or truncate the family.
The borrowed graph's existing capture budget stays unchanged and is distinct
from this new preparation budget. A valid graph may exceed preparation work.

Campaign/admission report, outcomes, artifacts and market/provenance documents
are not parsed by this first slice. Their original bytes remain in the graph;
no semantic meaning/eligibility is assigned to them. The previously captured
campaign/snapshot/dataset membership is historical consistency, not live proof.
Do not add a second campaign re-freeze or parse an excluded document silently.

After preflight, parse each listed stored document exactly once with the
unchanged default serde_json policy; the resolved config uses the original
hardware-independent parser-cores rule. Keep parsed strategy definition/optional
DSL, attempt input/engine and hypothesis content/applicability in separate
private vectors aligned one-to-one with the borrowed rows. Use borrowed Values
thereafter; no serialize/reparse cycle or manual policy-header parser is needed
for preparation. Marked/unknown documents may be syntactically prepared but
must be rejected by the later supported-format semantic gate. A nonrepresentable
legacy number refuses here; no alternate parser or token repair is attempted.

## 3. One shared run-plan path and combined pre-enumeration limit

The existing [selector](../../alpha-factor-forge/src-tauri/src/research/confirmation_inputs/source_rows/selection.rs)
already validates original config, raw Cartesian count, B/Q scratch, enumeration
accounting/identities and deterministic cores before returning candidate keys.
It discards the full plan. Do not enumerate once per candidate or independently
reimplement those rules in the consumer.

Extract that exact existing run-config parse/preflight/enumerate/plan-validation
sequence into one helper private to source_rows:

```rust
prepare_run_plan(
    original: &OriginalJson, expected_dataset_hash: &str,
    prior_bytes: usize, prior_nodes: usize,
) -> AppResult<PreparedRunPlan>
```

PreparedRunPlan privately owns raw_config: Value, config: ResolvedDiscoveryConfig,
plan: CandidatePlan and expansion work totals. No public constructor/serde or
mutable access. An internal consuming parts accessor may preserve the existing
selector's moves; the consumer gets borrowed config/plan access. This helper is
not a source/semantic factory. Its numeric initial work is internal accounting,
never a configurable ceiling or externally supplied allowance.

Existing derive_selection passes the exact frozen binding dataset hash and zero
initial work, preserving its behavior, error codes, original campaign checks,
output and caps. Keep config dataset-hash comparison before scratch/enumeration
and the plan dataset check against that same expected hash. The new sealed-graph
preparation passes its captured dataset hash and exactly its complete P sums;
no external expected-hash/allowance input is exposed. Validate initial
work before parsing; map internal failures to the new static preparation error.
The same config is parsed once and enumerated once per preparation, not once
inside derive_selection and then again. Existing capture and a later separate
preparation are separate bounded operations, not one retained plan/cache.

Keep the existing scratch_preflight formula and raw-count check unchanged:
for each base, p is the raw axis product, S/T are serialized base strategy
bytes/value nodes, I is serialized base-id bytes, and
A=sum(serialized axis-key bytes + maximum serialized finite axis-value bytes +3).
B=S+I+2+2*A; Q=T+2+2*axis_count. E_bytes=sum(p*B), E_nodes=sum(p*Q).
Require the raw product total nonzero and <=declared candidates cap<=4096 before
Cartesian cloning, even when pruning/dedup would shrink it.

Before entering enumerate_candidates, checked combined work must satisfy
**P_bytes+E_bytes<=33554432 and P_nodes+E_nodes<=262144**, inclusive. Do not give
parse and expansion separate fresh allowances or count only final unique rows.
Keep the existing odometer/prune/dedup/hash-sort/base attribution/seeds and plan
validation exactly. The fixture's shared-dedup case has raw 6, final 4.
All prospective stored-row parsing is reserved in P before enumeration, even
when those row Values are materialized afterward. No hash/semantic comparison
or runtime work is added by preparation. All errors become the static
`original_lineage_invalid`, without saved content/IDs or partial preparation.

## 4. Independent pre-code preparation controls

The [new work fixture](../../alpha-factor-forge/fixtures/research/original-lineage-preparation-v1.json)
references #221's unchanged literal producer documents. A standalone Python
JSON/arithmetic oracle, without importing/running project code, counted UTF-8
lengths and JSON values (object keys are not nodes), included config twice,
every original strategy/optional DSL, every input/engine and each distinct saved
hypothesis content/applicability once, then independently applied B/Q above.
It does not compute expected values by running the future adapter.

| Case | P bytes/nodes | E bytes/nodes | Combined bytes/nodes | Raw/final |
| --- | --- | --- | --- | --- |
| v1-ordered-axes | 9704 / 461 | 1743 / 120 | 11447 / 581 | 3 / 3 |
| v1-shared-dedup | 13392 / 628 | 3309 / 228 | 16701 / 856 | 6 / 4 |
| v2-fixed-dsl | 6503 / 331 | 460 / 39 | 6963 / 370 | 1 / 1 |
| v3-walk-forward | 6923 / 345 | 460 / 39 | 7383 / 384 | 1 / 1 |

The fixture freezes reference-document checksums, candidate/hypothesis counts,
manifest entries, and every P/E/combined count. Engine is charged for every
stored attempt, even when the JSON is identical. A shared stored hypothesis is
charged once, not per reference. Optional absent DSL is omitted, not null.
No real data, random sampling, calibration or reserved seed is acquired.

First-code positive tests seed complete raw graphs using these **literal**
producer documents plus the existing complete-graph market/reference fixture.
Use original row IDs/references consistently (strategy index+1, run 1, dataset 7;
syntactic trial event IDs and train/validation jobs are fixture claims only).
Build all original strategy, hypothesis, attempt/input/engine rows from frozen
literals, never from future adapter outputs. Capture through actual guarded SQL
and caller transaction. Do not turn the old synthetic graph into a semantic
positive by repairing its claims. Retain explicit preparation of uninterpreted
synthetic claims as evidence that preparation grants no semantic authority.

## 5. Required first-code verification and delivery

Reuse actual bounded OriginalJson and capture infrastructure. New fixture roots
must use bounded create_dir/AlreadyExists skipping, an actual isolated workspace
lease/owner, close handles before deleting only their newly created roots, and
never reuse/remove old PID/counter directories. No broad test-TEMP refactor.

Required tests: all four independently counted sealed graphs and exact parsed
row/value/plan alignment; shared hypothesis count vs per-attempt engine charging;
legacy long-float interpretation/no numeric repair; excluded syntactically valid
uninterpreted JSON and out-of-graph malformed diagnostic claims stay unparsed; nonrepresentable selected JSON refuses; ordinary selector
manifests remain identical; combined P+E exact/one-over with nonempty prior work;
checked arithmetic overflow; raw-count/pruning/dedup refusal before Cartesian
cloning; borrowed graph/transaction/owner/mutation unchanged on success/refusal.
At least one real captured graph must hit each parse and combined byte/node
boundary; bounded private arithmetic tests supplement, not replace those cases.
Use parse/enumeration entry counters/tripwires to prove refusal order. Instrument
only the new parse/shared enumeration boundary under cfg(test), no production
callbacks or global parser changes.

Four temporary runtime mutants must assertion-fail: count config once; charge
shared hypotheses per candidate; reset P before E; materialize stored JSON before
P preflight. Restore exact source bytes. Full isolated locked Rust precedes
Node, then locked all-target check/Clippy/scoped format/typecheck/test/build;
six exact-head/base CI, fresh reviews/main, expected-head merge and clean-main
full-tree verification precede subsequent code. Format only extracted helper /
new code, not legacy files globally. Keep original failure evidence.

This design PR validates frozen reference checksums, independent counts/B/Q,
source rules/visibility, links, scope and task lifecycle. No local runtime suite
is rerun only for docs/data; normal six-job CI still gates delivery. Green design
CI does not mean the future preparation controls ran.

## 6. Later semantic gates remain mandatory

After bounded preparation delivers, finalize post-preparation copy/hash-work
budget, immutable output and literal/mutant controls before semantic code. Each
candidate must match the original wrapper's declared supported numeric policy,
type/hash/cost-bearing definition and exact enumerated winning base. Preserve
original bytes for raw digests; allow compatible whitespace/key order while
refusing contradictory saved DSL. param_schema_json/name/source/lifecycle are
mutable diagnostic fields outside the captured semantic graph; do not query
or promote them to authority. Nested original DSL params remain executable data.

Input needs its exact 12 keys, config canonical hash, dataset/interval/run/index/
strategy IDs, applied axes and seeds. Engines require the explicit nine-key
literal original catalog and exact original configContracts, never a replacement
current binary fingerprint. Shared hypothesis requires the complete original
nine-key content, six-key applicability, scalar projections, original mechanism/
axis order/failure-mode/variation and canonical content hash; strategy-doc-v1
base hash and NULL IDs remain distinct from candidate strategy-v2. All original
winning bases and every retained referenced hypothesis must agree, not any base
with a matching hash or a selected candidate subset.

Legacy checksum/identity agreement is historical consistency, not proof of
original numerical execution. No parser feature/dual-hash/token repair/numeric
policy invention follows. Warmup/market/trial, authenticated files/candles and
missing original cost/action evidence remain separately scoped. Final current
owner/registry/IMMEDIATE admission and policy/mediation authority stay unavailable.
Parents 4b-2c/4b-2/4b/P13c-4 remain incomplete. No RegisteredSource or execution.
