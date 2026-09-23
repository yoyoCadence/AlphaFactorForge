# Research precision precheck (`research-precision-v1`)

Status: implemented in P12a (2026-09-23) as a pure Rust contract. It is **not
yet called by any runtime path**: no runner, command, migration, UI or
statistical test uses it today. The later P12 sub-items (trial ledger,
confirmation batch) are its intended callers.

| Concern | Location |
| --- | --- |
| Parser + evaluator | `alpha-factor-forge/src-tauri/src/discovery_core/precision.rs` |
| Authored acceptance | `alpha-factor-forge/fixtures/rs-core/research-precision-v1.json` |
| Plan requirement | `docs/plans/active-plan.md` §4.5 (P12 row in §5) |
| Counterexample source | AlphaBTC `docs/AUTONOMOUS_RESEARCH.md` §4 |

## 1. Question it answers

Before a confirmation batch spends compute, can its declared Monte Carlo /
bootstrap sample count **ever** produce a Holm-adjusted p-value below alpha
for the whole trial family — and estimate that p-value precisely enough to be
worth reading? If not, the plan is `NOT_ELIGIBLE` before it runs, so a
rejection caused by sampling resolution is not misread as "the strategy has no
edge".

It does not run a bootstrap, choose block lengths, compute p-values, spend
alpha across batches, or decide what counts as a trial. Those remain P12/P13
work.

## 2. Plan (strict JSON)

All fields are required; unknown fields are rejected. Every count is a JSON
integer (a float literal such as `1000.0` is rejected) no larger than
`9007199254740991`, so a future TypeScript reader sees the same integers.

| Field | Domain | Meaning |
| --- | --- | --- |
| `contractVersion` | `"research-precision-v1"` | exact |
| `correction` | `"holm"` | only correction in v1 |
| `alphaPpm` | integer `[1, 999999]` | family-wise alpha allocated to **this** confirmation, parts per million (`0.05` = `50000`); not a campaign total to be reused per batch |
| `maxRelativeStandardErrorPpm` | integer `[1, 999999]` | largest accepted relative Monte Carlo standard error at the strictest Holm threshold |
| `priorTrials` | integer `[0, MAX]` | effective trials already recorded in the family |
| `plannedTrials` | integer `[1, MAX]` | trials this batch adds |
| `testsPerTrial` | integer `[1, MAX]` | hypothesis tests per trial (e.g. 2: net vs zero and vs benchmark) |
| `bootstrapSamples` | integer `[1, MAX]` | declared sample count `B` |
| `maxBootstrapSamples` | integer `[1, MAX]` | compute budget cap; `bootstrapSamples` must not exceed it |

Alpha and the error limit are integer ppm so every decision is exact integer
arithmetic (`u128`); no float rounding can move a boundary. No default is
defined for either — a plan must declare them.

Rejection order (part of the contract): not an object → first unknown field
in sorted order → fields in the table order → `bootstrapSamples` above
`maxBootstrapSamples` → family tests `(priorTrials + plannedTrials) ×
testsPerTrial` above `MAX`.

`priorTrials` must come from the trial ledger once it exists. Passing a smaller
number (for example, a fresh database) is exactly the reset the plan forbids;
this contract cannot detect it on its own, which is why the ledger is the next
sub-item.

## 3. Rules

With family size `m = (priorTrials + plannedTrials) × testsPerTrial`, alpha
`α = alphaPpm / 1e6`, error limit `r = maxRelativeStandardErrorPpm / 1e6`:

- **Resolution.** The `(1 + extreme) / (B + 1)` estimator can never return less
  than `1 / (B + 1)`, so Holm's smallest adjusted p is at best
  `min(m, B + 1) / (B + 1)`. Reachable iff `m / (B + 1) ≤ α`, i.e.
  `B ≥ ceil(1e6·m / alphaPpm) − 1`.
- **Precision.** At the strictest Holm threshold `p = α / m`, the estimate's
  relative standard error is `sqrt((1 − p) / (p·B))`. Required
  `B ≥ ceil((1e6·m − alphaPpm) · 1e12 / (alphaPpm · sePpm²))`.
- **Budget.** If the larger requirement exceeds `maxBootstrapSamples`, the cap
  can never satisfy the plan.

Every failing check is reported, in this fixed order:
`bootstrap_resolution_insufficient`, `monte_carlo_precision_insufficient`,
`bootstrap_budget_insufficient`. Any reason ⇒ `NOT_ELIGIBLE`; none ⇒
`ELIGIBLE`. `ELIGIBLE` only means the plan's sampling can support its trial
count; it is not evidence that any candidate passes.

Adding trials never lowers a requirement or improves the best adjusted p
(tested), so growing the family can only make a plan harder to admit.

Assumptions and limits (PR #115 review):

- `sqrt((1 − p) / (p·B))` is the binomial relative standard error of an
  unsmoothed proportion under independent resampling. It is a planning
  quantity, not an exact error description of the `(1 + extreme) / (B + 1)`
  estimator, and it says nothing about bootstrap model bias (block length,
  dependence). It is kept as a conservative pre-run screen.
- `ELIGIBLE` means only that the sampling plan is feasible. It is not evidence
  of statistical power, sufficient sample length, or a valid strategy, and a
  passing precheck must never by itself produce a confirmation `PASS`.
- Alpha and the error limit must be frozen before any confirmation result is
  read. Cross-batch alpha spending is not defined here.
- The public typed API (`evaluate_precision_plan`) re-checks every field
  against the domain in §2 before any arithmetic, so a directly constructed
  out-of-domain plan returns an error instead of overflowing.

## 4. AlphaBTC regression

AlphaBTC's next run counted `49` prior + `24` planned trials with two tests
each and a fixed `1000` samples at α = 0.05:

| `bootstrapSamples` | Best adjusted p | Result (with `r` = 20%) |
| --- | --- | --- |
| 1000 (AlphaBTC) | `146/1001 ≈ 0.145854` | `NOT_ELIGIBLE`: resolution + precision |
| 2918 | `146/2919 ≈ 0.050017` | `NOT_ELIGIBLE`: resolution + precision |
| 2919 | `146/2920 = 0.05` | `NOT_ELIGIBLE`: precision only |
| 72975 | `146/72976 ≈ 0.002001` | `ELIGIBLE` |

2,919 matches AlphaBTC's own note that it "only just reaches 0.05" — and, as
that note warns, reaching the threshold is not adequate precision. The 20%
error limit is a fixture choice, not a product default. The counts are the
AlphaBTC counts as recorded; whether its baselines/diagnostics should have been
in the family is the trial ledger's classification question, not this one.

## 5. Report

`precheck_precision(&Value)` returns (camelCase JSON):

- `contractVersion`, `status` (`ELIGIBLE` / `NOT_ELIGIBLE`), `reasons`.
- `familyTrials`, `familyTests`.
- `minAttainableRawP` and `bestAdjustedP` as exact unreduced
  `{ numerator, denominator }`, plus `bestAdjustedPValue` (one IEEE division
  of two integers below 2^53).
- `requiredSamplesForResolution`, `requiredSamplesForPrecision`; `null` when
  the requirement exceeds `MAX` (the budget reason is then always present).

## 6. Remaining P12 work

Not in this sub-item: the persistent, non-resettable trial ledger and its
trial-kind classification (hypothesis / variant / diagnostic / benchmark /
reproduction), inner walk-forward and sample-length feasibility, campaign
freezing, the block-bootstrap/Holm implementation itself, alpha spending,
and noise-data false-positive simulation. A TypeScript mirror is not needed
until a frontend reads the report; the fixture is language-neutral for it.
