# Handoff: P12d-2c campaign-bound discovery runs and stored admission

Date: 2026-09-30
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12d2c-campaign-runs` (rebased onto `main` `3cb58e7` after PR #128 merged)
PR: [#129](https://github.com/yoyoCadence/AlphaFactorForge/pull/129) (draft)
Status: Published as draft PR #129 for CI and review; P12d-2d (command/service surface and UI authoring) is next.

## Summary

P12d-2c connects the P12d-2a snapshot resolution and the P12d-2b admission
evaluator to the discovery runner. A run can now be started for one declared
instrument of a frozen campaign; its binding is checked before any write,
its trials register under the declared snapshot's family, and its admission
decision is stored in the same transaction that queues it. Contract:
[`docs/research-campaign-declaration-v1.md`](../docs/research-campaign-declaration-v1.md)
("P12d-2c campaign-bound runs").

It builds on the PR #127 review fixes (verified snapshot ID and batch ID in
admission), which are published separately as PR #128.

## Decisions

- Maintainer decisions of 2026-09-29 apply unchanged: one run per instrument;
  `NOT_ELIGIBLE` runs still explore; freeze the run's `feePct`/`slipPct`.
- **Technical choice (this slice):** the campaign binding is a separate start
  argument (`CampaignStart { declaration, instrumentId }`), not a new
  `discovery-config-v4` field. A config envelope change would also require the
  TypeScript mirror parser and parity fixtures; persisting the binding in the
  decision row keeps the run config and its hash unchanged. Revisit if the UI
  needs to author campaign runs as a single config document.
- The existing v3 preflight still refuses a run whose fold plan is infeasible,
  so `walk_forward_not_eligible` is defence in depth for campaign runs.

## What changed

- Workspace migration `0010_campaign_admission`: append-only
  `research_campaigns` and `campaign_run_admissions` (one decision per run,
  report JSON, frozen costs, epoch).
- `db::campaign`: `record_campaign_admission` (inside the enqueue transaction;
  refuses a decision whose candidates differ from the queued lineage by index
  or strategy hash) and `get_campaign_admission` (re-freezes the stored
  declaration).
- `db::discovery::start_discovery_run_for_campaign`, sharing the existing
  enqueue body with `start_discovery_run_bound` (behaviour unchanged).
- Runner: `start_campaign_for_request`; `bind_campaign` before any write;
  `register_lineage_with` takes the declared snapshot; admission evaluated
  from the registration's count. `candidate_walk_forward_report` is shared
  with `declared_walk_forward_plan`.
- `research::campaign_snapshot::resolve_campaign_instrument`,
  `research::campaign_admission::validate_campaign_binding`,
  `trial_ledger_workspace::unknown_legacy_families` made crate-visible.
- Migration pins in `runtime/mod.rs` and `db/repositories.rs` tests updated
  from `0009`/9 to `0010`/10, with structure checks for the new tables.

## Required Action / Decision

1. **P12d-2d:** expose campaign starts through the command/service surface
   (request envelope with the declaration and instrument, typed client, and
   an idempotent replay), and a read-only view of a run's decision.
   **Maintainer decision (2026-09-30): the UI must be able to author
   campaigns** — not only start already declared ones. P12d-2d therefore also
   needs a campaign authoring flow (instrument/snapshot selection, sample
   policy with rationale, sampling, freeze preview showing the campaign ID)
   whose output is validated by the backend `freeze_campaign`, never trusted
   from the frontend. Split it into reviewable slices before implementing.
2. **P13:** call `TrialLedger::fence_admission` synchronously with
   confirmation admission; a stored decision or `Unchanged` result is not a
   durable permit.
3. Still not covered: numeric P06 cost profiles, raw source artifact byte
   re-verification, pre-P05 history resolution.

## Verification

- `cargo test --locked`: **488 passed (101 library + 385 desktop + 2 service)**
  = 483 on PR #128 + 5 campaign runner tests.
- New runner tests (real P06 snapshot and trial ledger): eligible decision
  stored with the enqueue and covering exactly the queued candidates;
  `NOT_ELIGIBLE` run still completes its attempt; contradicting fold policy,
  undeclared instrument, missing snapshot and v2 config write nothing (no run
  row, no admission, no registered trial); a dataset with two snapshots
  registers under the declared family; missing/substituted candidates refused
  at the store, plus append-only triggers.
- Mutation checks, each caught: registering with a guessed snapshot; skipping
  the pre-write binding check; skipping the store's completeness check; not
  storing the decision with the enqueue.
- `cargo check --locked --all-targets` pass; clippy shows only the five
  existing warnings; rustfmt applied to the two new Rust files only;
  `git diff --check` pass.
- No TypeScript, UI or e2e change; npm/Playwright not rerun.
