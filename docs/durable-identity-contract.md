# Durable Identity Contract v2

Status: active for new strategy saves and dataset imports. This contract is the
IDENTITY-001 prerequisite recorded in the PR #66 handoff Resolution.

## Purpose

Discovery reuse must mean the same strategy and the same market data in every
runtime. A display label, time range, database id, import order, or available
browser API is not an identity.

New durable identities therefore use SHA-256 only and carry an algorithm/
encoding version in the stored value:

- `strategy-v2:<64 lowercase hex characters>`
- `dataset-content-v2:<64 lowercase hex characters>`

If SHA-256 is unavailable, creation fails. `ephemeral-fnv1a:*` values are
explicitly non-durable and must never be persisted or used for discovery reuse.

## Shared primitive encoding

Strategy identity uses a type-tagged canonical binary encoding of JSON values:

| Value | Encoding |
| --- | --- |
| null / false / true | one-byte tags `00` / `01` / `02` |
| number | tag `03`, then finite IEEE-754 f64 in big-endian order; `-0` becomes `+0` |
| string | tag `04`, u32 big-endian UTF-8 byte length, then UTF-8 bytes |
| array | tag `05`, u32 item count, then encoded items in array order |
| object | tag `06`, u32 field count, then key/value pairs sorted by UTF-8 key bytes |

Undefined values, unsupported types, non-finite numbers, and lengths beyond u32
fail closed.

## Strategy v2

The SHA-256 preimage is:

1. UTF-8 `strategy-v2`, followed by one NUL byte.
2. Canonical binary encoding of `{ definition, execModel }`.

For the current params/blocks/code persisted definition, `execModel.feePct`
comes from `definition.feePct` and `execModel.slippagePct` comes from
`definition.slipPct`. The Rust save boundary parses `original_definition_json`,
checks that its `mode` matches the row `type`, recomputes the hash, and rejects
legacy or forged values. Display name, provenance, lifecycle, and database ids
are deliberately not part of strategy identity.

## Dataset content v2

Rows are copied, normalized, and sorted by timestamp. The input is rejected if
it is empty, contains duplicate or non-JavaScript-safe integer timestamps, has
non-finite OHLCV values, or supplies blank/untrimmed exchange, symbol, or
interval metadata. `-0` OHLCV values normalize to `+0`.

The SHA-256 preimage is:

1. UTF-8 `dataset-content-v2`, followed by one NUL byte.
2. Length-prefixed field mapping version
   `ohlcv(timestamp:i64-ms,open:f64,high:f64,low:f64,close:f64,volume:f64)-v1`.
3. Length-prefixed UTF-8 exchange, symbol, and interval.
4. Big-endian i64 start and end timestamps, then u32 candle count.
5. For every sorted candle: big-endian i64 timestamp followed by big-endian f64
   open, high, low, close, and volume.

`source` is provenance rather than content identity. However, once a hash is
stored, a re-import must match the complete stored payload, including source;
otherwise it is rejected instead of silently updating provenance.

Rust recomputes the hash and verifies derived bounds/count before any write.
The dataset row and all candles then commit in one SQLite transaction using
strict inserts. Re-importing the identical payload returns the existing id;
any contradictory row or candle payload fails without modifying the database.

## Fixture and legacy policy

`alpha-factor-forge/src/core/hashing/identity-v2.fixture.json` is the committed
TypeScript reference fixture. Both TypeScript and Rust tests must match its
hashes exactly; changing an encoding, version string, or expected hash is a
reviewed contract change.

Existing unversioned rows remain readable for historical results, but they are
not eligible for discovery deduplication or cross-run reuse. Re-saving a
strategy or re-importing a dataset creates/verifies its v2 identity; there is no
silent migration that claims old metadata-only dataset hashes represent candle
content.
