# Handoff: P12d-2b campaign admission evaluation and ledger freshness fence

Date: 2026-09-29
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12d2b-campaign-admission` (from merged PR #126, `b5667fb`)
PR: [#127](https://github.com/yoyoCadence/AlphaFactorForge/pull/127) (draft)
Status: PR #127 merged (`226f043`) before its review fixes were committed; the fixes below were carried unchanged to `fix/pr127-review-fixes` (`257d3d9`) and published as PR #128. P12d-2c (persistence and runner wiring) is next.

## Summary

After PR #126 closed the R1–R5 ledger gaps, the next open P12 item was the
rest of P12d-2. It was split again: this slice (P12d-2b) adds the admission
decision and the trial-ledger §6.4 freshness fence as backend functions with
no schema, command or runner change. P12d-2c will persist and wire them.

## Maintainer decisions (answered 2026-09-29)

The user chose the recommended option for all three product questions:

1. **Campaign ↔ run: one run per instrument.** A campaign-bound run names one
   `campaignId` + `instrumentId`. Rationale: the runner is single-dataset and
   the trial family is single-instrument; a multi-instrument run would need a
   runner rewrite.
2. **`NOT_ELIGIBLE`: exploration still runs; only confirmation is blocked.**
   Consistent with trial-ledger-v1 §7. Refusing enqueue would not undo the
   already registered trials anyway.
3. **Costs: freeze the run's `feePct`/`slipPct`.** P06 `cost-profile-v1` has no
   numeric values; P12d-2c stores the run's numbers with the decision and
   requires a confirmed snapshot cost status. The missing numeric cross-check
   is a recorded limitation.

These are recorded in
[`docs/research-campaign-declaration-v1.md`](../docs/research-campaign-declaration-v1.md)
("P12d-2b" section).

## What changed

- `research::trial_ledger` (binding module): `check_binding`'s prefix decision
  is extracted into a helper (behaviour unchanged) and reused by the new
  `TrialLedger::fence_admission(batchId, &AdmissionSnapshot) -> AdmissionFence`,
  which reads the prefix proof and the current count in one registry
  transaction. New public types: `AdmissionSnapshot` (serializable, from
  `AdmissionCount::snapshot()`), `AdmissionFence`
  (`Unchanged` / `Grew` / `Blocked`) and `FenceBlocked`
  (`Prefix` / `Admission` / `Inconsistent`).
- New `research::campaign_admission` (`research-campaign-admission-v1`):
  `evaluate_campaign_admission` over a frozen campaign, one declared
  instrument, the P12d-2a `ResolvedInstrument`, an optional stored snapshot
  observation, the batch ID, the ledger input (`Registered(&Admission)` or
  `Fenced(&AdmissionFence)`), `legacy_trials_unknown` and per-candidate P12c
  reports. Binding contradictions are errors; feasibility failures are ordered
  `NOT_ELIGIBLE` reasons. Never a `PASS`.
- Docs: campaign contract P12d-2b section; trial-ledger-v1 §21.

## Required Action / Decision for P12d-2c

1. Workspace migration (next is `0010`): immutable campaign declarations
   (re-frozen on read and compared with the stored ID) and append-only
   admission decisions linked to the run, holding the report JSON, the
   `AdmissionSnapshot`, the snapshot observation and the frozen
   `feePct`/`slipPct`.
2. Run configuration: an explicit campaign binding (`campaignId`,
   `instrumentId`); refuse to start when the dataset, interval, walk-forward
   declaration or costs contradict the campaign (the evaluator's errors).
3. Runner: derive each candidate's P12c report from verified inputs (reuse the
   `declared_walk_forward_plan` derivation, returning the report instead of
   failing); resolve the declared snapshot (P12d-2a) inside the start path;
   register the batch with the **declared** snapshot's instrument rather than
   `unique_snapshot` (a dataset with two snapshots currently registers as
   `family_unknown`, which would force `NOT_ELIGIBLE`); write the decision in
   the enqueue transaction.
4. Keep the fence callerless until P13 schedules confirmation work, or expose a
   read-only "refresh decision" for status display — decide in 2c.
5. Still not covered: raw source artifact bytes and a numeric cost profile
   (decision 3), and pre-P05 history resolution (§9).

## Verification

- `cargo test --locked`: **479 passed (101 library + 376 desktop + 2 service)**
  = 472 after PR #126 + 7 new tests (5 campaign admission, 2 ledger fence).
- `cargo check --locked --all-targets`: pass.
- `cargo clippy --locked --all-targets`: the five existing warnings only
  (`discovery_core/backtest.rs`, `score.rs`, `commands/file_commands.rs`).
- rustfmt applied to the two new files only (the crate is not rustfmt-clean
  overall); `git diff --check` pass.
- Mutation checks, each caught by at least one new test: fence skips the prefix
  proof; fence treats growth as unchanged; precision never blocks; a changed
  snapshot is ignored; the walk-forward plan is not compared with the policy.
- No TypeScript, UI, e2e, migration or dependency change; npm and Playwright
  were not rerun.

## Resolution — PR #127 acceptance review (2026-09-30)

Reviewed PR head `35d02a9e11aea0fadaa638e3ea879d8218383def` against merged
PR #126 (`b5667fb`). The direction is sound: one instrument per run, exploration
separate from confirmation eligibility, and a pure evaluator/fence before
persistence and runner integration. The fixes below are in the existing feature
branch's working tree; they have not been committed or pushed.

Three correctness gaps were reproduced and fixed:

1. **Snapshot identity (P2).** The evaluator checked instrument and dataset,
   then copied the declaration's snapshot ID into the report. A resolution for
   snapshot A could therefore be reused with a declaration naming snapshot B
   for the same dataset. `ResolvedInstrument` now carries the verified snapshot
   ID, the evaluator compares it to the declaration, and the observation uses
   that verified ID. The existing real-database resolver test checks the returned
   identity; the new admission regression rejects cross-snapshot reuse.
2. **Batch identity (P2).** `AdmissionCount` and `AdmissionSnapshot` lacked the
   batch ID. The evaluator accepted a count belonging to another batch, and the
   fence returned `Unchanged` for another equal-sized batch in the same family.
   The count now retains the batch ID from the registry read, the saved snapshot
   carries it, and both evaluator and fence compare it. Existing fixtures now
   pass actual registered IDs. This adds no migration: the new admission
   snapshot is not persisted until P12d-2c.
3. **Walk-forward evidence consistency (P2).** A valid but infeasible plan with
   its report status changed to `ELIGIBLE` was accepted. The evaluator now
   recomputes the P12c report and compares all fields, rejecting inconsistent
   status, reasons, bounds or folds. Negative candidate indexes are also refused.
   Genuine infeasibility still returns `NOT_ELIGIBLE`, preserving exploration.

Four new tests failed against the original implementation, then passed after
the fixes:

- `admission_rejects_a_different_declared_snapshot_for_the_same_dataset`
- `admission_rejects_a_count_from_another_batch_in_the_same_family`
- `admission_rejects_a_walk_forward_status_that_disagrees_with_its_plan`
  (also checks missing and altered folds)
- `admission_fence_rejects_another_equal_sized_batch_in_the_same_family`

Verification after fixes:

- `cargo test --locked`: **483 passed (101 library + 380 desktop + 2 service)**.
- `cargo check --locked --all-targets`: pass.
- `cargo clippy --locked --all-targets`: pass with the same five existing
  warnings in `backtest.rs`, `score.rs` and `file_commands.rs`.
- `git diff --check`: pass. Rustfmt applied only to the admission module and
  its tests, avoiding unrelated formatting changes.
- PR head `35d02a9` already passed all six CI jobs in
  [run 36630471840](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36630471840).
  Those checks predate these local fixes; no new remote CI run is claimed.
- No frontend changes; npm/Playwright were not rerun locally.

Integration acceptance remains required, with the boundary now explicit in the
campaign and ledger contracts:

- P12d-2c must derive the **complete** candidate set and each strategy's embargo
  from frozen run inputs. Report self-consistency alone does not prove candidate
  membership/completeness or a strategy-derived embargo. Include missing or
  substituted candidate counterexamples in the runner tests.
- P12d-2c must use the declared snapshot when a dataset has multiple snapshots,
  and persist the decision plus the run's frozen fee/slippage values atomically
  with enqueue. The numeric P06 cost-profile limitation remains as agreed.
- The fence commits its read transaction before returning; it observes a moment
  in time. P13 must synchronize the final fence with confirmation admission,
  preventing intervening registration/import from invalidating the decision.
  A stored `Unchanged` result is not a durable scheduling permit.

### Publication of the review fixes (2026-09-30)

PR #127 was merged at `35d02a9` (merge `226f043`) while the fixes above were
still uncommitted in the old feature branch's working tree. They were stashed,
moved unchanged onto `fix/pr127-review-fixes` from `226f043`, and committed as
`257d3d9`. Re-verified there: `cargo test --locked` **483 passed
(101 + 380 + 2)**, the four regressions pass, clippy shows only the five
existing warnings, and `git diff --check` passes. Published as
[PR #128](https://github.com/yoyoCadence/AlphaFactorForge/pull/128); remote CI
is the authority for the six jobs.
