# P13c confirmation batch, exposure and recovery contract v1

Status: Design for bounded implementation slices. This document adds no
production reserve/execute/reveal caller, schema or statistical permission.
P13b complete alpha transfer is its prerequisite. Task status belongs to
[`tasks.md`](../../tasks.md).

## 1. Authority and unchanged boundaries

The [active plan §4.4](active-plan.md), [persistence plan](research-reservation-registry-plan-v1.md),
[campaign admission](../research-campaign-declaration-v1.md),
[alpha reservation](../research-alpha-reservations-v1.md) and
[complete alpha transfer](../research-alpha-transfer-v1.md) remain binding.

- A P12 `ELIGIBLE` decision is a feasibility observation, never a durable
  execution permit or confirmation PASS. Its read transaction has ended.
- The final registry transaction must use the current whole family and actual
  alpha share. No caller-supplied trial count, reservation subset or stale
  precision report can authorize a reveal.
- Exploration can continue when confirmation is blocked. Preserve historical
  results, trial counting, P12e's frozen computation and manual-only code mode.
- One instrument per confirmation batch; no cross-instrument qualification or
  overall research-level false-positive guarantee follows from family budgets.
- Hashes prove content equality, not that a human has never seen data or that
  an offline publisher disclosed all history. Joint rollback of every local
  registry/workspace observation remains the existing offline limitation.

The proposed records/APIs below are responsibilities for later slices, not
claims that serialized JSON or the current storage primitive has authority.

## 2. Freeze before protected evaluation

Validation and Test are separate confirmations, each with its own frozen
batch, final admission, scheduled alpha share and exposure claim. A Validation
reservation cannot authorize Test, and Test cannot reuse that share.

Use separate immutable documents for the candidate batch and the post-Validation
selection. Derive their identities with the existing domain-separated
type-tagged canonical-byte identity convention. New identity documents contain
bounded integers/strings, ordered references and strict tagged variants; they
do not normalize existing strategy floats or rewrite NUMERIC-JSON-001 identities.
Verify original artifact bytes with their own recorded versions and hashes.

### 2.1 Confirmation batch

Proposed `research-confirmation-batch-v1` freezes:

| Group | Required bound evidence |
| --- | --- |
| Source | Campaign identity/document, source workspace/run and registered trial batch, canonical instrument/family, current engine/build manifest. |
| Complete candidates | Ordered unique candidate indexes, strategy identities, exact definition/execution fingerprints, original candidate-result artifact hashes and lineage. Missing, duplicate, substituted or unregistered candidates are errors. |
| Data and split | Exact P06 snapshot/instrument revision, verified dataset identity, calendar/action/cost evidence, frozen numeric execution costs, original outer Train/Validation/Test split, embargo and warm-up policies. Record both tested bars and all protected input intervals actually accessed. |
| Statistics template | Frozen v2 version, block rule/length, seed, bootstrap count/cap and precision error bound. Bind the immutable family allocation ID. The final allocated share is inserted into the exact declaration only at admission. |
| Selection plan | Finite, versioned deterministic Validation selection/ranking rule and tie-break order, thresholds, candidate limit and the original Test region. Freeze before any Validation output. |
| Stage | A strict Validation variant, or a Test variant referencing the immutable selection and parent Validation batch/result. |

Batch freezing reads verified Train-derived candidate artifacts and metadata;
it performs no protected backtest, bootstrap, ranking or preview. A Validation
batch cannot contain a candidate chosen by already-seen Validation/Test output.
The Test region is fixed before Validation; selection cannot move it afterwards.

Freeze does not re-register the same research as a smaller family or add a
second copy of its trials. Verify selected membership against the existing
complete registered enumeration. New strategy variants still go through the
normal registration path before becoming a future batch.

### 2.2 Selection

Proposed `research-confirmation-selection-v1` binds the parent Validation
batch, its sealed result receipt/hash, the already-frozen selection rule and
its exact ordered candidate subset/decision. Reproduce the rule from the sealed
report; no unreported metric or hand-picked tie-break is allowed.

Freeze the selection before Test admission. Test uses exactly that selection,
original definitions, costs, engine, seed policy and Test region. Any changed
field is a new request requiring its own admissibility/exposure checks, never
an edit to the original batch. An empty selection schedules no Test work.
Selection is protected-derived evidence: it cannot return to generation,
tuning, training memory or prompts.

### 2.3 Operation identity

The backend creates a CSPRNG `reservationKey` (32 lowercase hex) and saves
the complete frozen operation in an owner-checked workspace commit **before**
its first registry mutation. Store `originRegistryId`, batch/stage/selection,
all frozen references and the request identity. Origin/key are independent of
the transport request ID and of a later ownership epoch.

Retries load that exact saved body. They look up its original origin/key,
including after registry replacement; they do not call
`reserve_alpha_evidence` to request the replacement registry's next slot.
A missing original operation under an unproven replacement is unknown outcome,
not permission to allocate under a new origin. Reusing a key for another body
retains conflicting evidence and permanently stops affected confirmations.

## 3. Exposure identity and overlap

Exposure lives in the **same external registry** as trial and alpha evidence.
A hash, dataset row ID, source, filename or interval label cannot reset it.

Use the canonical [`market-instrument-v1`](../market-contract.md) ID (`market:venue:symbol`) and
verified instrument revision. Source adapters must resolve aliases to that
identity; ambiguous/unmapped aliases refuse qualification. Different venues
remain different series under the existing market contract. This identity
does not promise independence between correlated instruments or venues.

Represent occupied time as sorted disjoint UTC millisecond half-open intervals
`[fromMs, toExclusiveMs)`, with checked integer bounds and `from < to`.
Build intervals from the frozen calendar and actual input support, including
protected warm-up, higher/lower timeframe inputs and any benchmark/event data
that would expose the protected period. Do not approximate a daily bar as an
instant. Unknown session close or overflow is an error. Merge touching
intervals within one claim; between claims, touching endpoints alone do not
overlap. `[a,b)` overlaps `[c,d)` iff `a < d && c < b`.

The collision domain is instrument + occupied time, **across** Validation/Test
roles, campaigns, family declarations, snapshots, sources, raw/adjusted price
bases, resolutions and dataset revisions. Retain these details for audit, but
do not put them in a key that would permit the same period again.
A revised split or another hash for the same instrument/time remains occupied.
Where an input's exposure cannot be mapped, refuse it rather than infer fresh
history. Cross-instrument dependence remains the stated statistical limitation.

An append-only exposure claim binds its origin/key, stage, batch/selection,
alpha reservation and final admission receipt, canonical instrument, occupied
intervals and provenance. Reserve it in the final admission transaction before
execution. Cancellation, failure and never-started work retain occupancy.
Multiple candidates selected together share that one frozen batch claim; a
later added candidate is not part of it.

A different operation overlapping any occupied region is refused. Exact
operation replay may recover the same receipt/job/sealed output; it is not a
second unseen evaluation. Validation selection may lead to the originally
declared **disjoint** Test region, after freezing the selection. It cannot
re-evaluate the Validation region as Test.

Examples below compare a new operation with an existing `[0,10)` claim on
the same canonical instrument; times are toy milliseconds, not market bars:

| New support | Other change | Decision |
| --- | --- | --- |
| `[10,20)` | None | No overlap; other admission gates still apply. |
| `[9,11)` | None | Refuse: one millisecond of shared support is enough. |
| `[2,3)` | None | Refuse: contained support is still exposed. |
| `[0,10)` | Another provider, dataset hash or price basis | Refuse: these are not freshness keys. |
| `[0,20)` | Coarser interval | Refuse: aggregate support intersects. |
| `[0,10)` | A different verified canonical instrument | Distinct occupancy domain; no independence or overall-alpha claim. |

## 4. Legacy and manual exposure

`legacy_exposure_unknown` is separate from unknown trial counts and
`alpha_history_unknown`. None can clear another.

Existing manual Holdout, charts, result exports, arbitrary file import or raw
data inspection cannot establish that a proposed protected period was unseen.
Keep unknown/seen evidence by instrument/region (or a conservative containing
region when exact support is unavailable). Renaming/reimporting a dataset,
creating a campaign or migrating a binding cannot turn it into known-unseen.
No checkbox or self-attestation removes permanent seen/unknown evidence.

Managed protected acquisition needs a backend-controlled provenance path
that starts before any frontend/raw-data/analytics exposure. Its absence
blocks qualification. A first-time import of old data is not such proof.
Future manual reveal paths must append their exposure/unknown marker before
returning data, charts, Holdout metrics or exports; a marker write failure
refuses that reveal. Uninstrumented old paths remain legacy/manual-only and
cannot qualify their regions.

Every raw-candle, chart, worker/backtest and export path must use backend
exposure mediation; frontend role labels are insufficient. Race manual access
against confirmation admission under the same registry serialization. Manual
access cannot bypass an active protected claim; after sealing, permitted
historical access retains the seen marker. Production confirmation stays
disabled until this mediation and its bypass audit are complete, even if
execution code or typed commands have already been implemented behind gates.

Keep ordinary exploration usable with explicit historical/contaminated
provenance. A later genuinely unexposed period may be considered with complete
evidence; cutting an exposed period into a fresh dataset is insufficient.
This contract adds no retroactive trust in an operator's prior knowledge.

## 5. Final admission: ordering and linearization

### 5.1 Preparation and workspace observation

1. Parse/refreeze all documents. Resolve the declared snapshot, complete
   registered candidates, artifacts, engine and numeric costs. Check every
   gate that does not require the current registry state.
2. Save the immutable operation/original origin/key under the current workspace
   owner/epoch. Use an ordinary workspace transaction and commit it.
3. Capture a workspace read observation: workspace identity, owner epoch,
   `runtime_state.mutation_seq`, immutable reference rows and content hashes.
   Materialize and verify costly inputs in backend-owned read-only buffers.
   These integrity reads must not evaluate protected returns/statistics or
   expose data to UI, ranking, prompts or logs.
4. At final admission acquire the workspace DB mutex and an owner-checked
   SQLite IMMEDIATE transaction. Compare the captured observation before
   incrementing mutation_seq; `write_transaction_quiet` is a possible helper.
   Recheck all reference identities, epoch and materialized hashes.
   A mismatch refuses/retries preparation without changing the frozen request.

A lease alone is not a dataset/artifact observation. Use verified pinned bytes,
not a filename that a worker later rereads without checking. No expensive
artifact IO, market download, bootstrap or UI/event emission happens while
holding the registry write lock.

### 5.2 Lock order and one registry commit

While holding the verified workspace guard:

1. Acquire registry connection mutex, then registry SQLite IMMEDIATE transaction.
   **Order: workspace DB guard -> registry mutex -> registry transaction.**
   Registry helpers never acquire a workspace/control lock or perform callbacks.
   Enqueue/dispatch never acquire runner control locks while holding these DB
   locks. Other registry writers may wait; none may wait back on this workspace.
2. Verify the saved event prefix and every restrictive binding identity,
   current family/batch/protocol/count, quarantines, alpha/provenance and full
   exposure evidence. Lost or equally-sized substituted evidence is rollback.
3. Look up the original operation. A committed exact match returns its original
   immutable receipt, rather than taking a new number. A contradictory body
   preserves permanent conflicts. Replay does not grant permission to rerun.
4. For a new operation, review complete alpha history and choose the actual
   next share. Require known legacy trial/alpha/exposure history, current
   transport coverage, the frozen batch/selection, complete manual-exposure
   mediation and the statistical-policy gate.
5. Re-run P12a with current whole-family trial/test counts and the actual share,
   preserving the frozen bootstrap count/cap/error bound. Construct the exact
   frozen statistical declaration with that share. Never use only the selected
   candidate count as Holm's family size; family tests cannot decrease.
   Growth recomputes eligibility in this transaction. Exhaustion or a failed
   gate refuses without starting a computation or spending a new share.
6. Check overlap and append **alpha reservation + exposure claim + immutable
   final admission receipt** in this same transaction. The receipt binds the
   batch/selection, input observation/digests, engine/statistics versions,
   allocated number/share, current count/protocol, final P12a/fence evidence
   and restrictive-history observation.
7. Commit the registry once. This is admission's linearization point.
   Other registrations/imports/admissions cannot interleave between final
   count/overlap checks and spend. A late write failure rolls everything back.

Then enqueue one workspace job referencing the original receipt/reservation,
update the restrictive binding and increment mutation_seq in the still
owner-checked workspace transaction. Commit workspace and release guards
before scheduling workers/events. The registry commits first; there is no
cross-database atomicity claim. Workspace failure never refunds registry spend.

An untrusted serialized report or frontend token cannot construct the private
backend admission capability. Existing public storage/pure APIs remain
non-authoritative; production callers must use the integrated path.

## 6. Execution, sealing and recovery

Before touching protected values, a uniquely owner/epoch-claimed job appends
an idempotent `execution_started` event under the same registry operation.
A second worker cannot get a fresh execution authorization for that operation.
No result reaches the frontend before its complete immutable artifact and
bound registry sealed-result receipt are durable; workspace result/history is
an owner-checked projection of that seal. Result publication and projection
must be idempotent. Historical sealed output can remain readable after a stop,
but must retain its consumed/conflicted/unknown provenance and cannot mint
current qualification authority.

Version 1 permits **no automatic re-execution after execution_started**.
Determinism/identical seeds do not make a second attempt new unseen evidence.
An unsealed interrupted computation is `interrupted_spent`; its share and
exposure remain. This conservative rule avoids introducing checkpoint/reveal
semantics before they are specified.

| Crash/failure point | Recovery |
| --- | --- |
| Before saved operation commit | No registry spend; no executable operation. |
| Operation saved, no registry commit | Same frozen original origin/key may attempt admission against current evidence. A replacement with unproven absence refuses. |
| During registry writes | One transaction rolls back alpha, exposure and admission together. |
| Registry committed, workspace enqueue absent | Exact lookup recovers the committed receipt and may enqueue its unique missing job after owner/input checks. No new share or overlap allocation. |
| Enqueued, response lost | Transport/workspace idempotency returns that job/receipt. |
| Before execution_started | Recover the same job/receipt; current owner and exact inputs remain required. |
| Execution started, no durable seal | Mark interrupted/failed spent; no automatic rerun or new job. |
| Complete artifact written, seal not committed | Keep spend and incomplete state; an orphan artifact alone is not execution authority. |
| Seal committed, workspace result absent/response lost | Verify exact sealed artifact and project/replay it idempotently; do not recompute. |
| Missing/mismatched original artifact, data or engine | Preserve spend and explicit failure. Never substitute latest engine/data or fabricate output. |
| Cancellation, shutdown, stale epoch | Preserve spend/exposure; refuse stale writes. No reservation release/renumbering. |

Recovery uses the original operation identity even through a complete registry
replacement/import. Uncertain absence, lost stops or missing original-origin
evidence cannot be repaired by assigning a new key/number.

## 7. Transfer and rollback obligations before runtime

P13b v4 archives/binding v4 cover trial/alpha/provenance, **not exposure or
execution receipts**. They cannot establish P13c-complete history.
The storage slice must version the registry/binding/complete archive again
and define bounded union for batches, selections, operations, admission,
exposure, execution/seal events and permanent conflicts/unknown markers.

Preserve original IDs, intervals, origins, keys and all conflicting copies;
do not renumber or merge independent spends. Binding coverage is identity-set
coverage for all restrictive evidence, never counts. Old formats remain
explicit archival compatibility paths with unknown P13c provenance and cannot
launder complete history. Old builds refuse unsupported schema/binding.

Validate the complete input and proposed union before writes, retain one
commit/rollback for trial/alpha/exposure evidence, and test exact replay,
two-hop/replacement, missing/equally-sized substituted evidence, conflicting
claims, capacity and injected late failure. No runtime slice may bypass this
by importing only alpha v4 into a seemingly empty exposure table.

## 8. Prompt and generation boundary

This implements the boundary already required by
[AI provider contract §7](../ai-provider-contract.md).

Construct generation context from a new typed **Train-only allowlist**:
verified Train scope, approved Train summaries/hypotheses/failure classes and
their origin identities. No generic report serialization, recursive key
redaction or arbitrary string attachment is a safe boundary.

Validation/Test values, plots, returns, failure reasons, chosen candidate IDs,
selection-derived rationale, sealed report references and human notes derived
from those outputs are protected. Do not expose them in generation/tuning/
ranking prompts, logs the provider can read, training memory or tool results.
Metadata such as a content hash is not declassification.

Reject mixed/unknown provenance or a field outside the allowlist. Reject
Train-labeled data whose intervals intersect known protected exposure.
Provider absence does not waive this requirement: P13 implements/tests the
pure builder first; P15 alone adds provider execution/isolation. Tests must
inject protected values in nested fields, strings/notes, selection and stale
scope references and prove refusal without calling a provider.
Manual research history may display protected results outside generation,
with their consumed/unknown status; it never becomes a proposal context.

## 9. Statistical policy remains an explicit gate

Frozen v2's acceptance is exactly the tested set in
[confirmation statistics v2 §6](../research-confirmation-statistics-v2.md#6-status-and-limits):
{256,512,1024} bars, generator `ar1-uniform-sum`, coefficients {0,0.3},
two single-candidate confirmations, no prior trials and B=799, with the
recorded limits. These are simulated configurations, not an empirical-return
classifier. A real series of 512 bars is not made supported by its length.

Storage/pure-contract implementation can proceed without this policy decision.
Production statistical confirmation on real-market returns, other family
shapes/schedules/counts, or other parameters remains blocked until a separate
maintainer decision and predeclared evidence contract answer recalibration
plan §11, including whether each declaration must be simulated.
Do not invent a calibration ID, infer distribution from protected results,
rerun final seed 20261117, increase B, tune alpha or loosen a threshold here.
A diagnostic/reproduction tool, if later authorized, must be labeled and
separated from qualification; this design enables neither route.

## 10. Bounded slices and required evidence

Each slice gets its own task/PR, from current merged main. No production
caller before all prerequisite integrity/transfer/policy gates exist.

| Slice | Scope | Acceptance before completion |
| --- | --- | --- |
| P13c-1 | Pure batch/selection/operation documents and interval overlap; no IO/execution | Independent identity/overlap examples; strict fields/integers/versions, candidate membership, split/selection tampering, touching/contained/disjoint/timeframe/source-revision cases. Freeze explicit candidate/document/interval bounds before implementation. |
| P13c-2 | Pure Train-only generation-context builder; no provider | Positive Train fixture plus mixed provenance, nested protected data, notes/selection/hash leaks and exposed-time refusal. |
| P13c-3 design | Exact append-only exposure/receipt schema and successor transfer/binding wire contracts, capacities and migration/legacy behavior | Agree exact tables/IDs/byte-row bounds and union/conflict policy before migration changes. |
| P13c-3 implementation slices | Persistence then complete binding/transfer, separately reviewed | Real independent connections, reopen, permanent occupancy/conflicts, rollback/equal-count substitution, old-format refusal, duplicate/two-hop import, bounds and late atomic failure. |
| P13c-4 | Integrated final admission and registry-first enqueue; no protected execution | Real cross-process trial/import/overlap contention; mutate snapshot/artifacts between prepare/commit, stale epoch, P12a growth and actual-share refusal, original-origin exact retry, crash at every commit boundary. |
| P13c-5 | Dormant gated execution/seal/recovery and typed service/Tauri surfaces; no production activation before policy and mediation acceptance | Deterministic crash injection before/after execution_started and seal, no duplicate spend/job/execution, missing engine/artifact, protected-data capability refusal and native acceptance. |
| P13c-6 | Managed/manual exposure paths and user-facing workflow, only after backend gates | Manual-access/admission race; no raw/chart/Holdout/export bypass; legacy unknown state, canonical overlap across reimport/source/timeframe, mixed AI context refusal and real native restart/receipt/history evidence. |

P13c-1 is the next bounded implementation task after this design merges.
Other slices remain Backlog in this order. P13c-1 owns the exact wire fields
and testable input limits in its own small contract addition; it must not add
storage or quietly decide statistical policy. Existing full verification,
six exact-head CI jobs and normal time limits remain required for code slices.

