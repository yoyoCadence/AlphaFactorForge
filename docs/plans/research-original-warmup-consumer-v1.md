# P13c-4b-2c-3b bounded original warmup consumer contract v1

Date: 2026-10-27. Baseline: verified [#231](https://github.com/yoyoCadence/AlphaFactorForge/pull/231)
(`f53a228`). Design/data only; 3c implements this contract after verified delivery.
The [producer contract](research-original-warmup-projections-v1.md),
[lineage contract](research-original-lineage-semantics-v1.md) and
[mode catalog](research-original-producer-mode-coverage-v1.md) remain binding.

## 1. One sealed historical observation

Freeze `observe_original_warmup(rows: &SourceRowsObservation) ->
AppResult<OriginalWarmupObservation>` at the existing graph boundary, visible
only inside `research::confirmation_inputs`. Its sole input is the sealed,
immutable captured graph. It prepares original lineage exactly once, reserves
the entire next stage, verifies complete original lineage, then calls the actual
executor for every retained candidate in original candidate-index order.

`OriginalWarmupObservation` owns one existing `OriginalLineageObservation` and
one parallel `Vec<EmbargoDerivation>`. Both lengths agree and are nonempty;
derivation i belongs to lineage candidate i, including failed members. Private
fields, `Clone/Eq/PartialEq`, borrowed `lineage()` and `derivations()` getters
only. No Deserialize/Default/parts constructor, mutable getter, public graph
builder or independently constructible warmup result. No copied definitions,
serialized output, work ledger or validator errors survive in the observation.

Holding comes only from the prepared original config. Each definition is the
actual referenced stored strategy whose complete original lineage has just
agreed with that candidate. Never substitute the base strategy, first candidate,
maximum across the family, nullable trial split or supplied lookback/allowance.
Do not filter by job/attempt status. A late refusal discards the whole result;
no partial observation, truncation, retry, refund or fresh per-item budget.
Every failure at this entry maps to static `original_warmup_invalid`.

This observes historical history-bar conventions. It authenticates neither
candles/files, market/trial/action/cost originals, executable folds, first usable
signal/trade, numerical execution, current registry/owner nor final admission.
In particular v3 minimumTrainBars20 remains below some lookbacks. No
RegisteredSource/numericCostsHash, command, schema, dependency, provider,
calibration, acquisition, registry spend or reserved seed use is added.

## 2. Narrow reuse, before any new payload work

3c may add a private `original_lineage/warmup.rs` child and its tests. Refine
the existing private semantics context into fixed preflight, verification and
finish stages; keep fields/constructors private and expose only the narrow
sibling operations needed by warmup. Preflight retains its borrowed ID indexes
and unchanged Work. Verification performs the same original config, candidate,
cost bits, saved DSL, input, engine, winning-base and shared-hypothesis checks.
Finish alone copies final lineage strings. The old lineage entry uses those
same stages with its old ceilings, order, error and output. No generic callback,
arbitrary document/domain factory or caller-selectable resource policy.

Warmup uses that context once: prepare -> borrowed context and all-work reserve
-> complete lineage verification -> all actual candidate derivations -> final
lineage/output assembly. Keep the temporary scalar derivation vector private
until every derivation succeeds. Do not call the old graph observer and prepare
again, re-enumerate, or duplicate hashing/canonicalization to obtain references.

The only new executor bridge is a fixed crate-internal function in the private
execution module, reached through discovery_runner. It takes a borrowed Value
and the internally selected holding integer, calls the unchanged
`CandidateStrategy::parse(definition)?.embargo(holding)`, and returns the existing
three-field derivation. CandidateStrategy/signals remain private. No new analyzer,
catalog/closure injection, candles or full execution. The sealed graph entry
never accepts those scalar arguments from its caller. The bridge is internal
plumbing and grants no source capability. Retain all 3a producer controls and
add direct bridge parity against the same frozen 47 inputs in 3c.

## 3. Composed complete-operation reservation

Unchanged original capture and preparation are earlier, separately bounded
phases: selected parses P and raw expansion E share <=33554432 bytes and
262144 JSON value nodes. Keep their checks before their existing materialization
and enumeration. Original config validation during preparation stays in that
phase. The following reservation covers **all post-preparation** lineage and
candidate warmup work; it cannot reset or erase P/E or legacy W/C/Q/O charges.

Let Wl/Cl/Ql/Ol be the unchanged whole-lineage preflight from its contract.
J(V) is the exact compact serde JSON UTF8 length measured without owning output;
N(V) counts value nodes including each container, excluding keys. Byte lengths
of strings mean decoded UTF8 bytes. Use checked additions/multiplications and
conversions; refuse atomically on overflow or any whole-stage overage.

For a params candidate let S be the summed lengths of entrySig, exitSig,
fillMode and direction. Reserve additional copy `4*S+1024`, analysis `S+256`
and zero additional Value clone nodes. This covers the actual two owned signal
IDs and bounded field/derive/context error text, including an invalid signal.
Missing/non-string fields may refuse in the borrowed preflight; no replacement
default may reach the executor.

For a DSL candidate let V be its actual nested DSL, J=J(V), N=N(V), and S the
summed fillMode/direction lengths. Reserve additional copy
`16*J+4096*N+8192+4*S`, analysis `128*J+4096*N+8192`, and N additional Value
clone nodes. Before this reservation, borrowed inspection refuses any root
params-object key longer than64 UTF8 bytes. This is an explicit resource gate
for this new entry, not a change to the producer validator or old lineage
capacity; unknown/invalid shorter declarations still go to the actual validator
after lineage agreement. No independent scalar/string payload is cloned while
measuring. An absent/malformed DSL may refuse without invoking validation.

The deliberately conservative DSL charge accounts for the nested Value clone,
serialization, lowercase text (reserve up to3*J), all26 suspicious-token scans,
parameter key copies/BTreeMap work, both expression walks, temporary child
vectors, bounded paths and diagnostics, errors.join and wrapping error text.
The64-byte key gate prevents a long parameter name from being repeated across
arbitrarily many unknown fields without a linear bound. Paths stop at depth9;
arity is at most3 and the validator's64-node/depth8 rules are unchanged. N
covers unused params and malformed extra subtrees too. The4096*N allowance
dominates per-node/map/diagnostic metadata and repeated bounded paths;16*J
covers textual copies, including failed validation. Analysis is a conservative
logical work reservation, not exact instruction count or a time guarantee.
No OS allocation/whole-process peak-memory measurement is claimed.

For n candidates, including every failed member, freeze:

| Whole-stage quantity | Reservation | Inclusive ceiling |
| --- | --- | --- |
| W hash bytes | Wl | 33554432 |
| C copy units | Cl + sum(additional copy) + 24*n | 67108864 |
| A analysis units | sum(additional analysis) | 268435456 |
| Q Value clone nodes | Ql + sum(nested DSL nodes) | already reserved P+E nodes |
| O output units | Ol + 24*n | 4194304 |

The24*n charge is the logical fixed width of three i64 scalars per derivation,
not JSON decimal text. It is additional to existing lineage strings, not a
second set of their hashes/IDs. Fixed Vec/String headers and scalar indexes
retain the <=4096-member metadata bound. Q is audited against P+E, not a new
independent allowance. Do not invent an unreachable standalone exact-Q test;
retain existing real P/E byte/node limits and assert the composed Q on captures.

Reserve all W/C/A/Q/O before the first post-preparation hash, Value/text copy,
DSL clone/serialization/lowercase/validation or final output allocation.
No serde-to-string/validator call merely to estimate J or N. Existing counting
sinks and bounded structural walks suffice. Once admitted, use unchanged
canonical/hash functions and the actual executor; counts do not replace them.

## 4. Frozen complete graphs and references

The [new fixture](../../alpha-factor-forge/fixtures/research/original-warmup-consumer-v1.json)
was authored independently with Python json/hashlib/struct and no project code
imports/execution. A separate Node built-in checker verifies identities, seeds,
history projections, complete work components and graph recipes. This design
claims no future consumer/capture test has run. Source package pins follow LF
checkout packaging only; decoded original JSON strings and product identities
are never normalized. New LF-packaging SHA256:
`b0da3af1b12fe35dcb58c3622f1abf5dff45211eac8c9489ce7ccfa550307578`.

Two positive literal families add seven candidates; eight unchanged earlier
families retain all23 candidates and now have complete composed warmup work.
The new params base fastMA5/slowMA21 varies slowMA9/21/33. Hash-sorted candidate
lookbacks are [22,10,34], so two differ from their base22; holding7 gives
[29,17,41]. The v3 mixed config declares DSL first but candidates begin with
the three params members, then DSL lookback4/embargo11. Its original pretty
config bytes differ from canonical hash bytes. No changed DSL/numeric convention.

| Positive literal | W | C | A | Q | O |
| --- | --- | --- | --- | --- | --- |
| v1-varied-slow | 5504 | 14859 | 855 | 114 | 589 |
| v3-mixed-varied-dsl-first | 8037 | 149077 | 164951 | 183 | 762 |

Reuse the original full SQL fixture/table capture helper with these independently
frozen config/definition/input/engine/hypothesis strings. SQL IDs are not
candidate order: the mixed reference control assigns strategy IDs[41,11,31,21]
and first-encounter hypothesis IDs[91,51], changing only the literal input
strategyId references. Freeze actual ID lookup, not positional zipping. A
separate status control marks params index2 and final DSL index3 failed in the
attempts and both jobs; all four outputs still agree with their literal triples.

A third whole original-lineage family freezes holding=9007199254740969
(safe maximum minus22). Candidate0 derives the exact safe maximum, candidate1
succeeds, candidate2 lookback34 refuses embargo; its failed status must not hide
the refusal. Candidate3 would succeed but no whole output is returned. This is
a real coherent-lineage warmup failure, separate from later-invalid resource
claims. All config/input/hash/seed/history literals are independently frozen.

Late nested/saved DSL identity, failed-member fee, final engine catalog and
final ID-reference contradictions must refuse before any warmup bridge entry.
The65-byte params-key resource gate refuses before payload/analysis. No skipped
row, first-base shortcut, old nullable split or successful prefix may pass.

## 5. Feasible real boundaries

Small recipes freeze large captures without committing large blobs. Build
normal rows with actual original producer/repository/history helpers in fresh
owned SQLite test workspaces; then apply only the stated stored-claim edits.
No candles, execution/artifacts or registry events. Exact resource acceptance
of identity-invalid claims proves only preflight acceptance; afterward the
whole observer must refuse. Never repair identities to manufacture success.

- **W:** reuse the unchanged eight-candidate hash recipe from the lineage
  fixture. New params charges compose to C65326944/A2280/O1480 at W33554432.
  Its original one-byte entryCode edit gives W33554433; all other ceilings fit.
  The old entry's exact W/C/O controls remain unchanged and must still pass.
- **A:** v2 fixed-DSL template, cap4096, base warmup-resource with original DSL
  name Resource, plus the varied params base named warmup-params with no axes.
  Only stored nested/saved DSL name becomes2095882 ASCII x bytes; append99 x
  bytes to the sidecar's stored entrySig. A=268435456 exactly. Appending one
  more sidecar x gives268435457; all other limits fit. The sidecar makes +1
  reachable despite the DSL charge's128-unit byte increments.
- **C:** v2 same single DSL base, stored params p00000..p11999 each integer0,
  first12 changed to10, stored nested/saved DSL name744623 x bytes, and stored
  wrapper resourceCounter integer0. C=67108864 exactly. Change only that
  wrapper counter to integer10: JSON grows1, binary number width stays9,
  so C=67108865; A161523456 and every other ceiling still fit. The dormant
  wrapper is an intentionally later-invalid identity claim, not a new grammar.
- **O:** actual original whole-positive production, main ASCII m ID3648 bytes,
  slowMA2000, axes fastMA1..44 and emaPeriod1..25 (1100 candidates); other
  ASCII o ID876 bytes, empty axes and distinct emaPeriod51. Then
  `64+1100*(164+3648)+(164+876)=4194304`. One extra o gives4194305.
  Every actual derivation is lookback2001/holding10/embargo2011. Exact totals
  include C7073906/A313785/P+E10453124 bytes/123650 nodes, all within limits.

The fixture pins every P/E/W/C/A/Q/O total/component for A/C/O and the prior
W reference. Each original JSON is <=4 MiB; every axis is within64 values,
candidate/job counts and mechanism text stay within capture limits. In 3c,
real exact preflight and +1 captures must match the literals. Payload and
warmup-entry tripwires must prove overage cannot hash/copy/analyze/output.
Small private overflow tests only supplement these real graph controls.

## 6. 3c verification, delivery and remaining gates

Retain all old preparation, semantic/mode/producer controls and fixture bytes.
Add exact whole results and every manifest/work field for the two positives,
all eight prior families, nonmonotonic IDs, compatible original byte formatting,
failed members, coherent late embargo failure, all negative controls and the
real boundaries above. Assert one preparation/enum and exact materializations;
immutable captured bytes/rows and repeat determinism. Count actual bridge/DSL
analysis entries separately from earlier config validation. Static-error,
preflight and no-partial-result assertions must not rely only on error text.

For success and each refusal phase, call inside an already active caller write
transaction/savepoint. Compare all table snapshots, leave pending caller writes
and transaction/reservation intact, keep another zero-timeout writer busy until
the caller rolls back, and prove no lease/connection/file/event side effect.
Use existing snapshot/unchanged/writer_busy helpers and fresh exclusive owned
roots; verify exact paths/no reparse points before cleanup. No stale PID reuse.

Temporary assertion mutants must catch a base-definition proxy, skipped failed
member, supplied/omitted holding, per-candidate reset, uncharged DSL copy/work,
analysis before preflight and positional reference lookup; restore exact bytes.
Run the complete isolated locked Rust suite before Node-heavy checks, locked
all-target check/Clippy and scoped format, then Vitest/typecheck/build and all
six exact-head/base CI jobs. Broad unrelated cleanup/producer/parser/identity
changes remain out of scope. Source authority and other parent gates stay open.

This design runs independent artifact/source/link/scope checks only locally;
six-job CI verifies unchanged #231 runtime (1098 Rust/one ignored,1242 Vitest,
98 E2E/native roundtrip). Green design CI is not consumer conformance. Record
exact CI/merge/readback/clean-main evidence in the PR receipt and handoff, then
stop. Only a later explicit resume may promote 3c; parent3 remains incomplete.
