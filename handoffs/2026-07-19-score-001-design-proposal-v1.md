# Handoff: SCORE-001 design proposal — §5.2 ranking score (reviewer decisions needed)

Date: 2026-07-19
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/handoff-score-001-proposal
PR: (this handoff PR)
Status: open question — implementation must not start until a Resolution records the decisions below

## Summary

Phase B's validation pipeline is now assembled through GATE-001 (split → segmented backtests → embargo derivation → complete §6 benchmark suite → §5.1 hard gate). The next slice, SCORE-001, implements the `STRATEGY_DISCOVERY.md` §5.2 weighted ranking score for gate-passing candidates. The doc records the formula but not the numeric conventions needed to make it a deterministic pure function. This handoff proposes a complete v1 design; the maintainer asked for reviewer sign-off before implementation (Mode A).

Everything below is proposal only — no product code has been written.

## Required Action / Decision

Answer D1–D5 (accept the recommendation or state an adjustment), then append a `## Resolution`. Implementation follows the Resolution, not this proposal, wherever they differ.

### D1 — Normalization approach (blocks everything else)

Raw §5.2 metrics live on incompatible scales (CAGR ≈ 0.5, Sortino ≈ 2, PF ≈ 1.8, inverse monthly σ unbounded). A raw weighted sum would let the largest-scaled metric silently dominate.

- **Option A (recommended): fixed recorded squashing.** Each component maps to [0, 1] via `clamp01(raw / cap)`-style transforms with explicit caps in `ScoreConfig`. Deterministic absolute score per candidate; same input, same output; caps are conventions but recorded and overridable (same guard-rail spirit as the sweep's `Inf → 99`).
- **Option B: cohort normalization (rank / z-score across tested candidates).** Statistically cleaner for ranking but the score changes as the cohort grows, there is no per-candidate absolute value, and no candidate store exists yet (the validation-run record slice is still pending).

### D2 — RegimeRobustness (w4) handling

「跨牛/熊/盤整一致性」 needs a regime classifier (segmenting the period into bull/bear/sideways) that does not exist yet and is a full slice of its own.

- **Recommended: defer.** Ship the component as a typed placeholder — `raw: null`, `normalized: 0`, default weight 0 — so the interface is stable and a later REGIME-001 slice only fills in the implementation (same deferral pattern as the `stoch*` signals).
- Alternative: implement a minimal classifier now (e.g., SMA-slope regime labels + per-regime return dispersion) — grows this slice well beyond one session.

### D3 — Positive component definitions and caps (Validation-segment metrics only)

| Component | Raw source | Normalization → [0, 1] | Default weight |
|---|---|---|---|
| OOS_CAGR | `metrics.cagr` | `clamp01(cagr / 1.0)` (100%/yr caps; negative → 0) | 1 |
| Sortino | `metrics.sortino` | `clamp01(sortino / 5)` | 1 |
| Calmar | `metrics.calmar` | `clamp01(calmar / 5)`, `Inf → 1` | 1 |
| RegimeRobustness | — | deferred per D2 | 0 |
| ProfitFactor | `metrics.profitFactor` | `clamp01((pf − 1) / 2)` (PF 1 = breakeven → 0; PF ≥ 3 → 1; `Inf → 1`) | 1 |
| Consistency | inverse std-dev of `metrics.monthlyReturns` | `clamp01((1/σ) / 10)`; fewer than 2 months → 0 (fail-closed: no evidence, no points) | 1 |

### D4 — Penalty definitions and caps

| Penalty | Raw source | Normalization → [0, 1] | Default weight |
|---|---|---|---|
| Complexity | params mode: 2 + count of indicators the signals actually use (reuses the VAL-003 usage-aware machinery); blocks: rule count + distinct operands; code: total interpreter AST nodes of both expressions | `clamp01(units / 40)` | 0.5 |
| Turnover | `metrics.turnover` (trades per bar, already computed) | `clamp01(turnover / 0.1)` (one trade per 10 bars = full penalty) | 0.5 |
| DataMining | `testedCombinations` N — a REQUIRED caller input (from sweep/discovery counts; Deflated-Sharpe spirit; no default, 1 must be stated explicitly, same philosophy as the embargo holding allowance) | `clamp01(log10(max(1, N)) / 4)` (N = 1 → 0; 100 → 0.5; 10⁴ → 1) | 1 |

DataMining carries the highest default weight, matching the doc's emphasis on multiple-comparison risk.

### D5 — Interface and discipline rules

```ts
scoreCandidate({
  validationResult,        // Validation-segment BacktestResult
  strat,                   // ParamsStrategy — complexity source
  testedCombinations,      // required, >= 1
  config?,                 // Partial<ScoreConfig>; every cap/weight overridable
}): ScoreBreakdown         // { score, components[], penalties[], config }
// each entry: { id, raw, normalized, weight, contribution } — fully recordable
```

- Pure function; runs no backtests. Invalid config throws (`RangeError`); insufficient evidence zeroes the affected component (fail closed), mirroring GATE-001.
- Test discipline: v1 consumes Validation results only. 「Test 分數只在晉級裁決時計算一次」 belongs to the future hidden-Test reveal flow, not this slice.
- Deliverables on approval: `alpha-factor-forge/src/services/score.ts` + focused tests + `docs/score-contract.md` + board/CHANGELOG updates. Gate→Score orchestration (only score gate-passers) stays with the validation-run/runner slice.

## Review Notes

- Every cap (CAGR 1.0, Sortino/Calmar 5, PF 3, 1/σ 10, complexity 40, turnover 0.1, log10/4) is a proposed convention, not a doc-mandated number. Changing a cap changes rankings — that is expected tuning, and all of them live in `ScoreConfig` with the defaults recorded in the score contract doc.
- §5.2 says weights are UI-adjustable and saved; that UI belongs to Results Explorer, not this slice. The config object is the seam it will use.
- Score is only meaningful for gate-passing candidates; SCORE-001 does not enforce that ordering itself.

## Verification

Proposal only — no code, no tests to run. Current baseline on `main`: 264 vitest + 25 Playwright e2e green (post-GATE-001, PR #60).

## Resolution (added when acted on)

(Reviewer: record D1–D5 decisions here, then implementation may start.)
