# Handoff: P12c-2b walk-forward runner binding

Date: 2026-09-25
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12c2b-walk-forward-runtime` (from merged PR #122, `a77de2d`)
Status: Implementation and local verification complete; P12d admission remains open.

## Summary

`discovery-config-v3` is a new, strict run declaration for fixed-candidate Train-only walk-forward evaluation. The runner preflights every candidate against its derived embargo and the verified dataset before creating a run; a short history returns `NOT_ELIGIBLE` without run or attempt rows. Workers execute the P12c-2a folds, and the coordinator saves their complete evidence inside the existing attempt-linked, immutable candidate artifact path as `candidate-result-v2`.

## Decisions and scope

- v3 retains v2's params/fixed DSL candidates and adds only `walkForward` (`minimumTrainBars`, `foldValidationBars`, `foldCount`) plus pinned planner/evidence contracts. Total bars and embargo are derived for each candidate, never caller-supplied. The same raw declaration is saved in `discovery_runs.config_json` and rechecked on resume.
- Any ineligible candidate rejects the whole run before enqueue; an omitted worker fold result fails the run rather than writing an incomplete completed attempt. Existing v1/v2 parsing and `candidate-result-v1` artifacts stay valid.
- `candidate-result-v2` keeps the v1 fields and adds `walkForward`. The current content-addressed artifact store writes the file before the one-candidate database transaction links it to the attempt, jobs, summaries, record and progress. The read path can return both versions.
- No new migration, dependency, command or UI flow. No fold result affects Gate, Score, ranking or qualification yet.

## Required Action / Decision

1. P12d must freeze campaign instrument/snapshot identity, minimum sample lengths and protocol versions, then connect P12a, the P12b ledger and P12c fold evidence to admission. Caller-provided valid lengths here do not establish sufficient market history by themselves.
2. P12e/P13 retain statistical confirmation and one-time Validation/Test reveal. Do not use fold evidence to claim a confirmation `PASS`.
3. A live campaign, packaged Tauri UI and long-running v3 resume across process hand-off have not been run. The in-memory integration tests cover start, execution, immutable readback and failure boundaries.

## Verification

`npm test`: 975 Vitest tests pass. `npm run typecheck` and `npm run build` pass. `cargo test --locked --quiet`: 444 Rust tests pass (94 library, 348 desktop, 2 service smoke). `cargo check --locked --all-targets` passes. `cargo clippy --locked --all-targets` passes with five pre-existing warnings in unrelated files. `git diff --check` passes. Remote PR CI pending.
