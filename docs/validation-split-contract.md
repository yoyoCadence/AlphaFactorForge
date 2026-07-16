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

## Non-goals

- Replacing the existing two-way, user-adjustable Holdout display/sweep split.
- Walk-forward windows.
- Gate, Score, benchmarks, lifecycle, duplicate reuse, or hidden-Test reveal policy.
- SQLite or Tauri command changes.
- Rust discovery-runner integration or parity. That belongs in a later task when the backend runner consumes this contract.
