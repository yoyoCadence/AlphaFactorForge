# Pure confirmation documents and overlap v1 (P13c-1)

This is the bounded pure implementation of
[P13c design §2–3](plans/research-confirmation-exposure-recovery-plan-v1.md).
Checked records prove consistency with supplied evidence, not registry spend,
artifact authenticity, complete history, calibration, unseen data or execution
authority. There is no IO, reservation, bootstrap, provider or runtime caller.

## 1. Versions, encoding and bounds (frozen before implementation)

Three stored records have exactly `{id, body}`; the body's `contractVersion`
is respectively `research-confirmation-batch-v1`,
`research-confirmation-selection-v1` or `research-confirmation-operation-v1`.
`id` is 64 lowercase hex SHA-256 of version UTF-8, one zero byte, then the
existing type-tagged `canonical_bytes(body)`. No floats enter these bodies.
Parsing the original text rejects duplicate/unknown/missing fields at every
level, unsupported versions and float/exponent literals for integer fields.
Checked records have private fields, no Deserialize and immutable accessors.

| Bound | Value |
| --- | --- |
| Each raw body, stored record and sealed-report JSON | 4 MiB UTF-8, checked before parsing; stored output must also fit |
| Complete/selected candidates | 4096 (existing discovery hard cap); Validation nonempty, selection may be empty, Test nonempty |
| Each protected interval set | 1–128 sorted, disjoint intervals; touching intervals must already be merged |
| Instrument ID | 256 ASCII bytes, existing canonical market parser/formatter |
| Calendar/cost-profile identifier | 1–128 ASCII letters/digits or `-_.:/`; no trimming/aliases |
| Block rationale | 1–1024 UTF-8 bytes, not blank |
| Ordinary counts/indexes/seeds/row IDs | JSON integers no larger than 9007199254740991; run IDs positive, indexes/nonnegative requirements may be zero |
| Times | existing market range: 2000-01-01 inclusive to 2100-01-01 exclusive; an interval end may equal that exclusive maximum |

All hash fields below are 64 lowercase hex, except `strategyHash`
(`strategy-v2:<hash>`), `datasetHash` (`dataset-content-v2:<hash>`), `familyId`
(`trial-family-v1:<hash>`) and workspace/origin/key/request IDs (32 lowercase
hex). Family ID reproduces the existing registry's SHA-256 of canonical JSON
`{instrumentId, version:"trial-family-v1"}`; no old identity is changed.
Reject oversized input, never truncate or renumber it. Limits are structural,
not an expansion of P12e's supported statistical configurations.

## 2. Batch body

Exactly `contractVersion`, `stage`, `source`, `data`, `split`, `statistics`,
`selectionRule`, `candidates`:

| Object | Exact fields |
| --- | --- |
| `source` | `campaignId`, `campaignDocumentHash`, `workspaceId`, `runId`, `trialBatchId`, `instrumentId`, `familyId`, `engineManifestHash` |
| Each candidate | `candidateIndex`, `strategyHash`, `definitionHash`, `executionHash`, `resultArtifactHash`, `lineageHash`, `minimumWarmupBars`, `minimumEmbargoBars` |
| `data` | `snapshotId`, `instrumentRevisionHash`, `datasetHash`, `interval`, `calendarId`, `calendarHash`, `corporateActionsHash`, `costProfileId`, `costProfileHash`, `numericCostsHash`, `sourceEvidenceHash` |
| `split` | `contractVersion:"validation-split-v1"`, `totalBars`, `embargoBars`, `trainWindow`, `validationWindow`, `testWindow`, `validationSupport`, `testSupport` |
| Each window/support interval | `fromMs`, `toExclusiveMs` |
| `statistics` | `contractVersion:"research-confirmation-statistics-v2"`, `allocationId`, `bootstrapSamples`, `maxBootstrapSamples`, `maxRelativeStandardErrorPpm`, `blockLengthRationale`, `validation`, `test` |
| Each statistics stage | `blockLength`, `seed` |
| `selectionRule` | `version:"both-holm-reject-index-v1"`, `maxCandidates` |

`corporateActionsHash` is mandatory even when null; it must be nonnull for
ETFs. Data `interval` uses the existing market interval allowlist. Artifact,
numeric-cost and campaign references bind their **original** bytes/version;
this module does not parse/rehash legacy strategy floats or claim their bytes
exist. P06 snapshot/cost/calendar authenticity stays with the later host.

Candidates must be strictly ascending by index and equal the complete
`RegisteredSource` context for Validation. That typed pure input contains
`source` and `candidates`; all its fields/domains are revalidated, including
the cap, before comparison. The host must eventually prove it is the complete
actual registered enumeration, not a caller's invented subset. Missing,
substituted or extra members/source references are errors here.

Recompute the existing P12 split from total/embargo bars. Require nonempty,
ordered, nonoverlapping Train/Validation/Test windows of sufficient millisecond
length for their computed bar counts. Every candidate's warm-up/embargo
requirement must fit the frozen embargo count. Support sets must cover the
whole respective evaluation window, may conservatively include gaps, and
remain between the preceding role's end and the current role's end. Validation
and Test supports therefore cannot overlap or enter Train. The later calendar
adapter must prove actual bar/window/extra-input support; metadata alone does
not prove coverage. No instantaneous approximation of a daily bar is made.

Both stage block lengths must equal R3 of their computed bar count and satisfy
`(2L)^2 <= n`; samples/cap/seeds/error bound retain existing P12 domains.
This checks algorithm-domain consistency only, without running the statistic
or P12a or granting calibration support. Actual alpha is deliberately absent
until the final reservation. `maxCandidates` is positive and no greater than
the complete candidate count.

`stage` is exactly `{kind:"validation"}` or
`{kind:"test",validationBatchId,selectionId}`. Validation freezing requires
the registered context. Test freezing requires the checked Validation parent
and checked selection; every common field is identical to the parent, and
candidates are exactly the selected subset with original bindings. No changed
seed, Test region, split, cost, source, engine or rule is accepted. Empty
selection can be retained but cannot produce a Test batch.

## 3. Selection body and finite rule

Exactly `contractVersion`, `validationBatchId`, `alphaReservationId`,
`sealedReceiptId`, `reportSha256`, `selectedCandidates` (ordered indexes).
Freeze/parse requires the checked Validation batch, checked P13a reservation,
the supplied seal receipt ID and exact original report JSON bytes. Bind its
SHA-256, reservation family/allocation/batch/alpha and all report settings to
the parent. The receipt ID is not authentication in this pure API; the future
host must verify its durable seal and original bytes before supplying it.

Read the existing full v2 report with strict nested fields. Require exactly
both tests of every complete candidate, ordered by index then net-return /
benchmark-excess, correct observation counts, and a positive even whole-family
`familyTests` at least twice the complete batch size. Recompute raw ratios,
Holm rank/adjusted ratios and rejection flags from integer extreme counts,
frozen B, report alpha and whole-family m. Tie order is existing count/index/test
order. Reject inconsistent flags, denominators, ranks, missing/extra rows or
settings. Finite display floats are not selection inputs or new identity data;
original report bytes remain unmodified (NUMERIC-JSON-001).

The single rule selects candidates for which **both** tests reject, in ascending
candidate-index order, taking at most frozen `maxCandidates`. The supplied
selection must exactly reproduce it, including empty output. This is a
predeclared deterministic selection rule, not a new scoring/statistical test,
qualification PASS or a way to reduce the family's Holm count.

## 4. Operation and overlap

Operation body: exactly `contractVersion`, `originRegistryId`, `reservationKey`,
`requestId`, `workspaceId`, `confirmationBatchId`, `familyId`, `allocationId`,
`stage` (`validation`/`test`) and mandatory nullable `selectionId`. Freeze/parse
requires the checked batch and exact corresponding bindings. No epoch enters
the identity. Original origin/key/request body remains fixed when transport or
ownership changes. Constructors generate no random keys and spend nothing.
Pure relation: same ID is exact replay; same origin/key with another body is
an operation conflict; otherwise distinct. Future registry recovery must look
up the original origin/key rather than manufacture another operation.

Checked exposure footprints contain only canonical instrument and canonical
half-open intervals. The overlap routine compares those domains with a linear
two-pointer scan; source, dataset hash, interval label, role or price basis
cannot exempt intersecting support. Touching endpoints do not overlap.
Different verified instruments remain separate occupancy domains without an
independence or overall-alpha claim. Footprints are not durable exposure claims.

## 5. Verification and remaining authority

Independent committed TypeScript-compatible identity fixtures cover Validation,
selection, Test and original operation. Rust and Vitest reproduce their IDs.
Integer selection cases and tampering cover every binding, whole-family Holm,
empty/limited selection, report corruption and changed Test inputs. Strict
wire/size/count/time/canonical-interval bounds and a finite-set overlap oracle
cover malformed and exact-capacity inputs. No calibration or final-seed run.

The host still must authenticate complete lineage, campaign/snapshot/artifact
bytes and seal receipts, establish unseen/legacy provenance, union persistent
history and synchronize final admission/exposure/alpha writes. This pure slice
cannot see a missing suffix or authenticate supplied context. P13c-2 Train-only
context is Next after this implementation merges; runtime and statistical
policy gates from the design remain unchanged.
