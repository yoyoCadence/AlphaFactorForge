# Research confirmation statistics, version 2 (`research-confirmation-statistics-v2`)

> **Status: P12e-7b, FROZEN — ACCEPTED FOR THE DECLARED TESTED SET (2026-10-04).** The method
> that [`confirmation-recalibration-plan-v1`](plans/confirmation-recalibration-plan-v1.md)
> §6 selected in P12e-6b — the draft candidate S2 with block rule R3 — frozen
> here before its final acceptance (plan §7, P12e-7b). The single final run on
> seed 20261117 passed all three checks in all six cells. Support is limited to
> **256, 512 and 1024 bars**, coefficients **0 and 0.3**, and the exact generator,
> family shape and bootstrap count in §6. The accepted limits are 3% per
> confirmation and 6% per family, not proof of exact nominal-alpha control.
> No runtime caller: only the simulation engine
> ([`research-noise-simulation-v2`](research-noise-simulation-v2.md)) selects
> it, and P13 decides how a confirmation consumes it.
> Implementation: `alpha-factor-forge/src-tauri/src/discovery_core/confirmation_v2.rs`,
> whose computation is the draft S2's own code path
> (`confirmation_candidates::bootstrap_s2`).

## 0. Decisions

1. **Selection (plan §6, P12e-6b, 2026-10-03):** S2 with block rule R3. The
   rule ranked it first among three pairs whose power counts differed by 1
   and 6 of 4,000; it is the declared rule's pick, not evidence that it has
   more power.
2. **Block length (maintainer, 2026-10-03):** still a declared integer with a
   rationale, frozen before results (v1 decision 3), and it must equal R3 of
   every series' bar count (§3). Any other length is refused: the acceptance
   tests R3 only.
3. **Order (maintainer, 2026-10-03):** this contract and the acceptance
   declarations are reviewed and merged (P12e-7a) before the acceptance seed
   is used (P12e-7b).

## 1. What it is

[`research-confirmation-statistics-v1`](research-confirmation-statistics-v1.md)
with two changes: the block rule of §3 and the extreme rule of §4. Everything
else is v1's: the declaration fields (§2), the inputs and their checks, the
SplitMix64 streams and block draws (v1 §4), `rawP = (1 + extremeCount) /
(B + 1)`, Holm against the whole family (v1 §6), and the report (v1 §7) with
`contractVersion` `research-confirmation-statistics-v2`. There is no status,
verdict or `PASS` field.

## 2. Declaration (strict JSON)

v1 §2 unchanged — the same nine fields, domains and rejection order — with
`contractVersion` `"research-confirmation-statistics-v2"`. A v1 declaration
is refused here and a v2 declaration by v1 (tested).

## 3. Inputs and block rule R3

Inputs as v1 §3. For each candidate, after v1's own checks (as many benchmark
returns as returns; at least two bars):

```text
R3(n) = round(n^(1/3)) = the smallest integer L ≥ 1 with (2L + 1)³ > 8n
```

in exact integers (an odd cube never equals `8n`, so there is no tie).
`blockLength` must equal `R3(n)` for **every** candidate of the batch —
candidates whose lengths have different R3 cannot share one — and then S2's
own rule `(2L)² ≤ n` applies.

| `n` | 256 | 512 | 1024 |
| --- | --- | --- | --- |
| `R3(n)` | 6 | 8 | 10 |

Consequence (tested): v2 accepts `n` in 36–42, in 64–91 and every `n ≥ 100`,
and refuses every other length. Tested directly up to 400 bars; beyond, for
every `L ≥ 6` the first `n` with `R3(n) = L` exceeds `(L − ½)³ ≥ 4L²`. Being
accepted says nothing about being supported (§6).

Rejection order for a candidate: unequal lengths → fewer than two bars →
`blockLength ≠ R3(n)` → `(2L)² > n` → a non-finite return, excess or sum →
an intermediate outside the range of §4. The declaration's domain, empty or
repeated candidates and `familyTests` are checked first, as in v1.

## 4. When a resample is extreme

For one test the series is `x_0 … x_{n−1}` (net returns, or
`returns − benchmarkReturns`).

**Constant series.** If every bar equals the first (IEEE `==`), nothing
below is computed for that test: every resample is extreme when the value is
`≤ 0` (`rawP = 1`), and none is when it is `> 0` (`rawP = 1/(B + 1)`).

**Normalization.** Otherwise the series is multiplied by `2^−e`,
`e = floor(log2 max|x_t|)` read from the bits of the largest magnitude, so
that magnitude lies in `[1, 2)`; a power of two scales every sum, difference
and product below exactly. `S` is the left-to-right sum of the normalized
series; the reported `observedMean` is the sum of the series as given over
`n`.

**Range.** Every product and quotient below must be a finite normal double
whenever its operands are non-zero; otherwise the call is **refused** with an
error, never rounded to infinity or zero.

**Block variance over all circular blocks**, for a block length `ℓ`:

```text
C_t(ℓ) = x_t + x_{t+1} + … + x_{t+ℓ−1}       indices modulo n, summed in that order from 0
B(ℓ)   = ( Σ_{t=0}^{n−1} ( C_t(ℓ) − ℓ·(S / n) )² ) / (n·ℓ)
```

`S / n` is one division, `ℓ·(S / n)` one multiplication, the squares are
accumulated for `t = 0, 1, …` from 0, and the total is divided once by the
integer `n·ℓ`.

**The stretch.** With `L = blockLength`:

```text
F      = 2·B(2L) − B(L)                     the flat-top lag window
(p, q) = (F, B(L))   if B(L) > 0 and F > B(L)
         (1, 1)      otherwise
```

A resample with sum `S*` (accumulated in draw order, as v1) is **extreme**
when `c·(S* − S) ≥ S` with `c = √(p / q) ≥ 1`, decided without a square
root or a division as `a·√p ≥ s·√q` with `a = S* − S`, `s = S`:

```text
left  = (a·a)·p        right = (s·s)·q
a ≥ 0 and s ≥ 0 :  left ≥ right
a ≥ 0 and s < 0 :  true
a < 0 and s ≥ 0 :  left = 0 and right = 0
a < 0 and s < 0 :  left ≤ right
```

in IEEE-754 double arithmetic in exactly this order, the range rule applying
to `a·a`, `(a·a)·p`, `s·s` and `(s·s)·q`. No square root, division or
transcendental function enters a decision.

This is the draft's §2 and §4 unchanged
([`research-confirmation-candidates-draft-v1`](research-confirmation-candidates-draft-v1.md));
the derivation, the edge cases and the partial-block approximation are
explained there.

## 5. Fixture and reference

[`research-confirmation-statistics-v2.json`](../alpha-factor-forge/fixtures/rs-core/research-confirmation-statistics-v2.json)
holds six fixed cases of 64 to 125 bars (two candidates; a partial last
block; a smoothed series that is stretched; a negative mean; a small positive
mean rejected with two extreme resamples; a constant positive series). Its
`expected` blocks come from an independent TypeScript reference
(`src/parity/confirmationStatisticsV2Fixture.ts`,
`npm run fixtures:confirmation-statistics-v2`): the draft S2 reference, with
R3 found by scanning odd cubes in BigInt where Rust bisects. Rust reproduces
every decision field exactly and the three display floats to one unit in the
last place (NUMERIC-JSON-001). Tests also show that v2 reports exactly what
the draft S2 reports on every case, and that under the simulation engine it
reproduces the committed checkpoint counts of the six S2-R3 diagnostic runs.

## 6. Status and limits

- **Accepted for the tested set.** P12e-7b ran plan §7 once (seed 20261117,
  20,000 simulations per cell). All 18 checks passed; the lowest supported
  tested length is 256 and the exact supported lengths are {256, 512, 1024}.
  The [plan result](plans/confirmation-recalibration-plan-v1.md#record--2026-10-04-p12e-7b-final-acceptance-not-an-amendment)
  records every count, estimate and Wilson interval. Reports and their
  predeclared checkpoints are committed; only checkpoints join normal tests.
- **Scope:** the `ar1-uniform-sum` generator at coefficients 0 and 0.3, two
  confirmations of one candidate with no prior trials, 799 bootstrap samples,
  and this statistic. Not other lengths, coefficients or distributions, and
  not other family shapes (plan §7, §11).
- **Diagnostics, not acceptance.** On 4,000 simulations of seed 20261005,
  S2-R3 passed the point screen in five cells and failed it at 256 bars with
  coefficient 0.3 (127 rejecting simulations on the first confirmation
  against 120). That is a selection record, not a result of §7.
- `B(L)` is only approximately the variance the block bootstrap reproduces
  when the last block is partial; the draft states the exact form.
- **Frozen.** A change to the computation — the shared S2 code path, the
  block rule or the comparison — is a new contract version and requires
  separately declared calibration.
- Rust only; the reference in `src/parity` is test support. Whether a
  confirmation may run at a length outside the supported set is a P13
  decision (plan §11).
