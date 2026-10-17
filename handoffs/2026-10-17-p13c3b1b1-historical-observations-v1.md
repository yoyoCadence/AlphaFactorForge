# Handoff: P13c-3b-1b-1 historical source reconstruction

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b1b1-historical-observations
Status: Locally verified; delivery CI/merge pending

## Summary

From merged #195 (`9d05e9b`), reconstruct independent original trial prefix,
candidate membership and historical count/protocol observations from a bounded
complete source snapshot. No receiver counts, audit receipt authority, IO,
SQL writes, persisted witness/schema/wire extension or runtime writer switch.
[Contract](../docs/research-historical-source-observations-v1.md),
[plan](../docs/plans/research-historical-observation-plan-v1.md).

## Responsibilities / Boundaries

- `trial_ledger_transfer/historical_observation.rs` bounds original bytes to
  8 MiB, runs the existing full trial decoder/kind checks with a closed reference
  resolver, checks exact v5 prefix/restrictive trial evidence and original full
  index/hash/workspace/family membership, derives counts/protocol and returns
  immutable historical observations plus original archive SHA.
- `trial_ledger_transfer.rs` adds only the child-module/reexports and a narrow
  resolver extraction. Existing import validation keeps its SQL fallback;
  reconstruction supplies no external reproduction resolver. No global format.
- The existing pure registered-source domain/order validator becomes public
  for backend reuse, without changing its checks or arithmetic.
- `source_claims()` deliberately preserves the trust boundary: artifact,
  campaign/run/engine metadata absent from trial rows are supplied claims,
  not authenticated files/source or current authority. No current AdmissionCount,
  alpha reservation, fresh owner observation or execution capability is returned.
- Root plan/board split the context prerequisite explicitly, preserving task
  order. The codec handoff records #195's actual six-job CI and merge receipt.

## Verification

**805 Rust** (270 library + 533 desktop + 2 service; one ignored),
**1236 Vitest / 78 files**, typecheck, production build, all-target cargo check
and Clippy pass; only the five existing warnings remain. Scoped rustfmt and
`git diff --check` pass. Source fixtures use real registered batches/protocols and
original snapshots. Tests include receiver growth/reopen, actual same-origin
fork/union conflict, same-head protocol upgrade, complete membership, every
prefix/restrictive mismatch, audit-number independence, closed reproduction
versus real SQL import fallback, nonportable benchmark refusal, rehashed invalid
payloads, exact 8 MiB original input and static diagnostics. The test fixture
closes SQLite before its owned Windows temporary tree is removed.

Four executable mutations failed by test assertions rather than compile errors:
use audit receipts as family-count authority, ignore original registry ID,
compare candidate counts without index/hash membership, and ignore protocol
upgrades. Source was restored before the passing full suite. An interrupted
first protocol-mutation attempt was not counted; its isolated rerun failed by
the expected test assertion. No owned `aff-historical-source-*` temp directories
remained. Local TEMP logs: `aff-historical-mutant-*.log` and
`aff-p13c3b1b1-{rust,check,clippy}.log`.

## Required Next Work

After final-head six-job CI/merge, 3b-1b-2 freezes durable context storage,
graph dependencies, capacities, migration/legacy behavior and complete transfer
before new persisted formats are implemented. Current schema-6 graph storage
still requires supplied GraphContext; this primitive does not persist it.
Do not synthesize independent facts from receipts or receiver-primary rows.
Whole-registry observation/adoption and real old-build refusal remain 3b-1b-3.
Production qualification still requires later policy/mediation/final-fence gates.
No bootstrap, calibration/final seed or protected execution was performed.
