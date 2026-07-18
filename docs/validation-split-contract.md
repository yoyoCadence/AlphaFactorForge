# Validation Split Contract (VAL-001)

Status: adopted Phase B foundation, 2026-07-16.

## Purpose

`alpha-factor-forge/src/core/validation/split.ts` defines the deterministic v1 bar-index contract for the time-ordered Train, Validation, and hidden Test segments. It is pure TypeScript and intentionally has no UI, IO, persistence, ranking, prompt, or discovery-runner behavior.

The ranges use zero-based inclusive `from` / `to` indexes so a future caller can pass them to the existing backtest boundary contract without translating endpoints.

## V1 decisions

- Input candles are already ordered oldest to newest; this module operates only on their count.
- V1 ratios are fixed at Train 60%, Validation 20%, and Test 20%.
- The caller supplies one explicit non-negative integer `embargoBars`. The same gap size is placed between Train/Validation and Validation/Test.
- Two embargo gaps are removed first. The remaining usable bars are allocated 60/20/20.
- Allocation uses the largest-remainder method. Equal fractional remainders are resolved in Train, Validation, Test order. The implementation uses exact integer `3:1:1` quotient/remainder arithmetic, so every accepted safe-integer input avoids floating-point ratio drift.
- Every source bar belongs to exactly one evaluated segment or one embargo gap. Ranges are ascending and never overlap.
- Test is always the final evaluated range. This slice does not expose Test to generation, tuning, ranking, prompts, or UI.
- Invalid input and fewer than five usable bars fail closed with `RangeError`; the planner never shortens an embargo or returns an empty segment.

The planner does not calculate the embargo size. A future discovery configuration must derive it explicitly, for example from the strategy's maximum indicator lookback plus an approved holding-period allowance, and record that value for reproducibility.

## Embargo derivation (VAL-003)

`alpha-factor-forge/src/services/embargo.ts` implements that derivation: `deriveEmbargoBars(strat, holdingAllowanceBars)` returns `embargoBars = maxSignalLookbackBars + holdingAllowanceBars` plus the recordable breakdown.

- Usage-aware: only the indicators the strategy's active-mode entry/exit signals actually reference count (params signal ids, blocks rule operands, or the code expressions' validated ASTs). Unused configured periods never inflate the embargo.
- Lookback conventions (bars of history; first defined output index is `lookback - 1`, matching `core/indicators` warm-up): `sma(p)`/`ema(p)`/`bbands(p)` → `p` (the EMA seed convention is recorded and accepted); `rsi(p)` → `p + 1`; MACD line → `max(fast, slow)`; MACD signal/hist → `max(fast, slow) + signalPeriod - 1`; raw price/volume series → 1; `prev`/`crossUp`/`crossDown` add one bar because they read bar `i - 1`.
- `holdingAllowanceBars` remains a caller-approved explicit value (0 must be stated, never implied) covering trades that could span a segment boundary.
- Fail closed: unsupported `stoch*` signals, invalid code expressions, non-positive used periods, and a negative or non-integer allowance throw instead of guessing.
- The returned breakdown must be recorded alongside the run (a later persistence slice owns where).

## Non-goals

- Replacing the existing two-way, user-adjustable Holdout display/sweep split.
- Walk-forward windows.
- Gate, Score, benchmarks, lifecycle, duplicate reuse, or hidden-Test reveal policy.
- SQLite or Tauri command changes.
- Rust discovery-runner integration or parity. That belongs in a later task when the backend runner consumes this contract.
