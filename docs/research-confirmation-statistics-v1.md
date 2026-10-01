# Research confirmation statistics (`research-confirmation-statistics-v1`)

> **Status: P12e-1, pure calculation only (2026-10-01).** No runtime caller.
> It computes p-values and Holm adjustments; it never produces a confirmation
> `PASS`, reserves alpha, or reads Validation/Test data — P12e-2/-3 and P13 own
> those. Implementation: `alpha-factor-forge/src-tauri/src/discovery_core/confirmation.rs`.
> Upstream: [`plans/active-plan.md`](plans/active-plan.md) §4.5;
> [`research-precision-v1.md`](research-precision-v1.md) (P12a, the planning
> bound this calculation realizes); [`trial-ledger-v1.md`](trial-ledger-v1.md)
> §22 (two tests per trial).

## 0. Maintainer decisions (2026-10-01)

1. **Two tests per trial**, reported separately: mean net return above zero,
   and mean excess over buy-and-hold above zero. They are not merged into one
   p-value (AlphaBTC took the larger of the two; that hides which claim holds).
2. **Circular block bootstrap** with one fixed block length.
3. **The block length is a declared integer**, with a rationale, frozen before
   any confirmation result is read. Nothing estimates it from the data.

## 1. Question it answers

Given per-bar returns of the candidates in one confirmation batch, a frozen
bootstrap declaration, and the trial family's total test count `m`: what is
each test's one-sided p-value, and which null hypotheses does Holm reject at
the allocated alpha when **every** test the family has ever registered counts?

It does not decide whether a strategy is confirmed. A rejected null is a
statistical statement about one test under this protocol.

## 2. Declaration (strict JSON)

All fields are required; unknown fields are rejected; integers are JSON
integers (a float literal such as `1000.0` is rejected) no larger than
`9007199254740991`.

| Field | Domain | Meaning |
| --- | --- | --- |
| `contractVersion` | `"research-confirmation-statistics-v1"` | exact |
| `correction` | `"holm"` | only correction in v1 |
| `scheme` | `"circular-block"` | only resampling scheme in v1 |
| `prng` | `"splitmix64"` | §4 |
| `alphaPpm` | integer `[1, 999999]` | family-wise alpha allocated to **this** confirmation (P12e-2 will define the allocation) |
| `blockLength` | integer `[1, MAX]` | bars per block, `L` |
| `bootstrapSamples` | integer `[1, MAX]` | sample count `B` |
| `seed` | integer `[0, MAX]` | §4 |
| `blockLengthRationale` | non-empty, trimmed text, at most 1024 UTF-8 bytes | why this `L` was chosen, written before results exist |

Rejection order (part of the contract): not an object → first unknown field in
sorted order → the fields in table order. No field has a default.

The rationale's presence does not prove the choice is statistically adequate.

## 3. Inputs at evaluation

- The declaration (re-checked against §2's domain even when constructed
  directly).
- `familyTests` — `m`, the ledger's `familyEffectiveTrials × testsPerTrial`
  from the current admission count. It must be a multiple of 2, at least
  `2 × candidates`, and at most `MAX`. **It is the whole family, not this
  batch**: trials tested or merely registered earlier still tighten every
  threshold. The pure function cannot verify where the number came from; P13
  must pass the fenced ledger count.
- One series per candidate: a unique `candidateIndex`, `returns` (per-bar net
  strategy return after costs) and `benchmarkReturns` (buy-and-hold return of
  the same bars), equal length `n ≥ 2`, all finite.

**Block length rule.** `blockLength² ≤ n` for every series, otherwise the call
fails. A block as long as the series only rotates it: every resample then has
the observed sum, the centered statistic never reaches it, and any positive
mean would get the smallest possible p-value. Requiring `L ≤ √n` keeps at
least `√n` blocks per resample; it is the usual upper end of block-length
growth, not a tuned threshold. `L = 1` is the ordinary i.i.d. bootstrap.

A non-finite return, excess (`returns − benchmarkReturns`) or sum fails the
call rather than being skipped.

## 4. Resampling

SplitMix64 (Steele, Lea & Flood; Vigna's reference constants), all arithmetic
modulo 2^64:

```text
mix(z):  z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9
         z = (z ^ (z >> 27)) * 0x94D049BB133111EB
         return z ^ (z >> 31)
next():  state = state + 0x9E3779B97F4A7C15;  return mix(state)
stream(seed, candidateIndex):  state = mix(seed ^ mix(candidateIndex + 1))
below(n): t = 2^64 mod n;  repeat x = next() until x >= t;  return x mod n
```

Each candidate has its own stream keyed by its index, so its resamples do not
depend on which other candidates share the batch or on their order. `below`
discards the biased prefix, so every index is equally likely.

For one candidate with `n` bars, `excess[i] = returns[i] − benchmarkReturns[i]`,
`blocks = ceil(n / L)`; for each of the `B` samples:

```text
repeat blocks times:  start = below(n)
                      take the next min(L, bars still needed) bars
                      start, start+1, … wrapping modulo n
```

The **same** resampled bars feed both tests of that candidate.

## 5. P-values

Sums are plain left-to-right `f64` additions in the order the bars are taken
(observed: bar order). For each test with observed sum `S` and resampled sum
`S*`, a sample is *extreme* when `S* ≥ 2·S` — the centered statistic
`mean* − mean` reaching the observed `mean`, the bootstrap null for
`H0: mean ≤ 0` against `H1: mean > 0`.

```text
rawP = (1 + extremeCount) / (B + 1)
```

the estimator P12a's precision precheck assumes; its smallest value is
`1 / (B + 1)`.

## 6. Holm against the whole family

Sort this batch's tests by `(extremeCount, candidateIndex, test)` with
`net_return` before `benchmark_excess`; rank `j` starts at 1. With `m =
familyTests`, in exact integers over the shared denominator `B + 1`:

```text
adjusted_j = max(adjusted_{j−1}, min(B + 1, (m − j + 1) × (1 + extremeCount_j)))
rejectsNull = adjusted_j × 1e6 ≤ alphaPpm × (B + 1)
```

The running maximum gives tied p-values the same adjusted value. Using `m`
rather than the batch size is conservative and matches P12a: with
`extremeCount = 0` at rank 1 the adjusted value is P12a's `bestAdjustedP`
(tested). Growing the family never lowers an adjusted p-value (tested).

## 7. Report

`evaluate_confirmation` returns (camelCase JSON): `contractVersion`,
`correction`, `scheme`, `prng`, `alphaPpm`, `blockLength`, `bootstrapSamples`,
`seed`, `familyTests`, and `tests` ordered by candidate index then test. Each
test has `candidateIndex`, `test` (`net_return` / `benchmark_excess`),
`observations`, `observedMean`, `extremeCount`, `rawP` and `adjustedP` as exact
unreduced `{numerator, denominator}`, `rawPValue` / `adjustedPValue` (one IEEE
division), `holmRank`, and `rejectsNull`.

There is no status, verdict or `PASS` field.

## 8. Fixture and reference

[`research-confirmation-statistics-v1.json`](../alpha-factor-forge/fixtures/rs-core/research-confirmation-statistics-v1.json)
holds hand-written inputs. Its `expected` blocks and PRNG vectors are produced
by an independent TypeScript/BigInt reference written from this document
(`src/parity/confirmationStatisticsFixture.ts`,
`npm run fixtures:confirmation-statistics`), not by the Rust module. A Vitest
fails if the committed fixture stops matching the reference; the Rust tests
must reproduce the fixture bit for bit, including the `f64` means. The PRNG
vectors are also the published SplitMix64 outputs for states 0 and 1234567.

Cases: a four-bar series; two candidates in a family of eight; steady positive
returns that reject at `m = 2` but not at `m = 2000` (same data, same
p-values — only the family differs); and a negative-mean series.

## 9. Limits

- Bootstrap validity depends on the declared block length and on the series
  being roughly stationary; this contract enforces only the `L² ≤ n` floor on
  the number of blocks. P12e-3's noise simulation is the check on false
  positives for a declared protocol.
- The buy-and-hold series is an input; this module does not derive it, and
  "excess over buy-and-hold" says nothing about other benchmarks.
- The calculation is deterministic for a given declaration and data. Choosing
  a seed after seeing results would defeat it, which is why the declaration is
  frozen first (P13 enforces the order).
- Rust only. A TypeScript reader would have to reproduce the `f64` summation
  order; the reference in `src/parity` is test support, not a product path.

## 10. Remaining P12/P13 work

P12e-2 (cross-batch alpha allocation that supplies `alphaPpm`), P12e-3
(seeded noise-data false-positive simulation of this protocol), and P13
(freezing a confirmation batch, the synchronized ledger fence that supplies
`familyTests`, reserving alpha, revealing Validation/Test once, and the only
place a statistical `PASS` may be produced).
