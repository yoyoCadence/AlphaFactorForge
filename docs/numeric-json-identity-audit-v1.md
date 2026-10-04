# JSON float round-trip and durable identity audit (NUMERIC-JSON-001)

Date: 2026-10-04. Audit of main `f0459b0`, then rebased onto PR #153.
Status: audit complete; the production parser and identities are unchanged.

## Reproduction and confirmed impact

serde_json is locked at **1.0.150**, without `float_roundtrip`. The decimal
`0.0036944444444444438` is read as `3f6e43cfc21ad9ff`; Rust `str::parse::<f64>`
and JavaScript `JSON.parse` read `3f6e43cfc21ad9fe`. Their one-ULP difference
changes the project's binary canonical encoding and `strategy-v2` SHA-256.
Five control literals in the committed six-case fixture agree in both modes;
this small set does not estimate how often other inputs drift.

For the exact params definition in case `number-3`:

| Interpretation | `strategy-v2` digest |
| --- | --- |
| Frontend / standard Rust / roundtrip-enabled serde_json | `ec10d4148ee58393248db1e28a22056eed60ba0decf922bd830827e8b278aaf6` |
| Current default serde_json | `b317bbc5d4a1d39ce62186dffe173c716a65e58222c9e205bf3a3a5acee1b1a3` |

The test uses the real migrated SQLite repository, not a replacement store:

1. A definition carrying the frontend hash is rejected by
   `insert_verified_strategy` with `identity mismatch`; no row is written.
2. The same text carrying the default backend hash saves successfully.
   `list_strategies` returns the original JSON text unchanged, and
   `verify_strategy_identity` accepts that row under the current parser.
3. The roundtrip-enabled audit example interprets that exact definition with
   the frontend hash. A global feature flip would therefore invalidate an
   existing row whose identity was made with the old interpretation.

This is a **P2 correctness/compatibility finding (NUMERIC-JSON-002)** on legal
numeric identity inputs, with fail-closed saves. It does not establish that
any user's existing records contain this decimal or that a live native bridge
round trip has been tested. No user workspace or registry was inspected.

Separately, canonical research JSON written from the correctly rounded f64
can be read to the other bits; canonicalizing it again changes its checksum.
The real `ArtifactStore::put/read` still verifies and returns the original
bytes successfully. Byte verification and parse/reconstruction are distinct
operations; this finding does not mean existing artifact bytes are corrupt.

## Product boundary inventory

The two hash encodings must be considered separately: `identity::canonical_bytes`
uses exact big-endian f64 bits; `research::canonical_json` uses sorted JSON text.
Both are sensitive to a changed parsed numeric value.

| Boundary | Behavior and exposure | Evidence / applicability |
| --- | --- | --- |
| Manual strategy save | `identity.rs` parses `original_definition_json`, hashes definition plus costs; repository verifies before save | Reproduced rejection for frontend hash and successful legacy save/read, six cases |
| Strategy list/read | `repositories::list_strategies` returns stored original text; later verification reparses it | Original text preserved; old hash depends on the old interpretation |
| Pure strategy enumeration / execution | `discovery_core::identity` hashes a Value; runner serializes candidate definitions and repository reparses them | A computed value can drift across that boundary; inventory finding, no end-to-end runner failure asserted here |
| Discovery resume | `discovery_runner::resume` parses `run.config_json` and re-enumerates candidates; `run_lineage` hashes canonical config JSON | Float axes, strategy values and costs are exposed to interpretation changes; existing config/hash compatibility must be preserved |
| Command idempotency | loopback API parses body, `runtime::commands::payload_hash` hashes canonical JSON; stored payloads/responses are reparsed | Float-bearing payload/retry equality can change with parser policy; a body-byte checksum is not what this path uses |
| Hypothesis content | `HypothesisDraft::hash` uses canonical JSON; `register_hypothesis` serializes `applicability`, `get_hypothesis` parses it | Free-form float applicability can reconstruct differently; no observed user-row incident claimed |
| Campaign declarations | `freeze_campaign` strictly rebuilds a declaration of bounded integers, strings and nulls; DB canonicalizes, parses and re-freezes | Current declared numeric fields are integers; no direct long-float declaration exposure. Costs are separate admission f64 fields |
| Market instruments / actions / cost profiles | instrument identity hashes typed spec floats and free-form source capabilities; ingestion hashes action and cost JSON | Float-bearing creation/reconstruction needs policy review. Stored hash strings do not become wrong merely because they are read |
| Dataset identity | binary encoding hashes typed Candle f64 values; SQLite REAL reads preserve those values; JSON transports can parse before hashing | SQLite storage itself does not introduce a JSON step. A transport parse may change values or cause identity refusal; native transport not exercised by this audit |
| Snapshot authority | `campaign_snapshot::resolve` reconstructs snapshot content from integer/string metadata and opaque dataset/instrument/source hashes | Reconstructed snapshot metadata has no direct f64 field today; underlying component identities retain their own exposure |
| Trial ledger / transfer / binding | family/protocol/event identities mostly use strings and bounded integer counts; original identity hashes are opaque; exports/imports parse records | Current counting/binding records have no long-float field. Do not rewrite historical params/config hashes or reclassify events after a parser change |
| Artifact/raw source integrity | artifact store hashes exact file bytes; provenance hashes original response bytes; parsed reports may contain floats | Real artifact byte read verified in the regression. A later parse-and-rehash operation can change reconstructed content without changing original bytes |
| Statistical fixtures / simulator | fixture inputs can contain JSON floats; simulation/calibration declarations and acceptance reports have separate evidence ownership | Audit example only; no established fixture, algorithm, report or full acceptance was regenerated or rerun |

Source entry points: `src-tauri/src/identity.rs`, `discovery_core/identity.rs`,
`db/repositories.rs`, `discovery_runner/mod.rs` and `execution.rs`,
`runtime/control_api.rs` and `commands.rs`, `research/history.rs`,
`research/artifacts.rs`, `discovery_core/campaign.rs`, `db/campaign.rs`,
`market/registry.rs`, `market/{ingest,tiingo_ingest,tw_etf_ingest}.rs`,
`research/campaign_snapshot.rs` and `research/trial_ledger*.rs`.
The inventory records conditional exposure, not an executable failure at
every listed boundary. The confirmed product reproductions are those above.

## Isolated feature comparison and timing

The [upstream feature definition](https://github.com/serde-rs/json/blob/v1.0.150/Cargo.toml#L59)
describes `float_roundtrip` as preserving f64 through JSON, with more parsing
work; it differs from `arbitrary_precision`, which preserves number text.
The locked [parser implementation](https://github.com/serde-rs/json/blob/v1.0.150/src/de.rs)
has separate feature-gated paths. Enabling it for one audit invocation does
not edit the production Cargo manifest or lockfile.

From `alpha-factor-forge`:

```powershell
npx --no-install vite-node scripts/audit-json-floats.ts
cd src-tauri
$env:CARGO_TARGET_DIR='C:/tmp/aff-target'
cargo build --release --locked --example numeric_json_audit
# Run the built example and capture stdout as numeric-json-default-output.json.
cargo build --release --locked --example numeric_json_audit --features serde_json/float_roundtrip
# Run the built example and capture stdout as numeric-json-roundtrip-output.json.
```

In all six cases the feature-enabled parser agrees with JavaScript and
standard Rust bits, canonical bytes and strategy identities; the feature's
serialize/read result also preserves the intended identity. Default parsing
differs in case `number-3`. The reports are committed independently and tests
compare their identity evidence; elapsed times are not test thresholds.

The same 12,288-number array, 200 Value parses/sample, three release samples:

| Build | Sample times (ms) | Median (ms) |
| --- | --- | --- |
| Default | 47.042, 46.178, 46.049 | 46.178 |
| `float_roundtrip` | 79.839, 66.184, 66.737 | 66.737 |

Median ratio **1.445×** on this machine/document, approximately +45%.
Construction and printing are excluded; Value allocation is included.
This is a microbenchmark, not a desktop latency guarantee or universal
parser cost. No parallel default/feature runs were timed against each other.

## Proposed repair, requiring a separate compatibility decision

Do not adopt a global feature change as an unversioned fix. Cargo features
are [unified for the dependency](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification):
enabling the feature changes its runtime callers in that build, including
reads of old definition/config JSON.

Recommended next bounded design slice (NUMERIC-JSON-002):

1. Define a correctly rounded parser policy for **new** documents and bind it
   to versioned identities/configs. Decide how the old interpretation is
   implemented for legacy verification; merely accepting both hashes after
   reparsing loses which numerical inputs were executed.
2. Retain original JSON, old hash, attempts, event links and artifact bytes.
   Classify old records read-only as verified under their declared policy or
   unresolved. Never silently rehash frozen records or change their lineage.
3. Require an explicit, additive re-freeze/new identity for any selected
   conversion, with source lineage and the resolved numeric policy. Old
   execution evidence remains associated with its original identity.
4. Before enabling new runtime writes, cover frontend save → DB read,
   runner candidate admission/resume, idempotent retries and artifact replay
   with adversarial decimal cases under both policies. Review statistical
   fixture/calibration compatibility separately before changing those reads.

An alternative is to keep the parser and explicitly reject non-roundtripping
inputs before new writes, while exposing why; this avoids silent old rehashes
but leaves frontend/backend numeric parity unresolved. Returning a replacement
backend hash alone is insufficient because it silently changes the candidate
the frontend intended. No migration or production repair is implemented here.

## Evidence

- `fixtures/research/numeric-json-audit-v1.json`: literals as strings, exact
  definition text, independent frontend bits/canonical bytes/strategy hashes.
- `numeric-json-default-output.json` and `numeric-json-roundtrip-output.json`:
  release probe reports plus timings.
- `src/parity/numericJsonAudit.test.ts`: frontend identity and both report modes.
- `src-tauri/src/identity/numeric_json_audit_tests.rs`: real backend encoding,
  verified SQLite save/read, changed reconstructed identity, exact artifact read.
- `src-tauri/examples/numeric_json_audit.rs`: repeatable isolated comparison.

The default-parser assertions characterize the known legacy behavior and must
be replaced deliberately by the repair's legacy-policy tests; they are not a
claim that lossy behavior is the desired new contract.
