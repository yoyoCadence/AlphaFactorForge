# Benchmark Suite Contract (BENCH-001)

Status: adopted Phase B foundation, 2026-07-18.

## Purpose

`alpha-factor-forge/src/services/benchmarks.ts` produces the deterministic baseline results every candidate strategy must eventually beat on the same candles × segment (`STRATEGY_DISCOVERY.md` §6). It is pure TypeScript with no UI, IO, persistence, ranking, or Gate behavior — this slice only produces per-benchmark metrics; how they are judged belongs to the Gate slice.

## V1 decisions

- Four deterministic benchmarks run in a fixed order: `buyHold`, `smaCross`, `rsiReversion`, `bollingerReversion`.
- Benchmarks inherit the candidate's fee and slippage (legacy percent units) so the comparison is cost-fair, but always run long-only, 100% sizing, `close` fill, and no SL/TP — a benchmark is a baseline, not a tuned strategy.
- `buyHold` enters at the first tested bar's close via hand-built signals and holds until the engine's normal EOD settlement at the segment end.
- `smaCross` is the doc's standard 50/200 MA cross; timeframe-adapted periods are deferred. On segments too short for SMA200 warm-up it simply produces 0 trades; the Gate slice owns how that is judged.
- `rsiReversion` is the textbook RSI 14 with 30/70 thresholds (`rsiOversold` entry, `rsiOverbought` exit).
- `bollingerReversion` buys the lower-band touch and exits on the upper-band touch (period 20, mult 2).
- Segment restriction uses the same inclusive `from`/`to` contract as the engine and the validation split; indicators see the full series for warm-up (the same causal pattern as Holdout/VAL-002).
- An empty candle series fails closed (`RangeError`).

## Non-goals

- Random Entry Monte Carlo (BENCH-002): needs a matched holding-period distribution, run-count, seed, and percentile convention before it can be implemented deterministically.
- Gate comparison rules ("must beat all benchmarks"), Score, ranking, persistence, UI.
- Rust/discovery-runner integration.
