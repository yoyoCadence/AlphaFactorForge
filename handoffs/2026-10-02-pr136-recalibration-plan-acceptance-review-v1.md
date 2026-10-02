# Handoff: PR #136 recalibration plan acceptance review

Date: 2026-10-02
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p12e4-recalibration-plan`
PR: [#136](https://github.com/yoyoCadence/AlphaFactorForge/pull/136)
Reviewed head: `c4ec35aec160e12d697777effc7d06536f7f04f0`
Base: `1764c4807e999cc18a23111bf182d19868e514d9` (merged #135)
Status: One P2 scope clarification required before merge. Candidate definitions are acceptable as experimental variants, with the clarifications below. No calibration simulation was run; P12e-FINDING-1 remains open and P13 blocked.

## Summary

This is a documentation-only plan: eight Markdown files, no product code,
fixture or test changes. The declared selection/acceptance seeds, matrix,
per-confirmation and family checks, power scenario, record-keeping and future
slices are specified. Independent arithmetic checks reproduce the plan's
derived numbers. All six exact-head CI jobs pass.

The main defect is how the finite calibration grid becomes a supported range:
plan §7 promises more sample lengths, coefficients and noise distributions
than §3 tests, despite §11 explicitly excluding other lengths. Fix the claim
before it becomes the next statistic's contract or the condition for closing
the finding. This does not require running anything or expanding this plan's
matrix.

## Required Action / Decision

### R1 — P2: Keep the supported range within the declared calibration grid

Location: `docs/plans/confirmation-recalibration-plan-v1.md:223`, especially
lines 227–230; compare §3 and §11.

The plan tests only `n ∈ {256, 512, 1024}`, `φ ∈ {0, 0.3}` and the named
`ar1-uniform-sum` generator. However, §7 says the successor is valid for
**at least `n*` bars, on light-tailed noise with serial correlation up to
0.3**. This includes untested lengths (e.g. 2048), intermediate coefficients
(e.g. 0.15), and other light-tailed processes. Neither a monotonicity argument
nor an acceptance declaration covers them. Passing the endpoints is not a
proof for those combinations. The same section would close the finding for
this broader claim, and downstream P13 could consume it as eligibility.

Required before merge:

- State that the evidence covers only the declared generator/coefficient
  values, tested lengths, family shape, bootstrap count and frozen block rule.
- The minimum is the **lowest supported tested length**; the supported set
  contains the declared lengths at or above it that pass both noise cells,
  not every integer above it. Make §7 consistent with §11 and with the task
  wording that turns the outcome into a contract condition.
- Other lengths, intermediate coefficients and other distributions remain
  unvalidated. Expanding the applicability claim requires a separately
  declared justification/plan, not an assumption after this grid passes.

It is fine to retain the predeclared suffix rule for selecting the supported
tested lengths after final acceptance. This finding is about extrapolation,
not about demanding a different selection rule or a larger matrix.

## Answers to the screenshot's design questions

These are reviewer recommendations, not a record that the maintainer has
already adopted new decisions.

1. **Keep S2 as a candidate; no fourth method is required.** The weights of
   `2·B(2L) − B(L)` do form the trapezoid flat-top window: weight 1 through
   lag L, then linearly decreasing to zero at 2L. Stretching the centered
   deviations with the proposed ratio is a defined experimental correction,
   not a guarantee that finite-sample calibration improves. All six proposed
   block lengths satisfy `(2L)² ≤ n`.
2. **Keep S1's proposed formula as an explicitly named block-variance
   studentized variant, but clarify its estimator.** Its `v*` uses the drawn
   blocks, whereas `B(L)` uses all overlapping circular blocks. It is not a
   recomputation of the same estimator on the resampled series. A deterministic
   counterexample: `x = [0,1,2,3]`, L=2, draws starting at 0 and 2 reproduce
   x itself; observed `B(L)` and recomputed `B*(L)` are 1, but the declared
   `v*` is 2. This does not prove the proposed variant invalid. If the intended
   candidate is instead a bootstrap-t using the same estimator on both
   series, change the definition to `B*(L)` now and record that change before
   diagnostics; do not silently switch at implementation time.
3. **S2's denominator is also a finite-sample approximation.** With q full
   blocks and a final partial block of r bars, the exact conditional variance
   of the bootstrap sum divided by n is
   `(q·L·B(L) + r·B(r))/n`, not generally `B(L)`. Four of the six declared
   n/L combinations have a partial block. Keep the proposed S2 formula if
   that is the candidate being studied, but avoid claiming its ratio exactly
   restores the actual finite-sample variance. This is a clarification, not a
   request to add another candidate or run.
4. **V1 may be selected.** A verified restriction on the tested configurations
   is consistent with the earlier decision to consider sample length as an
   applicability condition. Preserve the original 6.45% result. Passing a
   later supported subset does not erase its failed 256-bar configuration.
5. **Accept the two block rules, one family shape, diagnostic point screen,
   power metric and selection order for this bounded experiment.** The
   result must retain these restrictions. The single net-return power metric
   is an explicitly chosen selection metric, not evidence of every kind of
   power or a minimum power guarantee.
6. **Accept the declared confidence-bound rule and phase separation.** Keep
   the diagnostic seed reusable only for declared diagnostics; freeze the
   chosen method/rule before spending the final seed. The 95% interval uses
   z=1.96 (the upper end of a two-sided 95% Wilson interval), not a one-sided
   95% z=1.645 bound. Keep the documented convention and thresholds. These
   are per-check intervals; the approximately 99% pass probability is for
   one confirmation check at nominal, not a claim for the entire grid.
7. **Accept the cost workflow.** Measure release cost in P12e-5 before the
   diagnostic matrix, retain the matrix cap and v1 normal-suite runs, and
   declare reproducible prefixes before committing reports. The CI suite
   totals cannot be converted into a guaranteed release runtime. Neither
   S1/S2 success nor wall-clock cost has been verified by this plan review.

Candidate draft contracts must settle zero/negative variance estimates,
partial blocks, summation order, signs and comparison arithmetic before
their first diagnostic run, as §4 already requires. The v2 engine needs wide
integer arithmetic for the Wilson comparison; intermediate products at
N=20,000 already exceed u64. These implementation details are appropriately
left to P12e-5/P12e-6, not grounds to start implementing them in this PR.

Also keep the distinction between nominal alpha (2.5% / 5%) and the accepted
simulation limits (3% / 6%). Meeting this calibration criterion is not proof
of exact nominal-alpha control on arbitrary returns.

## Verification

Independent deterministic arithmetic checks, without calibration runs:

- Evaluated every x from 0 through 20,000 for both limits. The BigInt rule
  matches the floating-point Wilson upper-bound formula on all 40,002
  checks. Maximum accepted counts: **552** and **1134**.
- Wilson upper bounds at 552/553: **2.9962797% / 3.0014783%**; at
  1134/1135: **5.9991173% / 6.0042491%**.
- Binomial probability summation, not simulation: a single confirmation
  passes with probability **99.0511%** at p=2.5%, **2.3384%** at p=3%.
  The family check passes with probability **99.9991%** at p=5%,
  **2.4833%** at p=6%. The plan's approximate descriptions are reasonable.
- Block tables, all six declared power shifts and matrix size units
  (168, at most 168, 140) reproduce. Flat-top lag weights and the deterministic
  S1 estimator example above were checked algebraically/numerically.
- Formula source:
  [NIST Wilson confidence intervals](https://www.itl.nist.gov/div898/handbook/prc/section2/prc241.htm).
  The candidate discussion does not infer finite-sample efficacy from general
  block-bootstrap or flat-top-kernel literature.
- All changed files are Markdown; all relative Markdown file targets exist;
  `git diff --check 1764c48..HEAD` passes.
- [CI run 37011136261](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37011136261):
  all six exact-head jobs pass; logs confirm **535 Rust passed (137 + 396 +
  2), 1 ignored**, and **1019 Vitest passed (59 files)**. This run's library
  and desktop totals are 14.86 s / 88.92 s, consistent with reporting suite
  timings as variable observations rather than fixed simulation cost.

No product source, fixture or test was edited; no calibration grid,
acceptance seed or new statistic was run. Only this review note and its
task-board record were added. No GitHub review submission, commit, push or
merge was performed.
