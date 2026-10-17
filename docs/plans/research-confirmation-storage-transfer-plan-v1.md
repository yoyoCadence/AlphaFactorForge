# P13c-3 confirmation storage and complete transport contract v1

Status: Design only, from merged P13c-2 (#189). No migration, runtime caller,
admission permit, execution or statistical-policy decision is added here.
Task status belongs to [tasks.md](../../tasks.md). This freezes implementation
details required by [P13c §7/10](research-confirmation-exposure-recovery-plan-v1.md).

## 1. Existing identities and authority

Keep [P13c-1 documents](../research-confirmation-contracts-v1.md),
[Train context](../research-train-context-v1.md), [alpha storage](../research-alpha-registry-storage-v1.md),
[binding v4](../research-alpha-workspace-binding-v1.md) and
[complete alpha v4](../research-alpha-transfer-v1.md) unchanged as historical
contracts. A checked row or imported receipt proves structural consistency
with supplied evidence, never authentic unseen history or permission to run.
Only P13c-4's private final transaction may later create live admission rights.
P13c-5 remains dormant; production additionally requires statistical policy
and P13c-6 manual mediation. Never infer support from bar count or a hash.

P13c-1 `source.trialBatchId` stores the 64-digit digest. The existing SQL
trial batch key is exactly `trial-batch-v1:<digest>`; resolve that prefix
explicitly without rehashing, changing either identity or accepting aliases.
The source family, complete original membership and stored trial evidence must
agree. Candidate/artifact authenticity still requires the later host's pinned
original bytes; the storage reader performs no filesystem access.

No trial re-registration or new trial count follows from freezing or selecting.
For Test, the original registered batch's complete effective count remains
the count input even though its evaluated candidate subset is smaller.

## 2. Acyclic immutable records

New stored documents are exactly `{id,body}`. Body has the version below;
ID is lowercase SHA-256(version UTF-8 + zero byte + existing type-tagged
canonical body bytes). All fields are closed, integers are safe JSON integer
literals, nullable fields remain mandatory. Ordinary IDs are 64 lowercase
hex; origin/key/workspace/execution keys are 32. Family/instrument/time and
interval canonicality retain P13c-1. No new identity preimage contains floats.
Original report bytes have a separate ordinary SHA-256, without JSON rewriting.

The reference direction is deliberately one way:

1. Validation batch, then operation and its alpha reservation.
2. Confirmation exposure references operation + alpha reservation.
3. Admission references operation + alpha reservation + exposure.
4. Started event references admission; seal references started + original report.
5. Selection references Validation batch + seal + report + alpha reservation.
6. Test batch references that selection/parent; its own operation starts a new
   exposure/admission/share sequence.

An exposure is bound to its final admission by the admission's checked reverse
reference, not a circular hash field. Imported closed graphs are validated
with bounded iterative dependency resolution before writes; missing references,
cycles, wrong stages/parents and orphan selections/seals refuse the archive.
Batches/operations may exist before admission. A confirmation exposure and its
admission must be present together at transaction commit; a missing half is
corruption, never an unspent operation. Manual/legacy claims need no admission.

### 2.1 Exposure (`research-confirmation-exposure-v1`)

Exactly `contractVersion`, `originRegistryId`, `reservationKey`, `instrumentId`,
`intervals`, `claim`. `claim` is a closed tagged object:

| `kind` | Other exact fields |
| --- | --- |
| `confirmation` | `operationId`, `alphaReservationId` |
| `manual` | `accessKind`, mandatory nullable `afterSealId` |
| `legacy_unknown` | `reason`, mandatory nullable `sourceEvidenceSha256` |

`accessKind`: `raw`, `chart`, `holdout`, `worker`, `export`, `history`.
`reason`: `migrated_workspace`, `legacy_import`, `unmediated_access`.
Confirmation origin/key/instrument, alpha and exact stage support must match
its operation/batch. Manual/legacy claims preserve seen/uncertain occupancy;
there is no known-unseen claim or clearing operation. `afterSealId`, when set,
must reference a seal for this instrument with containing exposure support.
It authorizes only the later host's historical-read mediation check, never
another qualification. Raw row insertion itself authorizes no read.

### 2.2 Admission (`research-confirmation-admission-v1`)

Exactly `contractVersion`, `operationId`, `alphaReservationId`, `exposureId`,
`workspaceObservation`, `registryObservation`, `precisionPlan`, `precisionProof`.

`workspaceObservation`: exactly `workspaceId`, positive `ownershipEpoch`,
`mutationSeq`, `referencesSha256` (digest of the host-pinned input/reference
observation). Match the operation's workspace. Epoch is audit evidence here;
it does not change original operation identity or grant a future owner a lease.

`registryObservation`: exactly `registryId`, `seq`, `chainHead`, `bindingSha256`,
`familyEffectiveTrials`, `registeredBatchEffectiveTrials`, `testsPerTrial`,
`protocolSha256`. These are the **pre-spend**, same-transaction observation.
`bindingSha256` references the exact original binding-v5 JSON bytes in the
blob table; validate its full structure, prefix and identity-set coverage.
Its sets must not contain this new admission, exposure or future events.
The prefix must remain proven by the existing local/imported chain rules.

`precisionPlan` has the existing nine `research-precision-v1` fields. Rebuild
it using current whole-family/registered-batch counts, actual reserved alpha,
frozen B/cap/error bound and `precision_plan_from_count`; never double count
the already registered batch or reduce m to selected Test candidates.
The defined two-test confirmation protocol requires Holm/testsPerTrial=2;
another current protocol is refused, never reduced to two.

`precisionProof` is exactly the integer/enum projection of the existing P12a
report: `contractVersion`, `status`, `reasons`, `familyTrials`, `familyTests`,
`minAttainableRawP`, `bestAdjustedP`, `requiredSamplesForResolution`,
`requiredSamplesForPrecision`. Ratios contain numerator/denominator; nullable
requirements are mandatory. Omit only the display float `bestAdjustedPValue`.
Recompute with the unchanged P12a implementation and require exact equality
and `ELIGIBLE`. This creates new integer evidence; it never reserializes an
old float-bearing report or changes P12a/statistics/calibration identities.

P13c-4 must authenticate and generate these observations under the workspace
guard followed by registry mutex + IMMEDIATE transaction, commit alpha,
exposure and admission once, then owner-checked workspace enqueue/binding.
Storage/import cannot reconstruct a private execution capability from JSON.

### 2.3 Execution (`research-confirmation-event-v1`)

Exactly `contractVersion`, `operationId`, `admissionId`, `event`. Closed event
objects have these fields:

| `kind` | Other exact fields |
| --- | --- |
| `execution_started` | `workspaceId`, positive `ownershipEpoch`, `executionKey` |
| `sealed` | `executionStartedId`, `reportSha256`, `resultArtifactSha256`, positive `resultArtifactBytes`, `engineManifestHash` |
| `interrupted_spent`, `failed_spent`, `cancelled_spent` | mandatory nullable `executionStartedId`, `reason` |

Reasons: `owner_lost`, `cancelled`, `engine_missing`, `input_missing`,
`artifact_missing`, `execution_failed`, `seal_missing`. Null started ID is
permitted only for terminal work that never started; it cannot imply a refund.
Seal must reference the same operation/admission's started event, frozen engine
and original report. Verify the existing v2 report's settings, actual share,
whole-family m, exact candidate/test rows and integer Holm evidence against
that stage. Selection additionally reuses P13c-1's deterministic rule.

Artifact hash/length bind opaque original result bytes. Their format, durable
file writer and availability checks must be specified before P13c-5 executes;
this design does not invent an artifact format or claim those files exist.
Missing files retain spend and block projection, not registry-history import.
No automatic re-execution after a started event; only verified sealed replay.

### 2.4 Conflicts (`research-confirmation-conflict-v1`)

Exactly `contractVersion`, `kind`, `claimIds` (two distinct sorted IDs),
`affectedFamilyIds`, `affectedInstrumentIds` (sorted unique derived targets).
Kinds: `origin_key`, `admission`, `exposure_overlap`, `manual_overlap`,
`execution_started`, `seal`, `terminal`. References must demonstrate the kind
and exact targets; arbitrary self-asserted stops are invalid documents.

- Preserve altered operations/claims with the same original origin/key;
  different independent keys cannot exempt intersecting confirmation support.
  An operation and its exactly linked confirmation exposure share one key as
  one logical claim. They do not conflict with each other. Reuse between a
  manual/legacy claim and an independent operation, or altered manual/legacy
  claims, is an origin-key conflict.
- Multiple admissions/started/sealed/contradictory terminal claims retain all
  copies and stop the affected operation/families. No UNIQUE origin/key,
  confirmation number, operation/admission or event-kind constraint may drop
  independently valid contradictory history.
- Manual/unknown occupancy intersecting confirmation is restrictive. A checked
  manual read with containing `afterSealId` is historical access and creates
  no manual-overlap conflict; all other such intersections stop the affected
  confirmation. Manual/manual intersections simply retain seen history.
- Origin/key reassignment can affect two instruments with no existing trial
  family. Targets may therefore include zero families but one/two instruments;
  future confirmations of a stopped instrument remain blocked.

Within a collision group, choose the smallest claim ID as representative and
pair it with each other claim. Preserve previous conflict IDs even if a new
minimum arrives; do not generate every quadratic pair. Never delete a stop.
Local final admission refuses occupied regions before creating a new claim;
import must preserve independently allocated overlaps and record conflicts.
Conflict status blocks confirmation, not ordinary trial exploration.

## 3. Registry schema 6 (separate from workspace schema)

Append `0006_confirmation_history.sql`; do not rewrite migrations 0001–0005.
All ten new tables reject UPDATE and DELETE. `recorded_at TEXT NOT NULL` is
local first-observed audit time, excluded from identity and transfer ordering.
TEXT IDs/JSON are NOT NULL except the explicit nullable columns below. Foreign
keys are DEFERRABLE INITIALLY DEFERRED; graph validation precedes insertion.

| Table | Exact columns apart from `recorded_at` |
| --- | --- |
| `confirmation_batches` | `batch_id TEXT PRIMARY KEY`, `family_id TEXT REFERENCES trial_families`, `stage TEXT`, nullable `parent_batch_id TEXT REFERENCES confirmation_batches`, nullable `selection_id TEXT REFERENCES confirmation_selections`, `document_json TEXT` |
| `confirmation_selections` | `selection_id TEXT PRIMARY KEY`, `validation_batch_id TEXT REFERENCES confirmation_batches`, `sealed_receipt_id TEXT REFERENCES confirmation_events`, `alpha_reservation_id TEXT REFERENCES alpha_reservations`, `report_sha256 TEXT REFERENCES confirmation_blobs`, `document_json TEXT` |
| `confirmation_operations` | `operation_id TEXT PRIMARY KEY`, `origin_registry_id TEXT`, `reservation_key TEXT`, `workspace_id TEXT`, `batch_id TEXT REFERENCES confirmation_batches`, `family_id TEXT REFERENCES trial_families`, `document_json TEXT` |
| `confirmation_exposures` | `exposure_id TEXT PRIMARY KEY`, `instrument_id TEXT`, `kind TEXT`, nullable `operation_id TEXT REFERENCES confirmation_operations`, nullable `alpha_reservation_id TEXT REFERENCES alpha_reservations`, `document_json TEXT` |
| `confirmation_intervals` | `exposure_id TEXT REFERENCES confirmation_exposures`, `interval_index INTEGER`, `instrument_id TEXT`, `from_ms INTEGER`, `to_exclusive_ms INTEGER`, PRIMARY KEY (`exposure_id`,`interval_index`) |
| `confirmation_admissions` | `admission_id TEXT PRIMARY KEY`, `operation_id TEXT REFERENCES confirmation_operations`, `alpha_reservation_id TEXT REFERENCES alpha_reservations`, `exposure_id TEXT REFERENCES confirmation_exposures`, `binding_sha256 TEXT REFERENCES confirmation_blobs`, `document_json TEXT` |
| `confirmation_events` | `event_id TEXT PRIMARY KEY`, `operation_id TEXT REFERENCES confirmation_operations`, `admission_id TEXT REFERENCES confirmation_admissions`, `kind TEXT`, `document_json TEXT` |
| `confirmation_blobs` | `blob_sha256 TEXT PRIMARY KEY`, `kind TEXT`, `byte_length INTEGER`, `content_bytes BLOB` |
| `confirmation_conflicts` | `conflict_id TEXT PRIMARY KEY`, `document_json TEXT` |
| `confirmation_unknown_origins` | `origin_registry_id TEXT PRIMARY KEY` |

`confirmation_intervals` has no recorded_at: it is a complete deterministic
index projection of its immutable exposure document. Zero-based indexes must
be contiguous. Check positive half-open bounds, canonical sorted/merged sets,
exact instrument and document equality on every load; extra/missing index rows
are corruption. SQL CHECKs enforce stage/kind enums, lowercase ID lengths,
integer/time domains and byte lengths; readers also recompute all identities,
cross-reference/stage/alpha bindings, interval projections and conflict targets.

Indexes: operations `(origin_registry_id,reservation_key)` and `family_id`;
exposures `operation_id`; intervals `(instrument_id,from_ms,to_exclusive_ms)`;
admissions `operation_id`; events `(operation_id,kind)`; batches `family_id`.
No new trial event or trial-chain change is needed for a confirmation write.
The binding's checked identity sets cover this evidence outside that chain.

## 4. Explicit joint bounds

| Resource | Bound |
| --- | --- |
| New stored batch/selection/operation/exposure/admission/event/conflict record | 4 MiB UTF-8 each, including `{id,body}` |
| Statistics blob | 4 MiB exact original UTF-8 report bytes |
| Binding-observation blob | 8 MiB exact original binding-v5 UTF-8 bytes |
| New document/blob rows combined (all tables except intervals/unknown origins) | 10,000 rows and 64 MiB combined stored JSON/BLOB bytes |
| Derived interval rows | 128,000 total; at most 128 per exposure |
| Confirmation-unknown origins | 10,000 |
| Binding-v5 text | 8 MiB; old binding v1–v4 text retains 4 MiB bound |
| New binding's confirmation identities | 10,000 record/blob IDs + 10,000 unknown origins |
| Complete v5 archive | 256 MiB UTF-8; embedded v4 projection 128 MiB |
| Archive header / other line | 4096 bytes / 16 MiB + 4096 bytes |

Existing alpha/trial capacities remain in addition. All bounds apply jointly:
escaping and nesting can make a file fail even when individual stored inputs
fit. Fail before publication/writes, never truncate/split away restrictive
history. Preflight SQL byte/row aggregates before loading blobs; validate both
input and proposed union, counting exact deduplicated additions, before mutation.
Overflow/capacity or injected late failure rolls back the entire transaction.
Duplicate idempotent rows do not consume additional capacity.

## 5. Unknown provenance and downgrade

Add a distinct `confirmation_unknown_origins` marker, separate from trial
uncertainty, `alpha_unknown_origins` and interval-specific `legacy_unknown`
manual exposure. None clears another; there is no remove/adopt-to-empty API.

For genuinely new registries, current code inserts immutable registry metadata
`confirmation_history_birth = 0006_confirmation_history` together with the
registry ID when it creates migration 0001. Existing registries lack this key.
Migration 0006 backfills the local origin when that key is absent, plus all
prior import/checkpoint/genesis origins. It never adds the birth key to an
existing registry. Capacity failure rolls back 0006 and leaves its migration
unapplied. Fresh empty schema-6 history is not proof of unseen manual data;
P13c-6 still must mediate/record old workspace exposure before admission.

Every legacy v3 trial or v4 alpha import records its source and carried origins
as confirmation-unknown in the same transaction, including empty files. Complete
v5 union carries markers permanently. Qualification import requires v5 and
known source/destination trial/alpha/confirmation provenance before any write.
Archival union may retain unknown/conflicted history without granting rights.
Do not conflate this with alpha-only stops or block ordinary exploration.

Old builds refuse schema 6 and binding v5. Legacy v3/v4 export projections may
be used internally by the new complete writer. A public legacy export refuses
any nonempty confirmation document/blob/interval/conflict/unknown table so it
cannot omit new evidence. Internal projection is not a known-empty assertion.

## 6. Binding v5

Preserve every v4 field, changing `version` to `trial-ledger-binding-v5` and
adding mandatory `confirmation` with exactly sorted unique arrays `batches`,
`selections`, `operations`, `exposures`, `admissions`, `events`, `blobs`,
`conflicts`, `unknownOrigins`. The first eight contain checked record/blob IDs;
the last contains 32-digit origins. Intervals are covered by their checked
exposure document and full derived-index validation, not by counts.

Observe trial prefix, restrictive trial evidence, complete alpha/provenance
and the entire confirmation graph in **one deferred registry transaction**.
Verify original blob bytes/hash and every row/column/reference before returning
sets. Every saved ID must remain present: growth is allowed, equal-sized valid
substitution or loss is rollback. Corruption is invalid state. Neither failure
overwrites the last good workspace binding. Valid conflict/unknown sets remain
readable/adoptable for exploration, while confirmation admission is blocked.

v1–v4 read with unknown confirmation evidence, never known-empty sets. Verified
adoption can write a complete current v5 observation, including its permanent
unknown markers; it cannot clear those markers or declare history unseen.
Only v5 is written after this slice; the workspace SQL schema is unchanged.

## 7. Complete archive v5 and one-commit union

`trial-ledger-export-v5` is canonical UTF-8 JSON Lines with final newline.
Header is exactly `version`, `registryId`, `schemaVersion`
(`0006_confirmation_history`), `exportedAt`, `counts`, `bodySha256`.
`counts` has exactly `batches`, `selections`, `operations`, `exposures`,
`admissions`, `events`, `blobs`, `conflicts`, `unknownOrigins`.

Body sections, in order, strictly ascending by record ID within each section:

1. Exactly one `{type:"alphaLedger",export:<canonical complete v4 text>}`;
   its source ID equals the outer source. It includes the unchanged trial v3
   projection and all prior alpha/provenance history.
2. `confirmationBatch`, `confirmationSelection`, `confirmationOperation`,
   `confirmationExposure`, `confirmationAdmission`, `confirmationEvent`,
   `confirmationConflict`: each has exactly `type`, `record` (`{id,body}`).
3. `confirmationBlob`: exactly `type`, `sha256`, `kind`, `text`, ordered by
   digest. `kind` is `statistics` or `binding`; UTF-8 text round-trips the
   original bytes. `byte_length` is recomputed, not supplied by the publisher.
4. `confirmationUnknownOrigin`: exactly `type`, `originRegistryId`, origin order.

Header counts include all corresponding records, regardless of section order.
No interval projection or local timestamp is transferred; regenerate and
verify indexes from the checked exposure documents in the import transaction.
Strict original JSON parsing rejects duplicate/unknown fields, noncanonical
lines, wrong versions, counts, order, incomplete graph or body-byte SHA.

Export takes one read transaction for v4 projection + all confirmation evidence.
Import fully validates input first, then acquires the registry mutex and one
IMMEDIATE transaction, validates current state, preflights complete trial +
alpha + confirmation union and capacities, inserts all additions and derived
permanent conflicts/unknown markers, validates the resulting graph and commits
once. Reuse transaction-scoped trial/alpha helpers; no nested commit/lock.
Missing dependencies, unequal same-ID bytes, duplicate source identities,
late SQL failure or invalid final state leaves **all** destination tables,
checkpoints/import receipts and bindings unchanged.

Exact IDs deduplicate; original origin/key/number/share, bytes and stops never
change. Preserve independent conflicting claims and two-hop/replacement
history. An unseen valid suffix omitted with fresh hashes remains undetectable
without prior observations: this format is not publisher authentication.
Joint rollback of all local registry/workspace evidence remains the offline
limitation. Recovery looks up original operations; imports never re-execute.

## 8. Ordered implementation tasks and acceptance

Each task is a separate PR from current merged main; continue only after its
predecessor's six exact-head CI jobs and merge. No production activation.

| Task | Scope and required evidence |
| --- | --- |
| P13c-3a-1a | Pure checked exposure/event/conflict records; independent identities, closed variants, origin/key/stage/interval bindings and exact capacities. Reference IDs remain non-authoritative until full graph validation. No IO/schema. |
| P13c-3a-1b | Pure admission/precision proof, original statistics/binding blobs and complete dependency/conflict graph; independent acyclic fixtures, unchanged integer P12a/report decisions, missing/forged/stage/selection/alpha binding refusal and exact capacities. No IO/schema. |
| P13c-3a-2 | Registry migration 0006 + bounded append-only storage/load/lookup and transaction-scoped helpers; real independent connections, reopen, every trigger/index projection, original-origin replay/conflicts, legacy birth/backfill/capacity, all late-failure rollback. No live admission capability. |
| P13c-3a-2a | First bounded storage PR: unchanged schema 0006 and SQL constraints/capacities, fresh-only birth marker, permanent legacy backfill/import markers and old-format omission guards, with real migration/independent-connection/rollback tests. No graph writer/reader yet. |
| P13c-3a-2b | Second storage PR after 3a-2a merges: bounded checked graph loads, exact column/interval projections, original-origin lookup and transaction-scoped append/union helpers; real connection/reopen/corruption/late-failure rollback. Completes 3a-2 before 3b-1. |
| P13c-3b-1 | Binding v5 and whole-graph observation; exact/equal-count substitution, corrupted byte/column/index/reference, legacy unknown adoption, failed binding preservation, real old-build schema/binding refusal. |
| P13c-3b-1a | First bounded binding PR: reuse the existing original-text v5 codec as an immutable pure snapshot with canonical bounded serialization and exact confirmation identity-set coverage. No runtime reader/writer switch or registry observation; the snapshot proves format only. |
| P13c-3b-1b | Complete 3b-1 after 3b-1a: whole-registry checked observation/adoption and downgrade acceptance. Before activation, specify durable independent historical context for reopen and conflicting imported branches; admission receipts must not become their own independent count witnesses. |
| P13c-3b-1b-1 | First context primitive: bounded complete original trial snapshot validation, exact v5 prefix/restrictive-trial evidence and registered candidate membership, independently recomputed historical counts/protocol. No current-count or receipt-total substitution, IO, persistence or runtime activation. [Detailed plan](research-historical-observation-plan-v1.md). |
| P13c-3b-1b-2 | Freeze durable historical-context storage/dependency/downgrade design after the reconstruction primitive is verified; separate implementation slices before the observer. No implicit blob/schema/wire extension. |
| P13c-3b-1b-3 | Complete the original whole-registry observation/adoption task after durable context exists; exact ID coverage, failed binding preservation, legacy adoption and actual old-build schema/binding refusal. |
| P13c-3b-2 | Complete v5 export/union and legacy import/export guards; atomic trial/alpha/confirmation rollback, independent overlaps/starts/seals/stops, two-hop/replacement/duplicate import, exact bounds and lost marker/dependency refusal. |

P13c-4 then integrates current-family/actual-share P12a, authenticated inputs,
workspace ordering and registry-first admission/enqueue with cross-process
contention and every crash boundary. P13c-5 specifies opaque result bytes and
dormant execution/sealing; P13c-6 owns manual mediation and native acceptance.
Neither storage nor six green CI jobs substitute for statistical policy or
authentic unseen provenance. Do not run reserved final seed 20261117.
