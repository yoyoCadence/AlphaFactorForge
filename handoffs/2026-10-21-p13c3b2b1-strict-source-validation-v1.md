# Handoff: strict complete witnessed v5 source validation

Date: 2026-10-21
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b2b1-strict-archive-source
Status: Implemented and verified; authoritative delivery receipt is in the containing PR.

## Scope

From merged #203 (`1958e3d`), split 2b into strict source validation then atomic
destination union. The tracked implementation plan records this refinement.
The backend-only validator checks original closed canonical framing, inclusive
encoded/decoded bounds, exact nine counts/order/IDs/hash and complete nested v4.
Original blobs are never normalized. Exact disposable source reconstruction
preserves primary chains, protocols, foreign history and all restrictive facts;
strict independent witnessed graph/projection/FK verification precedes return.
No destination lock/write, importer activation, qualification or execution right.

## Verification

Independent existing Node byte/hash oracles and real SQLite fixtures exercise
complete managed/manual/empty/unknown/conflicted sources, all three original
blob kinds, rehashed dependency/conflict/proof loss, nested duplicate keys,
same-head upgrades, foreign events/checkpoints/receipts/genesis, unincorporated
witness stops and exact unknown-origin capacity. Existing registry/audit snapshots
and reopen prove validation never writes source state; the internal exact stager
also refuses an occupied registry before writes. Failure diagnostics stay static.

Complete restored checks, assertion-caught mutants and exact-head CI delivery
will be recorded in Resolution and the containing PR. No new calibration or
reserved-seed experiment; normal committed-report reproductions remain.

## Next action

Finish this section's full verification, fresh-main rebase, draft PR, all six
exact-head CI checks and guarded merge. Immediately continue 2b-2 with precise
proposed-state preflight design, then one-commit trial/alpha/confirmation union,
all independent claims/stops, two-hop/replacement/replay, qualification provenance,
joint capacities and late all-table/receipt rollback. Final admission remains later.

[Contract](../docs/research-confirmation-source-validation-v5.md),
[plan](../docs/plans/research-confirmation-transport-implementation-v1.md),
[previous delivered section](2026-10-21-p13c3b2a-witnessed-export-v1.md).

## Resolution (2026-10-21 - implemented and verified)

The complete restored locked suite passes **905 Rust** (297 library, 606 desktop,
2 service; one existing ignored), **1242 Vitest / 80 files**, typecheck/build,
all-target check/Clippy and scoped rustfmt. Only five existing Clippy warnings
remain. Sixteen new regressions include original nested event semantics with
all identities/chains/framing hashes rebuilt, inclusive file/header/projection/
line bounds and the exact 10,000 unknown-origin limit. Four deliberate mutants
(nested canonical comparison, source ID, original blob digest, body hash) failed
assertions and were restored byte for byte before complete verification.

Initial test-fixture errors (first-line size assumption and accidentally replacing
existing binding/statistics blobs while adding a witness) were fixed in tests;
production limits, graph guards and timeouts were not relaxed. The PowerShell
mutation harness was corrected to capture expected native stderr without stopping
at compiler progress output; its finally block restored exact bytes in both runs.

The containing PR records fresh-main rebase/rechecks, six exact-head CI jobs,
fresh base/reviews, expected-head guarded merge and clean-main delivery. Continue
2b-2 immediately afterward; complete v5 destination import is still unimplemented.
