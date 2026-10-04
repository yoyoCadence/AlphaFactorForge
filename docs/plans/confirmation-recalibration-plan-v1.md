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
selection rule, the acceptance rule and what a passing result may and may not
claim.

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
  studentized observed mean. To avoid a division in the decision it is
  evaluated as `(S* − S)·√B(L) ≥ S·√v*`.

  **The two sides use different estimators, on purpose** (PR #136 review).
  The observed side uses `B(L)`, over all `n` overlapping circular blocks.
  The resampled side uses `v*`, computed from the blocks that were **drawn**
  (their sums and lengths) — it is *not* `B(L)` recomputed on the resampled
  series. Example: `x = [0, 1, 2, 3]`, `L = 2`, blocks drawn at 0 and 2
  reproduce `x` itself; `B(L) = 1`, a recomputed `B*(L)` would also be 1, and
  the declared `v* = 2`. The drawn blocks are the independent units of the
  resample, which is why their variance is the scale used there; `v*` is
  centered on the resample's own mean, so its bootstrap expectation is about
  `B(L)·(1 − 1/b)` for `b` blocks. S1 is this named variant — a
  *block-variance studentized* bootstrap — and not a bootstrap-t with one
  estimator on both series. Replacing `v*` by `B*(L)` would be a different
  candidate and needs an amendment (§9); it must not happen silently in the
  implementation.
- **S2** keeps v1's comparison but stretches the resampled deviations by the
  ratio of a less biased long-run standard deviation to the one the block
  bootstrap approximately reproduces. `2·B(2L) − B(L)` is the flat-top
  (trapezoid) lag window: weight 1 up to lag `L`, then falling linearly to
  zero at `2L`, which removes the Bartlett window's first-order downward bias
  under positive autocorrelation. The correction never shrinks (`c ≥ 1`). It
  requires `(2L)² ≤ n`, which holds for every block length of §3.

  **The denominator is an approximation** (PR #136 review). With `q` full
  blocks and a last partial block of `r` bars, the exact conditional variance
  of the resampled sum divided by `n` is `(q·L·B(L) + r·B(r)) / n`, not
  `B(L)`; four of the six declared `n`/`L` combinations have a partial block.
  S2 is the stated ratio as it stands. It is an experimental correction, and
  nothing here claims that it restores the finite-sample variance exactly.

These are the plan's **proposed** definitions, kept after the acceptance
review as experimental variants; whether either improves calibration is
decided by §6 and §7 and by nothing else. Each candidate's complete contract
— zero or negative variance estimates, partial blocks, signs, summation
order, the exact integer or floating-point form of every comparison — is
committed as a draft contract before its first diagnostic run (§10). A
departure from this section is a plan amendment (§9), made before the run it
affects.

V1 is a candidate too. If the unchanged statistic passes at some of the
tested lengths and no new statistic does better, the outcome is "v1,
restricted to the tested configurations that passed" (§7), which decision 1
allows as a verified condition of applicability. That would not erase the
failed 256-bar record.

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
| runs | size (null) in every cell; power only for eligible pairs, in the cells at and above their lowest supported tested length (defined below) |

All candidates see the same noise, because the data streams depend only on
the seed and the cell.

**Screen** (a point estimate; 4,000 simulations cannot support the bound of
§7): a cell passes when each confirmation rejected in at most 120 simulations
(3.0%) and the family in at most 240 (6.0%).

**Lowest supported tested length on diagnostics** of a candidate/rule pair: the
smallest `n` of §3 such that both noise models pass the screen at that `n`
and at every larger declared `n`. A pair with no such length is not eligible.

**Selection**, in this order:

1. the smallest such length;
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
a commit as `research-confirmation-statistics-v2` (or as v1 restricted to the
tested configurations).

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
simulations (2.76%) for a confirmation and **1134** (5.67%) for the family.

- "95%" is the upper end of the **two-sided** 95% Wilson interval, `z = 1.96`
  — not a one-sided 95% bound (`z = 1.645`). That convention and these
  thresholds are fixed.
- The intervals are per check. By exact binomial summation, one confirmation
  check passes with probability 99.05% when the true rate is the nominal 2.5%
  and 2.34% when it equals the 3% limit; the family check passes with 99.999%
  at 5% and 2.48% at 6%. These are single-check figures, not a statement
  about the whole grid of checks.
- The limits (3% and 6%) are accepted simulation limits, not the nominal
  alphas (2.5% and 5%). Meeting them is not proof of exact nominal-alpha
  control.

A cell passes when its three checks pass. Point estimates and the Wilson
intervals are reported for every check whatever the outcome.

**Supported tested configurations** (corrected after the PR #136 review; the
earlier text claimed more than the grid tests). The **lowest supported tested
length** `n*` is the smallest declared `n` such that both noise models pass at
`n` and at every larger declared `n`. The **supported set** is the declared
lengths at or above `n*` — at most `{256, 512, 1024}` — and nothing else.

- If `n*` exists, the evidence covers exactly: the lengths in the supported
  set; the `ar1-uniform-sum` generator at coefficients 0 and 0.3; the family
  shape and the 799 bootstrap samples of §3; the frozen candidate and block
  rule. The new contract may state that, and only that. P12e-FINDING-1 is
  then answered for those configurations.
- **Not covered, and not to be inferred from passing:** any other length,
  including lengths between or above the tested ones (700 or 2,048 bars are
  as unvalidated as 100); coefficients between 0 and 0.3 or beyond; any other
  noise distribution, light-tailed or not. No monotonicity is assumed in
  length or in the coefficient. Widening the claim needs its own declared
  justification or plan, not an assumption made after this grid passes.
- A declared length below `n*` is not supported, whether it failed itself or
  a longer one did. Whether a confirmation may run at an untested length at
  all, and on what evidence, is not decided here (§11).
- If `n*` does not exist, the acceptance has failed. The result is recorded,
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
| **P12e-5** | `research-noise-simulation-v2`: selectable statistic, per-confirmation and family checks with the rule of §7 and the screen of §6 (in integers wide enough — the products at `N = 20000` exceed 64 bits), the power scenario, prefix checkpoints declared before any report is committed, a release-build runner command. Measures the release-build cost before the diagnostic matrix is run. No new statistic. | only small fixture cases |
| **P12e-6** | draft contracts and implementations of S1 and S2 with independent references; the diagnostic grid of §6 on seed 20261005; the selection, recorded | diagnostics |
| **P12e-7** | freeze the selected method; the final acceptance of §7 on seed 20261117, once; record the result and the supported tested configurations; close or keep P12e-FINDING-1 | final acceptance |

P13 remains blocked until P12e-7 records supported tested configurations.
What P13 may do outside them is a separate decision (§11).

## 11. Not in this version

- **Fat tails and volatility clustering.** They need a noise model that is
  still bit-reproducible across Rust and TypeScript. Until a later plan
  covers them, the new contract claims nothing about such data, and a
  confirmation on real returns must say so.
- Other family shapes (several candidates per confirmation, prior trials,
  unequal schedules, more than two confirmations) and other bootstrap sample
  counts. The chosen shape is the least conservative one for Holm; it is not
  a proof for every shape.
- Every coefficient other than 0 and 0.3 — values in between, stronger or
  negative autocorrelation — and every length other than the three declared,
  longer ones included. The grid's endpoints are not a proof for what lies
  between or beyond them.
- A minimum power requirement. The single power figure (first confirmation,
  net-return test) is a chosen selection metric, not evidence of every kind
  of power.
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
- **One family shape and easy noise.** Passing here is evidence for the
  tested configurations it names and is not a general validity proof.
- **The release-build cost is unmeasured.** CI suite totals do not convert
  into a release runtime; P12e-5 measures it before the matrix is run.

## Record — 2026-10-02, P12e-5 (engine slice; not an amendment)

Nothing in §1–§12 changes: no cell, candidate, rule, seed or size was added
or altered, and no diagnostic or acceptance cell has been run. This section
records what §8 and §10 left to the engine slice.

- **Engine.** [`research-noise-simulation-v2`](../research-noise-simulation-v2.md):
  selectable statistic (only V1 exists so far), per-confirmation and family
  checks with the screen of §6 and the rule of §7 in 128-bit integers, the
  shifted scenario of §5, checkpoints, and the runner
  `cargo run --release --locked --example noise_simulation_v2 -- [--timing-only] <runs.json>`.
- **Declared prefix** (§8): every diagnostic and acceptance declaration
  carries exactly one checkpoint, at `4096 / bars` simulations — 16 at 256
  bars, 8 at 512, 4 at 1024. The normal test suite re-runs each committed
  report's declaration cut at that checkpoint and compares the counts.
- **Measured release-build cost, V1 only.** A timing-only run (statistic V1,
  seed 1, AR(1) 0.3, the family shape of §3, block rule R3, 500 simulations
  per cell; its outcomes were not printed or stored) took, one thread per
  cell on the development machine:

  | `n` | 500 simulations | per 4,000 (diagnostic cell) | per 20,000 (acceptance cell) |
  | --- | --- | --- | --- |
  | 256 | 0.366 s | 2.9 s | 14.6 s |
  | 512 | 0.797 s | 6.4 s | 31.9 s |
  | 1024 | 1.496 s | 12.0 s | 59.8 s |

  One unit of §8 is therefore about 1.5 s. For V1 the size grid of one
  candidate/rule pair is about 43 s of CPU and the whole final acceptance
  about 3.5 minutes, before parallelism; the first release build took
  5 min 37 s. The same run in a debug build was 5 to 13 times slower.
- **Still unmeasured:** S1 and S2, which do not exist yet. Their extra
  variance arithmetic is measured in P12e-6 before their grids are run.

## Record — 2026-10-02, P12e-6a (candidate contracts; not an amendment)

Nothing in §1–§12 changes, and no diagnostic or acceptance cell has been run.

- **P12e-6 is done in two steps.** P12e-6a: the candidates' draft contracts,
  implementations and references, so they can be reviewed before any result
  exists. P12e-6b: the diagnostic grid of §6 and the selection. The content
  and order of §10 are unchanged.
- **Draft contracts committed before any diagnostic run** (§4, §10):
  [`research-confirmation-candidates-draft-v1`](../research-confirmation-candidates-draft-v1.md),
  with the names `research-confirmation-candidate-s1-v1` and
  `research-confirmation-candidate-s2-v1`. They state the definitions of §4
  unchanged — S1 with `v*` from the drawn blocks, S2 with
  `c² = max(1, (2·B(2L) − B(L)) / B(L))` — and settle what §4 left open: the
  summation order of `B(ℓ)` and `v*`, a comparison `a·√p ≥ s·√q` evaluated
  without a square root or a division, the zero-variance cases, a partial
  last block, and refused inputs.
- **Measured release-build cost** (timing-only, seed 1, AR(1) 0.3, the family
  shape of §3, block rule R3, 500 simulations per cell; outcomes not printed
  or stored):

  | `n` | V1 | S1 | S2 |
  | --- | --- | --- | --- |
  | 256 | 0.402 s | 0.731 s | 0.451 s |
  | 512 | 0.832 s | 1.259 s | 0.870 s |
  | 1024 | 1.547 s | 2.238 s | 1.626 s |

  Measured on the final draft (after the PR #138 review added normalization
  and range checks; S1 became about 20% slower). S1 costs about 1.45 to 1.8
  times V1 and S2 about 1.05 to 1.1 times. One candidate/rule pair's size
  grid (two noise models, 4,000 simulations per cell) is therefore about 45 s
  of CPU for V1, 68 s for S1 and 47 s for S2; the whole size grid of §6 about
  five and a half minutes of CPU, and the power runs at most as much again.

## Record — 2026-10-03, P12e-6b (diagnostic grid run; not an amendment)

Nothing in §1–§12 changes: no cell, candidate, rule, seed, size or threshold
was added or altered. The acceptance seed 20261117 has not been used.

- **Declarations first.** All 72 declarations of §6 (36 size, 36 power) were
  generated from this plan's tables by
  `alpha-factor-forge/scripts/declare-recalibration-diagnostics.ts` and
  committed in `41d68eb` before any of them was run:
  [`fixtures/research/recalibration-plan-v1-diagnostics.json`](../../alpha-factor-forge/fixtures/research/recalibration-plan-v1-diagnostics.json).
  Seed 20261005, 4,000 simulations, the family of §3, one checkpoint at
  `4096 / bars`, the screen of §6 on size runs and no rule on power runs.
- **Runs.** Release runner, 16 threads on the development machine: the 36
  size runs in 395.6 s wall, then the 18 power runs that the size screen
  required in 304.9 s wall. The other 18 power declarations stay declared and
  unrun, as §6 says. Every report is committed in the same file, unedited
  (each equals the runner's output); `scripts/record-recalibration-diagnostics.ts`
  adds them and applies the rule of §6.
- **Size** — rejecting simulations of confirmation 1 / confirmation 2 / the
  family, out of 4,000; the screen allows 120 / 120 / 240; ✗ marks a cell
  that fails it:

  | Pair | φ | n = 256 | n = 512 | n = 1024 | Lowest supported tested length |
  | --- | --- | --- | --- | --- | --- |
  | V1-R3 | 0 | 121 / 71 / 190 ✗ | 109 / 57 / 164 | 98 / 47 / 144 | 1024 |
  |  | 0.3 | 156 / 100 / 253 ✗ | 124 / 73 / 194 ✗ | 116 / 55 / 169 |  |
  | V1-R4 | 0 | 117 / 63 / 178 | 98 / 48 / 145 | 97 / 53 / 149 | 1024 |
  |  | 0.3 | 172 / 100 / 268 ✗ | 143 / 79 / 219 ✗ | 118 / 70 / 185 |  |
  | S1-R3 | 0 | 96 / 54 / 149 | 92 / 44 / 136 | 86 / 40 / 126 | 512 |
  |  | 0.3 | 127 / 71 / 196 ✗ | 112 / 61 / 173 | 101 / 51 / 151 |  |
  | S1-R4 | 0 | 105 / 48 / 151 | 89 / 44 / 132 | 93 / 47 / 139 | 1024 |
  |  | 0.3 | 145 / 86 / 227 ✗ | 136 / 72 / 206 ✗ | 113 / 70 / 180 |  |
  | S2-R3 | 0 | 104 / 64 / 166 | 91 / 49 / 139 | 89 / 43 / 131 | 512 |
  |  | 0.3 | 127 / 82 / 206 ✗ | 100 / 57 / 156 | 98 / 47 / 143 |  |
  | S2-R4 | 0 | 102 / 54 / 154 | 83 / 41 / 123 | 87 / 48 / 135 | 512 |
  |  | 0.3 | 132 / 74 / 203 ✗ | 103 / 65 / 166 | 100 / 56 / 155 |  |

  The ten failed cells: every pair at 256 bars with φ = 0.3; V1-R3 at 256
  bars with φ = 0 (121, one over the screen); V1-R3, V1-R4 and S1-R4 at 512
  bars with φ = 0.3. Confirmation 1 fails in all ten; the family check also
  fails in two (V1-R3 and V1-R4 at 256 bars, φ = 0.3: 253 and 268);
  confirmation 2 fails in none. All six pairs are eligible; none supports
  256 bars.
- **Power** — confirmation 1, net-return test, simulations that rejected out
  of 4,000, in the cells at and above each pair's lowest supported tested
  length (← the length that ranks the pair):

  | Pair | n | φ = 0 | φ = 0.3 | Smaller of the two |
  | --- | --- | --- | --- | --- |
  | V1-R3 | 1024 | 3162 | 3260 | 3162 ← |
  | V1-R4 | 1024 | 3147 | 3296 | 3147 ← |
  | S1-R3 | 512 | 3066 | 3187 | 3066 ← |
  | S1-R3 | 1024 | 3100 | 3203 | 3100 |
  | S1-R4 | 1024 | 3112 | 3279 | 3112 ← |
  | S2-R3 | 512 | 3067 | 3116 | 3067 ← |
  | S2-R3 | 1024 | 3083 | 3118 | 3083 |
  | S2-R4 | 512 | 3061 | 3130 | 3061 ← |
  | S2-R4 | 1024 | 3080 | 3143 | 3080 |

- **Selection (§6): S2 with block rule R3** (block lengths 6 / 8 / 10 at
  256 / 512 / 1024 bars). Step 1 leaves the three pairs whose lowest supported
  tested length is 512 — S1-R3, S2-R3, S2-R4; step 2 ranks them 3067
  (S2-R3), 3066 (S1-R3), 3061 (S2-R4); step 3 is not reached.
- **What the selection does not show.** The three counts differ by 1 and 6
  simulations out of 4,000, all candidates saw the same simulated data (§6),
  and no confidence interval for the differences was computed: this record does not
  claim that S2-R3 has more power than S1-R3 or S2-R4. The rule was declared
  to pick one method, and it did. The screen is a point estimate (§6), so a
  passing cell here is not a pass of §7; the 256-bar, φ = 0.3 cell failed the
  screen for every pair, S2-R3 included (127 on confirmation 1). Which lengths
  S2-R3 supports is decided by §7 on its own seed, and only there.
- **The normal suite** re-runs each of the 54 committed reports' declarations
  cut at its checkpoint and compares the counts (§8, P12e-5 record), and a
  Vitest re-derives the screen, the required power runs, the lowest supported
  lengths and the selection from the committed counts. Counts after the
  checkpoint are covered only by re-running the documented command. **Cost
  above the estimate of §8:** the 54 prefixes take 16.6 s of debug time on
  one thread on the development machine (1.25 s on its 16 threads; the test
  spreads them across the available cores), against "about ten seconds". The
  prefix and the reports cannot change after the run; the time it takes in
  CI is read from this slice's CI run and recorded with P12e-7.
- **Next (§10): P12e-7** freezes S2 with R3 as
  `research-confirmation-statistics-v2` and runs the final acceptance of §7
  once, on seed 20261117.

## Record — 2026-10-03, P12e-7a (freeze and acceptance declarations; not an amendment)

Nothing in §1–§12 changes: no cell, rule, seed, size or threshold was added
or altered. The acceptance seed 20261117 has not been used.

- **P12e-6b merged** as PR #139 (`e9e73e8`) after an independent review that
  replayed all 54 reports in full. The CI time the P12e-6b record left open:
  in the reviewed head's CI run the library tests took 21.16 s, and the
  prefix test finished 10.44 s after the library suite started (a bound on
  its completion, not an isolated measurement). The review kept all 54
  prefixes.
- **P12e-7 is done in two steps** (maintainer decision 2026-10-03), as P12e-6
  was: **P12e-7a** freezes the method and commits the six declarations of §7,
  to be reviewed and merged before anything runs; **P12e-7b** runs them once
  on seed 20261117 and records the result and the supported tested
  configurations. The content and order of §7 and §10 are unchanged.
- **Frozen method** (§7: "frozen in a commit as
  `research-confirmation-statistics-v2`"):
  [`research-confirmation-statistics-v2`](../research-confirmation-statistics-v2.md),
  the draft S2 with block rule R3. Maintainer decision 2026-10-03: the
  block length stays a declared integer and must equal `round(n^(1/3))` of
  every series — the smallest `L` with `(2L + 1)³ > 8n`, in exact integers —
  so only the rule the acceptance tests can be used. Its computation is the
  draft S2's own code path; tests show it reports exactly what S2 reports on
  every fixture input and, in the engine, reproduces the committed
  checkpoint counts of the six S2-R3 size runs.
- **Power of the final method** (§5 records power "for the final method"):
  v2 is S2-R3 on the same code path, so the power runs of P12e-6b above are
  its power — 3067 / 3116 at 512 bars and 3083 / 3118 at 1024 bars (φ = 0 /
  0.3, of 4,000); 256 bars was not run because S2-R3 did not pass the screen
  there. No power cell is added to §7.
- **Declarations**:
  [`fixtures/research/recalibration-plan-v1-acceptance.json`](../../alpha-factor-forge/fixtures/research/recalibration-plan-v1-acceptance.json),
  written by `alpha-factor-forge/scripts/declare-recalibration-acceptance.ts`
  from §3 and §7: the six cells (φ 0 and 0.3 × 256, 512, 1024 bars),
  statistic v2, block lengths 6 / 8 / 10, 799 bootstrap samples, the family
  of §3, 20,000 simulations, seed 20261117, the Wilson rule with limits
  30000 and 60000 ppm (at most 552 and 1134 of 20,000), and one checkpoint at
  `4096 / bars`. The suite parses them with the engine and checks them
  against §3 and §7; nothing simulates them, not even a prefix, before
  P12e-7b. Its run time is measured and recorded there (§8 caps the matrix,
  not the time).

## Record — 2026-10-04, P12e-7b (final acceptance; not an amendment)

PR #140 merged as `b9b63a3` (reviewed head `759ecde`). The six declarations
were byte-identical to that head (Git blob `ae3f1e9d73fe2baac83e5ed64090f21c6a10c1aa`)
before this run. No method, rule, cell, seed, sample count or limit changed.
The maintainer requested continued autonomous task-board work after FU-9;
P12e-7b was the next unfinished implementation slice.

The full matrix ran once on seed **20261117**, 20,000 simulations per cell,
with the unchanged release runner on six threads. Build command:

```powershell
$env:CARGO_TARGET_DIR='C:/tmp/aff-target'
cargo build --release --locked --example noise_simulation_v2
```

The built `C:/tmp/aff-target/release/examples/noise_simulation_v2.exe` was
launched once with the frozen `fixtures/research/recalibration-plan-v1-acceptance.json`
path; stdout was redirected directly to the committed
[`recalibration-plan-v1-acceptance-output.json`](../../alpha-factor-forge/fixtures/research/recalibration-plan-v1-acceptance-output.json).
An exclusive pre-launch marker prevented repeating the full run. Start:
2026-10-04 14:46:01 UTC. Raw input SHA-256:
`9436c5b8986cc93efac0bc6a12450d58a6142c55228a2b022733d8eb1bc44bd0`;
raw stdout SHA-256:
`347c857286c41e608a52415ccea07a1cb25f0092f7114491857d34810e05e5e6`.
`scripts/record-recalibration-acceptance.ts` attaches those exact report values
under the preplanned `reports` field and records §7's supported set; it runs
no simulation. Vitest checks raw-output equality, original declarations,
all rates/Wilson bounds, and every decision via independent BigInt inequalities.

Every entry below is **count / 20,000; rate%; [95% Wilson lower%, upper%]**.
Intervals use the unchanged two-sided 95% convention and outward whole-ppm
rounding. Each confirmation must have at most 552 rejecting simulations;
the family at most 1134. All 18 checks pass.

| φ | bars | confirmation 1 | confirmation 2 | family |
| --- | --- | --- | --- | --- |
| 0 | 256 | 435; 2.1750%; [1.9818, 2.3866] | 256; 1.2800%; [1.1332, 1.4455] | 682; 3.4100%; [3.1672, 3.6707] |
| 0 | 512 | 391; 1.9550%; [1.7721, 2.1564] | 234; 1.1700%; [1.0300, 1.3287] | 618; 3.0900%; [2.8590, 3.3390] |
| 0 | 1024 | 410; 2.0500%; [1.8626, 2.2558] | 212; 1.0600%; [0.9271, 1.2117] | 618; 3.0900%; [2.8590, 3.3390] |
| 0.3 | 256 | 541; 2.7050%; [2.4890, 2.9391] | 320; 1.6000%; [1.4351, 1.7835] | 850; 4.2500%; [3.9790, 4.5385] |
| 0.3 | 512 | 456; 2.2800%; [2.0821, 2.4963] | 273; 1.3650%; [1.2132, 1.5355] | 721; 3.6050%; [3.3554, 3.8724] |
| 0.3 | 1024 | 461; 2.3050%; [2.1060, 2.5224] | 243; 1.2150%; [1.0722, 1.3765] | 699; 3.4950%; [3.2492, 3.7586] |

**Lowest supported tested length: 256. Supported tested set: {256, 512, 1024}**,
each at φ 0 and 0.3, the `ar1-uniform-sum` generator, two confirmations of
one candidate, no prior trials, 799 bootstrap samples, frozen v2 and R3.
No other length (including 700 or 2048), coefficient, distribution or family
shape is supported by this result. The 3%/6% acceptance limits do not prove
exact nominal-alpha control. P12e-FINDING-1 is answered **only for this set**;
the original v1 129/2000 (6.45%) failure remains unchanged and tested.
This closes the calibration blocker for P13; runtime reveal/alpha/fence
implementation and native campaign acceptance remain separate work.

Measured per-cell release time: φ0 at 256/512/1024 = **17.769 / 35.234 /
66.214 s**, φ0.3 = **17.646 / 35.320 / 66.162 s**; total **66.217 s wall**
on six threads. Release incremental build: 33.59 s. All six declared
checkpoints (16 / 8 / 4 simulations by length) join the Rust suite; they
reproduce full counts and extreme-total digests, not just rejection status.
The full acceptance is never rerun by tests. CI timing is recorded on this
slice's PR and in its handoff after the final head finishes.
