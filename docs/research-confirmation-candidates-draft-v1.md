# Confirmation statistic candidates — draft contracts (`research-confirmation-candidate-s1-v1`, `-s2-v1`)

> **Status: P12e-6a, DRAFT candidates (2026-10-02).** These are the two
> experimental statistics of
> [`confirmation-recalibration-plan-v1`](plans/confirmation-recalibration-plan-v1.md)
> §4, written out in full **before any diagnostic run**. They are not product
> contracts: nothing but the simulation engine
> ([`research-noise-simulation-v2`](research-noise-simulation-v2.md)) may call
> them, neither has been shown to be calibrated, and neither may be used to
> confirm a strategy. No diagnostic or acceptance cell of the plan has been
> run. Whether either improves on
> [`research-confirmation-statistics-v1`](research-confirmation-statistics-v1.md)
> is decided by the plan's §6 and §7 and by nothing else.
> Implementation: `alpha-factor-forge/src-tauri/src/discovery_core/confirmation_candidates.rs`.

## 1. What is shared with v1

Everything except the rule that decides whether a resample is *extreme*:

- the declaration (v1 §2) and the evaluation inputs (v1 §3): `familyTests`,
  one series per candidate, finite values, `n ≥ 2`;
- the two tests per candidate — net return, and excess over the benchmark —
  on the **same** resampled bars;
- the resampling (v1 §4): the candidate's SplitMix64 stream, `ceil(n / L)`
  blocks per resample, each starting at `below(n)` and taking
  `min(L, bars still needed)` bars circularly. A candidate draws exactly the
  block starts v1 draws, so all three statistics see identical resamples;
- `rawP = (1 + extremeCount) / (B + 1)`, Holm against the whole family, the
  exact comparison with `alphaPpm`, and the report (v1 §5–§7). The report's
  `contractVersion` is the candidate's name.

## 2. Quantities

For one test, the series is `x_0 … x_{n−1}` (net returns, or
`returns − benchmarkReturns`).

**Constant series first.** If every bar of the series equals the first
(IEEE `==`, so `−0 = +0`), nothing below is computed for that test: every
resample is extreme when the common value is `≤ 0`, and none is when it is
`> 0`. This is the limit of the studentized statistic as the variance goes
to zero, it is what v1 does with a constant series, and it holds whatever
the value's decimal or binary representation (PR #138 review R2). The other
test of the same candidate is unaffected.

**Normalization.** Otherwise the series is multiplied by `2^−e`, with
`e = floor(log2 max|x_t|)` read from the bits of the largest magnitude and
the factor built from bits, so the largest magnitude lies in `[1, 2)`. A
power of two scales every sum, difference and product below exactly, so
no decision depends on the series' overall scale (PR #138 review R1). An
all-zero series is constant and never reaches this step. `S` below is the
left-to-right sum of the **normalized** series; the reported
`observedMean` is still the sum of the series as given, divided by `n`.

**Range.** Every product and quotient below must be a finite normal double
whenever its operands are non-zero. An intermediate that would overflow,
or underflow to zero or a subnormal, is **refused** with an error — never
rounded to infinity or zero. After normalization an overflow is not
possible for any realistic length, and an underflow needs non-zero values
or deviations hundreds of orders of magnitude below the largest value (a
square below about `2^-1022`, or a value more than about `2^1022` times
smaller than the largest, which normalization itself refuses). Return
series do not come near it; the rule makes the boundary explicit.

**Block variance over all circular blocks.** For a block length `ℓ`:

```text
C_t(ℓ) = x_t + x_{t+1} + … + x_{t+ℓ−1}     indices modulo n, summed in that order from 0
B(ℓ)   = ( Σ_{t=0}^{n−1} ( C_t(ℓ) − ℓ·(S / n) )² ) / (n·ℓ)
```

`S / n` is one division, `ℓ·(S / n)` one multiplication, the squares are
accumulated for `t = 0, 1, …` from 0, and the total is divided once by the
integer `n·ℓ`. `B(ℓ)` is the long-run variance estimate with Bartlett weights
`1 − |h|/ℓ` on the circular autocovariances (tested).

**Block variance of one resample.** A resample consists of `b = ceil(n / L)`
drawn blocks; block `j` has `take_j` bars and sum `U_j` (its bars in draw
order, from 0). `S*` is the sum of all its bars in draw order, accumulated
continuously across blocks exactly as v1 does (so `S*` is not `Σ U_j`
recomputed).

```text
v* = ( Σ_{j=1}^{b} ( U_j − take_j·(S* / n) )² ) / n
```

`v*` uses the blocks that were **drawn**. It is not `B(L)` recomputed on the
resampled series: for `x = [0, 1, 2, 3]`, `L = 2`, blocks drawn at 0 and 2
reproduce `x`; `B(2) = 1` and `v* = 2` (tested).

**Comparison without a square root.** Both candidates decide
`a·√p ≥ s·√q` for finite `a`, `s` and `p, q ≥ 0`, with the range rule
applied to `a·a`, `(a·a)·p`, `s·s` and `(s·s)·q`:

```text
left  = (a·a)·p        right = (s·s)·q
a ≥ 0 and s ≥ 0 :  left ≥ right
a ≥ 0 and s < 0 :  true
a < 0 and s ≥ 0 :  left = 0 and right = 0
a < 0 and s < 0 :  left ≤ right
```

in IEEE-754 double arithmetic in exactly this order. No square root, no
division and no transcendental function enters a decision.

## 3. S1 — block-variance studentized bootstrap (`research-confirmation-candidate-s1-v1`)

Requires `L² ≤ n`, as v1.

A resample is **extreme** when its studentized deviation reaches the
studentized observed mean, `(S* − S)/√v* ≥ S/√B(L)`, evaluated as §2's
comparison with

```text
a = S* − S      p = B(L)      s = S      q = v*
```

The two sides use different estimators on purpose (plan §4): the observed
side `B(L)` over all overlapping circular blocks, the resampled side the
variance of the drawn blocks. `v*` is centred on the resample's own mean, so
its bootstrap expectation is about `B(L)·(1 − 1/b)`.

Edge cases:

- A **constant** series is decided by §2's rule: a positive constant is
  rejected (`rawP = 1 / (B + 1)`), zero or a negative constant never is
  (`rawP = 1`). An earlier version of this draft promised `rawP = 1` for
  every constant series; that held only when the arithmetic happened to be
  exact (16 bars of 0.125) and not for 16 bars of 0.1, whose rounding
  residue decided instead (PR #138 review R2, corrected before any
  diagnostic run).
- `v* = 0` (every drawn block has the same per-bar mean) while `B(L) > 0`:
  with `S > 0` the resample is extreme exactly when `S* ≥ S`.
- `B(L) = 0` for a series that is not constant (every circular block has
  the same sum, for instance an alternating series): with `S > 0` a
  resample is extreme exactly when `v* = 0`.
- `S ≤ 0`: every resample with `S* ≥ S` is extreme, and so are those below it
  whose studentized deviation is still above the (negative) observed one.
- A series that is nearly but not exactly constant is decided by the
  arithmetic above, where rounding residue can matter. This is stated, not
  hidden by a tolerance chosen after the fact.

## 4. S2 — centred bootstrap with a flat-top variance correction (`research-confirmation-candidate-s2-v1`)

Requires `(2L)² ≤ n`; a shorter series is refused.

```text
F = 2·B(2L) − B(L)
(p, q) = (F, B(L))   if B(L) > 0 and F > B(L)
         (1, 1)      otherwise
```

A resample is **extreme** when `c·(S* − S) ≥ S` with `c = √(p / q) ≥ 1`,
evaluated as §2's comparison with

```text
a = S* − S      s = S
```

`2·B(2L) − B(L)` is the flat-top (trapezoid) lag window: weight 1 up to lag
`L`, falling linearly to 0 at `2L` (tested). The correction never shrinks the
deviations (`c ≥ 1`); when the flat-top estimate is not above `B(L)`, or
`B(L)` is zero, S2 uses `c = 1`. A constant series follows §2's rule, as
for S1.

Limits stated in the plan and repeated here: `B(L)` is only approximately
the variance the block bootstrap reproduces — with `q` full blocks and a last
partial block of `r` bars the exact conditional variance is
`(q·L·B(L) + r·B(r)) / n`. S2 is the stated ratio and makes no claim to
restore the finite-sample variance exactly.

With `c = 1` S2 compares `S* − S ≥ S` where v1 compares `S* ≥ 2·S`; the two
can differ in the last bit of a tie. S2 is its own contract, not v1 with a
switch.

## 5. Refused inputs

As v1: mismatched or too-short series, a non-finite return, excess or sum,
duplicate candidates, an invalid `familyTests`, a declaration outside v1's
domain. In addition S2 refuses `(2L)² > n`, and both refuse any series or
resample whose intermediates leave the range of §2 (the error names the
candidate). Simulated noise never does.

## 6. Fixture and reference

[`research-confirmation-candidates-draft-v1.json`](../alpha-factor-forge/fixtures/rs-core/research-confirmation-candidates-draft-v1.json)
holds small fixed inputs for both candidates. Its `expected` blocks come from
an independent TypeScript reference written from this document
(`src/parity/confirmationCandidatesFixture.ts`,
`npm run fixtures:confirmation-candidates`), not from the Rust module; a
Vitest fails if the fixture stops matching and the Rust tests must reproduce
it exactly. None of the cases is a cell of the recalibration plan.

## 7. Status and limits

- **Draft.** A change to §2–§4 before the first diagnostic run is an edit of
  this draft; after it, a plan amendment (plan §9). Replacing `v*` by a
  recomputed `B*(L)` would be a different candidate.
- **Edited before any diagnostic run** after the PR #138 review: §2 gained
  the constant-series rule, the normalization and the range rule; §3, §4 and
  §5 follow. On every non-constant fixture input the results are unchanged
  (tested), because a power of two scales the arithmetic exactly.
- Not validated for anything. In particular nothing here is evidence about
  size or power; that is what the plan measures.
- Only the simulation engine may select these statistics. No runtime path,
  command or UI can reach them.
- Rust only; the reference in `src/parity` is test support.
