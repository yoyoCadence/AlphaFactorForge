# Research campaign declaration (`research-campaign-declaration-v1`)

Status: P12d-1 declaration primitive. **No runtime caller or admission authority.**

`discovery_core::campaign::freeze_campaign` accepts an explicit JSON declaration
and returns an immutable, validated document and its `campaign_id`. This freezes
the inputs that P12d-2 must check before starting a campaign. It does not create
a run, persist a document, query market snapshots, count trials, or return an
evidence status. A valid declaration can still be infeasible or untrustworthy.
The existing discovery runner and its v1/v2/v3 behavior are unchanged.

## Declaration fields

All fields below are required; all objects reject unknown keys, including
caller-supplied `priorTrials`, result metrics, status, or an alleged campaign ID.
No numeric defaults, string coercion, float literals, or silent trimming apply.
The authored example is
[`research-campaign-declaration-v1.json`](../alpha-factor-forge/fixtures/rs-core/research-campaign-declaration-v1.json).
Its hashes and thresholds are test data, not a production research policy.

| Field | Requirement |
| --- | --- |
| `contractVersion` | `research-campaign-declaration-v1` |
| `contracts` | Exact pins in the table below |
| `sampling` | Four explicit P12a sampling fields; no family counts |
| `instruments` | Between 1 and 128 entries, unique by exact normalized instrument ID |

| `contracts` key | Required value |
| --- | --- |
| `datasetIdentity` | `dataset-content-v2` |
| `marketSnapshot` | `market-snapshot-v1` |
| `execution` | `backtest-execution-v1` |
| `metrics` | `metrics-v2` |
| `split` | `validation-split-v1` |
| `precision` | `research-precision-v1` |
| `trialLedger` | `trial-ledger-v1` |
| `walkForward` | `research-walk-forward-v1` |
| `walkForwardEvidence` | `walk-forward-evidence-v1` |

`sampling` contains `alphaPpm`, `maxRelativeStandardErrorPpm`,
`bootstrapSamples`, and `maxBootstrapSamples`, using the domains and the
samples-within-budget rule of [P12a](research-precision-v1.md). Validation reuses
that parser with synthetic unit counts strictly as a domain check; those counts
are never evaluated, returned, hashed, or used for admission. A small sampling
budget is a valid declaration even if the real family later cannot qualify.
Tests per trial and every prior/planned trial count remain ledger-owned.

Each instrument entry contains exactly:

| Key | Meaning and domain |
| --- | --- |
| `instrumentId` | Existing P06 `<market>:<venue>:<symbol>` parser; no aliases or case folding |
| `listedAtMs` | Inclusive declared listing instant, integer `[0, 2^53−1]` |
| `delistedAtMs` | Required nullable field; null means no delisting declared, otherwise exclusive safe-integer endpoint |
| `snapshotId` | 64 lowercase hexadecimal characters, naming a P06 snapshot |
| `datasetHash` | Existing `dataset-content-v2:` prefix plus 64 lowercase hexadecimal characters |
| `interval` | Existing P06 canonical interval set; no legacy fallback |
| `fromMs`, `toMs` | Inclusive first/last bar-start timestamps, safe nonnegative integers; `listedAtMs ≤ fromMs ≤ toMs < delistedAtMs` when delisted |
| `samplePolicy` | Per-instrument declaration below |

The v1 bound is one snapshot and interval per instrument. Changing interval or
snapshot does not permit a duplicate instrument entry. IDs use durable content
references, not workspace-local row IDs. The snapshot identity transitively
binds its calendar, corporate actions, cost profile, provenance and instrument
revision; **only the future authoritative lookup proves those relationships**.
Listing history and all bindings here remain caller assertions until that lookup.

Each `samplePolicy` contains `minimumTotalBars` (positive safe integer),
`minimumTrainBars`, `foldValidationBars`, `foldCount` (the P12c domains), and
`rationale` (nonblank, trimmed, at most 1024 UTF-8 bytes). Rationale records why
the operator predeclared these lengths; text presence does not prove statistical
sufficiency. No AlphaBTC sample-length threshold is copied or supplied by default.
These are bar counts, not elapsed-time duration estimates; calendar gaps and
actual coverage are resolved from the authoritative snapshot later. Actual
dataset length, strategy lookback and embargo are not caller overrides here.

## Identity and immutability

Sort `instruments` by exact `instrumentId` UTF-8 bytes. Convert the validated
typed declaration to JSON, then encode it with the existing
`discovery_core::identity::canonical_bytes` (type-tagged encoding; object keys
sorted by UTF-8 bytes; all numbers validated as safe integers).

`campaign_id = lowercase_hex(SHA256(UTF8(contractVersion) || 0x00 || canonical_bytes(document)))`

This is a declaration identity, not the checksum of a JSON artifact file.
Whitespace, JSON object order and instrument order do not change it. Membership,
listing range, data range, either binding, interval, sampling policy, sample
lengths or rationale do change it. Unknown protocol pins are rejected instead
of producing an apparently supported identity. The fixture ID, independently
derived using Node's SHA-256 and a separate binary encoder, is
`2ffaed4e40e8cd81c1d8631f802c8b744876f83cfef29e2d7db01db5e5f26c45`.

The returned Rust type has private fields, immutable accessors and no
deserializer. Restoring an exported document must re-run validation and compare
the recomputed ID to its trusted stored reference. Cloning a caller's JSON and
then changing it cannot alter an existing frozen declaration. Persistence and
proof that freezing occurred before exposure are not implemented by this type.

## P12d-2 integration requirements

1. Load each exact snapshot and instrument revision; verify current dataset
   identity, coverage, interval/range, listing history, cost profile and source
   eligibility. Reject demo/degraded/legacy/unknown data. Do not resolve a
   declaration by whichever snapshot happens to be newest.
2. Persist the declaration immutably and bind its identity to the run before
   execution. Freeze the concrete strategy/execution/cost configuration and
   all additional admission policies there; this feasibility declaration is
   not a complete autonomous research protocol.
3. Derive total bars and per-candidate lookback/embargo from verified inputs;
   require the declared minimum total history and every P12c plan/evidence.
   Thresholds must be justified before results, never tuned from Validation/Test.
4. Use current P12b `admissionCount`, never a receipt or caller count, to build
   P12a precision plans. Block unknown pre-P05 history, unknown/quarantined
   families, missing/rolled-back registries and changed protocol bindings.
   Fence the current ledger watermark before acting on admission.
5. P12e/P13 still own statistical confirmation and reveal/alpha consumption.
   Neither this document nor a passing feasibility check can produce `PASS`.

P12d and P12 remain In Progress. No database migration, dependency, frontend,
command, admission endpoint or confirmation execution is part of P12d-1.
