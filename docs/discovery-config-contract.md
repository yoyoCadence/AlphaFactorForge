# Discovery run configuration and candidate enumeration (RUNNER-CONFIG-001)

Status: implemented as pure TypeScript reference + pure Rust port, locked by
`fixtures/rs-core/runner-config-v1.json`. No SQLite, threads, Tauri events, UI,
or Test-segment execution exists in this slice.

Authority: the PR #66 handoff Resolution
(`handoffs/2026-07-19-runner-001-design-proposal-v1.md`), sections D2 and D4.
Where this document and the original proposal differ, the Resolution wins.

Owning modules:

| Concern | TypeScript reference | Pure Rust port |
| --- | --- | --- |
| Config admission | `src/services/discoveryConfig.ts` | `discovery_core/config.rs` |
| Candidate enumeration | `src/services/candidateEnumeration.ts` | `discovery_core/enumerate.rs` |
| Sub-seed derivation | `src/services/discoverySeed.ts` | `discovery_core/seed.rs` |
| `strategy-v2` identity | `src/core/hashing/index.ts` | `discovery_core/identity.rs` |

## 1. `discovery-config-v1` (input-only envelope)

The envelope is what a run stores in `discovery_runs.config_json`. It records
INPUT only: no progress, no results, no derived counts. Parsing is total and
fail-closed — the first problem throws a `RangeError` (TypeScript) or returns
`ConfigError` (Rust) with a path-qualified message, and nothing partially
validated is ever returned.

```jsonc
{
  "envelopeVersion": "discovery-config-v1",
  "contracts": {
    "strategyHash": "strategy-v2",
    "datasetHash": "dataset-content-v2",
    "split": "validation-split-v1",
    "embargo": "embargo-derivation-v1",
    "backtest": "backtest-execution-v1",
    "metrics": "metrics-v1",
    "benchmarks": "benchmark-suite-v1",
    "randomEntry": "random-entry-v1",
    "gate": "gate-v1",
    "score": "score-v1",
    "seed": "seed-v1",
    "enumeration": "discovery-enumeration-v1"
  },
  "dataset": { "id": 7, "contentHash": "dataset-content-v2:<64 hex>" },
  "bases": [
    {
      "id": "ma-cross",
      "presetVersion": "discovery-preset-v1",
      "strategy": { /* the exact 25-key params strategy */ },
      "axes": [{ "key": "fastMA", "min": 5, "max": 11, "step": 3 }]
    }
  ],
  "embargo": { "holdingAllowanceBars": 10 },
  "execution": { "startEquity": 10000 },
  "benchmarkCosts": { "feePct": 0.05, "slipPct": 0.02 },
  "randomEntry": { "runs": 200 },
  "gateConfig": { /* the full resolved GateConfig */ },
  "scoreConfig": { "caps": { }, "weights": { } },
  "rootSeed": 20260726,
  "caps": { "candidates": 256 },
  "maxConcurrency": null
}
```

Rules that hold at every level:

- **Exact key sets.** A missing key and an unknown key are both errors. Missing
  keys are reported in declaration order first, then unknown keys in **UTF-8
  byte order**, so both languages report the same problem first. UTF-8 is
  specified explicitly because JavaScript's default string sort compares UTF-16
  code units while Rust's `String: Ord` compares UTF-8 bytes, and the two
  disagree for non-ASCII keys (`"\u{1F600}"` sorts before `"＀"` in UTF-16 and
  after it in UTF-8). Both sides sort by UTF-8, matching the canonical identity
  encoder in `core/hashing`.
- **Version pinning.** `envelopeVersion`, `presetVersion`, and every entry in
  `contracts` must equal the build's own constants. A recorded run from a
  different contract generation is rejected, never reinterpreted. The Rust port
  reads its expected values from the owning modules
  (`gate::GATE_CONTRACT_VERSION`, `split::SPLIT_CONTRACT_VERSION`, …) so a
  bumped contract cannot be forgotten here.
- **Finite numbers only.** Every numeric field must be a finite number; integer
  fields must additionally be JavaScript-safe integers.
- **Durable identity only.** `dataset.contentHash` must be a complete
  `dataset-content-v2:<64 lowercase hex>` value. The version prefix alone is
  not enough: an empty, truncated, uppercase, or non-hex digest is rejected,
  because such a value would otherwise flow into `seed-v1` and seed a real
  random stream. A legacy unversioned hash is ineligible, per the
  Resolution's D0.
- **Decoupled from the caller.** The resolved config deep-clones the dormant
  `entryRules`/`exitRules` arrays instead of aliasing the caller's input
  object, so a later mutation upstream cannot silently change what a recorded
  candidate hash describes.

### Field notes

- `dataset.id` — the SQLite row id, `1..=MAX_SAFE_INTEGER`. Revalidating that
  this id still carries this `contentHash` happens at run START and belongs to
  RUNNER-STORE/EXEC; this slice cannot read the database.
- `embargo.holdingAllowanceBars` — the explicit holding allowance consumed by
  `embargo-derivation-v1`; `>= 0`.
- `execution.startEquity` — finite and `> 0`.
- `benchmarkCosts` — **resolved numeric values**, not a pointer to a mutable
  source. Because benchmarks inherit the candidate's costs
  (`docs/benchmark-suite-contract.md`), these must equal every base preset's
  `feePct`/`slipPct`; a contradiction fails closed rather than leaving the
  fairness convention unauditable.
- `randomEntry.runs` — `1..=1000` (`MAX_RANDOM_ENTRY_RUNS`).
- `gateConfig` / `scoreConfig` — full resolved configs, not partial overrides,
  so the recorded run is self-describing. They are validated with the owning
  modules' own messages; the Rust port calls `gate::resolve_gate_config`
  directly.
- `rootSeed` — an explicit stored `u32` in `[0, 4294967295]`. See §4.
- `caps.candidates` — `1..=4096`. 256 is the v1 UI/default budget; 4096 is the
  engine hard cap and may only move with a config-contract bump plus
  performance evidence.
- `maxConcurrency` — `null` (machine default) or an integer within
  `1..=logicalCores`. `logicalCores` is supplied by the caller, not stored, so
  the module stays pure; the default is `max(1, logicalCores - 1)`. Concurrency
  affects performance only and never a result.

## 2. Candidate space (Resolution D2)

**Params mode only.** `strategy.mode` must be `"params"`. Blocks-mode and AI
DSL candidates are later phases; code mode is permanently excluded from
discovery (manual-only contract). This module is the single place that
rejection happens — the Rust engine has no non-params path at all, which is why
`discovery_core/embargo.rs` and `signals.rs` are allowed to assume an
already-validated params projection.

The base preset carries the exact 25-key `ParamsStrategy` shape. `entryRules`,
`exitRules`, `entryCode`, and `exitCode` are dormant in params mode but are
part of `strategy-v2` identity, so they must be present and well-typed; their
CONTENTS are never interpreted.

`entrySig`/`exitSig` must be one of the 12 supported ids. `stoch*` awaits a
core STOCH indicator and is rejected at admission.

### Numeric parameter domains

| Domain | Fields | Rule |
| --- | --- | --- |
| period | `fastMA`, `slowMA`, `emaPeriod`, `rsiPeriod`, `macdFast`, `macdSlow`, `macdSignal`, `bbPeriod` | integer `>= 1` |
| level | `rsiBuy`, `rsiSell` | finite in `[0, 100]` |
| positive | `bbMult` | finite `> 0` |
| percent | `slPct`, `tpPct`, `feePct`, `slipPct` | finite in `[0, 100]` |
| size percent | `sizePct` | finite in `(0, 100]` |

`percent` is bounded at 100, not merely at 0. `backtestRunner` divides these
legacy percent units by 100 and the engine's `assertNormalizedFraction` rejects
any normalized fraction above 1, so admitting `feePct: 101` would queue a run
that is **guaranteed** to throw once a job executes — strictly worse than an
unchecked field, because the failure would land after jobs already exist. The
inclusive endpoint stays legal: 100 maps to exactly the engine's 1.0 cap.
`level` shares the same numeric range for an unrelated reason (RSI is defined
on 0..100), so the two remain separate domains and can diverge independently.

### Axis whitelist

Axes are the indicator and risk parameters — the HYPOTHESIS. `feePct`,
`slipPct`, and `sizePct` are execution model and are deliberately **not**
axis-eligible: sweeping them would let discovery "win" by assuming cheaper
fills instead of by finding a better signal.

Whitelisted keys: `fastMA`, `slowMA`, `emaPeriod`, `rsiPeriod`, `rsiBuy`,
`rsiSell`, `macdFast`, `macdSlow`, `macdSignal`, `bbPeriod`, `bbMult`,
`slPct`, `tpPct`.

An axis is `{ key, min, max, step }`:

- `step > 0`, `max >= min`, all finite; period axes additionally require
  integer `min`/`max`/`step`.
- Values are inclusive `min + i*step` while `<= max`, computed by
  **multiplication, never by accumulating `+= step`** — accumulation drifts
  differently once a float step is involved, and both languages must produce
  bit-identical values. The parity fixture locks a case whose third value is
  `0.30000000000000004`, not `0.3`.
- At most 64 values per axis (matching the recorded parameter-sweep v1
  convention). Above that, or if any generated value leaves its key's domain,
  the config is rejected.
- An axis key may appear at most once per base. Base ids match
  `^[a-z0-9][a-z0-9-]*$` and are unique within a run.

An axis has ONE representation in each language. The Rust port deliberately
does not keep a separate "serialized" and "parsed" axis list: the field written
into the run's audit record and the field the enumerator walks are the same
one, so a recorded config can never describe a different grid from the one that
actually produced the candidates.

`DiscoveryAxis.key` is a closed `AxisKey` enum whose only constructor from text
is `AxisKey::parse`, so an axis naming a non-whitelisted key is
unrepresentable. (A `&'static str` would NOT give this guarantee — `'static`
only promises the text outlives the program, and every string literal
qualifies.) The string whitelist is derived from `AxisKey::ALL`, so the two
cannot drift.

Multiple bases are allowed: different signal families are different bases, each
with its own grid. Signal ids themselves are never an axis.

## 3. `discovery-enumeration-v1`

Enumeration is the run's LINEAGE. It fixes which hypotheses exist, Score's
data-mining `N`, and each candidate's stable identity, so it must depend only
on config content — never on map iteration, thread scheduling, completion
order, or a row id.

1. **Preflight the raw product.** `raw` is the sum over bases of the product of
   their axis value counts, computed with a safe-integer guard. If
   `raw > caps.candidates` the run fails **before any candidate is built**, so
   no jobs can exist for an over-budget run.
2. **Expand.** Per base, a row-major odometer over the declared axes (the LAST
   axis varies fastest) patches values onto a clone of the base preset. The
   iteration order does not affect the plan — candidates are hash-sorted — but
   it is fixed so generated fixtures stay reproducible.
3. **Prune** with the cross-field validity rules, in this fixed order:
   `fastMA<slowMA`, `macdFast<macdSlow`, `rsiBuy<rsiSell`. A grid axis is
   independent by construction, so a legal Cartesian product inevitably
   contains combinations that are not valid hypotheses; these are counted as
   `prunedInvalid`, not rejected. The rules apply regardless of which signal a
   base selects, so the pruned count stays a property of the grid alone.
4. **Deduplicate** by canonical `strategy-v2` hash across ALL bases. The first
   occurrence (config order) keeps provenance in `baseId`; later repeats count
   as `duplicates`. Every candidate holds its own deep copy of the strategy —
   sharing one `entryRules` array would let a mutation on a single candidate
   change the content of every other candidate while their already-computed
   hashes and seeds stayed put.
5. **Sort** survivors by strategy hash (byte order) and assign
   `index = 0..n-1` **afterwards**, so input order can never change a
   candidate's identity or index.
6. **Seed** each candidate (see §4).

Fail-closed: nothing surviving pruning is an error
(`enumeration produced no valid candidates`), and
`prunedInvalid + duplicates + finalUnique != raw` is treated as a defect rather
than persisted, because the four counters are an audit record.

`testedCombinations` is `{ n: finalUnique, basis: 'lineage-final-unique' }`.
Per the SCORE-001 Resolution the runner derives `N` here and shares it across
the whole lineage; callers may never supply it.

## 4. `seed-v1` sub-seed derivation

The Resolution forbids deriving seeds from a SQLite run id, a thread id,
enumeration order, or completion order: none of those survive a re-import, a
resume, or a differently scheduled run. A run stores ONE explicit `rootSeed`
`u32`; every per-candidate stream is derived from durable inputs only.

Preimage (identical bytes in both languages):

```
"seed-v1" 0x00
u32be(rootSeed)
u32be(len) utf8(datasetContentHash)
u32be(len) utf8(strategyHash)
u32be(len) utf8(purpose)
```

The seed is `SHA-256(preimage)[0..4]` read big-endian, directly usable as a
`mulberry32` seed. Length-prefixed strings keep the concatenation unambiguous,
so no two different inputs can share a preimage.

Both identity arguments must be a complete `<version>:<64 lowercase hex>`
value. A bare prefix, a truncated digest, uppercase hex, or non-hex characters
fail closed rather than producing a seed from a malformed identity.

`purpose` is whitelisted; v1 defines only `random-entry`. An unknown purpose
fails closed so a typo cannot silently create a second unreviewed stream.

## 5. Cross-language parity

`src/parity/runnerConfigFixture.ts` + `npm run fixtures:runner-config` own the
committed `runner-config-parity-v1` envelope
(`fixtures/rs-core/runner-config-v1.json`).

Unlike the engine fixtures, `expectedNumericPolicy` is **`exact-v1`**: every
expected leaf compares exactly. This slice produces identifiers, integers,
counters, indexes, and axis values that both languages derive with identical
IEEE-754 operations, so no tolerance is admissible.

`exact-v1` also fixes the sign of zero: no leaf may be negative zero. IEEE-754
defines `-0.0 == 0.0`, so a comparison alone would silently accept a sign flip
and "exact" would be weaker than it reads. Both languages assert the invariant
directly instead of relying on the comparison, and both carry negative-control
tests proving the guard actually fails on `-0`, `NaN`, and `±Infinity`.

The TypeScript guard must run on the LIVE builder output, never on the parsed
artifact: `JSON.stringify(-0)` is `"0"`, so a negative zero is erased the
moment the fixture is written and an assertion on the file could never observe
one. On the Rust side every numeric comparison — including axis values —
routes through one shared helper, so no call site can fall back to a bare `==`.

Error cases are HELD by the TypeScript reference: generation and the vitest
freshness test both execute the real functions and require a `RangeError`
carrying the recorded fragment, so the fixture can never claim a rejection the
reference does not actually perform. Rust must reject the same input with a
message containing the same fragment.

70 rejections are held in total (10 seed + 1 axis + 4 concurrency + 53
admission + 2 enumeration). Both languages assert the **exact ordered ID
inventory** of the admission group and the 70 total, so a deleted case fails a
test instead of being masked by a replacement that preserves the count, and the
totals quoted in prose are derived from the artifact rather than hand-carried.

## 6. Deliberately out of scope

- Dataset id/content-hash revalidation at run start, run/job rows, migrations,
  atomic candidate commit — RUNNER-STORE-001.
- Worker pool, pause/resume/cancel, single-writer serialization, versioned
  events — RUNNER-EXEC-001.
- Frontend wrappers and progress UI — RUNNER-UI-001.
- Cross-run result reuse: current summaries are mutable UPSERT views, so reuse
  needs a separate versioned immutable execution cache (Resolution D5).

## 7. Known follow-up

`discovery_core/identity.rs` and the binary crate's `identity` module hold two
copies of the same canonical `strategy-v2` encoder. The split exists because
discovery core must not depend on Tauri/rusqlite while the write boundary
needs `AppError` and the repository row types. Both are locked to the same
committed `src/core/hashing/identity-v2.fixture.json`, so a divergence fails a
test. Consolidating them behind the pure module touches the product write
boundary and is proposed as its own change, not smuggled into this slice.
