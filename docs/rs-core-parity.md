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

RS-CORE-002 extends this harness with backtest trades, equity, and metrics. It
must not introduce runner orchestration or execute the hidden Test segment.
