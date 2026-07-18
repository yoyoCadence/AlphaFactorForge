# Handoff: SCORE-001 design proposal — §5.2 ranking score (reviewer decisions needed)

Date: 2026-07-19
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/handoff-score-001-proposal
PR: #61
Status: resolved — D1–D5 decided (see Resolution); SCORE-001 stays blocked until METRIC-001 is completed

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

Date: 2026-07-19. Decider: Codex (reviewer), delivered as a PR #61 comment by @yoyoCadence; transcribed verbatim below. **Implementation authority: this Resolution > the original proposal above, wherever they differ.**

> 結論：方向同意，但不可按提案原文直接實作。

### Mandated execution order

1. Append these D1–D5 decisions to this Resolution (done — this section).
2. After the Resolution is appended, this handoff PR proceeds through normal review/merge.
3. First create and complete the small correctness task **METRIC-001**.
4. **SCORE-001 must not move to In Progress, and product implementation must not start, until METRIC-001 is complete.**
5. SCORE-001 delivers the pure score service only; Gate→Score ordering and min-score / top-K promotion policy belong to the later runner slice.

### Blocking: METRIC-001 (core metrics correctness)

Current `alpha-factor-forge/src/core/metrics/index.ts` is inconsistent with the proposal's assumptions:

- `index.ts:102-105` takes a plain standard deviation of the negative excess-return subset; with a single negative sample `downside = 0`, so Sortino returns 0.
- `index.ts:111` makes Calmar 0 whenever `maxDrawdown === 0`; a positive-CAGR, zero-drawdown candidate gets no Calmar credit.
- `profitFactor` can already be `Infinity` (`index.ts:140`); `JSON.stringify(Infinity)` silently becomes `null`, so a breakdown must never persist non-finite values directly.

METRIC-001 decisions:

- `downsideDeviation = sqrt(mean(min(0, excessReturn)^2))` over ALL bar returns (not the negative subset).
- `downsideDeviation === 0 && meanExcess > 0` → Sortino = `+Infinity`; every other zero-denominator case → 0.
- `maxDrawdown === 0 && cagr > 0` → Calmar = `+Infinity`; every other zero-denominator case → 0.
- Tests must lock: single downside observation, no downside at all, positive CAGR + zero drawdown, and non-positive-return zero-denominator cases.
- Non-finite values in DB/JSON must be represented by an explicit status, never by relying on JSON's implicit `null` conversion.

### D1 — Normalization (final)

Option A adopted: fixed, recorded, deterministic normalization; cohort rank/z-score rejected.

- v1 caps provisionally keep the proposal's values and are stored in full in the resolved `ScoreConfig`.
- The breakdown gains `formulaVersion: score-v1`.
- The score is the UNclamped raw weighted sum: `sum(positive) - sum(penalties)`.
- Only scores with the same formulaVersion/config are directly comparable; a 0–100 presentation may only ever be a UI projection.

### D2 — RegimeRobustness (final)

Deferred to REGIME-001. Fixed placeholder:

```ts
{ raw: null, normalized: null, contribution: 0, status: deferred, weight: 0 }
```

Until REGIME-001 exists, any non-zero regime weight must throw `RangeError` — never silently ignored, and never `normalized = 0` pretending to be a tested-but-poor result.

### D3 — Positive components (final)

CAGR / Sortino / Calmar / ProfitFactor caps provisionally keep the proposal's values, but METRIC-001 must land first:

- CAGR `clamp01(cagr / 1.0)`; Sortino `clamp01(sortino / 5)`; Calmar `clamp01(calmar / 5)`; PF `clamp01((pf - 1) / 2)`.
- A legitimate positive Infinity → `normalized = 1`, but `raw` must be a JSON-safe status.
- NaN / negative Infinity / insufficient evidence → `normalized = 0` recorded as `invalid` / `insufficient`; the final score is always finite.

Consistency REVISED (the proposal's `clamp01((1/σ)/10)` is rejected — it would give every σ ≤ 10% a perfect score):

- Only finite Validation monthly returns; at least **3 months**.
- σ is the population standard deviation.
- `normalized = 1 / (1 + 10 * σ)`.
- Evidence records at least `monthCount` and `monthlyStdDev`.

### D4 — Penalties (final)

Complexity — the proposal's per-mode counting is rejected; adopted instead:

```text
complexityUnits =
  canonicalDecisionNodeCount
  + activeIndicatorParameterCount
  + enabledRiskRuleCount
```

- params: map each SignalId to canonical operator/function + operand/literal nodes.
- blocks: each rule counts operator + operands/literal, plus AND connectors.
- code: the interpreter's actual AST node count.
- Active indicator parameters count only the distinct fields the entry/exit signals actually reference.
- Enabled SL and TP each +1; fee/slippage/size/fill/direction never count.
- Semantically equivalent params/blocks/code MA-cross strategies MUST yield identical units, locked by test.
- Cap 40, weight 0.5 provisional.

Turnover — cap 0.1, weight 0.5 provisional. The contract must state that `metrics.turnover = closedTrades / totalBars` is a trade-frequency proxy, not notional turnover; evidence records the proxy definition version plus closedTradeCount/totalBars where available.

DataMining — N must be a positive safe integer >= 1. N = the number of **unique hypotheses** finally considered in the candidate's full search lineage; cache reuse / duplicates do not double-count. All candidates in a lineage use the FINAL N, recomputed after the search completes; running counts are forbidden (order dependence). Manual one-offs pass N = 1 explicitly. Keep `clamp01(log10(N) / 4)`, weight 1. Evidence records N and basis `lineage-final-unique`. Sharing N within a lineage does not change intra-lineage ranking, only absolute scores / cross-lineage comparison; this is a heuristic and must not be presented as a full Deflated Sharpe correction. The later runner must define the promotion policy.

### D5 — Interface and discipline (final)

- v1 reads only `ValidationRunResult.validation`; Train must not be read; Test must still never be executed or enter ranking.
- The breakdown is fully JSON-safe: every numeric field is a finite number or null.
- Each entry at minimum: `{ id, raw, rawStatus, normalized, weight, contribution, evidence? }`.
- `rawStatus` distinguishes at least: `finite | positive_infinity | insufficient | invalid | deferred`.
- Top level at minimum: `formulaVersion`, `segment: validation`, finite `score`, components, penalties, resolved config, tested-combination evidence.
- Caps must be finite and > 0; weights finite and >= 0; the regime weight may only be 0 until REGIME-001.
- The score service does not pretend to enforce gate ordering; the future runner only calls score for `GateVerdict.pass === true`.

### SCORE-001 acceptance checklist (from the reviewer)

- Deterministic: same input/config → identical breakdown.
- Resolved config fully recorded.
- Invalid caps/weights/N → `RangeError`.
- Non-zero weight on the deferred regime component is rejected.
- Positive Infinity → normalized 1, JSON round-trip preserves the status.
- NaN / negative Infinity / insufficient months fail closed; score always finite.
- Consistency cases: σ = 0, low σ, high σ, and fewer than 3 months.
- params/blocks/code equivalent-strategy complexity parity.
- The same lineage uses the same final N regardless of execution order.
- The Test segment is never read or executed.
