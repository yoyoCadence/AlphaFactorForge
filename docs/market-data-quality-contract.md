# Market-data quality contract (`market-data-quality-v1`)

> Added by `DATA-QUALITY-001` on 2026-08-09. Specification:
> [`improvement-backlog.md` → DATA-QUALITY-001](improvement-backlog.md).
> Adjudicated planning decisions:
> [`../handoffs/2026-08-09-data-quality-001-planning-decisions-v1.md`](../handoffs/2026-08-09-data-quality-001-planning-decisions-v1.md).

Dataset identity (`dataset-content-v2`) proves only that bytes were not altered.
It never proves the bytes describe a possible market. This contract is that
missing semantic gate.

**The dataset hash preimage is unchanged.** The validator is a separate module
invoked at admission, never a step inside identity encoding. `src/core/hashing`
and `src-tauri/src/identity.rs` were not modified by this task, and
`identity-v2.fixture.json` still commits the same hash.

---

## 1. Version and constants

| Name | Value | Meaning |
| --- | --- | --- |
| Contract version | `market-data-quality-v1` | Exported from both runtimes |
| `MIN_MARKET_TIMESTAMP_MS` | `946_684_800_000` | `2000-01-01T00:00:00Z`, **inclusive** |
| `MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE` | `4_102_444_800_000` | `2100-01-01T00:00:00Z`, **exclusive** |

This is a **product plausibility boundary, not a language limit**. The lower
bound rejects the common epoch-seconds-as-milliseconds mistake; the upper bound
rejects microsecond/nanosecond values and implausible future data.

The contract is not persisted to any table. Admission is a gate, not evidence,
so there is no schema or migration change.

## 2. Ordered rules

Evaluation stops at the **first failing candle** and reports that candle's
index, its timestamp, and the rule id. A dataset is admitted whole or rejected
whole — individual bad candles are never dropped, repaired, or re-hashed.

| Order | Rule id | Rejects when |
| --- | --- | --- |
| 1 | `timestamp_not_integer` | not a safe integer (TS) / not integral or beyond `Number.MAX_SAFE_INTEGER` (Rust) |
| 2 | `timestamp_out_of_range` | `timestamp < MIN` or `timestamp >= MAX_EXCLUSIVE` |
| 3 | `timestamp_not_representable` | `new Date(ts).getTime()` not finite / `Utc.timestamp_millis_opt(ts).single()` is `None` |
| 4 | `price_not_positive` | any of open/high/low/close is non-finite or `<= 0` |
| 5 | `volume_negative` | volume is non-finite or `< 0` |
| 6 | `high_below_low` | `high < low` |
| 7 | `ohlc_out_of_range` | `open < low`, `open > high`, `close < low`, or `close > high` |

Volume is **non-negative**, not strictly positive: a zero-volume bar is real
data. A flat bar where `open == high == low == close` is admissible.

### Rule 3 is deliberately unreachable

The product range is strictly inside both JavaScript's `Date` limit (±8.64e15 ms)
and chrono's UTC millisecond range, so nothing that survives rule 2 can fail
rule 3. It stays in the code as defence in depth against a future range change,
and representability is asserted **independently** rather than left as an implied
consequence of the range.

It therefore has no shared fixture row. The two runtimes deliberately disagree
*outside* the product range — `8_500_000_000_000_000` is representable in
JavaScript but not in chrono — which is exactly why representability cannot be a
parity row. Each language unit-tests the predicate directly instead.

## 3. Mount points

One rule set, four call sites. No mount point re-implements or partially applies
the rules.

| # | Site | Behaviour |
| --- | --- | --- |
| 1 | `dbClient.ts` `prepareDatasetImport` | Throws **before** `db.importCandles`, so a rejected import makes no boundary call. Covers the real and `?mock=1` clients through one edit. |
| 2 | `BacktestPanel.tsx` candle-load effect | On failure `setLoadedCandles` is not called, so `candles` stays `NO_CANDLES` and `liveContext` stays `null`; Run/Save/Export remain disabled through the **existing** guard. A zh-TW message names the failing candle and asks the user to re-import. No second disabling mechanism was added. |
| 3 | `repositories.rs` `import_dataset_with_candles` | Validates **before** `conn.transaction()`. That ordering is what makes atomicity provable rather than incidental. |
| 4 | `discovery_runner/mod.rs` `load_verified_dataset` | Validates **after** `verify_dataset_identity`, so a tampered payload still reports the identity mismatch first and the two failure classes stay distinguishable. |

The reported index refers to **normalized (timestamp-sorted)** order at mount
points 1, 3, and 4, because identity normalization runs first there.

## 4. Fail-closed behaviour for data stored before this contract

Invalid stored candles are never automatically repaired, rewritten, dropped,
quarantined, or re-hashed. The dataset fails closed where it is consumed and the
user re-imports:

- `start` — `load_verified_dataset` runs **before** any run row is inserted, so
  `start` returns `Err` and **nothing** is written: no `discovery_runs` row, no
  jobs, no progress, no emitted event, no registered coordinator control.
- `resume` — a paused run stays paused with no run, job, progress, event, or
  coordinator write.
- UI — dataset selection surfaces zh-TW re-import guidance and leaves the result
  actions disabled.

## 5. Known limitation — rule 1 is unreachable at the import mount point

`db::Candle.timestamp` is an `i64`, so a non-integral timestamp cannot exist at
mount point 3, and `identity::normalize_dataset_candles` already rejects
magnitudes above `Number.MAX_SAFE_INTEGER` *before* the quality gate is reached.
The TypeScript side is the same: `normalizeDatasetCandles` runs its own
`Number.isSafeInteger` check first.

Consequently an atomic-import mutation case for rule 1 **cannot** report the
quality rule id — the pre-existing identity rule fires first. The mutation table
keeps the case and asserts the rejection actually observed, together with the
full atomicity guarantee (no rows written, previously imported dataset still
byte-identical). Rule 1 remains reachable and parity-tested through the
validator's own API, where timestamps arrive as arbitrary numbers.

This was discovered during implementation; the specification's acceptance
criterion had assumed rule 1 was reachable at that mount point.

## 6. Cross-language parity

`fixtures/rs-core/market-data-quality-v1.json` holds the shared accept/reject
matrix: one fully valid dataset, one rejection row per reachable rule id, the
four boundary rows at `MIN - 1`, `MIN`, `MAX_EXCLUSIVE - 1`, and
`MAX_EXCLUSIVE`, and the two named audit values (`1704067200` and
`8_500_000_000_000_000`, both `timestamp_out_of_range` because rule 2 precedes
rule 3).

The fixture is **authored from this specification, not recorded from a running
validator** — `src/parity/marketDataQualityFixture.ts` deliberately does not
import either implementation. Regenerate with:

```bash
npm run fixtures:market-data-quality
```

Non-finite inputs travel as `explicit-numeric-status-v1` string tags because JSON
cannot encode them. Every leaf compares exactly; there is no tolerance, because
admission is a classification rather than a computation.

## 7. Explicitly out of scope

- **Interval cadence and gap/continuity validation, and the unknown-interval
  fallback** — that is `INTERVAL-CONTRACT-001`, still an open separate decision.
- Summary/trade bundle invariants — `PERSIST-INVARIANT-001`.
- Any schema, migration, or dependency change.
