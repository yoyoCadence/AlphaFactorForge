# P13c-4b-2c-4a original market registry producer projections v1

Date: 2026-10-28 (UTC+8)
Baseline: verified #233 (`841e5dd`). Design/data only.

The [ordered original semantic contract](research-original-semantic-adapters-v1.md#2-ordered-implementation-slices)
requires independent original-format evidence before each semantic adapter.
Warmup3 is delivered. Market4 starts with calendar/instrument registry evidence,
then provenance/snapshot evidence, then a bounded whole-market consumer design
and implementation. Trial follows market; file/candle/availability4b-3 and final
actual-owner/registry4d retain their own gates. This dependency refinement does
not start a source builder or change the existing product order.

## 1. Exact original producer boundary

The unchanged [registry writer](../../alpha-factor-forge/src-tauri/src/market/registry.rs)
and [foundation types/rules](../../alpha-factor-forge/src-tauri/src/discovery_core/market_foundation.rs)
define the supported historical inputs. These controls characterize those
writers, including their permissive details; they do not strengthen validation
or certify real exchange calendars, costs, corporate actions or trading rules.

| Producer | Exact original content and identity | Retained scalar projection |
| --- | --- | --- |
| register_calendar | Serialize the six SessionCalendar fields using camelCase, add version=session-calendar-v1. SHA256 of the existing research canonical JSON bytes, with recursively sorted object keys and preserved array order. No domain prefix. definition_json stores those same canonical bytes. | calendar_id, version, kind, timezone, content_hash. ID/created_at are storage metadata, excluded from content. |
| register_instrument | Serialize all thirteen InstrumentDraft fields using camelCase, including null optionals, suspensions and sourceCapabilities; add version=market-instrument-v1. SHA256 of the existing research canonical JSON, no prefix. content_json stores serde_json text of that Value (the current sorted map yields the same text for these literals). | instrument_id, version, parsed market/venue/symbol, base, quote, asset_type, session_calendar_id, timezone, the three optional SQLite REALs, listed_from/delisted_at, suspensions_json, source_capabilities_json, content_hash. ID/revision/created_at are excluded from content. |

Use [research canonical_json](../../alpha-factor-forge/src-tauri/src/research/mod.rs),
not identity::canonical_bytes or a strategy/dataset identity prefix. Key ordering
and UTF-8 bytes matter. Null options are not omitted. Preserve numeric REAL bits
and nested array order. Do not compare a lossy display DTO: the captured
[original graph rows](../../alpha-factor-forge/src-tauri/src/research/confirmation_inputs/source_rows/graph/rows.rs)
retain raw JSON and REAL bits precisely because registry readers can default
unknown enums or malformed projections.

## 2. Validation and storage behavior to preserve

Calendar ids obey the existing lowercase/digit segments plus -v<digits> rule;
leading-zero version digits are accepted. Unknown enum kinds refuse during
typed decoding. timezone.trim() must be nonempty; the original timezone text
is retained, with no timezone database lookup or whitespace normalization.
Continuous forbids tradingWeekdays/holidays but allows valid earlyCloses.
Trading-days requires weekdays, sorted unique ISO1..7. Holidays and earlyCloses
each require valid YYYY-MM-DD dates and strict independent ordering. No new
cross-list disjointness or “real holiday” claim follows from these checks.

Instrument id parsing preserves symbol case and USDT. It validates shape,
supported market, lowercase/digit/hyphen venue and ASCII alphanumeric/dot/hyphen
symbol. Asset type must match crypto→spot-crypto or us-etf/tw-etf→etf. Base,
quote and timezone must be nonempty without surrounding whitespace. Known
lotSize/priceStep/minNotional must be finite and >0; null remains unknown.
Delisting must exceed listing only when both exist. Suspensions have positive
extent and sorted nonoverlap; adjacent and negative intervals are allowed.
sourceCapabilities must be an object. The referenced calendar must exist with
continuous kind for spot-crypto or trading-days for ETF. The writer does not
require timezone equality, infer base/quote from symbol, or require suspension
bounds to match listing. Preserve these boundaries without granting admission.

Calendar id is immutable: exact retry returns its row/false; changed content
under that id refuses. Instrument identity is the entire draft hash: exact
retry returns the old row/false even after a newer revision exists; distinct
content creates MAX(revision)+1 for that instrument id. A historical consumer
must bind the exact captured row/revision, never latest_instrument. Append-only
SQL triggers preserve older rows; no producer helper should bypass them.

## 3. Independent frozen literals

The unconsumed [projection fixture](../../alpha-factor-forge/fixtures/research/original-market-registry-projections-v1.json)
contains four positive calendars, five positive instrument drafts, fourteen
calendar and nineteen instrument refusals, plus exact retry/conflict/revision
sequences. It is authored with standalone Python json/hashlib/struct, without
importing or executing project code. Its positive instrument numbers are null
or small dyadic fractions, avoiding an undeclared JSON numeric policy. Separate
Node built-ins independently check canonical bytes/hashes, UTF-8, REAL bits,
all rule phases, scalar projections and sequence identities.

Inputs are exact inputJson strings of typed drafts, not claimed stored originals.
expected definitionJson/contentJson, contentHash, scalars and realBits are fixed
literals for future actual-writer comparisons. Four calendar registrations in
a fresh migrated in-memory DB precede instrument controls. Calendar sequence
pins row1 retry, rows2..4, row2 retry, then a coherent changed-timezone conflict
under calendar0's id; count stays4. Instrument sequence pins eight calls,
including revision1 retry after revision2, and five final rows. BTC revisions
are1/2; all other instruments start at1. Compare raw SQL storage, not just DTOs.

The fixture-us/fixture-tw calendars and all trading specifications are synthetic
controls. The continuous-v01/space-timezone/early-close and Unicode-base/caseful
symbol/adjacent-negative-suspension cases deliberately pin real writer
permissiveness. No fixture represents authenticated market facts.

Fixture outer UTF-8 packaging SHA256 after physical CRLF→LF normalization:
`ea6ce3df35814450641d1e41e1997f039322a6c86464660a525fbd2532b90c92`.
This packaging rule never normalizes decoded inputJson/contentJson/definitionJson
bytes. The fixture separately pins four original source files at the full
baseline commit; a later test declaration must preserve those production bodies.

The [numeric audit](../numeric-json-identity-audit-v1.md) remains binding:
0.0036944444444444438 has correctly rounded bits3f6e43cfc21ad9fe versus legacy
JSON bits3f6e43cfc21ad9ff. numericBoundary references that existing evidence;
it does not claim register_instrument refuses the number. Future original
consumers must refuse non-reconstructible claims under the existing declared
policy, without silently changing parsers, repairing tokens, accepting two
hashes or rehashing stored history. No float_roundtrip/arbitrary_precision or
new dependency is introduced here.

## 4. Next code slice: actual registry producer controls (4a)

After this design's verified delivery, add one private cfg(test)-only child of
market/registry.rs (e.g. market/registry_projection_controls.rs). Existing writer,
type, hash, parser and display-reader bodies stay byte-for-byte unchanged apart
from that module declaration. No production helper/API extraction is needed.
Scope: test file/declaration, tasks/handoff, and the now-frozen fixture unchanged.

Use a fresh in-memory Connection with foreign_keys enabled and all migrations;
do not call ensure_builtin_calendars before the literal sequence. Decode exact
inputJson with the existing typed serde path. Compare input preservation,
calendar_content/draft.content/hash against frozen literals, then call actual
register_calendar/register_instrument and read raw SQL columns. Assert each
retained scalar/null, all three REAL to_bits, raw JSON text, exact row/revision,
repeat determinism and table counts. The built-in calendar function must equal
calendar0. No file, network, secret, candle, worker or trial spend is needed.

Every refusal has an explicit decode or validate/register phase. Decoding
success is required before asserting writer failures; errorContains pins the
existing stable rule or meaningful message fragment. Assert complete before/
after registry table equality for every refused attempt, including the calendar
conflict. Later SQL UPDATE/DELETE attempts must fail without changing either
table. Exercise direct in-memory NaN/+infinity/-infinity for each optional spec,
since valid JSON cannot encode them; they must refuse before writes. This is
additional producer coverage, not an invented JSON fixture value.

Six temporary actual-source mutants must fail at assertions, then restore exact
bytes: omit calendar version from identity; erase earlyCloses from calendar
content; rename quote USDT→USD in instrument content; omit minNotional from
instrument content; accept a calendar of the wrong kind; return latest revision
for an old-content retry. Compiler failures do not count. Keep failure logs and
restore source before the full suite; no mutant is shipped.

Run full fresh exclusive TEMP/TMP cargo test --locked before Node-heavy checks,
locked all-target check/Clippy, scoped format and frontend checks. All six
completed exact-head/base CI jobs, fresh reviews/main, expected-head merge,
GitHub readback and clean-main/full-tree equality are delivery requirements.
Do not globally format the existing registry file as part of test-only work.

## 5. Remaining market and source work

4a producer conformance is necessary before later registry consumer code.
Provenance/snapshot need separate independent original projections and actual
producer controls, including rejected/comparison/revision sources, full snapshot
source sorting, asOf exclusion and exact frozen instrument revision. They must
retain their own availability/candle/file gates; copying the synthetic raw SQL
graph cannot establish compatibility. Freeze their bounded consumer composition
and real complete-operation resource boundaries before implementation. This
document specifies no unchecked public constructor or per-row budget reset.

This PR changes only design/data and lifecycle receipts. Locally verify strict
outer/nested JSON, all independent projections/rules/sequences, source catalogs,
links, append-only receipts, unchanged runtime/older fixtures and git diff --check.
No runtime suites are rerun locally solely for this artifact; six-job design CI
checks unchanged #233 runtime (1114 Rust/one existing ignored,1242 Vitest/80 files,
98 E2E). Green design CI is not future 4a producer/consumer evidence.

Only this design completes on verified delivery. On a later explicit resume,
4a actual controls are next; provenance/snapshot refinement follows. Market4,
trial,4b-2c/4b-2/4b/P13c-4 and all source/live gates remain incomplete.
RegisteredSource/numericCostsHash remain unavailable. Stop after this verified
design merge; no manual user test is needed for this slice.
