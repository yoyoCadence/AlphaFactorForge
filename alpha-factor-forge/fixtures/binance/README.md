# Binance archive fixtures

Real files from the public Binance archive, kept byte for byte so the source
adapter's tests read what the publisher actually sends rather than something
we imagined it sends. They are small on purpose: one day of hourly bars each.

Retrieved 2026-09-20 from `https://data.binance.vision/data/spot/daily/klines/<symbol>/1h/<file>`,
each with its published `<file>.CHECKSUM` beside it.

| File | Bars | Source time unit | Published SHA-256 |
| --- | --- | --- | --- |
| `BTCUSDT-1h-2024-07-15.zip` | 24 | milliseconds | `e6fbeb74…4e0d` |
| `ETHUSDT-1h-2024-07-15.zip` | 24 | milliseconds | `94a8a961…de9c` |
| `BTCUSDT-1h-2026-09-18.zip` | 24 | **microseconds** | `070dd77b…2324` |
| `ETHUSDT-1h-2026-09-18.zip` | 24 | **microseconds** | `f796443a…e87f` |

The two dates are on either side of the change `docs/market-contract.md` §4
names: the archive published millisecond timestamps in 2024 and microsecond
timestamps from 2025 onwards. This was verified against the real monthly
files while implementing P07 — `BTCUSDT-1h-2024-12` is in milliseconds and
`BTCUSDT-1h-2025-01` is in microseconds — and these two days are the smallest
committed evidence of it.

The `.CHECKSUM` files are the publisher's own, unedited. The tests use them
as the digests they are: a fixture whose checksum we computed ourselves would
only prove that our own hashing is self-consistent.

These files are test input. They are **not** a dataset, they are never
imported by the application, and nothing here claims anything about coverage
beyond these two days.
