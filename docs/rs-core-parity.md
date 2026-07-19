# RS-CORE cross-language parity

Status: RS-CORE-001 indicator foundation implemented. TypeScript remains the
reference implementation; Rust remains a pure computation library.

## Contract and ownership

- `alpha-factor-forge/src/core/indicators/index.ts` is the indicator semantic
  source of truth for `indicator-v1`.
- `alpha-factor-forge/src/parity/indicatorFixture.ts` builds the deterministic
  reference envelope. `npm run fixtures:indicators` is the only supported way
  to rewrite `fixtures/rs-core/indicators-v1.json`.
- The committed fixture contains its schema and contract versions, candle
  inputs, indicator parameters, expected aligned series, source SHA-256 hashes,
  and tolerance policy. Source text is hashed as canonical UTF-8 with LF line
  endings (`utf8-lf-v1`), so Windows CRLF checkout policy cannot alter fixture
  metadata. `null` represents an exact warm-up `NaN` position; an infinite
  expected value is rejected during generation.
- Exact fields include versions, identifiers, parameters, timestamps, array
  lengths, and warm-up positions. Finite numeric leaves pass when
  `absolute error <= 1e-12` or `relative error <= 1e-10`.
- JSON objects compare structurally. Array order remains significant.

The fixture candles are deterministic synthetic test input only. They are not
discovery evaluation data, and the sample-candle generator is deliberately not
ported into Rust runtime code.

## Rust boundary

`alpha-factor-forge/src-tauri/src/discovery_core/` owns the Rust candle contract
and indicator implementation. The library has no Tauri, SQLite, thread-pool,
event, or UI dependency. Its fixture test reads the same committed JSON and
checks SMA, EMA, WMA, RSI, MACD, true range, ATR, Bollinger Bands, standard
deviation, rolling extrema, and ROC.

`indicator-v1` preserves the current TypeScript details, including aligned
`NaN` warm-up output, SMA-seeded EMA, population standard deviation, and ATR
using that EMA implementation. A semantic or tolerance change requires a
reviewed contract-version bump and regenerated fixture diff.

Parity inputs use finite OHLCV values and positive integer periods. Invalid
zero periods return unusable aligned `NaN` output in Rust; strict candidate and
config rejection is owned by the later configuration slice.

Malformed OHLC arrays fail closed at the Rust boundary instead of allowing
out-of-bounds or implicit missing values. Candidate/config validation remains a
later runner slice.

## Review and extension workflow

1. Change the TypeScript reference and its direct tests intentionally.
2. Bump the affected contract version when semantics change.
3. Run `npm run fixtures:indicators` and review the entire generated diff.
4. Update the pure Rust implementation.
5. Run `npm test`, `npm run typecheck`, `npm run build`, `cargo check --locked`,
   and `cargo test --locked`.

## RS-CORE-002: backtest engine and metrics

`src/parity/backtestFixture.ts` + `npm run fixtures:backtest` own the committed
`backtest-parity-v1` envelope (`fixtures/rs-core/backtest-v1.json`): 20
behaviour cases plus 3 fail-closed config error cases. The case set covers the
`docs/engine-parity-report.md` semantics — long/short/both across close and
nextOpen fills, same-bar exit+entry, `both` simultaneous-signal entry-wins,
gap-aware SL/TP with SL-first ambiguity, fee-budgeted 100% sizing, EOD
settlement, from/to boundaries including the empty boundaries (an empty candle
series and an inverted from/to range that evaluates no bar), zero-trade
metrics, METRIC-001 `+Infinity` Sortino/Calmar/profit-factor statuses, and two
180-day sample cases spanning multiple UTC calendar months with risk exits.
Expected outputs come from the real TypeScript engine; generation-time sanity
invariants fail the build if a scenario stops exercising its target branch.
The error cases are also HELD by the TypeScript reference: generation and the
vitest freshness test both run them against the TS engine and require a
`RangeError` carrying the recorded fragment, and both sides lock the exact
20-case inventory by id.

`discovery_core/backtest.rs` (`backtest-execution-v1`) and
`discovery_core/metrics.rs` (`metrics-v1`) are the pure Rust ports. Their
parity test compares trades (timestamps/side/bars exact; prices and PnL within
the declared tolerance), full equity curves, every metric leaf including exact
non-finite statuses and monthly-return keys, and the exact fail-closed error
fragments. Engine semantics changes require a contract-version bump, a
regenerated reviewed fixture diff, and a matching Rust update.

## RS-CORE-003: params signals, validation split, embargo derivation

`src/parity/signalsSplitFixture.ts` + `npm run fixtures:signals-split` own the
committed `signals-split-parity-v1` envelope
(`fixtures/rs-core/signals-split-v1.json`). Per the Resolution D2, only the
params-mode paths are ported; blocks/code signal building and their lookback
derivation stay TypeScript-only, and the expression interpreter is never
ported.

- 7 signal cases (a hand-verified exact MA-cross index plus one sample case
  per family: MA/EMA cross, price-vs-slow, RSI thresholds, MACD cross,
  Bollinger touch) lock the exact entry/exit boolean arrays, including bar-0
  never signalling and warm-up `NaN` comparing false.
- 9 split cases lock exact integer plans for all five usable-bar residues,
  zero and non-zero embargo, and the JavaScript safe-integer extreme.
- 8 embargo cases lock the derivation breakdowns (per-family lookbacks, the
  holding allowance, usage-awareness of unused periods, and a success case
  landing embargoBars exactly on `MAX_SAFE_INTEGER`).
- 11 error cases (1 signal + 4 split + 6 embargo: unsupported `stoch*`, split
  input/insufficient-bar failures, invalid used period, negative allowance,
  and the safe-integer boundary rejections) are HELD by the TypeScript
  reference — generation and the vitest freshness test execute the real TS
  functions and require the recorded fragment — and Rust rejects the same
  inputs with the same fragments. Derived-lookback additions use PRE-checked
  arithmetic (`safeAdd`, overflow tested before the add) because IEEE-754
  rounding can be cancelled by later subtraction; the TS-only blocks/code
  `macdSignal`/`macdHist` composite is regression-locked in the embargo unit
  tests.
- Every EXPECTED OUTPUT leaf compares EXACTLY (signal booleans; split and
  embargo integers). Inputs still contain floats (OHLCV, RSI thresholds,
  `bbMult`) — exactness is a property of the outputs. Derived embargo
  arithmetic is bounded to the JavaScript safe-integer range on BOTH sides
  (checked in TS via `safeLookback`, in Rust via bounded conversion +
  checked adds), with boundary cases locking a raw period past
  `MAX_SAFE_INTEGER`, a legal period whose derived lookback overflows, an
  allowance sum that overflows, and a success case landing exactly on
  `MAX_SAFE_INTEGER`. Both languages assert the exact success AND error case
  inventories by id.

`discovery_core/signals.rs` (`params-signals-v1`), `split.rs`
(`validation-split-v1`), and `embargo.rs` (`embargo-derivation-v1`) are the
pure Rust ports, all Tauri/SQLite/thread/event/UI-free.

RS-CORE-004 extends this harness with the deterministic benchmarks plus
mulberry32/Random Entry parity. Runner orchestration remains excluded, and the
hidden Test segment is never executed.
