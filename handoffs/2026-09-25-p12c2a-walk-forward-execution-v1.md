# Handoff: P12c-2a fixed-candidate walk-forward execution

Date: 2026-09-25
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12c2a-walk-forward-execution` (from merged PR #121, `5069805`)
Status: Implementation and local verification complete; P12c-2b and P12d remain open.

## Summary

The standalone Rust `execute_candidate_walk_forward` entry point evaluates an already enumerated params or fixed DSL candidate on every declared inner Train fold using the existing costed backtest engine. It returns `walk-forward-evidence-v1` with plan, window, metric, trade, protocol and candidate/dataset identity fields. Infeasible declarations return `NOT_ELIGIBLE` with no fold execution.

## Decisions and scope

- Before execution, check the candidate strategy content/hash and costs, dataset identity and bar count, derived embargo, and minimum initial Train length against signal lookback.
- Build signals separately up to each Train and inner validation endpoint. Only slice candles through the outer Train endpoint; no outer Validation/Test candle or future inner candle is needed for an earlier fold.
- Reuse the current execution/metrics contracts and encode non-finite metrics through the existing metrics codec. The evidence is serializable but is not yet stored or used for ranking/admission.
- Leave `discovery-config-v1/v2`, the production runner, database, commands and UI untouched. Existing immutable candidate artifacts do not gain a walk-forward field in this slice.

## Required Action / Decision

1. P12c-2b: declare the fold plan in a new versioned run contract, call the executor from the runner, and atomically persist its evidence with candidate/attempt lineage. Preserve existing v1/v2 config semantics and the P12c-1 fail-closed eligibility boundary.
2. P12d: freeze campaign sample thresholds, instrument/snapshot and protocol versions; decide how fold evidence affects admission. This slice makes no Gate, Score or confirmation `PASS` claim.
3. Keep Test unavailable to search, ranking, prompts and tuning. P12e/P13 continue to own statistical confirmation and one-time reveal.

## Verification

`cargo test --locked --quiet` passes 440 Rust tests (93 library, 345 desktop, 2 service smoke), including four new fold-execution tests for params/DSL, suffix isolation and invalid/ineligible declarations. `cargo check --locked --all-targets` passes. `cargo clippy --locked --all-targets` passes with five pre-existing warnings in unrelated files. `git diff --check` passes. No packaged Tauri UI or live research campaign was run; no UI or production runner path changed.
