# Hard Gate Contract (GATE-001)

Status: adopted Phase B foundation, 2026-07-19.

## Purpose

`alpha-factor-forge/src/services/gate.ts` implements `STRATEGY_DISCOVERY.md` §5.1: the hard elimination gate every candidate must fully pass before Score/ranking ever sees it. It is a pure judgment — it runs no backtests and computes no new strategy behavior; the caller supplies the candidate's segment result plus the complete §6 benchmark outputs for the same candles × segment.

The intended input is the **Validation** segment result. The hidden Test segment must never be fed through ranking-time gates.

## V1 decisions

- Thresholds are explicit configuration (`GateConfig`); `DEFAULT_GATE_CONFIG` records the §5.1 defaults: `minTrades` 30, `minAvgTradeReturn` 0 (strict `>`), `minRollingPositiveRatio` 0.55, `maxDrawdown` 0.35, `maxMonthlyContribution` 0.40, `maxSingleTradeContribution` 0.25, `minRandomEntryPercentile` 95.
- `rollingWindowBars` (default 30, step 1 over the segment equity curve) is a recorded v1 convention — the doc fixes the 55% ratio but not the window length. A window is positive only when its end equity is strictly above its start.
- Concentration criteria attribute the candidate's closed-trade `pnl` (already cost-inclusive): monthly by UTC `YYYY-MM` of `exitTime`, per-trade individually, each as a fraction of total profit.
- Benchmark wins require the candidate's `netReturn` to be STRICTLY greater than each of the four deterministic §6 benchmarks; ties lose. The Random Entry criterion consumes the BENCH-002 percentile (fraction of runs strictly beaten).
- The verdict lists every criterion in the fixed §5.1 order with observed value, threshold, and pass flag, plus the exact config judged with — record the whole verdict for reproducibility.
- Missing evidence never passes (fails closed with `value: null`): an equity curve shorter than one rolling window, or a non-positive total profit (concentration is then unverifiable).
- Structural problems throw (`RangeError`): a missing deterministic benchmark or an invalid threshold configuration.

## Non-goals

- Score, weights, penalties, ranking order (§5.2) — later slices.
- Segment-length-adjusted `minTrades` (the doc's 「依區段長度調整」) — deferred; callers may override the threshold explicitly.
- Persistence, UI, lifecycle transitions, Rust integration.
