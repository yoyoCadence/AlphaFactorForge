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

## Listing registration example

`listing-usdt-announcement-excerpt.txt` is a short operator-reviewed capture
from the [official USDT market announcement](https://www.binance.com/ru/support/announcement/detail/115001094331),
retrieved 2026-10-07 at 13:43 UTC. It is an excerpt, not an authenticated full
HTTP response. Its stated noon Beijing start converts to
`2017-08-17T04:00:00Z`; no candle was used to infer this date.

`listing-btcusdt-settings-v1.json` is the explicit BTC registration input used
by the isolated native campaign smoke. The same announcement covers ETH,
but each instrument requires its own registration input. The settings bind
the captured UTF-8 bytes (including their final LF) with SHA-256
`63b9c9ff539b5c51b540f02d77c21bf226d53e98af9d354857dbc053e3a6ab80`.
The scoped `.gitattributes` entry prevents checkout newline conversion.
The checksum verifies capture integrity; the operator confirmation attests to
the listing assertion. Review the source before using this example in an
operator workspace. It is never registered automatically by the application.
