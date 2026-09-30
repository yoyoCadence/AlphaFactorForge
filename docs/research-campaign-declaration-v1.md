# Research campaign declaration (`research-campaign-declaration-v1`)

Status: P12d-1 declaration primitive. **No runtime caller or admission authority.**
P12d-2a adds read-only snapshot resolution and P12d-2b a pure admission
evaluator with the §6.4 ledger fence (sections below); neither has a runtime
caller yet, and neither can produce a confirmation `PASS`.

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

## P12d-2a authoritative snapshot resolution (2026-09-29)

`research::campaign_snapshot::resolve_campaign_snapshots` is a read-only backend
boundary for a previously frozen declaration. For every instrument it loads
the exact `snapshotId`; it never substitutes the newest snapshot. It checks the
P06 snapshot and instrument versions, exact revision/listing period, calendar,
interval, dataset metadata and inclusive first/last bar, then recomputes the
`dataset-content-v2` hash from **all** stored candles and applies the market
data admissibility check. It requires the declared minimum total bars, clean
snapshot coverage with a matching bar count, a non-demo/non-degraded snapshot,
confirmed cost profile, ETF corporate-action version, and complete accepted
primary sources that have not been superseded. Finally it reconstructs the P06
snapshot content hash from those authoritative records and compares it with the
declared ID. A missing or inconsistent binding fails closed.

The returned row IDs and bar count are observations at read time, not a durable
admission result. This slice does not verify raw artifact bytes, numeric
execution costs, P12a/P12c feasibility, current ledger/history, or run links.
P12d-2b must repeat verification in its admission/freshness transaction and
freeze the concrete execution and cost settings before enqueue. P12d/P12 stay
In Progress; no UI, command, migration, or runner caller is added here.

## P12d-2b admission evaluation and freshness fence (2026-09-29)

### Maintainer decisions (2026-09-29)

1. **One run per instrument.** A campaign-bound discovery run names one
   `campaignId` and one declared `instrumentId`; admission is decided per
   instrument, matching the single-dataset runner and the single-instrument
   trial family. A multi-instrument run is out of scope.
2. **`NOT_ELIGIBLE` does not stop exploration.** The run still executes and its
   trials still count (registration precedes admission by design); the stored
   decision blocks only later confirmation/qualification, as trial-ledger-v1 §7
   already states for blocked ledger states.
3. **Costs are the run's frozen numbers.** P06 `cost-profile-v1` carries no
   numeric fees, so P12d-2c freezes the run's `feePct`/`slipPct` into the stored
   decision and requires the snapshot's cost status to be confirmed. That the
   numbers are not checked against a P06 numeric profile is a recorded
   limitation, not a verified property.

### `research-campaign-admission-v1`

`research::campaign_admission::evaluate_campaign_admission` combines, for one
declared instrument: the P12d-2a `ResolvedInstrument`, the current ledger state,
the workspace's `legacy_trials_unknown` families, every candidate's P12c
walk-forward report, and the campaign's P12a sampling. It reads no database,
registry, clock or randomness.

**Errors, not decisions.** The call fails when the binding contradicts the
campaign, because such a run must not be linked to it: an undeclared
instrument; a resolved snapshot for another instrument or dataset; an empty
batch ID; no candidates, a repeated candidate index, a blank strategy hash, or
a candidate without exactly one of report/error; a walk-forward report of
another contract version, or whose `totalBars` differs from the verified bar
count or whose `minimumTrainBars`/`foldValidationBars`/`foldCount` differ from
the instrument's sample policy; or a ledger count of another family.

**Reasons.** Every failing check is reported once, in this fixed order; the
status is `ELIGIBLE` only when none applies:

| Reason | Condition |
| --- | --- |
| `snapshot_changed` | re-evaluation only: the re-verified snapshot rows differ from the stored observation |
| `ledger_prefix_unproven` | fence step 1 failed (rolled back, diverged, replaced without evidence, chain broken) |
| `ledger_count_inconsistent` | fence step 2: family shrank, or its family, protocol or batch size changed |
| `family_unknown`, `family_quarantined`, `registry_chain_broken`, `no_effective_trials` | the ledger's `AdmissionBlocked`, at registration or at the fence |
| `legacy_trials_unknown` | the workspace reports uncounted pre-P05 history for this family |
| `insufficient_total_bars` | verified bars below `minimumTotalBars` |
| `walk_forward_not_eligible` | any candidate has no plan or a `NOT_ELIGIBLE` P12c report |
| `precision_not_eligible` | the P12a report built by `precision_plan_from_count` from the current count |

Precision is evaluated whenever a current count exists, even alongside other
reasons. `ELIGIBLE` only means the declared confirmation is feasible under the
current family; it is never a confirmation `PASS` (P12e/P13).

**Report.** `contractVersion`, `campaignId`, `instrumentId`, `familyId`,
`batchId`, `status`, `reasons`, `snapshot` (declared `snapshotId` plus the
observed snapshot/instrument/dataset rows, dataset hash, bar count and cost
profile version), `ledgerFence` (`null` for a first decision; `unchanged`,
`grew` or the fence's blocking code), `ledger` (the `AdmissionSnapshot` a later
fence compares against; `null` without a count), `precision` and `walkForward`.

**Freshness fence.** A first decision uses the `Admission` read with the
registration. Before anything acts on an `ELIGIBLE` decision, the caller
re-verifies the snapshot, calls `TrialLedger::fence_admission(batchId,
storedLedger)` and evaluates again with that fence and the stored snapshot
observation. The fence reads the prefix and the count in one registry
transaction; `grew` recomputes precision from the new count, which can only
raise the requirement, so a stale `NOT_ELIGIBLE` never needs a fence.

Acceptance: trial-ledger-v1 A20 (the assembled plan gives `146/1001` and
`NOT_ELIGIBLE`) and A32 (family 11 → `ELIGIBLE` at B = 10,975; five more trials
→ `grew`, m = 32, `NOT_ELIGIBLE` needing 15,975; no change → decision stands)
run through this evaluator and the real ledger.

### Still P12d-2c

Persist the frozen declaration and the decision (workspace migration), add the
campaign binding to the run configuration, derive each candidate's P12c report
in the runner, register the batch against the declared snapshot's family (not a
guessed unique snapshot), freeze `feePct`/`slipPct`, and write the decision in
the enqueue transaction. The fence then needs a caller at the point confirmation
work is scheduled (P13). P12d/P12 remain In Progress.
