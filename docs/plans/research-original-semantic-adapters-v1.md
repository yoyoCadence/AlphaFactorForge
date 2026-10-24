# P13c-4b-2c original semantic adapters and digest contract v1

Date: 2026-10-24
Baseline: merged corrected #217 (`d88312e`). Documentation only.

The [parent source contract](research-source-reference-observation-v1.md#5-original-byte-digests-and-semantic-assembly)
and [hypothesis correction](research-source-hypothesis-grounding-v1.md) remain
authoritative. The delivered complete SQL graph is historical consistency
evidence. Its synthetic fixture deliberately includes uninterpreted claims;
neither that fixture nor its actual-writer hypothesis control is an admitted
source, executed-result proof or permission to construct RegisteredSource.

## 1. Actual original formats and semantic obligations

Read original bounded bytes, not display DTOs, permissive Value::Null fallbacks,
latest summaries or a newly generated current-engine document. Preserve all
existing identities/prefixes and the locked legacy numeric parser. This table
fixes the supported format boundaries; later semantic code also requires
independent original-format fixtures and actual-producer positive controls.

| Original input / authoritative producer | Required check / supported boundary |
| --- | --- |
| [campaign declaration](../../alpha-factor-forge/src-tauri/src/discovery_core/campaign.rs) | Existing declaration-v1 re-freeze, stored/requested campaign identity, exact admission instrument and dataset/snapshot binding. Integers/strings/nulls remain the original codec; no market resolution by display name. |
| [config and enumeration](../../alpha-factor-forge/src-tauri/src/discovery_core/config.rs), [enumerator](../../alpha-factor-forge/src-tauri/src/discovery_core/enumerate.rs) | Existing discovery-config-v1/v2/v3 and their exact contract sets, params or fixed executable DSL only. Retain original order/prune/dedup/hash-sort/base attribution/seeds and deterministic parser cores. Existing raw-product/B/Q scratch preflight must precede any new enumeration. Code/blocks/unknown modes have no supported source adapter. |
| [candidate strategy writer](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs), [identity](../../alpha-factor-forge/src-tauri/src/identity.rs) | Candidate strategy-v2 must agree with original definition and existing admitted costs under the declared old policy. original_definition_json is the candidate wrapper; DSL-specific dsl_json comes from its nested DSL, not a replacement candidate hash. Verify original type/document projections using actual repository/producer controls before code; editable name/lifecycle/source is not authority. |
| run_lineage in the same runner file | Exact original input fingerprint keys: envelopeVersion, configHash, rootSeed, datasetId, datasetHash, interval, strategyId, strategyHash, baseId, candidateIndex, appliedAxes, seeds. Compare each against the exact enumerated candidate/config/dataset/attempt, including the original winning base after deduplication. Do not search any compatible base by hash. configHash remains the existing SHA-256 of canonical research JSON under the declared original policy, not a new raw-byte hash. |
| [engine_fingerprint](../../alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs) | Original package/configContracts/execution/metrics/benchmarks/gate/score/validationRecord/benchmarkRecord fields. Validate declared supported versions against an explicit original-format catalog and original config, never substitute this binary's current fingerprint. Hash all candidates' original engine documents in ascending index. |
| [HypothesisDraft/history registration](../../alpha-factor-forge/src-tauri/src/research/history.rs) | Versioned original content, scalar projections, canonical content hash and applicability must agree under the original policy. Normal discovery uses a shared base strategy-doc-v1 hypothesis with NULL strategy_id. Compare base attribution and original mechanism/axes/applicability/failure-mode/variation documents to the actual writer. Do not compare its base hash to a varied candidate hash. Raw candidate-bound/manual/AI graph claims do not automatically acquire a supported discovery semantic adapter. |
| [warmup/embargo](../../alpha-factor-forge/src-tauri/src/discovery_core/embargo.rs), [DSL validation](../../alpha-factor-forge/src-tauri/src/discovery_core/dsl.rs), execution.rs | For params use existing validated ParamsSignalConfig and used entry/exit max_signal_lookback_bars, including cross/prev conventions; derive_embargo_bars adds the declared holding allowance with checked safe integers. Fixed DSL uses its existing validated max_lookback_bars plus that allowance, as the actual executor does. Unsupported signal/mode/overflow refuses; no caller-provided warmup or artifact-result backfill. Freeze independent params/DSL controls before this adapter code. |
| [trial_split_identity / candidate_split_hashes](../../alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs), [trial ledger](../../alpha-factor-forge/src-tauri/src/research/trial_ledger.rs) | Reproduce original split/embargo/walk-forward identities and seeds/engine hashes; require every exact original batch event, workspace/instrument/family, kind/origin/index/hypothesis/strategy/dataset/snapshot link. Never accept mutual attempt membership or a selected event subset. A raw candle_count or stale registry observation is not authenticated candle/current-count evidence. Registry observation/final live fence keep their own bounded adapters. |
| [candidate result writer](../../alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs) | candidate-result-v1 has version, attemptKey, runId, candidateIndex, strategy{id,hash,definition}, dataset{id,hash,interval}, train, validation, record, digest. v2 additionally carries the declared original walkForward evidence. Exact bytes/hash/length/kind and all lineage must agree with the authenticated file; no mutable summary substitution or using Test outputs. File work remains 4b-3. |
| [calendar/instrument registry](../../alpha-factor-forge/src-tauri/src/market/registry.rs) | Reconstruct supported original typed documents and validate existing calendar/instrument rules, all retained scalar projections and original content hashes under their original numeric policy. The current registry's lossy display readers are insufficient. No latest instrument revision/default calendar. |
| [snapshot writer](../../alpha-factor-forge/src-tauri/src/market/snapshot.rs) | Original snapshot content contains version, instrumentId, instrumentContentHash, interval, datasetHash, priceBasis, calendarId, corporateActionVersion, costProfileVersion, kind and sorted sourceRecordHashes. Reproduce that existing identity and exact complete references; asOf is retained beside it and was deliberately excluded from the original snapshot identity. Refuse unsuitable linked comparison/rejected sources, never filter/promote them to make a family eligible. |
| [provenance writer](../../alpha-factor-forge/src-tauri/src/market/provenance.rs) | Validate original scope/series/role/acceptance/revision/raw-reference projections and supported record identity; require actual pinned original raw bytes/availability/candle evidence in 4b-3. Correct-looking row hashes or current files are insufficient. |
| action/cost/numeric cost evidence | P06 version-only snapshot references do not supply immutable supported action/cost originals. Missing originals remain unsupported. numericCostsHash requires finite nonnegative exact fee/slip bits consistent with original config and authenticated cost profile before framing. Do not hash a version string, trust an ID, synthesize empty originals or expose an unverified numericCostsHash from the first slice. |

The [numeric audit](../numeric-json-identity-audit-v1.md) remains unresolved:
legacy serde_json can reconstruct a different float from original tokens.
No float_roundtrip/arbitrary_precision change, dual-hash acceptance, token repair,
silent rehash or new numeric compatibility policy is part of this work. Exact
byte checksums and successful legacy identity reconstruction are distinct from
proof of original numerical execution. Unsupported reconstruction refuses before
any source builder. No new protected Validation/Test generation/ranking/prompt
input, statistical calibration or reserved-seed activity follows from these rows.

## 2. Ordered implementation slices

1. **4b-2c-1 raw original digest observations**, sections 3-6 below. Pure bounded
   framing only, over the sealed complete graph; no semantic suitability claim.
2. **4b-2c-2 original candidate/engine/hypothesis lineage adapters.** Before code,
   freeze original-format projection fixtures and producer-positive controls
   for the matrix above, including v1 params, v2 fixed DSL, v3 walk-forward,
   shared bases/dedup attribution and legacy numeric refusals. Never recreate the
   #217 synthetic-only compatibility gap. No source builder yet.
   The [producer projection refinement](research-original-producer-projections-v1.md)
   freezes independent controls and an initial private producer-helper/conformance
   slice before subsequent bounded consumer API/code refinement.
3. Warmup/market/trial semantic adapters require their same independent evidence;
   file/candle/availability authentication remains 4b-3, and current actual-owner/
   registry/IMMEDIATE final admission remains 4d. Missing action/cost originals
   must be resolved with a separately scoped supported storage/input design.

Only 4b-2c-1 enters Next after this verified design delivery. Parent 4b-2c/4b-2/
4b/P13c-4 remain incomplete. No RegisteredSource construction or runtime activation
ships until every mandatory original semantic input and digest is authenticated.

## 3. First code API: sealed graph in, raw digest observation out

Add private source_rows::graph::original_digests. It can read its parent's private
captured records without making fields mutable/public. Expose only within
confirmation_inputs:

```rust
observe_raw_digests(rows: &SourceRowsObservation) -> AppResult<RawDigestObservation>
```

This is the sole constructor. No caller row list, part iterator, domain string,
candidate subset, callback, alternate locator or per-field public hash factory.
No SQL/recheck/IO/registry/lock/finish/worker/clock/hardware access. The input is
the immutable graph produced by capture_source_rows_in; an old matching copy
remains historical consistency. Future final preparation must still recheck the
actual saved graph and authenticate all missing evidence before admission.

RawDigestObservation fields, in order: campaign_document_hash: String,
engine_manifest_hash: String, candidates: Vec<CandidateRawDigests>.
CandidateRawDigests fields: index: i64, definition_hash: String,
execution_hash: String, lineage_hash: String. Private fields; Clone/Eq/PartialEq
only, no Debug/serde/Default/unchecked constructor/RegisteredSource conversion or
research/runtime re-export. Ancestor-only read-only getters return borrowed
str/slices and copied indexes. Every digest is exactly 64 lowercase hex.

No whole source graph clone or parsed JSON trees are retained. At N<=4096 the
output has exactly 3*N+2 strings, <=786560 digest-text bytes (plus bounded Vec/
String/index overhead). This is a separate bounded observation, not an assertion
about whole-process memory. No cost/source/artifact/family/warmup field or empty
placeholder is added. Hashing an uninterpreted claim does not certify semantics.

## 4. Exact framing and bounded work preflight

Preserve the parent H framing: ASCII domain, one zero byte, u64be part count,
then each u64be byte length and exact bytes. A private fixed-domain enum/helper
may stream into existing SHA-256; it is never callable with an arbitrary external
domain or part factory. No concatenated payload buffer or JSON serialization.

| Output | Exact ordered bytes |
| --- | --- |
| campaign_document_hash | SHA-256 of exact campaign document_json UTF-8; no H prefix. Campaign re-freeze is already a separate root-selection check. |
| candidate.definition_hash | H(p13-definition-original-v1, [its strategy original_definition_json]) |
| candidate.execution_hash | H(p13-execution-original-v1, [original run config_json, its attempt engine_fingerprint_json]) |
| candidate.lineage_hash | H(p13-lineage-original-v1, [its referenced hypothesis content_json, its attempt input_fingerprint_json, its attempt engine_fingerprint_json, exact trial_event_id UTF-8]) |
| engine_manifest_hash | H(p13-engine-manifest-original-v1, [index u64be, original engine JSON] repeated for all attempts in ascending candidate index); part count is 2*N, never N/one/selected/latest. |

Frame size is domain.len+1+8+sum(8+part.len). Campaign direct SHA work is its
original byte length. Index parts have length 8 even for index zero. Preserve
all whitespace/escapes/long-float/huge-exponent tokens; never parse numbers,
normalize prefixes or use text indexes in framing. Existing strategy/dataset/
campaign/hypothesis/market/ledger identities are not replaced by these digests.

**Before the first SHA update**, preflight the entire operation with checked
integer conversions/additions/multiplications: N nonempty<=4096, exact ordered
nonnull index/event/reference resolution from the sealed complete graph, frame
part counts/lengths, output count and total hashed work. A shared hypothesis,
config or engine is charged every time its bytes appear in a different digest.
Deduplicated retained-row storage is not deduplicated hashing work.

Require total hashed bytes, including every domain/count/length/index prefix,
<=33554432 inclusive. This new dormant adapter's private fixed work ceiling
prevents a bounded 4 MiB root/hypothesis being rehashed thousands of times.
Do not reset per candidate/domain, truncate, memoize a different preimage, lower
the family, refund, hash before counting or retry under another cap. A graph can
remain a valid SQL observation while exceeding this additional digest-work cap.
Existing capture/runner/public behavior and statistical policy do not change.

All failure paths return static original_digests_invalid without content/IDs;
no partial digest observation escapes. No configurable ceiling/bypass or generic
public budget constructor. Bounded local arithmetic tests may exercise internal
states; real captured rows must exercise whole-operation exact/one-over work.

## 5. Independent frozen vectors before code

Python hashlib + struct u64be, without importing any project implementation,
independently framed the committed [corrected graph fixture](../../alpha-factor-forge/fixtures/research/source-row-graph-v1.json).
Its input graph has 6102 retained text bytes/184 nodes; that is separate from
hash work. Original campaign bytes=1061. Total work=7938 bytes; output text=704.
The fixture contains raw claims unsuitable for semantic admission; these values
are framing controls only, not successful source/execution attestations.

| Field / candidate | SHA-256 | Framed bytes |
| --- | --- | --- |
| campaign_document_hash | a9087d0ba19e390887639986a7d738d95fdcda1b0e24aaa7af65878eec72ec67 | 1061 (direct) |
| definition_hash, each 0/1/2 | 93a7e97bfdc4ead1fbcf32fe12223c0548304898779e7e2d4c2fe828c9f7c2f3 | 86 each |
| execution_hash, each 0/1/2 | 9b050faf9bec8072320a5b2fc9687dc438852cb99abf004cf3e787daa73b58e2 | 1936 each |
| lineage_hash, 0 | 57b88054c233a12680af6cccd96b74abaa57abda57adb71389fbef2a3953e313 | 231 |
| lineage_hash, 1 | 8c7b6d09aff50fb7b7ec2ba69777d308d03590c7d53a71344bf9b5865a21805d | 231 |
| lineage_hash, 2 | 13ce9ea17324a51dc47310b8c8385d283c1e9b184ca74885cdc6811e16481725 | 231 |
| engine_manifest_hash | 951e9ff8b565178772c63629e7288a146c10b372ae6f73a06cfb0ad8c363bdcb | 118 |

Framing controls below use literal ASCII parts; whitespace is space, LF, null,
TAB. They freeze byte layout only and are not additional production domains or
public APIs. One empty part is distinct from no parts (production shapes fixed).

| Domain suffix / parts | SHA-256 | Bytes |
| --- | --- | --- |
| definition-original-v1 / [ab] | a7f9ca48346f44385d28948d98ceb8ed9a5bc30c44fe0b720972300f07ace634 | 45 |
| definition-original-v1 / [a,b] | f102583b717e6f7c1813185865310e247ead39ec1a4748f2c2133cb767270c4e | 53 |
| execution-original-v1 / [a,b] | 8801a745f2aef393b8d11a4d1595596395a8d9b49482f3f52b3252066ebb70d5 | 52 |
| execution-original-v1 / [b,a] | 3446090eb2a57af561580c31d30031b9b3bee293ea5be5dc7ae8adc3f6bf7b89 | 52 |
| definition-original-v1 / [space LF null TAB] | 28ef1ab2ae2856edcfbcdeb50167e376fec049b36ba59e8bb4bbd17e546b1689 | 50 |
| definition-original-v1 / [empty] | 2b987984ae2df60e04a4d4b219b53035c06e5ffce8716591b8b7d97d898f265f | 43 |
| definition-original-v1 / [leading-zero-138] | 006edaa51939502173b18a36b2135a5d5ae6a9272a4f5fbd941a255cb925d393 | 59 |

Every suffix in this control table has the literal p13- prefix. No implicit
domain shortening is allowed. Runtime code/test serializers must not generate
their own expected hashes or rewrite original fixture bytes to match a result.

## 6. Required first-slice evidence and delivery

- Use actual captured migrated file-backed graphs and isolated leases, including
  the independent three-candidate oracle and the actual writer's shared-base
  positive control. Compare all digests/order/count/work/output bytes and original
  graph equality. Uninterpreted raw JSON remains unchanged.
- Same bytes repeat deterministically. Whitespace/part order/event/engine/index/
  definition changes affect their precise outputs; shared hypothesis charges per
  candidate. All candidate engines enter the manifest, including failed members.
  No latest summary or artifact file is read. No claim becomes a source.
- Real rows exercise exact/one-over 32 MiB hashed work while the SQL graph remains
  valid under its own retained bounds; add checked overflow/nonempty prior usage
  and 4096 candidate/output bounds. A test-only SHA-update counter/tripwire may
  prove whole preflight occurs before hashing without processing oversized work.
- Preserve caller transaction/reservation/rows/schema/total_changes/mutation_seq
  across success/refusal; observer itself has no DB/IO argument. Saved old/copy
  equality grants no live-owner or registry capability.
- Six explicit runtime mutants: omit part lengths, omit part count, use decimal
  index bytes, select only one engine, skip whole-operation work preflight, and
  reset work per domain. Each
  must assertion-fail; restore exact source before full locked Rust, frontend,
  check/Clippy/scoped format and six exact-head CI jobs.

This design runs no runtime/statistical tests solely for Markdown. Verify the
independent vectors, source grounding, links/scope/lifecycle and six-job delivery;
then expected-head merge and clean-main verification before 4b-2c-1 code. No
source/schema/dependency/workflow change, execution, command/provider/UI,
reservation/spend, policy/calibration or protected acquisition. Record evidence
in the [handoff](../../handoffs/2026-10-24-p13c4b2c-semantic-adapters-v1.md).
