# Crypto listing registration v1

Date: 2026-10-07
Status: Approved repair scope; implementation in progress.
Task: CAMPAIGN-LISTING-001, authorized by the maintainer's request to correct the acceptance finding.

## Problem and bounded interface

The official Binance fetch can create a usable BTC/ETH snapshot whose instrument
has no listing date. Campaign authoring correctly refuses that snapshot, but
there was no operator entry to register evidence-backed listing metadata.

Add `alpha-factor-forge-service register-crypto-listing --settings <json-file>
[--data-dir <workspace>]`. It returns a JSON receipt and follows the existing
bounded CLI ownership lifecycle; stop an existing owner before registering.
This slice supports the adapter's existing Binance BTCUSDT/ETHUSDT defaults.
It adds no DB schema, provider credential, UI metadata editor or automatic
listing-date inference. Existing unknown-listing refusal remains authoritative.

## Settings and evidence

The strict, versioned settings document has `version` =
`crypto-listing-registration-v1`, `instrumentId`, `listedFrom`, `sourceUrl`,
`retrievedAt`, `rawFile`, `rawSha256`, `mediaType`, `operatorConfirmed` and
`evidenceExcerpt`. Unknown or duplicate fields are refused. Times are canonical
UTC RFC3339, representable as integer milliseconds, not future; retrieval cannot
precede the listing assertion. Listing time must fit the market timestamp range.
The source URL must be an HTTPS Binance support announcement with no credentials,
query, fragment or nonstandard port. The lower-case SHA-256 must match the actual
raw file bytes. Resolve relative rawFile paths against the settings directory.

Read bounded settings (32 KiB) and UTF-8 source evidence (64 KiB) before opening
the workspace. Supported source media types are text/plain, text/html and
application/json. Source capture may be an operator-reviewed excerpt: preserve
the supplied original bytes without trimming or newline normalization, and do
not claim the file is an authenticated full HTTP response. The nonempty excerpt
must occur verbatim in the supplied source and identify the declared pair.
`operatorConfirmed` must be true. The operator is attesting that the source
supports the declared UTC listing time; a checksum proves content integrity, not
the truth of that assertion. The CLI does not parse arbitrary announcement prose
into dates or infer a date from candles.

## Persistence and compatibility

Use the existing owner epoch and immediate SQLite transaction. Read the latest
instrument inside that transaction, retain all other metadata and capabilities,
and append the bounded evidence at `sourceCapabilities.listingRegistration`.
It includes version, instrument/listing assertion, URL, retrieval time, media
type, excerpt, confirmation and original UTF-8 content/byte length/SHA-256. The
existing instrument canonical content hash covers the complete evidence; no
fake candle interval or artifact reference is needed. Inline originals stay
within the evidence size bound.

An absent supported instrument starts from its existing default draft. Unknown
listing becomes the declared date. A different already-known listing is refused
and requires a separately scoped correction contract. Identical metadata is
idempotent against the latest revision and returns the same row. A repeat whose
complete content matches a historical revision is refused: it cannot silently
reactivate that old row while re-fetch would still select the latest revision.
Different evidence for the same date may append a revision; no old row is
rewritten. Existing reserved evidence must be
valid before replacing it. Bump workspace domain mutation sequence only for an
actual insertion, retaining the existing transaction/ownership checks.

Re-fetch after registration to build a new snapshot pinned to the new revision.
Existing snapshots, campaigns, hashes and their unknown-listing refusals remain
unchanged. Fetch already preserves a richer latest revision rather than replacing
it with the original default. A registration receipt distinguishes new revision
from exact retry and reports row/revision/content hash plus source checksum.

Invalid settings or source content refuse before acquiring the workspace;
transactional failures write no instrument metadata. Workspace acquisition may
perform the existing ownership/recovery lifecycle, so do not promise that every
domain refusal leaves the entire database byte-identical.

## Acceptance

- Strict input/source bounds, date/URL/hash/excerpt/confirmation validation,
  duplicate/unknown fields and unsupported pair rejection.
- Real migrated SQLite append/idempotence, preserved trading metadata, refused
  conflicting known listing, source-byte round trip and owner-held refusal.
- Old instrument/snapshot remain unchanged; a formal re-fetch pins a new
  instrument revision and produces a different snapshot identity.
- Isolated official CLI registration/fetch, then actual native UI add/preview/
  freeze/start, bounded discovery completion, admission/history inspection and
  process restart/reopen. Use explicit smoke costs and a frozen one-candidate
  strategy; no tuning or research PASS claim.
- Existing frontend/Rust/build checks and final-head CI pass. All owned local
  app/WebView2 processes and temporary directories are cleaned.

## Source used for controlled acceptance

[Binance's USDT market announcement](https://www.binance.com/ru/support/announcement/detail/115001094331)
specifies BTC/USDT and ETH/USDT trading from 2017-08-17 at noon Beijing time.
Converting that stated UTC+8 time gives 2017-08-17T04:00:00Z; it is not derived
from the archive's first candle. Controlled acceptance records an operator-reviewed
short capture of this source. Other operator assertions require their own evidence.

## Operator commands

Review the source and prepare strict settings before registration. The tracked
example is `alpha-factor-forge/fixtures/binance/listing-btcusdt-settings-v1.json`;
its rawFile resolves beside that file, independently of the working directory.
Use an explicitly selected workspace and stop its desktop/service owner first.
From `alpha-factor-forge/`, after building the service binary:

```powershell
& ./src-tauri/target/debug/alpha-factor-forge-service.exe register-crypto-listing `
  --settings ./fixtures/binance/listing-btcusdt-settings-v1.json `
  --data-dir <workspace>

& ./src-tauri/target/debug/alpha-factor-forge-service.exe fetch `
  --instrument crypto:binance:BTCUSDT --interval 1h `
  --from 2025-01-01 --to 2025-02-01 --no-rest `
  --cost-profile <operator-cost-profile> --json --data-dir <workspace>
```

The registration receipt contains `version`, `instrumentId`, `rowId`,
`revision`, `contentHash`, `listedFrom` (UTC milliseconds), `rawSha256` and
`created`. An exact retry returns `created=false`. In the desktop campaign
panel, select the new snapshot from the re-fetch, then preview and freeze it.
Choosing an old snapshot continues to use its old listing metadata.

The separate Windows acceptance harness takes explicit reviewed settings and
source paths. It creates and cleans its own temporary workspace, registry and
WebView2 profile; it downloads a bounded public archive and exercises the real
native UI twice, before and after process restart. It is an operator smoke,
not part of CI's offline fixtures or a statistical confirmation run:

```powershell
& ./scripts/native-campaign-smoke.ps1 `
  -Settings ./fixtures/binance/listing-btcusdt-settings-v1.json `
  -SourceFile ./fixtures/binance/listing-usdt-announcement-excerpt.txt
```
