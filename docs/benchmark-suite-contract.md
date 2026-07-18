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

## Random Entry Monte Carlo (BENCH-002)

`alpha-factor-forge/src/services/randomEntry.ts` implements §6's key alpha test: the candidate must beat random entries with the same exposure and holding time, or its return is just beta / time-in-market. Conventions approved 2026-07-19:

- Each simulated run places the candidate's closed-trade COUNT at random non-overlapping positions in the same `[from, to]` segment; holding periods are sampled with replacement from the candidate's own closed-trade `bars` (clamped to >= 1 — a same-bar exit cannot be reproduced by close-fill signals).
- Execution mirrors the deterministic benchmarks: long-only, 100% sizing, `close` fill, no SL/TP, candidate-inherited fee/slippage, real engine runs.
- Deterministic: one `mulberry32(seed)` stream with a fixed consumption order (per run: k durations, then k + 1 gap weights that spread the free bars between trades). The seed is a required explicit input; same input, same output.
- A trade that no longer fits the segment is clipped at the segment end (the engine's EOD settlement closes it); trades with no room left drop. Runs default to 200, explicit and capped at 1000.
- Output is the per-run `netReturn` distribution plus the candidate's percentile — the fraction of runs it STRICTLY beats, 0..100. No pass/fail verdict is rendered here; the ">= 95th percentile" threshold belongs to the Gate slice.
- Fail closed (`RangeError`): empty series or segment, a candidate with zero closed trades, invalid `runs`, or an invalid seed.

## Non-goals

- Gate comparison rules ("must beat all benchmarks", the 95th-percentile threshold), Score, ranking, persistence, UI.
- Rust/discovery-runner integration.
