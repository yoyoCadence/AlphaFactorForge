# P13c-4b-2c-5 original trial/split producer projections v1

Date: 2026-10-31 (UTC+8). Baseline: verified #242 (`bbf669a`).
Documentation/unconsumed data only; producer conformance precedes consumer code.

The [ordered original semantic plan](research-original-semantic-adapters-v1.md)
and delivered [lineage](research-original-lineage-semantics-v1.md),
[warmup](research-original-warmup-consumer-v1.md) and
[market](research-original-market-consumer-v1.md) contracts remain binding.
The new [fixture](../../alpha-factor-forge/fixtures/research/original-trial-projections-v1.json)
freezes original pure-function projections. It does not establish that a declared
bar count belongs to a content-hash dataset, that any strategy was executed, or
that supplied registry records are complete/current/authentic. It contains no
AdmissionCount, registry receipt, live fence or source-construction capability.

## 1. Original producers and exact formats

Use the pinned original functions, not a new consumer's current output:

- [execution.rs](../../alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs):
  trial_split_identity, candidate_walk_forward_report and the unchanged actual
  CandidateStrategy parse/embargo boundary.
- [runner mod.rs](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs):
  candidate_split_hashes re-enumerates the original final-unique candidates;
  register_lineage_with binds each actual lineage hypothesis/attempt to the
  matching index/strategy and original split/seeds/engine hashes. Normal discovery
  uses request-origin Variant events and DISCOVERY_TESTS_PER_TRIAL=2.
- [trial_ledger.rs](../../alpha-factor-forge/src-tauri/src/research/trial_ledger.rs):
  original family/event/member/batch framing and prepare_batch ordering/validation.
  Its generic API accepts other historical kinds; this first runner projection
  catalog does not manufacture their executor/reproduction/backfill proof.
- [split](../../alpha-factor-forge/src-tauri/src/discovery_core/split.rs),
  [walk-forward](../../alpha-factor-forge/src-tauri/src/discovery_core/walk_forward.rs),
  [embargo](../../alpha-factor-forge/src-tauri/src/discovery_core/embargo.rs) and
  [research canonical JSON](../../alpha-factor-forge/src-tauri/src/research/mod.rs)
  retain their original integer/null/number/string rules. No numeric policy flip.

The fixture pins19 source files at the recorded baseline, including all seven
original registry migrations, and9 immutable source packages. Historical pins
must continue to reference that baseline when later cfg(test) declarations are
added; do not silently rewrite v1 to track current code. Inputs represented as
camelCase JSON are a fixture-only carrier for typed Rust arguments. They are
not a new runtime request format and cannot serialize BenchmarkEvidence.

## 2. Split identity and eligibility are separate results

trial_split_identity always returns a six-key document:
contracts, totalBars, holdingAllowanceBars, embargo, split, walkForward.
contracts has split/embargo strings; a successful embargo has exactly
maxSignalLookbackBars, holdingAllowanceBars and embargoBars. Preserve every
explicit null; never omit a candidate whose derivation failed.

The original outer split has eight keys: totalBars, usableBars, embargoBars,
train, trainValidationEmbargo, validation, validationTestEmbargo, test. Every
range is inclusive with from/to/count. Zero embargo gives null gaps. Allocation
uses exact integer60/20/20 largest remainders with Train/Validation/Test tie order.
At least five usable bars are required after two gaps. Primitive controls also
freeze usable7 (Validation wins the remaining Validation/Test tie), negative
embargo refusal and the safe-integer limit. They are original split-function
arguments, not permission to provide a caller-selected embargo to a consumer.

For v1/v2, walkForward is null. For v3 it has declaration/report; declaration
has minimumTrainBars, foldValidationBars, foldCount. A report has eight keys:
contractVersion, plan, status, reasons, outerTrain, availableTrainBars,
requiredTrainBars, folds. Its plan has five integer fields and no contractVersion.
Fold records have index/train/embargo/validation. Reasons use the original snake
case; status uses ELIGIBLE/NOT_ELIGIBLE. Preserve insufficient-outer versus
insufficient-Train history, null requiredTrainBars above the safe bound and empty
folds. Derivation/evaluator error gives report:null with the declaration retained.

The original identity's generic WF evaluator omits the additional candidate
lookback check made by candidate_walk_forward_report. With minimumTrainBars20,
mixed candidates with lookback22/34 can have an ELIGIBLE identity report while
the candidate report refuses at its lookback guard. An Ok(NOT_ELIGIBLE) report
also differs from declared_walk_forward_plan's required successful eligibility.
Never turn an existing splitHash into successful warmup, folds or enqueue proof.

## 3. Original hash boundaries and event membership

All JSON is original research canonical UTF-8: recursively sorted keys, compact
separators, original integer/null spelling. Retain the locked legacy numeric
parser and original number tokens where applicable. The counterfactual2^53
count is an exactly representable typed64-bit scalar; it is never allocated as
candles. Its original identity is hashable although split/report are null.

| Identity | Exact original preimage / prefix |
| --- | --- |
| splitHash | SHA256 of canonical {version:trial-split-v1,identity}; prefix trial-split-v1: |
| seedsHash | Plain SHA256 of the original seeds object canonical JSON |
| engineFingerprintHash | Plain SHA256 of the complete original engine document canonical JSON |
| familyId | SHA256 of canonical {version:trial-family-v1,instrumentId}; prefix trial-family-v1: |
| eventId | SHA256 of the exact canonical19-key event payload; prefix trial-event-v1: |
| batchId | SHA256 of canonical {version:trial-batch-v1,familyId,members}; prefix trial-batch-v1: |

The split document excludes datasetHash, strategyHash, configHash and seeds.
Different strategies with the same derivation/windows can legitimately share a
splitHash; candidate_split_hashes separately carries strategyHash alongside it.
Family identity excludes interval/dataset/workspace/protocol. testsPerTrial and
protocol JSON are absent from event/batch identity. Current protocol cannot be
inferred from those hashes or the original family protocol_json alone: the
existing registry records monotonic upgrades separately. This projection-only
protocol-three control asserts identity equality, not registration/replay outcome.

Every payload always has these19 keys: version, familyId, kind, effective,
idempotencyKey, workspaceId, requestId, candidateIndex, legacyAttemptKey,
hypothesisHash, strategyHash, datasetHash, snapshotId, splitHash, seedsHash,
engineFingerprintHash, benchmarkId, benchmarkParamsHash, reproductionOf.
Normal Variant has effective:true and explicit null legacy/benchmark/reproduction
fields. BenchmarkEvidence never appears in that JSON. The request key is exact
workspaceId:request:requestId:candidate:decimalIndex, without numeric padding.

prepare_batch sorts complete idempotency keys by UTF-8 byte order. Members are
two-element [key,eventId] arrays in that order, so index10 precedes2. Stored event
sequence follows that order for a fresh registry. Returned event IDs are restored
to the caller's original input positions. The12-candidate control and its reverse
input freeze all three orders. Do not zip numeric workspace attempts with sorted
registry rows or accept only their mutually matching subset.

The stored event projection freezes event_id/family_id/idempotency_key/kind/
effective/batch_id/payload_json and fresh-registry sequence order. Actual chain,
origin registry IDs, timestamps and receipts are not authored constants. Producer
controls must record/preserve those real originals in their isolated registry;
the projection's event count/sequence never supplies a current admission count.

## 4. Independent corpus and original validation distinctions

| Case | Candidates | Declared count / primary observation |
| --- | ---: | --- |
| v1-market-small | 3 | 4; valid embargo, null split, no WF |
| v1-market-full | 3 | 600; complete outer splits, no WF |
| v1-shared-dedup | 4 | 600; final-unique original base attribution/membership |
| v2-fixed-dsl | 1 | 600; DSL derivation and walkForward:null |
| v3-mixed-lookback-distinction | 4 | 600; generic eligible versus two candidate lookback refusals |
| v3-mixed-insufficient-outer | 4 | 4; non-null NOT_ELIGIBLE outer reports and lookback precedence |
| v3-dsl-insufficient-train | 1 | 100; NOT_ELIGIBLE Train report |
| v3-mixed-null-embargo | 4 | 600; complete original family retains the null-embargo member |
| v1-twelve-lexical | 12 | 600; same split/different strategies and index10-before2 |
| etf-declared-family | 4 | 600; original ETF family and original references |
| v3-outside-safe-count | 1 | 2^53; typed scalar identity with null split/report |

All41 seeds are independently rederived from original seed-v1 frames, never
copied after dataset rebinding. The new12-grid only changes the original ordered
axis bounds; the unchanged base strategy/id/axis names retain the original shared
hypothesis. No new indicator or execution/ranking is generated. Some declared
counts are deliberately counterfactual to source dataset metadata; they test
functions that accept a scalar and assert no dataset-count correspondence.

Six generic-ledger controls freeze reversed input, untrimmed workspace/request
strings, a nonempty unversioned split claim, absent optional hypothesis, diagnostic
kind and protocol-three identity. These accepted generic preparations do not
match every normal runner/source obligation. The original Variant ledger API
checks presence/nonblank text, not general hash length/prefix; within prepare_batch,
only reproduction requires the versioned split reference. It permits null hypothesis for Variant,
and family_unknown exactly when instrument/snapshot are both absent. Do not
invent original-producer refusals or silently trim/rewrite these inputs.

The24 actual producer-validation refusals cover empty events/workspace/request,
colons, protocol bounds, invalid instrument, snapshot-presence contradiction,
mixed requests, duplicate keys, legacy-kind/origin mismatch, blank optional hash,
missing required hashes and forbidden Variant benchmark/reproduction fields.
Nine split primitive controls fix the arithmetic boundaries independently. These
are finite original-format catalogs, not a general replacement ledger validator.

## 5. Required conformance and later source boundary

After verified design delivery, a separate producer-control PR must invoke the
unchanged original trial_split_identity/candidate_split_hashes and ledger batch
producer against these literals. Fixed cfg(test) access is acceptable; do not
introduce a runtime part/document factory or use a new consumer as the oracle.
Use migrated file-backed owned workspaces and separate sibling registries. The
small known-market case can use the already frozen actual four-bar dataset and
original verified loader for the complete runner registration control. Pure
counterfactual count cases must never mint VerifiedDataset from their scalar.

Verify every original candidate including null derivation, exact seeds/engine/
strategy/hypothesis references, fresh stored scalar/JSON projections, complete
members, restored input order, unchanged replay and append-only state. Wrong
payload under an existing key is idempotency_conflict; unchanged keys in a
different subset/superset batch are batch_conflict. Keep the original conflict
precedence and monotonic protocol policy. Preserve all prior workspace/registry
rows and compare complete schema/types/REAL bits/triggers/total_changes/original
metadata/artifacts across failure/replay, with only expected original-producer
rows added on new registrations. Preserve caller transactions/leases; close owned handles and verify containment/no-reparse
before cleanup. No protected worker run, benchmark attestation or reserved Test
seed activity is needed to verify these metadata producers.

Only then refine a bounded consumer API/work model. candidate_split_hashes is a
conformance target, not an automatic second enumeration inside a new consumer.
Preserve prior whole-operation reservation/one-preparation guarantees and do not
introduce caller-selected counts, prepared parts or resource policies as source
authority. This projection contract freezes no new consumer constructor.
Existing captured workspace
trial IDs are syntactic links; matching them to a selected event subset is not
the original complete batch. A future observer needs supported original registry
event payloads/scalars, exact batch members, workspace/instrument/family/kind/
origin/index and every lineage/market/hash link. Unsupported historical kinds
must never be filtered to lower history, mint an exemption or make a successful
source subset. Benchmark attestation/reproduction/backfill remain separate proofs.

The graph lacks authenticated candles/current bar count and complete trusted
registry custody/current observation. Old/copied observations, plain receipts,
unbound protocol claims and content hashes cannot fill those gaps. File/candle/
availability4b-3, supported action/cost originals and the actual-owner/registry
final fence4d remain mandatory. No numericCostsHash/sourceEvidenceHash or
RegisteredSource construction follows from this corpus or market-row delivery.

## 6. Artifact and delivery verification

From repository root:

```powershell
py alpha-factor-forge/scripts/generate-original-trial-projections-v1.py
node alpha-factor-forge/scripts/verify-original-trial-projections-v1.cjs
```

The stdlib Python author and separate Node built-ins verifier import no product
code. Node uses BigInt for exact split/WF arithmetic, verifies every original
strategy/seed/identity/reference/projection, closed catalog/order/refusal and all
19 historical source pins/9 immutable packages. Sandbox git-show child EPERM is
handled by rerunning the identical read-only verifier with escalation, without
dropping a source check. Preserve exact reproduction and deliberate invalid
artifact/assertion controls before freezing v1.

Frozen artifact:793019 LF bytes, SHA256
`a06fa7b074c11d6b13cae343f92e658c60a1c19ce5985ddf2276bb6e45c4d2ac`.
Exact reproduction and eight assertion-caught invalid-artifact controls pass:
coherently rehashed wrong split/numeric member order/wrong seed, ledger-valid
wrong runner kind, invented benchmark evidence, missing refusal, duplicate case
and changed primitive tie. These controls execute no product code or registry.

Finite artifact/source/link/scope/lifecycle/append-only checks and independent
review precede PR. This Markdown/unconsumed-data slice does not run local runtime
suites solely for documentation. Six completed exact-head/base jobs, fresh main/
reviews, normal expected-head merge, GitHub final readback and clean-main/full-tree
equality complete delivery. Only the design closes; producer controls, consumer
refinement and all source/live parents remain incomplete.
