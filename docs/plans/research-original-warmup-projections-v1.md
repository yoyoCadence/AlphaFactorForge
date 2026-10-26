# P13c-4b-2c-3 original warmup/embargo producer contract v1

Date: 2026-10-26
Baseline: verified #229 (`5de7685`). Design/data only.

The [ordered original adapters](research-original-semantic-adapters-v1.md#2-ordered-implementation-slices)
and [source field contract](research-source-reference-observation-v1.md#5-original-byte-digests-and-semantic-assembly)
remain binding. #227/#228/#229 complete original mode coverage, not warmup,
market/trial, file/candle or final source admission. Freeze independent original
executor controls before adding another private consumer.

## 1. Scope and delivery order

1. **3-design (this slice):** freeze this producer-level catalog and the new
   [literal fixture](../../alpha-factor-forge/fixtures/research/original-warmup-producer-projections-v1.json).
   Validate independently and deliver with six exact-head/base CI jobs.
2. **3a:** compare every literal against the unchanged actual private executor's
   `CandidateStrategy::parse` then `embargo`. Tests only; section 4 fixes scope.
3. **3b-design:** after verified producer delivery, refine the sealed graph API,
   complete-operation analysis/copy/node/output budgets and independent captured
   consumer/boundary controls. Do not implement a consumer in 3a.
4. **3c:** only after that contract's verified merge, implement the bounded
   private consumer and its frozen graph controls in a separate delivery.

Only 3a may become Next after this design's verified delivery. Parent3 stays
incomplete until the final consumer delivery; 4b-2c/4b-2/4b/P13c-4 and all other
source/live gates remain incomplete. No source builder, command, schema,
dependency, provider, registry spend, candles, backtest, protected acquisition,
calibration or reserved seeds are used here.

These are original history-bar conventions, not demonstrated first trade/signal
indices, warmup sufficiency, fold eligibility or numerical execution. In
particular, the signal builder's bar-zero guard and unused indicator precompute
do not redefine this recorded usage-aware lookback contract.

## 2. Actual source grounding

| Original implementation | Obligation |
| --- | --- |
| [CandidateStrategy parse/embargo](../../alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs) | Parse the complete candidate wrapper first. Params projects every period and the other execution fields; all projected periods must be positive integral numbers representable as usize, including unused ones. DSL clones/validates its nested document. Unsupported modes refuse. Then call the same embargo method used by execution; never replace a failed derivation with null/zero. |
| [Params embargo](../../alpha-factor-forge/src-tauri/src/discovery_core/embargo.rs), [signals](../../alpha-factor-forge/src-tauri/src/discovery_core/signals.rs) | Use both actual entry/exit signal IDs and only their dependent periods. Preserve cross/previous-bar increments, max(fast,slow), the original RSI/MACD conventions and checked safe-integer arithmetic. Standalone embargo's used-only validation does not bypass candidate parsing. |
| [DSL validator](../../alpha-factor-forge/src-tauri/src/discovery_core/dsl.rs) | Retain strategy-dsl-v1 whitelist/types, exact fields, parameter/default rules, suspicious-token checks, both expression trees, 64 total decision nodes and depth8. Use its actual max_lookback_bars report; no home-grown production analyzer. |
| [Original config](../../alpha-factor-forge/src-tauri/src/discovery_core/config.rs) | The original config has stricter full strategy/domain admission, supported envelope contracts and nonnegative safe holding allowance. Producer-level malformed inputs below are not admissible configs. Preserve existing parser/numeric/identity rules. |
| trial_split_identity / candidate_walk_forward_report in execution.rs | The split identity may retain null derivations and does not replace an explicit successful warmup result. The actual walk-forward candidate check separately rejects minimumTrainBars below lookback. No raw candle_count, nullable split or old report authenticates candles/eligibility. Trial/split adapters remain later work. |

Current supported desktop/CI targets are 64-bit. The deliberately unsafe-used-
period control uses 2^53, which fits their usize but exceeds JS safe range:
CandidateStrategy parses it; params embargo refuses it. The original config
parser would reject it earlier. No broader platform/admission promise follows.

## 3. Frozen conventions and independent literals

For params, take the maximum of the used entry/exit values below, at least1:

| Used signal pair | History bars |
| --- | --- |
| maCrossUp/Down | max(fastMA, slowMA) + 1 |
| emaCrossUp/Down | emaPeriod + 1 |
| priceAboveSlow / priceBelowSlow | slowMA |
| rsiOversold / rsiOverbought | rsiPeriod + 2 |
| macdCrossUp/Down | max(macdFast, macdSlow) + macdSignal |
| bbLowerTouch / bbUpperTouch | bbPeriod |

For DSL, price leaves/CONST read1; ordinary indicator leaves read len; RSI/ROC
read len+1. Pointwise arithmetic/comparison/boolean operations take child max;
CROSS_UP/DOWN add1; SHIFT/RISING/FALLING add their declared n. Both trees and
typed parameter defaults participate. Unused valid parameters do not inflate
lookback, but invalid unused parameter declarations still invalidate the DSL.

The [2,400] indicator length and [1,400] temporal argument rules apply to each
argument, not the final analysis. A valid depth8 path reaches
401+7*400=3201; the fixture contains that exact temporal path and an 802-bar
ROC/SHIFT/cross example. Do not silently clamp either result to400. These
formulas describe the existing grammar; no validator limit is changed.

For either mode, holding allowance is an integer in [0,9007199254740991].
Embargo is lookback+holding, also <=9007199254740991 inclusive. Preserve overflow
refusal; no wrapping/saturation/rounding or reduced allowance. Derived results
are the exact three original EmbargoDerivation fields, not a new contract.

The fixture freezes **47 standalone inputs:22 successes and25 refusals**, and
**eight existing families/23 candidates**. It includes exact candidate wrapper
JSON/SHA, integer allowance and either the complete original derivation or the
expected refusal phase (`parse` vs `embargo`). No new runtime error text is
defined. Cases cover all12 supported params signal IDs and successful examples
of all14 DSL indicators/all21 operators, exit dominance, swapped MACD periods,
unused periods, typed/nested temporal expressions, exact safe sums/one-over,
missing/type/fraction/zero periods, unsupported signals/modes and invalid DSL
references/parameters/root types/node/depth/argument limits.

The eight family entries reference the unchanged earlier projection and mode
fixtures by fixed filename/case ID, with every original candidate index/hash/
mode and expected triple. All19 params members have lookback22/embargo32; all4
DSL members have lookback4/embargo14 (holding10). Config order is not candidate
order. V3 params and mixed declarations retain minimumTrainBars20, below the
params lookback22; these warmup controls do not repair that declaration or
establish executable folds. Failed family members cannot later be filtered out.

`sourceFixtures[].lfPackagingSha256` pins only each outer fixture's UTF8 JSON
packaging with CRLF converted to LF, matching its Git blob. This accommodates
checkout line endings; embedded original JSON strings/escape sequences are
never normalized or rehashed as product identities. `strategySha256` hashes
the exact decoded strategyJson UTF8 bytes. All old fixture files stay untouched.
The new fixture's LF-packaging SHA256 is
`b67268643162eca6da5c97b056b61d5461c1a31ae0ea02372d8b395a8f31ee06`.

These finite ordinary/integral controls claim no long-decimal numeric-policy
repair. The [legacy audit](../numeric-json-identity-audit-v1.md) and prior numeric
identity refusals remain binding.

## 4. First code: actual executor conformance (3a)

Add one private cfg(test)-only child module under discovery_runner/execution.rs
(an explicit path to discovery_runner/warmup_controls.rs is sufficient). The
child can call its parent's existing private CandidateStrategy directly; no
production helper, constructor, parser extraction or public test bridge is needed.
Files: that test module/declaration, task status and a handoff only. Existing
runtime bodies/fixtures/parser/identity/budgets remain unchanged.

For every standalone input, parse its exact literal Value through the actual
candidate parser. Assert the stated parse success/refusal before embargo; for
successful parses call the actual embargo method with the literal i64 allowance.
Compare every returned field, or assert embargo refusal at that phase. Repeated
successes must be deterministic and preserve the input Value/bytes. Do not merely
deserialize ParamsSignalConfig or read the null-tolerant split identity.

For every referenced family, load only the two fixed fixture constants, verify
outer packaging pins, candidate/index/hash/mode and unchanged config allowance,
then feed all23 exact stored candidate definitions through the same actual calls.
Compare the frozen triples; expected values never come from another runtime
derivation. No DB, jobs, candles, indicator series or full execution is needed.

Seven temporary source mutants must assertion-fail, then restore exact original
bytes and preserve failure logs: omit params cross increment; ignore exit
lookback; include unused periods; omit DSL temporal offset; cap derived DSL
lookback at400; omit holding addition; bypass the final safe-integer ceiling.
Compiler errors do not count as caught assertions. These are future 3a tests,
not evidence claimed by this design fixture.

Full isolated locked Rust precedes Node-heavy checks. Then locked all-target
check/Clippy, scoped format/typecheck/Vitest/build and all six completed
exact-head/base CI jobs are required. Fresh reviews/main, expected-head merge,
GitHub merged readback and clean-main/full-tree equality precede 3b-design.

## 5. Mandatory bounded consumer refinement after 3a

Do not infer a public warmup API, output layout or fresh per-item allowance from
this scalar fixture. Refine the sole sealed graph constructor only after actual
producer delivery. It must verify the complete original lineage and actual
varied candidate definitions, derive holding from the original config, preserve
all failed members/actual references and use one preparation. No supplied
lookback/allowance, first-base proxy, selected subset or source capability.

Freeze complete-operation analysis/clone/text/node/output costs before code,
including CandidateStrategy's nested DSL clone and the validator's serialization,
lowercase text, parameter map and tree walks. Compose with the existing P/E/W/C/O
work; never reset pools, hash/copy/analyze before preflight or change the old
lineage entry's real capacities. Independent graph literals must include varied
candidates whose own lookbacks differ from their base, mixed/late/failed-member
refusals, immutable caller state and feasible real exact/one-over boundaries.
No generic callback/catalog factory or mutable/public graph parts.

Only successful bounded consumer delivery can supply a historical warmup
observation. It still cannot authenticate files/candles, action/cost originals,
market/trial links, numerical execution, current registry/owner/final admission,
RegisteredSource or numericCostsHash. All remain separate required gates.

## 6. Design verification and session boundary

Standalone Python json/hashlib authors the hand-calculated controls without
importing/executing project code. Separate Node built-ins/BigInt independently
check all47 phases/triples, all23 family references and source/strategy checksums;
strict outer/nested JSON, supported coverage and source/link/scope/lifecycle
checks accompany them. The finite checker is a local artifact verifier, not a
shipped general DSL/numeric parser or substitute for actual producer tests.

No local runtime suite is rerun solely for these docs/data. Six-job design CI
checks the unchanged #229 runtime (1090 Rust/one ignored,1242 Vitest/80 files,
98 E2E); green design CI does not mean future producer/consumer tests ran.
Record delivery evidence in the handoff/PR body, then stop after this verified
merge as requested. 3a may be Next for a later explicit resume; 3b-design/3c and
every parent/source/live gate remain incomplete.
