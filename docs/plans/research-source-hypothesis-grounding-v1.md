# Complete source graph: original hypothesis grounding correction v1

Date: 2026-10-24
Baseline: merged #216 (`4193eba`). Documentation only.

## 1. Error and authoritative writer evidence

The earlier [graph contract](research-source-graph-adapters-v1.md) incorrectly
treated every hypothesis as a candidate strategy identity with nonnull strategy
ID. Draft [#217](https://github.com/yoyoCadence/AlphaFactorForge/pull/217) faithfully
implemented that contract, and its synthetic fixture passed, but it would refuse
the normal discovery runner's stored rows. Its initial green checks do not prove
writer compatibility. Keep it draft until this correction and new evidence land.

Authoritative existing behavior:

| Writer | Existing behavior |
| --- | --- |
| [run_lineage](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs) | Hypothesis belongs to the original base: strategy-doc-v1 plus SHA-256 of canonical original base strategy (or its DSL). strategy_id and parent_strategy_id are None. Candidate-specific input fingerprint/strategy_id are in the attempt. |
| [HypothesisDraft / register_hypothesis](../../alpha-factor-forge/src-tauri/src/research/history.rs) | Versioned draft content is hashed and registered by content, so variations of one base share one hypothesis row. strategy_id remains nullable; no binding to each variation occurs. |
| register_run_lineage_with_events in the same file | Reuses the registered hypothesis ID for each attempt; supplies trial IDs without changing hypothesis hash or strategy ID. |
| [strategy registration](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs) | Each actual candidate strategy_def keeps its enumerated strategy-v2 hash. This identity is separate from the base hypothesis's strategy-doc-v1 hash. |

No runner/history/schema/identity behavior is changed by this correction. The
117-column projection, raw JSON/numeric policy, caps, complete candidate graph,
exact references, transaction ownership and full-fresh recheck stay fixed.

## 2. Corrected raw graph rules

1. strategy_def.strategy_hash and selection CandidateKey stay exact
   strategy-v2:<64 lowercase hex>. Dataset identities stay dataset-content-v2.
2. hypotheses.strategy_hash is an original claim in either existing domain:
   strategy-doc-v1:<64 lowercase hex> or strategy-v2:<64 lowercase hex>. Preserve
   its prefix and bytes. Do not convert/re-hash one domain into the other or
   treat a well-formed claim as semantic identity proof.
3. A referenced hypothesis.strategy_id may be SQL NULL. If nonnull, it must equal
   the referencing attempt's strategy_id. The graph must not compare a base
   hypothesis hash to the varied candidate's hash. Later original semantic
   assembly verifies the exact base/config/input/hypothesis content relationship.
4. Every attempt still resolves exactly one hypothesis row. Multiple attempts
   may share it; query/retain/count it once by distinct ID. Complete candidate,
   attempt, trial and two-job membership are unchanged. There is no subset,
   completed-only filter or new history expansion.
5. Parent IDs remain NULL or inside the already captured direct strategy set.
   No comparison-source promotion, source builder or permission follows.

This supports the actual runner's base hypothesis plus existing candidate-bound
claims as raw consistency evidence. It does not certify manual/AI/unknown semantic
documents. Unsupported original semantic reconstruction still refuses later.

## 3. Corrected independent fixture and real-writer control

Before changing the reader, update the independent fixture to one shared base
hypothesis ID 200, referenced by attempts 400/401/402, with strategy-doc-v1 and
NULL strategy_id. Keep candidate strategies 100/101/102, artifacts 300/301/302
and all other fixed joins. Independently freeze every ordered cell and new byte/
node totals; preserve the original failed grounding/counts in the handoff.

In addition, test against the actual unchanged writer path. A narrowly scoped
cfg(test)-only helper in discovery_runner/mod.rs may construct the existing
ScheduledCandidate/VerifiedDataset test metadata and invoke run_lineage from the
same module. Inputs are the captured fixture config, known strategy IDs, run ID
and exact interval. It returns the existing draft pairs only, with no DB, thread,
file, registry, execution or authority action. No production visibility/export,
unchecked production observation constructor or alternate lineage implementation.

The graph test calls that actual helper and existing
history::register_run_lineage_with_events inside its owned migrated fixture,
using the already-authored trial IDs. It must observe one real deduplicated base
hypothesis, three distinct attempts and successful full capture/recheck. Assert
original writer hash prefix, NULL identity, shared ID and full membership. Do not
call the graph adapter to manufacture expected hashes or evidence. This setup
registers test rows only, never trials in an actual registry or runs a candidate.

Also retain positive nonnull matching candidate-bound identity, refusal of a
nonnull cross-strategy ID and malformed/bare/uppercase/wrong-domain hashes;
shared hypothesis resource accounting, full 133-field/member comparisons and
all existing resource/transaction tests. Large cap fixtures may share the base
hypothesis while keeping distinct candidate/attempt/job counts.

Remove the old mutation expectation that changing a base hypothesis to a valid
other-domain claim must fail solely because it differs from the candidate hash.
Its semantic contradiction belongs to the later content adapter. No weakening
of candidate strategy_def hash agreement is permitted. Add explicit runtime
mutants that wrongly require nonnull hypothesis ID or candidate hash equality;
the real-writer positive control must assertion-catch both.

## 4. Ordered delivery

1. This focused documentation PR: task/links/static writer grounding, six exact-
   head CI jobs, expected-head merge and clean-main verification. No new test or
   full local rerun solely for docs.
2. Rebase draft #217 onto the corrected main. Implement the corrected guarded
   text domain/membership, independent fixture and cfg(test) writer control only.
   Reconcile task state and append the original failed design/evidence receipt.
3. Repeat the original four runtime mutants plus the two compatibility mutants,
   exact restoration, full locked Rust before Node, all-target check/Clippy/
   scoped format/frontend and six exact-head CI jobs. Preserve original CI run
   evidence; a previously green synthetic fixture is not the corrected result.
4. Expected-head merge and verified clean main precede the original 4b-2c design.

No source authentication, missing action/cost/source/file/warmup adapter,
RegisteredSource builder, schema/dependency/workflow change, protected acquisition,
policy/calibration/reserved-seed activity or runtime activation is introduced.
