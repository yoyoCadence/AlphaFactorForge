# Handoff: P12d-2b campaign admission evaluation and ledger freshness fence

Date: 2026-09-29
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12d2b-campaign-admission` (from merged PR #126, `b5667fb`)
PR: pending
Status: Implemented locally; P12d-2c (persistence and runner wiring) is next.

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
