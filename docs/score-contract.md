# Ranking Score Contract (SCORE-001)

Status: adopted Phase B foundation, 2026-07-19; params-only TypeScript/Rust structural parity added 2026-07-22. Implements the PR #61 handoff Resolution (`handoffs/2026-07-19-score-001-design-proposal-v1.md`), which overrides the original proposal wherever they differ.

## Purpose

`alpha-factor-forge/src/services/score.ts` computes the `STRATEGY_DISCOVERY.md` §5.2 weighted ranking score for one candidate's **Validation** segment result. It is a pure judgment: it runs no backtests, reads only `ValidationRunResult.validation` (never Train; Test is never executed anywhere in v1), and does not enforce Gate ordering — the future runner only scores candidates whose `GateVerdict.pass` is true.

## Formula (formulaVersion: `score-v1`)

`score = Σ(component contributions) − Σ(penalty contributions)`, UNclamped and always finite. Each entry contributes `weight × normalized` with `normalized ∈ [0, 1]`. Only scores with the same formulaVersion and config are directly comparable; any 0–100 presentation is a UI projection only.

### Components (fixed order)

| id | raw | normalized | default weight |
|---|---|---|---|
| cagr | `metrics.cagr` | `clamp01(cagr / 1.0)` | 1 |
| sortino | `metrics.sortino` | `clamp01(sortino / 5)` | 1 |
| calmar | `metrics.calmar` | `clamp01(calmar / 5)` | 1 |
| regime | — | **deferred placeholder** (`raw: null`, `normalized: null`, contribution 0); any non-zero weight throws until REGIME-001 | 0 |
| profitFactor | `metrics.profitFactor` | `clamp01((pf − 1) / (cap − 1))`, cap 3 | 1 |
| consistency | population σ of finite Validation monthly returns | `1 / (1 + 10σ)`; **≥ 3 finite months required**, else `insufficient` (0); evidence records `monthCount` + `monthlyStdDev` | 1 |

### Penalties (fixed order)

| id | raw | normalized | default weight |
|---|---|---|---|
| complexity | `complexityUnits` (below) | `clamp01(units / 40)` | 0.5 |
| turnover | `metrics.turnover` — a **trade-frequency proxy** (`closedTrades / totalBars`), not notional turnover; evidence records `proxy: closedTrades/totalBars@v1` + counts | `clamp01(turnover / 0.1)` | 0.5 |
| dataMining | `testedCombinations` N | `clamp01(log10(N) / 4)` | 1 |

`complexityUnits = canonicalDecisionNodeCount + activeIndicatorParameterCount + enabledRiskRuleCount`:

- params: each SignalId maps to its canonical operator + two operands/literals (3 nodes per signal).
- blocks: 3 nodes per rule plus AND connectors (`rules.length − 1` per non-empty list).
- code: the safe interpreter's actual AST node count for both expressions.
- Indicator params count only the DISTINCT strategy fields the active entry/exit signals reference (e.g. MA cross → `fastMA`, `slowMA`); enabled SL and TP add 1 each; fee/slippage/size/fill/direction never count.
- Semantically equivalent params/blocks/code MA-cross strategies yield IDENTICAL units (test-locked at 8).

`testedCombinations` is required: the number of unique hypotheses finally considered in the candidate's full search lineage (recomputed after the search completes — running counts are forbidden; cache reuse and duplicates do not double-count; manual one-offs pass 1 explicitly). Sharing the final N within a lineage does not change intra-lineage ranking; this is a heuristic, not a full Deflated Sharpe correction.

## Non-finite and missing evidence (`rawStatus`)

Every entry is JSON-safe (`raw` is a finite number or null) with `rawStatus ∈ finite | positive_infinity | insufficient | invalid | deferred`:

- positive Infinity (legitimate after METRIC-001, e.g. no-downside Sortino) → `normalized = 1`, `raw: null`, status `positive_infinity`.
- NaN / negative Infinity → `invalid`, `normalized = 0`.
- fewer than 3 finite months → `insufficient`, `normalized = 0`.
- the regime placeholder → `deferred`.
- Finite monthly returns use a scale-normalized population-σ calculation so extreme finite inputs do not overflow intermediate sums or squares.
- Negative zero is canonicalized to positive zero at the JSON boundary (raw values, resolved weights, and the final score).

The final score is always finite; the whole breakdown survives `JSON.stringify`/`parse` without information loss.

## Validation and failure semantics

- Caps must be finite and > 0 (`profitFactor` cap > 1); weights finite and ≥ 0; `weights.regime` must be 0 → otherwise `RangeError`.
- Invalid `testedCombinations` → `RangeError`.
- If otherwise finite resolved weights overflow a component/penalty aggregate, scoring throws `RangeError` instead of emitting a non-finite score.
- Unsupported `stoch*` signals or an uncompilable code expression propagate their errors (a strategy that cannot run cannot be scored).

## Non-goals

- Gate→Score orchestration, min-score / top-K promotion policy (later runner slice).
- REGIME-001 regime classifier; UI weight editing (Results Explorer); persistence of breakdowns.
- Hidden-Test one-time scoring (「Test 分數只在晉級裁決時計算一次」) — future reveal flow.
- Rust discovery candidates in v1 remain params-only; blocks/code complexity and the expression interpreter stay TypeScript-only.
