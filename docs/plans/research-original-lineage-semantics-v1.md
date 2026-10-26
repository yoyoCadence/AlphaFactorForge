# P13c-4b-2c-2b-2 original lineage semantics v1

Date: 2026-10-25
Baseline: merged #224 (`3241451`). Design/data only.

The [preparation contract](research-original-lineage-consumers-v1.md),
[producer controls](research-original-producer-projections-v1.md) and
[original adapter matrix](research-original-semantic-adapters-v1.md) remain
authoritative. Actual producer conformance and bounded preparation have delivered.
This contract fixes the next whole-family historical semantic observation.
It grants no source/admission/execution authority and changes no old producer.

## 1. First supported formats and sealed API

First support is deliberately the producer-grounded catalog: discovery-config-v1
with params bases, and discovery-config-v2/v3 with fixed DSL bases. A v1 family
may have multiple ordered/swept/deduplicated bases; v2/v3 may have multiple fixed
DSL bases. Every base must have the matching mode. Valid runner configs outside
this first catalog (v2/v3 params or mixed-mode families), manual/AI/code/blocks,
marked definitions and candidate-bound/parented hypotheses remain unsupported.
Do not change the runner or promote these formats through a permissive fallback.
Additional formats require independently frozen actual producer controls and a
separate support extension; the parent source adapter is not complete without it.

Add this sole ancestor-visible semantic constructor in private original_lineage:

```rust
observe_original_lineage(rows: &SourceRowsObservation)
    -> AppResult<OriginalLineageObservation>
```

It calls the existing prepare_original_lineage exactly once, then a private
child semantics implementation over that preparation. No public/ancestor entry
accepts arbitrary PreparedOriginalLineage, rows/parts/subsets, expected hashes,
locator, callback, budget or alternate catalog. No SQL/IO/registry/owner/clock/
hardware/worker/execution access, recheck/finish/commit or runtime/research export.

OriginalLineageObservation privately owns config_hash: String and ordered
candidates: Vec<CandidateLineageObservation>. Each candidate has index: i64,
strategy_id: i64, strategy_hash: String, hypothesis_id: i64,
hypothesis_hash: String, base_id: String. Clone/Eq/PartialEq only; no Debug/serde/
Default, unchecked factory, mutable access or RegisteredSource conversion.
Borrowed getters stay within confirmation_inputs; indexes/IDs may copy. A complete
family either returns once or refuses; no partial/selected success escapes.

## 2. Candidate and original declared-cost agreement

Resolve every prepared candidate by its captured attempt/strategy references;
require complete ordered plan/row alignment and every retained strategy exactly
once, preserving raw failed members. Strategy/hypothesis vectors are SQL-ID-
ordered independently of candidate/base order. Pair their parsed documents with
their own captured rows, then use bounded borrowed ID indexes to resolve attempt
references. Never zip those vectors directly to the candidate/base sequence. Use the original enumerated candidate and
winning base, not a replacement enumeration or any base with a matching hash.
Captured graph membership is historical consistency, never a current live fence.

First discovery definitions must have neither definitionVersion nor numericPolicy
marker. Stored type and original definition mode must agree with the supported
params/dsl branch, and strategy parent_strategy_id must be NULL in this adapter.
An otherwise valid library parent relation is unsupported here, not repaired or
classified as a producer bug. Mutable name/source/lifecycle/param_schema_json
remain outside this semantic graph and must not be queried or made authority.

Use the unchanged [core strategy-v2 helper](../../alpha-factor-forge/src-tauri/src/discovery_core/identity.rs)
with the parsed original definition and its finite nonnegative feePct/slipPct.
Recomputed identity must equal both the original stored identity and the exact
plan identity. Its existing tagged binary encoding covers the entire wrapper,
including dormant params fields, and normalizes numeric zero as before. Do not
use raw JSON SHA or strict Value equality to replace that legacy identity; for
example integer/float JSON spellings can have the same existing binary64 identity.
No parser/policy/dual-hash/token repair or identity migration is introduced.

Require original admission REAL fee_pct_bits/slip_pct_bits, parsed config
benchmark_costs, and each candidate's parsed fee/slip to be finite, nonnegative
and bit-identical. The original config parser already checks base costs. Preserve
SQL/parser zero bits: a -0/+0 disagreement refuses rather than being repaired.
This only compares retained declared costs; it does not authenticate the missing
original cost profile or produce numericCostsHash/original-execution proof.

Params rows require absent dsl_json. DSL rows require a nested original DSL and
present saved dsl_json. Compare their unchanged existing canonical binary value
encodings, preserving structure/strings/array order and legacy numeric meaning.
The preflight shape gate checks outer supported mode/type, cost fields and DSL
presence; it does not run a replacement deep DSL validator on unverified claims.
The full wrapper identity already binds the exact validated enumerated DSL;
DSL scalar validation uses binary64 in the existing executor. Compatible
whitespace/key order and equivalent existing numeric spellings may agree;
contradictory saved DSL refuses even though the writer preserves library rows.
Do not rebuild/refresh a saved definition or add a third identity encoder.

## 3. Exact original input, engine and shared hypothesis

Input must have exactly the original 12 keys from the producer contract.
Require integer-typed envelope-independent IDs/index/rootSeed/randomEntry fields
(using exact i64/u32 domains, not string/float coercion); literal envelopeVersion,
config canonical hash, dataset ID/hash/interval, strategy ID/hash, winning baseId,
candidateIndex, exact appliedAxes key set and seed object. Each applied-axis
number must have the original enumerated f64 bits, including zero sign; no
subset/default axis or current seed substitute. The complete graph already
binds attempt key/run/index/ref membership; preserve those invariants.

Read every saved engine. Require exactly the nine original top-level keys and
the explicit package 0.1.0 / execution backtest-execution-v1 / metrics metrics-v2 /
benchmarks benchmark-suite-v1 / gate gate-v1 / score score-v1 /
validationRecord validation-record-v2 / benchmarkRecord bench-record-v1 catalog.
configContracts must equal the complete explicit envelope catalog in the
producer plan and the original raw config contracts, including optional-key
omission, never null. No current env/package/constants/fingerprint replacement.
Unknown/missing/extra/changed versions refuse, including one bad member late in
the family. Config canonical hash is SHA-256 of existing research canonical JSON
of the originally parsed raw config, not raw bytes or newly resolved config.

Build only a bounded borrowed winning-base/reference index. For each original
winning base, all its candidates must reference the same hypothesis; different
winning bases require their own complete original attribution. Every captured
hypothesis must be referenced and agree; no chosen subset or lookup by base hash.
The equal-base-hash grid-low/grid-high control still has distinct content/rows.

Hypothesis content has exactly the nine original keys, version hypothesis-v1,
source discovery, NULL strategyId/parentStrategyId, original mechanism,
applicability, failureModes, strategyHash and variationKind. Applicability has
exactly the six original keys, integer datasetId, exact datasetHash/interval/
split/embargo and ordered base axis-key array. Content, separately saved
applicability and all scalar projections must agree. Duplicate hypothesis
identities under different row IDs refuse; actual history deduplicates complete
content. Do not compare shared base strategy-doc-v1 to a candidate strategy-v2.

For every winning base once, compute the original strategy-doc-v1 SHA over
existing research canonical JSON of base.strategy (params) or nested base DSL.
Compare that domain/hash to content/scalar claims. Compare original producer
mechanism/failure-mode/variation text exactly using the frozen params/DSL rules
and original winning base/axis order from the producer plan. A private streaming
string comparison may emit the fixed pieces into a borrowed comparison sink;
do not allocate a replacement hypothesis/Value/large mechanism string. Empty
axes keep the original none wording. Hash each retained complete original
hypothesis content once with existing canonical_json/sha256_hex and compare its
stored canonical content hash. Repeated candidates share the base/hash work;
per-attempt input/engine checks still cover every member.

## 4. Whole post-preparation copy/hash/output preflight

Preparation retains its unchanged complete P+E <=32 MiB / 262144-node budget.
The next stage has separate fixed **whole-operation** ceilings: hash work
W<=33554432 bytes, conservative owned-payload copy reserve C<=67108864 bytes,
and final string output O<=4194304 bytes, all inclusive. These are logical byte
work/reservation units, not allocator/whole-process peak-memory measurements.
The three pools are not allowances reset for each candidate/table/hash.
Original captured storage and preparation are prior separately bounded phases.

Perform borrowed shape/catalog/cost/reference checks, then complete O and W/C
preflight before the first new post-preparation payload clone/encoding/canonical
buffer/hash/output copy. Existing enumeration hashes occurred only inside the
already bounded preparation. Small borrowed-reference indexes have <=4096
entries; no copied document is needed to count. All size/count arithmetic and
conversions are checked, atomically, including nonempty prior usage. Refuse
before any payload work on overflow/one-over; no refund/subset/retry under a
fresh ceiling. Every failure remains static original_lineage_invalid.

J(V) is byte length of existing research canonical JSON. A no-output counting
sink using serde_json::to_writer can measure it without owning output (key
ordering does not change length). B(V) is exact existing tagged binary encoding
length: null/bool=1, finite numeric=9, string=5+UTF8 length, array=5+sum B,
object=5+sum(4+key UTF8 length+B(value)); check u32 lengths exactly as the old
encoder. Do not build/clone/encode V to measure B. Counts do not replace the
actual unchanged canonical/hash functions used after preflight.

For each stored definition D, P(D) is the existing two-key strategy identity
payload {definition:D,execModel:{feePct,slippagePct}} with its original f64
costs. Measure B(P(D)) structurally without building that payload.
Its SHA preimage is 12+B(P(D)): ASCII strategy-v2, zero, then old binary payload.

| Scheduled operation, once per stated unit | W bytes | C bytes |
| --- | --- | --- |
| original raw config V | J(V) | 2*J(V) |
| every stored strategy D | 12+B(P(D)) | J(D)+B(P(D))+176 |
| each DSL row, nested/saved documents A/Z | 0 | B(A)+B(Z) |
| each original winning base document V | J(V) | 2*J(V)+64 |
| each distinct saved hypothesis content V | J(V) | 2*J(V)+64 |
| final complete observation | 0 | O |

2*J reserves the canonical buffer and a conservative bound for temporary
escaped string/key/numeric text in the existing canonical writer. J(D) reserves
decoded string/key bytes in the helper's deep Value clone; its array/map/node
metadata is bounded separately. Strategy constant 176 is 36 constructed key
bytes (definition/execModel/feePct/slippagePct), 64 temporary hex bytes and
76 prefixed identity result bytes. DSL comparisons use the two existing binary
Vec encoders. Base/hypothesis rows each have a 64-byte temporary hash string;
config hash moves into the final result and is charged there. No other payload
copy, expected JSON tree or replacement string is permitted silently.

O=64+sum(76+64+base_id UTF8 bytes), once for every candidate in original index
order. IDs/indexes and Vec/String headers are fixed bounded metadata, not O.
Every identity/hash length must be its original fixed length. The only extra
Value tree clone is the old strategy helper: sum(nodes(D)+4) (definition plus
payload/execModel/two numbers). Audit that count; it is bounded by the already
reserved prepared P+E node count, not an invented independent node allowance.
Do not claim an unreachable standalone exact-node boundary for this stage.

After preflight, use the existing core strategy_hash/canonical_bytes and
research canonical_json/sha256_hex. Private measurement/comparison helpers have
no public arbitrary domain/hash factory. Retain no encoded buffers or parsed
JSON in the final observation. Allocate final result strings only after every
semantic comparison succeeds; their full O was reserved in advance.

## 5. Independent frozen controls and real boundaries

The [new fixture](../../alpha-factor-forge/fixtures/research/original-lineage-semantics-v1.json)
was authored with standalone Python JSON/hashlib/struct and no project imports/
execution. It independently verifies existing tagged strategy identities, base
and complete hypothesis hashes, then freezes every operation component, final
IDs/attribution/output and W/C/node/output totals over unchanged producer literals.
The separately seeded complete graph uses hypothesis IDs 200+i; those are not
the producer tests' fresh history DB IDs. No identity/fixture is regenerated from
the future consumer. No actual market, execution, sampling or reserved-seed use.

| Case | W hash bytes | C copy reserve | Extra clone nodes | O output bytes |
| --- | --- | --- | --- | --- |
| v1-ordered-axes | 5566 | 11490 | 114 | 508 |
| v1-shared-dedup | 8203 | 16941 | 152 | 657 |
| v2-fixed-dsl | 3623 | 8358 | 41 | 213 |
| v3-walk-forward | 3788 | 8688 | 41 | 213 |

The additional id-order control reuses shared-dedup with strategy IDs by
candidate [41,11,31,21] and hypothesis IDs by producer order [91,51]. Four
independently frozen input strategyId fields change; SQL strategy order is
candidate indexes [1,3,2,0], SQL hypothesis order is producer indexes [1,0].
Expected result follows actual references, with unchanged W8203/C16941/nodes152/
O657; P+E becomes16705 bytes/856 nodes. This positive must survive compatible
existing library row IDs, not assume insertion/candidate order matches row IDs.

Reuse actual SQL-captured literal graphs and #222 actual producer evidence.
Every result field and whole work count must match these literals. Whitespace/
key-order changes keep semantic observations while raw digests change; use the
existing identity-v2/numeric audits for integer/float spellings and intended-hash
refusal. Legacy compatible identity remains no proof of original numeric execution.

Real captured prepared graphs must hit W and C preflight exact/one-over limits;
whole public refusal plus cfg(test) payload-work tripwires prove no post-prepare
SHA/clone/encoding/output work occurs on overage. Resource-only controls may use
supported-mode wrappers with unchanged costs/types but changed dormant params
or nested/saved DSL claims whose identity would fail later. Exact **preflight** acceptance of those claims
is not semantic success; never repair them to manufacture compatibility. Use
finite numeric arrays for binary/JSON expansion and UTF-8 payload lengths to
independently tune W/C. The fixture freezes two feasible resource recipes:

- W: v1 params source template, cap4096, base resource-main, fastMA1..8, retain
  constant slowMA21 axis. Only stored definitions replace entryRules by 32000
  integer zeros and entryCode by ASCII x lengths 3905206 for first six ordered
  candidates, 3905205 for the last two. Exact W=33554432, C=65317632, prepared
  scratch=31775540 bytes / 257109 nodes. One extra first entryCode byte makes
  W=33554433 while other limits still fit.
- C: v2 fixed DSL source template, cap4096, five bases resource-dsl-{i} with
  original DSL names Resource {i}, empty axes. Stored nested and saved DSL both
  use params.noise arrays of 24000 integer zeros and ASCII x names of lengths
  3179885,3179885,3179884,3179884,3179884 in original candidate index order. In
  first candidate change one zero to integer10 in both documents. Exact
  C=67108864, W=16992360, prepared scratch=32303346 bytes / 241364 nodes. A
  second such zero-to-ten replacement makes C=67108865, with W unchanged and
  scratch=32303348 / 241364. These later-invalid expression claims pass only
  the outer shape gate; original wrapper identity fails after preflight.

Normal config/engine/input/hypothesis rows in these recipes come from the
original source format/enumeration/seed/lineage, then only the stated stored
claims change before explicit raw SQL capture. Each original JSON stays <=4 MiB.
No large input blob is committed; the small frozen recipe is the oracle. Keep
the prepared graph within its old byte/node limits; small private arithmetic
overflow tests only supplement real captures.

O has an independently feasible whole-observation boundary: one params base
with 1100 unique candidates and a 3672-byte base ID, another with one distinct
candidate and a 900-byte base ID. Then 64+1100*(140+3672)+(140+900)=4194304.
At 901 bytes for the other base ID it is one over. Use main axes fastMA1..44 and emaPeriod1..25 (44*25=1100; each
within the existing 64-value axis cap), base slowMA2000, and the other fixed
base emaPeriod51 (original source base is50; main values are1..25), empty axes.
Base IDs are ASCII m repeated3672 and o repeated900 bytes; original costs,
cap4096. Both mechanism texts must stay within
the captured 4096-byte scalar limit. Every row must come through actual original
producer/repository/history functions in isolated test DB setup, with only
syntactic trial IDs/jobs and no candles/execution/registry events. A minimal
cfg(test)-only bridge may call the existing private candidate helper/lineage;
it is not a production constructor. Independent expected output length, not
consumer-generated hashes, is this boundary oracle. Exact O succeeds as a whole
historical observation; one-over refuses before payload work.

## 6. First code verification and delivery

Only 4b-2c-2b-2 follows this design delivery: original_lineage entry/output,
private semantics/counting/comparison child modules and focused tests/required
test-only bridge. No existing parser/identity/producer/persistence/command policy
change. Preserve all existing fixture bytes and the preparation/selection API.
Format only new/scoped code; do not reformat the legacy runner file globally.

Tests must cover all literal results/work and nonmonotonic library ID/reference
order; compatible saved byte formatting;
integer/float legacy meaning; params unexpected DSL and DSL missing/contradictory
projection; wrong type/markers/parent/cost bits; every input field/type/key/axis/
seed; every engine key/version including a late changed member; original raw vs
canonical config hash; shared-base/equal-base-hash wrong attribution, complete
hypothesis scalar/content/applicability/order/hash/null projections and duplicate
identity/extra member refusal; unsupported v2/v3 params/mixed modes; failed
members retained; complete immutable graph/caller transaction/owner/mutation on
success/refusal. Keep old nonrepresentable numeric claims refused without repair.

Six temporary mutants must assertion-fail: ignore saved DSL contradiction;
replace saved engine by current fingerprint; choose any equal-hash base;
use candidate hash as hypothesis base hash; reset W/C per item; copy output before
whole O preflight. Restore exact source bytes and retain original failure logs.
All new DB roots use bounded create_dir/AlreadyExists skipping with actual
isolated lease/owner, close handles and remove only newly owned roots. No old
PID/counter roots are reused/deleted or broad TEMP refactor introduced.

Full isolated locked Rust precedes Node, then locked all-target check/Clippy/
scoped format/typecheck/test/build and six exact-head/base CI. Fresh reviews/
main, expected-head merge, GitHub readback and clean-main full-tree comparison
precede the next task. This design/data PR validates independent hashes/work/
recipe/source rules/links/scope/lifecycle without rerunning local runtime suites
only for artifacts; green design CI does not claim future semantic tests ran.

## 7. Remaining source obligations

This whole-family observation is historical consistency. No source builder,
RegisteredSource, numericCostsHash, warmup/file/market eligibility or execution
qualification follows. Unsupported envelope/mode coverage needs its own producer
controls. Original numerical execution, cost/action originals, warmup/market/
trial adapters, authenticated files/candles and final live-owner/registry/
IMMEDIATE admission remain separate mandatory gates; policy/mediation authority
is still unavailable. Parents 4b-2c/4b-2/4b/P13c-4 remain incomplete. After this
semantic slice, promote/refine the missing envelope/mode coverage extension
before warmup/market/trial adapter code;
never treat a lineage consistency result as final admission or research PASS.


The next [producer mode-coverage refinement](research-original-producer-mode-coverage-v1.md)
freezes v2/v3 params/mixed controls and separate producer/consumer deliveries.
It does not change this first catalog or claim broader runtime support before
those dependent code PRs deliver.
