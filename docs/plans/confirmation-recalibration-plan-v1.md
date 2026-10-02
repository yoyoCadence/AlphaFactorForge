# Confirmation recalibration plan (`confirmation-recalibration-plan-v1`)

> **Status: P12e-4, plan only (2026-10-02).** Nothing in this document has
> been implemented or run. It fixes, before any code or simulation exists,
> how a successor to the P12e-1 confirmation statistic will be chosen and
> accepted. It answers **P12e-FINDING-1**
> ([`research-noise-simulation-v1`](../research-noise-simulation-v1.md) §7.1):
> the declared serially correlated acceptance run measured a 6.45% family
> false-positive rate against a nominal 5% and a 6% limit. That record stays;
> P12e stays open and P13's confirmation work stays blocked until §7 is met.
> Upstream: [`research-confirmation-statistics-v1`](../research-confirmation-statistics-v1.md)
> (the statistic being replaced or bounded),
> [`research-alpha-allocation-v1`](../research-alpha-allocation-v1.md),
> [`active-plan.md`](active-plan.md) §4.5.

## 0. Maintainer decisions (2026-10-02)

Carried over from the PR #135 review:

1. Resolve the finding by **revising the statistic**: this plan first, then a
   comparison of a studentized bootstrap and a serial-correlation variance
   correction. A longer sample may become a condition of applicability once
   verified. Lowering alpha does not close the finding.
2. The new acceptance checks **each confirmation and the whole family**.
3. The 6.45% failure stays on record.

Decided for this plan:

4. **Acceptance rule: a confidence bound.** A check passes only when the
   upper end of the 95% Wilson interval of the observed false-positive rate
   is at most **1.2 × nominal**. The final acceptance uses **20,000**
   simulations per cell, is run once with a release build and committed; it
   is not part of every CI run. Point estimates are recorded as well.
5. **Scope of this first version:** 256, 512 and 1,024 bars × independent
   noise and AR(1) 0.3 noise. Fat tails and volatility clustering are a later
   version; until then nothing is claimed for them.
6. **Power is reported and used to choose between candidates**, with no
   minimum threshold.

## 1. What this plan fixes, and what it leaves

Fixed here, before anything runs: the targets, the simulated configurations,
the candidates and the constraints on them, the seeds, the sizes, the
selection rule, the acceptance rule and how the result becomes a condition of
applicability.

Left to the slices of §10: the simulation engine that can express these
rules, each candidate's full contract text (edge cases included), and the
runs themselves.

## 2. Targets

For a trial family that spends its alpha schedule over its confirmations, on
data where every tested null hypothesis is true:

| Check | Event counted in one simulation | Nominal |
| --- | --- | --- |
| confirmation `k` | confirmation `k` rejects at least one null | its share `α_k` |
| family | any confirmation rejects at least one null | `Σ α_k` |

Both kinds must hold. A family total within its limit is **not** evidence
that every share is calibrated: in the v1 run the second confirmation was
conservative and partly hid a first confirmation above its share.

## 3. Simulated configurations

All data come from [`research-noise-simulation-v1`](../research-noise-simulation-v1.md)
§3 (`ar1-uniform-sum`, same streams, same warm-up). Synthetic data only: no
market data and no Validation/Test segment takes part in calibration.

| Dimension | Values |
| --- | --- |
| noise | `autocorrelationPpm` 0 and 300000 |
| bars `n` | 256, 512, 1024 |
| family | two confirmations; schedule `[25000, 25000]` of a 50000 total; one candidate per confirmation; no prior trials — the configuration of the failed v1 run, so the `n = 256`, 0.3 cell is directly comparable and the family has the least Holm slack |
| bootstrap samples | 799, so every Holm threshold (`m = 2`, `4`) lies on the p-value grid |
| block length | a declared integer, by one of two rules: |

| `n` | rule **R3** = `round(n^(1/3))` | rule **R4** = `round(n^(1/4))` |
| --- | --- | --- |
| 256 | 6 | 4 |
| 512 | 8 | 5 |
| 1024 | 10 | 6 |

That is six **cells** (two noise models × three lengths) per candidate and
block rule. Nominal values: 25000 ppm for each confirmation and 50000 ppm for
the family; limits (× 1.2): 30000 and 60000 ppm.

## 4. Candidates

Every candidate must keep what the rest of P12 relies on:

- pure and deterministic; reproducible bit for bit by an independent
  TypeScript reference; no transcendental function in a decision;
- the same resampled bars as v1 (same PRNG streams and block draws), so
  candidates are compared on identical resamples;
- p-values of the exact form `(1 + extreme) / (B + 1)`, Holm against the
  whole family unchanged;
- a declared block length — nothing estimated from the confirmation data
  (maintainer decision of 2026-10-01);
- two one-sided tests per candidate, reported separately.

Notation for one test: series `x_0 … x_{n−1}` with sum `S`; block length `L`;
`C_t(L)` the sum of the `L` consecutive bars starting at `t`, wrapping
circularly; a resample made of drawn blocks `j` with sums `U_j` and lengths
`take_j`, and total `S*`.

```text
B(L) = (1 / (n·L)) · Σ_{t=0}^{n−1} ( C_t(L) − L·S/n )²        long-run variance, Bartlett weights
v*   = (1 / n)     · Σ_j         ( U_j − take_j·S*/n )²      the same quantity inside one resample
```

| Id | Candidate | A resample is extreme when |
| --- | --- | --- |
| **V1** | the current statistic (baseline) | `S* − S ≥ S` |
| **S1** | studentized circular block bootstrap | `(S* − S) / √v* ≥ S / √B(L)` |
| **S2** | centered bootstrap with a flat-top variance correction | `c · (S* − S) ≥ S`, `c² = max(1, (2·B(2L) − B(L)) / B(L))` |

- **S1** compares the studentized deviation of each resample with the
  studentized observed mean, each scaled by its own block-based variance. To
  avoid a division in the decision it is evaluated as
  `(S* − S)·√B(L) ≥ S·√v*`.
- **S2** keeps v1's comparison but stretches the resampled deviations by the
  ratio of a less biased long-run standard deviation to the one the block
  bootstrap reproduces. `2·B(2L) − B(L)` is the flat-top (trapezoid) lag
  window, which removes the Bartlett window's first-order downward bias under
  positive autocorrelation. The correction never shrinks (`c ≥ 1`). It
  requires `(2L)² ≤ n`, which holds for every block length of §3.

These are the plan's **proposed** definitions. Each candidate's complete
contract — zero-variance cases, summation order, the exact integer or
floating-point form of every comparison — is committed as a draft contract
before its first diagnostic run (§10). A departure from this section is a
plan amendment (§9), made before the run it affects.

V1 is a candidate too. If the unchanged statistic turns out to be supported
from some length upward and no new statistic does better, the outcome is "v1
with a minimum length", which decision 1 allows as a verified condition of
applicability.

## 5. Power scenario (reported; used only to choose)

The same noise, with a constant added to the strategy's returns only (the
benchmark stays at zero mean), so both tests have a true effect. The shift is
three long-run standard errors of the mean, `3·σ_LR/√n` with
`σ_LR = √(1/3) / (1 − φ)`, declared as integer millionths so no square root
enters the simulation:

| `n` | φ = 0 | φ = 0.3 |
| --- | --- | --- |
| 256 | 108253 | 154647 |
| 512 | 76547 | 109352 |
| 1024 | 54127 | 77324 |

**Power** of a candidate in a cell is the number of simulations in which the
first confirmation rejects the net-return null. It is recorded for every
eligible candidate of §6 and for the final method. It has no threshold.

## 6. Diagnostic phase — choosing the method

| | |
| --- | --- |
| seed | **20261005** |
| simulations per cell | 4,000 |
| grid | candidates {V1, S1, S2} × block rules {R3, R4} × the six cells |
| runs | size (null) in every cell; power only for eligible pairs, in the cells at and above their supported minimum length (defined below) |

All candidates see the same noise, because the data streams depend only on
the seed and the cell.

**Screen** (a point estimate; 4,000 simulations cannot support the bound of
§7): a cell passes when each confirmation rejected in at most 120 simulations
(3.0%) and the family in at most 240 (6.0%).

**Supported minimum length on diagnostics** of a candidate/rule pair: the
smallest `n` of §3 such that both noise models pass the screen at that `n`
and at every larger declared `n`. A pair with no such length is not eligible.

**Selection**, in this order:

1. the smallest supported minimum length;
2. then the larger power at that length, taken as the smaller of the two
   noise models' counts;
3. then V1 before S1 before S2, and R3 before R4.

If no pair is eligible, no final acceptance is run: the finding stays open
and a new plan version is required.

The selection is recorded with the whole grid, including every pair that
failed. An additional variant may be tried on the diagnostic seed only after
it has been added to this plan by an amendment (§9).

## 7. Final acceptance

Run **once**, after the selected candidate and block rule have been frozen in
a commit as `research-confirmation-statistics-v2` (or as a minimum-length
condition on v1).

| | |
| --- | --- |
| seed | **20261117** — never used before this run |
| simulations per cell | 20,000 |
| cells | all six, with the selected candidate and block rule |
| build | release; a documented command; the report is committed |

**Rule**, for each check of §2 with limit `l` ppm (30000 or 60000), `N`
simulations and `x` counted events — the 95% Wilson upper bound is at most
the limit, written without a square root:

```text
passes  iff  x·10⁶ < l·N   and   (l·N − x·10⁶)² · 10000  ≥  38416 · l · (10⁶ − l) · N
```

(`38416 / 10000 = 1.96²`.) With `N = 20000` this allows at most **552**
simulations (2.76%) for a confirmation and **1134** (5.67%) for the family. A
method exactly at its nominal rate passes one confirmation check with
probability about 99%; a method whose true rate equals the limit passes it
with probability about 2.5%.

A cell passes when its three checks pass. Point estimates and the Wilson
intervals are reported for every check whatever the outcome.

**Supported range.** The supported minimum length `n*` is the smallest
declared `n` such that both noise models pass at `n` and at every larger
declared `n`.

- If `n*` exists, the new contract states: valid for at least `n*` bars, on
  light-tailed noise with serial correlation up to 0.3, for the family shape
  of §3. Shorter series are refused. P12e-FINDING-1 is then answered for that
  range.
- If it does not exist, the acceptance has failed. The result is recorded,
  the finding stays open, P13 stays blocked, and a new plan version is
  needed. The seed is spent either way.

## 8. Cost and where things run

Unit: the v1 acceptance run (`n = 256`, 2,000 simulations) — the two of them
took the library suite to 8.17 s and 19.17 s in two CI runs and about 25 s
in a local debug build.

| Phase | Size, in units | Where |
| --- | --- | --- |
| diagnostics, size | 168 (6 pairs × 28) | a documented command, release build |
| diagnostics, power | at most 168 | the same |
| final acceptance | 140 | the same, once |

Cost grows with `n` (factors 1, 2, 4) and candidates S1/S2 add variance
arithmetic to every resample, so the engine slice measures a release build
before the grid is run. The plan caps the **matrix**, not the wall time: no
cell, candidate, rule or simulation count is added without an amendment.

Neither phase joins the normal test suite. What the normal suite keeps:

- the two v1 acceptance simulations, unchanged (maintainer decision);
- for every committed diagnostic or acceptance report, a re-computation of a
  declared prefix of its simulations, bounded to about ten seconds of debug
  time in total. The exact prefix is declared in the engine slice.

Anyone can reproduce a full report with the documented command.

## 9. Record-keeping rules

1. This plan is merged before any candidate is implemented or simulated.
2. An amendment is an appended, dated section stating what changes and why,
   committed before the run it affects. Past text is not rewritten.
3. Every run's declaration, seed and report is committed, failures included.
4. The diagnostic seed may be reused for declared diagnostics. The acceptance
   seed is used exactly once, after the method is frozen.
5. A threshold, seed, size or cell is never changed in response to a result.
6. The v1 results (97/2000 and 129/2000) and their declarations stay as they
   are, and stay in the test suite.
7. Until §7 passes, nothing may describe a confirmation as controlling its
   false-positive rate on serially correlated returns.

## 10. Slices

| Task | Content | Runs simulations? |
| --- | --- | --- |
| **P12e-5** | `research-noise-simulation-v2`: selectable statistic, per-confirmation and family checks with the rule of §7 and the screen of §6, the power scenario, prefix checkpoints, a release-build runner command. Measures the release-build cost. No new statistic. | only small fixture cases |
| **P12e-6** | draft contracts and implementations of S1 and S2 with independent references; the diagnostic grid of §6 on seed 20261005; the selection, recorded | diagnostics |
| **P12e-7** | freeze the selected method; the final acceptance of §7 on seed 20261117, once; record the result and the supported range; close or keep P12e-FINDING-1 | final acceptance |

P13 remains blocked until P12e-7 records a supported range.

## 11. Not in this version

- **Fat tails and volatility clustering.** They need a noise model that is
  still bit-reproducible across Rust and TypeScript. Until a later plan
  covers them, the new contract claims nothing about such data, and a
  confirmation on real returns must say so.
- Other family shapes (several candidates per confirmation, prior trials,
  unequal schedules, more than two confirmations) and other bootstrap sample
  counts. The chosen shape is the least conservative one for Holm; it is not
  a proof for every shape.
- Stronger autocorrelation than 0.3, negative autocorrelation, and lengths
  other than the three declared.
- A minimum power requirement.
- Whether P13 must also simulate each confirmation's own declared values
  before running it (noise-simulation contract §10) — a P13 decision.

## 12. Risks stated in advance

- **No candidate may pass.** S1 and S2 are reasoned choices, not guarantees.
  The plan then ends with a recorded failure and a new version, not with a
  relaxed rule.
- **Selection on 4,000 simulations is noisy** (standard error about a quarter
  of a point at 2.5%). A pair that looks eligible may fail the 20,000-run
  bound; that is what the separate seed is for.
- **The exploratory block-length runs** in the P12e-3 handoff are a
  hypothesis only. This plan tests both block rules for every candidate
  rather than assuming the cause.
- **One family shape and easy noise.** Passing here is necessary for the
  range it names and is not a general validity proof.
