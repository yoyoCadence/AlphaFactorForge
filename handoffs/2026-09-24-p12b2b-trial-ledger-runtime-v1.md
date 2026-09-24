# Handoff: P12b-2b trial-ledger runtime binding

Date: 2026-09-24
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12b2b-trial-ledger-runtime` (from `origin/main` `9a2a719`, merged PR #119)
Status: Implementation and local verification complete; GitLab MR publication waits for a GitLab project URL (this checkout has only a GitHub `origin`).

## Summary

Workspace migration `0009` links every newly queued discovery attempt to a registry event. The shared desktop/service open path verifies the registry binding and backfills P05 attempts before the runner starts. A registry commit precedes the workspace transaction that stores events and jobs; claim checks both the saved binding and current event existence. This closes the execution path for new NULL `trial_event_id` attempts while preserving legacy evidence.

## Decisions and scope

- Production registry directory: the user's local application-data directory, `com.alphafactorforge.trial-ledger`, outside the workspace. Unit and service smoke tests use isolated temporary registries. A bound workspace never recreates a missing registry.
- First binding groups legacy attempts by known instrument, or by `family_unknown` if the dataset has no unique snapshot. It keeps unavailable legacy split/seeds as NULL. The registry commits before a workspace transaction writes every legacy ID and the binding head. Reopening after a failed workspace transaction replays the same batches.
- Current discovery candidates register as effective `variant` events before enqueue. A pre-P05 queued run that has no attempt receives a `legacy` event before resume. The current discovery path declares `testsPerTrial = 1`; P12d must freeze and validate the full campaign protocol before any confirmation decision.
- Startup reports registry-only orphan event IDs and affected families with unmatched pre-P05 validation records. A NULL attempt link rolls back claim with `trial_not_registered`; the runner also checks the event exists in the current verified registry.
- No admission/confirmation result is enabled here. Non-effective benchmark portability remains fail closed as in P12b-1b.

## Required Action / Decision

1. Review the registry path and protocol declaration before merge. P12d must not assume a different `testsPerTrial` for a family already pinned at 1 without a new protocol contract.
2. P12d should consume the workspace's `legacy_trials_unknown` report to block affected family qualification and wire the existing P12a count into admission; never infer a zero count from a missing snapshot.
3. If an operator needs to resolve an orphan registration after a crash, inspect the startup report and replay only the identical request batch. No event deletion or count refund exists in v1.

## Verification

`cargo test --locked --quiet`: 431 Rust tests pass (88 core, 341 desktop, 2 real service smoke). `cargo check --locked --all-targets` passes. `cargo clippy --locked --all-targets` passes with only five pre-existing warnings. `rustfmt --check` for the new module and `git diff --check` pass. Focused cases cover terminal legacy backfill, recoverable split identity, frozen event links, unregistered enqueue/claim rollback, orphan replay/report, pre-P05 unknown-family reporting, missing bound registry, and desktop/service hand-over with registered event IDs. Packaged Tauri UI and remote CI have not been run.

## Publication blocker

This repository configures only `origin=https://github.com/yoyoCadence/AlphaFactorForge.git`. No GitLab remote or project URL is available, so a GitLab MR cannot be created or linked until the maintainer supplies the target project. The code, tests, documentation, and local commit can be completed independently.
