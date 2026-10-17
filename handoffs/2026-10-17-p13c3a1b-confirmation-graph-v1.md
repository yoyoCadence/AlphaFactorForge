# Handoff: P13c-3a-1b complete pure confirmation graph

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3a1b-confirmation-graph
Status: Merged as PR #192; storage is next

## Summary

The pure graph now connects the existing frozen batches, actual alpha shares,
exposures, integer admission proofs, original binding/statistics bytes, execution
events, Validation selection and Test. This completes the pure prerequisite to
P13c-3a-2 storage from merged #191 (`3432981`).

The [contract](../docs/research-confirmation-graph-v1.md) fixes separate host
observations, historical count/prefix handling, typed dependency closure,
bounded completion versus materialized validation, and actual conflict targets.
There is no database/schema/IO/provider/runtime change or execution permit.

## Review Notes

- Admission reproduces every integer/enum field of unchanged P12a from the
  declared share, frozen settings and original whole-family/registered-batch
  counts. A smaller Test selection cannot lower m or register new trials.
- Binding-v5 text preserves its original UTF-8 hash and strict dynamic family
  keys. Host evidence and typed pre-state IDs must cover saved references.
  Seals reuse the existing integer Holm/report checker for either stage.
- Iterative Kahn traversal rejects missing/wrong-kind/cyclic dependencies.
  Confirmation exposure/admission halves must match. A binding coverage list
  cannot legitimize an otherwise orphaned statistics or binding blob.
- Valid contradictory admissions/starts/seals/terminals remain append-only
  history. Completion derives missing witnesses; readers require them stored.
  All supplied witnesses independently prove the relation and exact targets.
- Temporal intersection is not transitive. The deterministic interval sweep
  uses real simultaneous witnesses, including the A/B/C chain and touching
  boundary cases, without quadratic all-pairs output. Manual-after-seal
  exemption applies only to the one containing confirmation exposure.
- Manual/legacy conflicts never gain family targets from later registration.
  Unknown origins remain readable restrictions. False `is_restricted()` does
  not clear occupancy and grants no qualification or recovery permission.
- Pure functions cannot authenticate consistently invented/truncated host
  observations, current database freshness or result-file presence. P13c-4's
  private final transaction must still supply complete current observations.

## Verification

The independent Node fixture preserves a full Validation-to-Test chain, original
decimal/whitespace report bytes, and independent BigInt precision expectations;
TypeScript independently reproduces all record/reservation/blob identities.
Fixture regeneration is byte-identical. No bootstrap or final calibration seed
was run and no supported statistical policy changed.

Four executable mutation checks were caught by failed assertions: bypassed
precision-proof comparison, bypassed exact family m, bypassed frozen seal engine,
and manual exemption wrongly hiding other operations. The fourth initially
survived the equal-start test order; a later manual start now exercises the
other sweep branch. Mutated files were restored in every case.

Focused regressions include exact 10,000-row/128,000-interval limits, refusal
when a required conflict would exceed the combined row cap, 64 MiB aggregate
bytes, 8/4 MiB original blob text and 10,000 unknown origins, plus closed JSON,
dynamic duplicate keys, rehashed forged evidence, original counts, actual
conflicts and a 10,000-node iterative cycle test. Full local results: 747 Rust
(259 library + 486 desktop + 2 service; one ignored), 1234 Vitest in 77 files,
typecheck, production build and all-target cargo check. All-target Clippy adds
no warnings (only the five existing backtest/score/file-command warnings).
The four mutation
checks passed after original sources were restored. Final-head delivery CI
must pass all six jobs before merge; its receipt belongs in the delivery PR.

## Next Step

After this slice's final-head CI and merge, move P13c-3a-2 through Next to
In Progress: append-only schema 0006, bounded graph loads/index projections,
original-operation lookup, birth/legacy evidence and real transaction rollback.
Binding v5, transport v5 and live admission remain their ordered later slices.
Production activation still requires statistical policy and manual mediation.

## Resolution (2026-10-17)

[PR #192](https://github.com/yoyoCadence/AlphaFactorForge/pull/192) merged as
`d873917c66f54fd547e733bd3a56e8d15c1108d4` using final head
`5e1d59fca7b1f72bc7152119dcb0ad40d69faf18` after all six jobs succeeded in
[CI run 38983005925](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38983005925).
CI confirmed 747 Rust (one ignored), 1234 Vitest, 98 first-pass E2E and real
native render/SQLite/metrics/events with owned-process/temp cleanup and exact
browser-policy restoration. Fresh review/thread checks had no blockers.
Local main was fast-forwarded and its tree matches the verified head.

The authorized loop continues with P13c-3a-2a, then 3a-2b: split the already
specified storage contract into schema/provenance guards and checked graph
read/write/lookup PRs to retain session-sized review scope. Fields, capacities,
versions, production policy and task ordering remain the merged design.
