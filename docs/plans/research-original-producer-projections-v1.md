# P13c-4b-2c-2 original producer projection controls v1

Date: 2026-10-24
Baseline: merged #220 (`59a19b6`). Design and unconsumed fixture only.

The [original adapter matrix](research-original-semantic-adapters-v1.md) and
[hypothesis correction](research-source-hypothesis-grounding-v1.md) remain
authoritative. Raw digests now exist; semantic/source admission does not. This
refinement freezes actual producer projections and independently authored
controls before the next code. Conformance tests must pass before subsequent
consumer adapters can rely on these shapes. Green generic CI on this design
does not prove the future controls ran.

## 1. Two producer boundaries that a consumer must preserve

[Runner start](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs)
currently constructs StrategyDef inline, then calls
[get_or_insert_verified_runner_strategy](../../alpha-factor-forge/src-tauri/src/db/repositories.rs).
That repository verifies the supplied definition, inserts if absent and uses
ON CONFLICT DO NOTHING. Existing library metadata and JSON bytes survive a hash
conflict. It does not certify the existing row's separate dsl_json projection.

Fresh-row exact serialization and historical-row semantic agreement are distinct
checks. A consumer must preserve the stored original bytes for raw checksums;
it cannot require every historical row to equal a newly serialized DTO, refresh
an existing row, or accept contradictory dsl_json because the producer preserves
it. Display name/source/lifecycle are not source authority. Any later supported
existing-row adapter needs original numeric-policy/type/hash/executable
projection agreement; incompatible claims refuse without repair.

Fresh StrategyDef has exactly these 11 fields (Rust serialization names):

| Field | Existing producer rule |
| --- | --- |
| id | None before insertion |
| name | Nested DSL name when it is a string; otherwise `Discovery {base_id} #{index+1:04}` |
| type | `dsl` only when candidate.strategy.mode is `dsl` and nested dsl exists; otherwise existing `params` branch |
| dsl_json | Exact serde_json::to_string of nested DSL in that branch; otherwise None |
| original_definition_json | Exact serde_json::to_string of the complete enumerated candidate wrapper |
| param_schema_json | Nested DSL params serialized when present; otherwise None |
| source | `traditional` in DSL branch; `sweep` otherwise |
| ai_prompt_hash | None |
| strategy_hash | Existing enumerated strategy-v2 identity, unchanged |
| lifecycle | `candidate` |
| parent_strategy_id | None |

The helper below preserves this producer, including its existing fallback and
error order. It is not a validator for arbitrary candidate modes. Config/DSL
validation and enumeration remain their existing earlier gates.

## 2. Literal original engine catalog

[engine_fingerprint](../../alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs)
has nine top-level keys: package=`0.1.0`, configContracts, execution=
`backtest-execution-v1`, metrics=`metrics-v2`, benchmarks=`benchmark-suite-v1`,
gate=`gate-v1`, score=`score-v1`, validationRecord=`validation-record-v2`,
benchmarkRecord=`bench-record-v1`. These strings are fixture literals, not
expectations imported from current constants or this binary's package version.

The exact base configContracts keys/values are strategyHash=`strategy-v2`,
datasetHash=`dataset-content-v2`, split=`validation-split-v1`,
embargo=`embargo-derivation-v1`, backtest=`backtest-execution-v1`,
metrics=`metrics-v2`, benchmarks=`benchmark-suite-v1`,
randomEntry=`random-entry-v1`, gate=`gate-v1`, score=`score-v1`, seed=`seed-v1`,
and enumeration. v1 uses `discovery-enumeration-v1` and omits optional keys.
v2 uses `discovery-enumeration-v2` and strategyDsl=`strategy-dsl-v1`.
v3 additionally has walkForward=`research-walk-forward-v1` and
walkForwardEvidence=`walk-forward-evidence-v1`. Omission is not JSON null.

Producer conformance calls the actual function and compares every field to
literals. A future original-document consumer must compare to its explicit
supported catalog and original config, never create a replacement fingerprint
using current code/environment. Unknown versions, missing/extra keys or changed
declared contracts need a separate support decision and refuse beforehand.

## 3. Original lineage and shared hypotheses

Use unchanged run_lineage, [HypothesisDraft/content/hash and history
registration](../../alpha-factor-forge/src-tauri/src/research/history.rs).
Input fingerprints have exactly 12 keys: envelopeVersion, configHash, rootSeed,
datasetId, datasetHash, interval, strategyId, strategyHash, baseId,
candidateIndex, appliedAxes, seeds. configHash remains SHA-256 of the existing
canonical research JSON of the original parsed config. Row IDs in these test
controls are declared references, not authenticated dataset/source evidence.

The hypothesis document has version, source, mechanism, applicability,
failureModes, strategyHash, strategyId, parentStrategyId and variationKind.
Discovery source/version, both NULL IDs, complete text, original axis order,
applicability's six keys and variation spelling must all match the actual
producer. params strategy-doc-v1 hashes the original base strategy; DSL hashes
its nested base DSL. Neither is replaced by a varied candidate strategy-v2.

The original winning base after deduplication is authoritative for projection.
The duplicate control has three grid-low candidates and one grid-high candidate.
Those bases have the same strategy-doc-v1 hash but different mechanism/variation
content attribution; selecting any base with a matching hash is insufficient.
History deduplicates their complete hypothesis content into two rows. v2/v3
controls intentionally share the same base hypothesis and candidate identity
while config/engine documents differ. Mechanically generated failure-mode text
remains the original unexplained/authoring claim, not scientific qualification.

## 4. Independently frozen controls before code

The new [literal fixture](../../alpha-factor-forge/fixtures/research/original-producer-projections-v1.json)
was authored with Python JSON/hashlib/struct without importing or running any
project implementation. Existing cross-language candidate identities and seeds
were checked with independent tagged binary identity / seed-v1 encoders before
using them. Complete DTO/engine/hypothesis/input JSON strings are frozen, rather
than generating expected values from the future helper or actual writer.

| Control | Candidates / hypotheses | Independent input lineage |
| --- | --- | --- |
| v1-ordered-axes | 3 / 1 | runner-config-v1 enumerate-single-axis, then append literal slowMA axis 21..21 step 1 after fastMA; existing binary identities unchanged |
| v1-shared-dedup | 4 / 2 | Unchanged enumerate-cross-base-duplicates and its original winning-base manifest |
| v2-fixed-dsl | 1 / 1 | Exact configJson and identity from source-selection-v1 |
| v3-walk-forward | 1 / 1 | Exact configJson and identity from source-selection-v1 |

All cases use declared run 1, interval 1h, strategy IDs index+1 and deterministic
parser cores 64, not current hardware. Candidate integer axes become f64 values
when the original enumerator patches them; serialized `8.0`/`21.0` is preserved
in candidate/input JSON, while integer-valued original config/base fields stay
integers. This representational distinction does not change binary f64 identity.
Only published fixture seed preimages are checksummed; no random/statistical
sampling, protected data acquisition, calibration or reserved seeds are used.

| Control | Config canonical SHA-256 | Engine JSON SHA-256 |
| --- | --- | --- |
| v1-ordered-axes | ecb023d26558412ebd951fa7e230f6486910ff5a623c55d4a4d959ad2881a063 | 3eac6bb5e63bd0444ac385dda23c0cdf3b25f53222e00c3ab97f17f1969dfd1f |
| v1-shared-dedup | fa39438268b4245ec73f83bbd854206688656a1e9c3402b8734de6959554a9e6 | 3eac6bb5e63bd0444ac385dda23c0cdf3b25f53222e00c3ab97f17f1969dfd1f |
| v2-fixed-dsl | a77e90b6f4ee037b12c989bb81c16460669c5b8a3b8a196f7d95e507fd1e5df3 | f2baa5d69c079dd65dcd1e690c5b97e303b3b8c1bc044ab735e7bb608f55edd6 |
| v3-walk-forward | 8aa940678d66a562d7ca9efee85705b5dcfc935dd573fbe05aaa83d60c5bd87d | 4b50f9a7b5fa087807c562cd104be9e0218ce502b209eda6626f709fb05f6c8b |

Full hypothesis/input/definition values and all candidate hashes/seeds are in the
fixture. v1 ordered-axis hypothesis hash is
`7d02c9f48e14dcf6177d46f3dcadd94436fd658228e7a172e441010d89128e9f`;
grid-low/grid-high are
`2d840c49f0b40cad3b3107bded44fb50bbbe7e121965a5b8985353c70e0cb2a6` /
`2ae35a712d6d742c15227e9289951e5de4a7f69f1c38d813701dd8c660764ef8`;
both DSL cases use
`3014349b7d10936707a90b9b32c2a5c4972b74f943bb81b053863d0626dccc41`.
These are pending producer-conformance expectations, not successful source proofs.

The existing [numeric audit](../numeric-json-identity-audit-v1.md) number-3 is
the negative control: intended fee bits 3f6e43cfc21ad9fe / strategy ec10d414...
serialize to text which the unchanged legacy repository interprets as
3f6e43cfc21ad9ff / b317bbc5.... It must refuse the intended-hash write before
insertion. Full hashes/reference are frozen in the fixture. No hash repair,
policy-marker invention, global parser feature or reinterpretation is allowed.
Successful legacy byte/identity reconstruction alone still does not establish
original numerical execution; the broader unresolved audit is unchanged.
This is a controlled producer/repository boundary value, not a supported config
or an admitted run.

## 5. First code slice: actual producer controls, before consumer code

**4b-2c-2a** extracts only this private helper in discovery_runner/mod.rs:

```rust
fn candidate_strategy_def(candidate: &EnumeratedCandidate) -> AppResult<StrategyDef>
```

Move the exact existing definition serialization / DSL selection / StrategyDef
construction block into it; keep its call in the same start-loop position,
inside the same lock, immediately before the same repository call. Preserve
every field, serialization/error order, ownership/failure behavior and existing
validation. Do not change resume, enumeration, config, identity, repository,
history, engine, worker, locking, schema or numeric policy. No new public or
generic projection API. No semantic consumer/source builder in this slice.

Add a private runner tests/projection_controls.rs child using the literal fixture
and actual helper / run_lineage / engine_fingerprint / history functions.
Tests must not duplicate those producers or compute their own expected outputs.
Use fresh migrated file-backed SQLite and an isolated lease/owner. Allocate each
test root with create_dir, skip AlreadyExists with a bounded retry, and clean
only that newly created root after closing handles; do not reuse/delete an old
PID/counter directory or refactor other suites' TEMP handling.
Each control uses its own empty database for index+1 IDs and fresh-row byte
checks; repeated-registration/conflict behavior is tested explicitly afterward.

Seed only declared dataset/run reference rows with explicit test SQL. Insert
actual fresh strategies through get_or_insert_verified_runner_strategy and
register actual lineage through history in the caller transaction, comparing
all original stored JSON/scalar projections and exact 1/2/1/1 hypothesis counts.
Controlled dataset metadata has candle_count=0, as the existing test bridge;
this establishes writer shape, not candle/source verification. Event IDs may be
fixed syntactic fixture claims in history; no registry event is registered.
Do not call start/resume, enqueue jobs, execute/simulate/backtest, create result
artifacts, contact providers or acquire registry/protected/statistical inputs.

Additional required cases: repeated registration deduplicates complete content;
the equal-base-hash/different-winning-base control preserves attribution; saved
DSL JSON/params projections match the actual nested documents; conflicts keep
existing display fields and whitespace/key order in original bytes. Include a
deliberately contradictory existing dsl_json claim: the current repository keeps
it unchanged, which is characterization rather than consumer acceptance. The
numeric intended-hash control must leave strategy rows untouched on refusal.

Six temporary runtime mutants must assertion-fail: full wrapper as dsl_json;
wrong DSL kind; replace declared engine configContracts with v1 defaults;
hypothesis uses candidate hash; always select the first base; reverse hypothesis
axis order. Restore exact source bytes, then full locked Rust before Node,
all-target check/Clippy/scoped format/typecheck/test/build and six exact-head CI.
Avoid formatting the legacy runner file globally; format/check only the new
helper/test code and preserve unrelated code.

## 6. Delivery and subsequent work

This PR changes only the plan, unconsumed fixture, task board and handoffs.
Verify fixture schema/scalar/JSON strings, independent identities/seeds/checksums,
source rules, literal catalog, numeric references, links/scope/lifecycle and
diff --check; do not rerun runtime/statistical suites locally just for these
artifacts. Six exact-head CI and fresh-base/review/expected-head merge plus clean
main still precede 4b-2c-2a code. Its actual producer controls are future required
evidence, not completed checks in this PR.

After those controls pass and deliver, refine bounded sealed-graph candidate /
engine / hypothesis consumer APIs and whole-operation parse/enumeration/scratch
budgets before their separate implementation. Only consistent private
observations are contemplated. Missing original cost/action/file/candle/trial
evidence and final live-owner/registry admission remain their own gates. Parent
4b-2c/4b-2/4b/P13c-4 remain incomplete. No RegisteredSource/runtime activation.

The [bounded lineage consumer refinement](research-original-lineage-consumers-v1.md)
now fixes sealed-graph preparation, a shared original run-plan path and combined
parse/expansion work before separate semantic comparison code. Actual producer
controls delivered in #222; this does not change the earlier design's historical
verification claims or grant source authority.
