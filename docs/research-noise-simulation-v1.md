# Research noise simulation (`research-noise-simulation-v1`)

> **Status: P12e-3, pure calculation plus two declared acceptance runs
> (2026-10-02). Result: independent noise within tolerance; serially
> correlated noise exceeds it (§7.1) — an open finding against P12e-1.**
> No runtime caller. It measures how often the declared
> confirmation protocol rejects a true null hypothesis on seeded noise; it is
> evidence about the test procedure on synthetic data, never a confirmation
> `PASS`, and it says nothing about any strategy. Implementation:
> `alpha-factor-forge/src-tauri/src/discovery_core/noise_simulation.rs`.
> Upstream: [`plans/active-plan.md`](plans/active-plan.md) §4.5 ("對純噪音資料
> 做偽陽性模擬，保存檢定實作與隨機種子");
> [`research-confirmation-statistics-v1.md`](research-confirmation-statistics-v1.md)
> (P12e-1) and [`research-alpha-allocation-v1.md`](research-alpha-allocation-v1.md)
> (P12e-2), the two contracts it exercises unchanged.

## 0. Maintainer decisions (2026-10-02)

1. **Noise to cover:** independent noise, and moderately serially correlated
   noise (AR(1) coefficient 0.3) — the case where a block bootstrap is most
   likely to be miscalibrated.
2. **Acceptance:** the observed family false-positive rate may exceed the
   nominal alpha by at most **one percentage point**, measured over **2,000**
   simulations (sampling error about half a percentage point).

Both are frozen in the declarations of §7 before any run. A threshold or a
seed is never changed because a result is unwelcome.

After the result (§7.1) and the PR #135 acceptance review, the same day:

3. **The 6.45% failure stays on record.** P12e stays open and P13's
   confirmation work stays blocked.
4. **Direction for P12e-FINDING-1: revise the statistic.** First a new
   calibration plan is submitted; then a studentized bootstrap and a
   serial-correlation variance correction are compared as candidates for a
   new P12e-1 contract version. A longer sample may become a condition of
   applicability once verified. Lowering alpha is not, on this evidence, a way
   to close the finding.
5. **The new acceptance checks each confirmation and the whole family**, so
   that a conservative later batch cannot hide a first batch that is too
   loose.
6. **These simulations stay in the normal test suite** (they take about 8 s
   in CI).

## 1. Question it answers

If nothing has an edge — every candidate's mean net return and mean excess
over its benchmark are exactly zero — how often does a trial family's whole
confirmation life, run exactly as P12e-1 and P12e-2 specify, reject at least
one null hypothesis? And is that within the declared tolerance of the alpha
the family's schedule hands out?

It does not test power (whether a real edge would be found), does not choose
a block length, and does not qualify data or a strategy.

## 2. Declaration (strict JSON)

All fields are required; unknown fields are rejected; integers are JSON
integers (a float literal such as `2000.0` is rejected).

| Field | Domain | Meaning |
| --- | --- | --- |
| `contractVersion` | `"research-noise-simulation-v1"` | exact |
| `confirmationContract` | `"research-confirmation-statistics-v1"` | the statistics under test |
| `noiseModel` | `"ar1-uniform-sum"` | only model in v1 (§3) |
| `autocorrelationPpm` | integer `[0, 900000]` | AR(1) coefficient φ in ppm; `0` is independent noise. The upper bound is what the fixed warm-up supports (§3) |
| `bars` | integer `[2, 1000000]` | bars of each candidate's confirmation segment, `n` |
| `candidatesPerConfirmation` | integer `[1, 1024]` | candidates in every confirmation, `c` |
| `priorTrials` | integer `[0, MAX]` | trials the family registered before its first confirmation and never confirms; they only enlarge the family count |
| `blockLength` | integer `[1, MAX]` | bootstrap block length `L`; `L² ≤ bars` |
| `bootstrapSamples` | integer `[1, MAX]` | bootstrap samples `B` per confirmation |
| `simulations` | integer `[1, 1000000]` | independent family lives, `N` |
| `seed` | integer `[0, MAX]` | §3 |
| `tolerancePpm` | integer `[0, 999999]` | allowed excess of the observed family rate over the nominal alpha |
| `allocation` | a [`research-alpha-allocation-v1`](research-alpha-allocation-v1.md) declaration | the family's budget; **every** scheduled confirmation is simulated |

`MAX` is `9007199254740991`. Rejection order (part of the contract): not an
object → first unknown field in sorted order → the fields in table order
(the nested allocation by its own contract's order, prefixed `simulation.`) →
`blockLength² > bars` → the family test count of the last confirmation above
`MAX`.

## 3. Noise

SplitMix64 as in P12e-1 §4 (`mix`, `next`). A stream is keyed by a list of
integers, each folded in the way P12e-1 keys a candidate's stream:

```text
stream(seed, parts):  state = seed
                      for p in parts:  state = mix(state ^ mix(p + 1))
                      SplitMix64 starting at state
uniform():            (next() >> 11) × 2^-53            exact, in [0, 1)
innovation():         ((u1 + u2) + u3) + u4 − 2         four uniforms, in order
series(φ, n):         x = 0
                      repeat 64 + n times:  x = φ × x + innovation()
                      keep the last n values
```

`φ = autocorrelationPpm / 1e6` (one IEEE division). Innovations are symmetric
about zero, so every series has mean exactly zero in expectation:
light-tailed, homoscedastic noise (§9).

**Warm-up and the supported coefficient** (PR #135 review R1). A series
starts at zero, so after `t` innovations it has `1 − φ^(2t)` of the
stationary variance; the first kept bar is the 65th value, short by `φ^130`.
The warm-up is a fixed 64 bars — changing it would change the declared
acceptance runs — so the coefficient is bounded instead:

| φ | Variance of the first kept bar, of stationary |
| --- | --- |
| 0 | 100% exactly |
| 0.3 (the acceptance run) | short by about `1e-68` |
| **0.9, the maximum** | short by about `1.1e-6` |
| 0.99 (refused) | 72.9% |
| 0.999999 (refused) | 0.013% |

Within `[0, 0.9]` the series is a stationary AR(1) to about one part per
million, which is far below what any simulation of this size resolves. A
larger coefficient is refused rather than simulated from a start that is not
stationary; supporting one needs a new contract version with a different
initialisation. The bias decays over the kept bars, so it is largest in the
first one.

For simulation `s` (from 0), confirmation `k` (from 1), candidate `j` (from 0):

```text
returns           = series from stream(seed, [0, s, k, j, 0])
benchmarkReturns  = series from stream(seed, [0, s, k, j, 1])
confirmation seed = first output of stream(seed, [1, s, k]) >> 11
```

Returns and benchmark are independent, so both of P12e-1's nulls (mean net
return ≤ 0, mean excess ≤ 0) are true and sit on their boundary — the least
favourable case for a one-sided test.

## 4. One simulated family life

For each scheduled confirmation `k = 1 … K` (`K` = schedule length), in order:

1. `alphaPpm` comes from `allocate_confirmation_alpha` (P12e-2) given the
   shares the simulated family has already reserved.
2. `familyTests = 2 × (priorTrials + k × c)` — the family only grows.
3. Candidate `j` has `candidateIndex = (k − 1) × c + j`.
4. `evaluate_confirmation` (P12e-1) runs with that alpha, the declared block
   length and sample count, the confirmation seed of §3 and `familyTests`.

Both production functions are called unchanged; the simulation adds no
statistical code of its own.

## 5. Counting and the decision

- A confirmation is a **false positive** in a simulation when it rejects at
  least one null (any candidate, either test).
- A simulation is a **family false positive** when any of its confirmations
  is. This is the rate the alpha budget bounds; averaging over candidates or
  looking at one confirmation would miss the accumulation across batches.
- `nominalAlphaPpm` is the sum of the schedule — the alpha actually handed
  out (equal to `totalAlphaPpm` when the schedule uses the whole budget).

```text
WITHIN_TOLERANCE  iff  familyFalsePositives × 1e6 ≤ N × (nominalAlphaPpm + tolerancePpm)
```

in exact integers; otherwise `EXCEEDS_TOLERANCE`. There is no other status
and no `PASS`.

## 6. Report

camelCase JSON: `contractVersion`, `confirmationContract`, `allocationId`,
`status`, `simulations`, `nominalAlphaPpm`, `tolerancePpm`,
`limitFalsePositives` (the largest count still within tolerance),
`familyFalsePositives`, `familyFalsePositiveRate` (exact
`{numerator, denominator}`), `familyFalsePositiveRatePpm` (floored),
`nominalStandardErrorPpm`, and `confirmations` — one entry per scheduled
confirmation with `confirmationNumber`, `alphaPpm`, `familyTests`,
`falsePositives`, `falsePositiveRatePpm`, `netReturnRejections` and
`benchmarkExcessRejections` (rejected nulls over all simulations, by test).

**Sampling uncertainty.** `nominalStandardErrorPpm =
floor(sqrt(a × (1e6 − a) / N))` with `a = nominalAlphaPpm`: the standard error
of a rate estimated from `N` simulations if the true rate were exactly
nominal. A tolerance of about two of these means a correctly sized procedure
is very rarely flagged; a tolerance far below one makes the verdict mostly
luck. The contract reports the number and does not enforce a ratio.

## 7. Declared acceptance runs

Two declarations, identical except for `autocorrelationPpm` (`0` and
`300000`), stored under `acceptance` in the fixture:

| Field | Value | Why (decided before any run) |
| --- | --- | --- |
| `simulations` | 2000 | decision 2 |
| `tolerancePpm` | 10000 | decision 2 |
| `allocation` | total 50000, schedule `[25000, 25000]` | a whole 5% budget spent over two confirmations, so the cross-batch accumulation is exercised; nominal = 50000 |
| `candidatesPerConfirmation`, `priorTrials` | 1, 0 | the smallest family: Holm gets no slack from family members that are never tested, so this is the least conservative, most demanding setting |
| `bars` | 256 | a short but plausible confirmation segment that keeps the run inside the test suite |
| `blockLength` | 6 | `round(256^(1/3))`, the usual cube-root growth rule; within `L² ≤ n` |
| `bootstrapSamples` | 799 | every Holm threshold of both confirmations (`m = 2`, `4`) is a multiple of `1/800`, so no threshold is rounded down by the p-value grid |
| `seed` | 20261002 | the date of the declaration |

With these, `nominalStandardErrorPpm = 4873` and
`limitFalsePositives = 120` (6% of 2,000).

The declarations were committed (`a0cd10a`) before the simulation was run at
this size; the size was chosen from a timing measurement that did not print
outcomes. The results below are reproduced by `cargo test` on every run.

### 7.1 Results (run once, 2026-10-02)

| Run | Family false positives | Rate | Limit | Status | Confirmation 1 (share 2.5%) | Confirmation 2 (share 2.5%) |
| --- | --- | --- | --- | --- | --- | --- |
| `independent-noise` | 97 / 2000 | 4.85% | 120 | `WITHIN_TOLERANCE` | 61 (3.05%) | 38 (1.90%) |
| `serially-correlated-noise` | 129 / 2000 | 6.45% | 120 | **`EXCEEDS_TOLERANCE`** | 83 (4.15%) | 49 (2.45%) |

- **Independent noise is within tolerance**, and within one standard error
  (0.49 points) of the nominal 5%. Its first confirmation alone rejected in
  3.05% of simulations against a 2.5% share — about 1.6 of that rate's own
  standard errors (0.35 points) above it; the family total stayed near
  nominal because the second confirmation, adjusted for a family of four
  with only two tests run, stayed below its share.
- **Moderately correlated noise exceeds the tolerance**: 6.45% is 1.45 points
  (about three standard errors) above nominal and nine simulations above the
  limit. Its first confirmation rejected in 4.15% against a 2.5% share.

**What this means.** The declared acceptance is **not met** for the
correlated run: 129 is above the limit of 120 that was fixed beforehand.
Nothing was adjusted afterwards: the seed, the size, the block length and the
tolerance are the ones committed beforehand, and this result stands as the
record (decision 3). Until the finding is resolved, a confirmation under
P12e-1 must not be described as controlling its false-positive rate on
serially correlated returns.

**How far the numbers go** (95% Wilson intervals, each computed on its own
row, no multiplicity adjustment; from the PR #135 acceptance review):

| Measurement | Rate | 95% interval |
| --- | --- | --- |
| independent, family 97/2000 | 4.85% | 3.99% – 5.88% |
| AR(1) 0.3, family 129/2000 | 6.45% | 5.45% – 7.61% |
| independent, confirmation 1, 61/2000 | 3.05% | 2.38% – 3.90% |
| AR(1) 0.3, confirmation 1, 83/2000 | 4.15% | 3.36% – 5.12% |

- The correlated family interval lies above the nominal 5%, which supports
  the concern that the test is miscalibrated there. It also **contains 6%**:
  the run fails the declared threshold, but it does not prove that the true
  rate is above 6%.
- The independent first-confirmation interval contains its 2.5% share, so 61
  rejections alone do not show the test is miscalibrated on independent
  noise.
- These intervals are not a new acceptance rule for this run. A later plan
  may declare a confidence-bound rule in advance.

## 8. Fixture and reference

[`research-noise-simulation-v1.json`](../alpha-factor-forge/fixtures/rs-core/research-noise-simulation-v1.json):

- `cases` — small hand-written declarations. Their `expected` reports come
  from an independent TypeScript/BigInt reference written from this document
  (`src/parity/noiseSimulationFixture.ts`,
  `npm run fixtures:noise-simulation`), not from the Rust module; a Vitest
  fails if the fixture stops matching, and the Rust tests must reproduce it
  exactly, including the generated noise bit for bit.
- `acceptance` — the two declarations of §7. At 2,000 simulations the BigInt
  reference is too slow to run, so their `expected` reports are produced by
  the Rust module (`cargo test --lib noise_simulation::tests::print_acceptance_reports -- --ignored --nocapture`)
  and re-computed by a Rust test on every run. The Vitest still re-derives
  every derived field (`status`, limit, rates, standard error,
  `allocationId`) from the committed counts.

## 9. Limits

- **The noise is easy noise.** Innovations are light-tailed, symmetric and
  homoscedastic. Real returns have fat tails and volatility clustering; a
  bootstrap can be worse calibrated on them than this simulation shows. A
  result within tolerance here is necessary, not sufficient.
- **One configuration is one data point.** The outcome depends on `bars`,
  `blockLength`, `bootstrapSamples`, the family size and the strength of the
  autocorrelation. The acceptance runs say nothing about a confirmation
  declared with other values; P13 should run this simulation with the values
  of the confirmation it is about to execute.
- The family rate is bounded by the alpha budget only if each confirmation's
  p-values are valid. Allocation arithmetic and Holm do not make a block
  bootstrap valid; that is exactly what the correlated run probes.
- Size only, not power: a procedure that never rejects would be "within
  tolerance".
- A report is one draw: with 2,000 simulations the measured rate carries a
  standard error of roughly half a percentage point.
- Rust only. The reference in `src/parity` is test support, not a product
  path.

## 10. Remaining P12/P13 work

**Open finding (P12e-FINDING-1).** The serially correlated acceptance run
exceeds its tolerance (§7.1). The maintainer's direction (§0, decisions 3–5):
keep this record; submit a **new calibration plan** first; then compare a
studentized bootstrap and a serial-correlation variance correction as
candidates for a new P12e-1 contract version; treat a longer sample as a
condition of applicability only after it is verified; do not close the
finding by lowering alpha. P12e stays open until then.

The acceptance review's minimum requirements for that plan — to be settled
when the plan itself is submitted — are: declare, before anything is run, the
per-confirmation and whole-family targets, models, sample lengths, family and
schedule, block rule, sample counts, Monte Carlo size, seeds and acceptance
rule; use separate fixed seed sets for choosing a method and for the final
acceptance; synthetic data only; keep failures on record. The exploratory
block-length runs in the P12e-3 handoff are a hypothesis, not evidence that
the block choice is ruled out.

P13: freezing a confirmation batch, the synchronized ledger fence, alpha
reservation (alpha-allocation §9), revealing Validation/Test once, and the
only place a statistical `PASS` may be produced. P13 should also decide
whether a confirmation requires a `WITHIN_TOLERANCE` simulation of its own
declared values before it may run.
