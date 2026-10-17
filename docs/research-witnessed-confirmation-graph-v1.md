# Strict source-witness confirmation graphs

Date: 2026-10-17
Scope: P13c-3b-1b-2a-2; implementation plan
[here](plans/research-witness-graph-integration-plan-v1.md).

`complete_witnessed_confirmation_graph` and
`validate_witnessed_confirmation_graph` extend the existing pure archival graph
with `BlobKind::SourceWitness`. Witnesses consume the existing logical blob,
10,000-row and 64 MiB joint quotas; their individual bound stays 8 MiB. All
stored witness bytes require exact matching `SourceWitnessObservation` host
facts, even when the witness is a standalone root. Unknown flags do not excuse
a missing/bad proof, and a valid same-prefix proof cannot cover another SHA.

`SourceWitnessObservation::from_host_facts` freezes an outer snapshot, original
anchor and generic trial facts with consistency/domain checks. It does not
parse/authenticate the nested archive. The backend must first reconstruct the
complete original archive through the existing original-only resolver. Source
metadata absent from trial rows remains supplied claims. All same original
`(workspaceId, runId, trialBatchId)` metadata must agree; equal source/anchor
facts coalesce and unequal facts fail regardless of order.

`GraphContext.witness_coverage` carries explicit host current-accounting
observations keyed by exact witness SHA. An empty gap vector asserts the WHOLE
original snapshot's event/protocol facts are incorporated; matching the local
header prefix alone is insufficient for foreign-origin rows. Every declared
unincorporated origin must have an actual permanent unknown/conflict stop.
Coverage IDs/domains/order/duplicates are checked. The current registry host
must establish coverage from actual rows in one transaction in the next slice;
the pure graph neither reads nor ingests any trials. Original quarantine and
origin-conflict witnesses must still be retained even with uncertainty.

Every Validation source needs a complete source witness, including frozen-only
sources with no admission. Test inherits its full original source. An admission
depends on a witness matching that source and EVERY original binding trial
anchor component. All supplied roots are checked first; choose the smallest
matching SHA for a deterministic material edge, avoiding quadratic fan-out
from audit-only duplicate witnesses. Witnesses have no binding dependencies.
Existing binding pre-state edges and iterative Kahn checks remain unchanged.
Every statistics/binding blob must still be directly used by an admission/event.

Generic archived protocol/count facts do not widen eligibility. Admissions
still require the original two-test/all-effective complete batch, exact
historical counts, P12a evidence and unchanged report checks. A same-head
protocol-3 witness cannot replace the original protocol-2 admission source.
Supplied legacy eligible trial observations must agree with selected facts.

Current binding protocol coverage now follows existing `LedgerEvidence::covers`:
an actual permanently quarantined family covers its saved higher HWM. Keep the
actual current map, exact original anchor comparison and restricted state.
This is archival consistency, never current qualification/execution/recovery.

For staged integration, the old `complete_confirmation_graph` /
`validate_confirmation_graph` remain explicit schema-6 compatibility APIs and
refuse witness blobs. The schema-6 writer also has a defensive refusal; old
SQL schema/contents are unchanged. These APIs do not satisfy the new strict
managed-history contract. Schema 7/current host coverage/restoration must wire
the strict entry points before binding observation or transport activation.

The new independent strict fixture uses #198's actual synthetic four-event
source and rebuilds every dependent record/binding/admission identity; the old
genesis fixture stays legacy compatibility only. Synthetic m=8 remains structural
evidence, without statistical support. No bootstrap/calibration/reserved seed,
runtime activation, file authentication, prompt/generation/tuning/ranking feed
or live alpha/owner/count/release permission is added.
