# Handoff: P12c-1 Train-only walk-forward feasibility

Date: 2026-09-24
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12c1-walk-forward-feasibility` (from merged PR #120, `7537fa3`)
Status: Resolved for P12c-1 (merged PR #121); P12c remains In Progress.

## Summary

The pure Rust `research-walk-forward-v1` precheck derives the existing outer split and builds expanding inner folds exclusively within Train. It reports `NOT_ELIGIBLE` with no partial folds when either the outer split or the declared inner sample requirement cannot fit. This makes the sample-length boundary reviewable before P12c-2 executes any fold.

## Decisions and scope

- The plan explicitly declares total bars, the embargo shared by the outer split and inner folds, minimum initial training bars, exact inner validation bars and 2–128 folds. There are no inferred sample thresholds or AlphaBTC constants.
- Required Train history is `minimumTrainBars + foldCount × (embargoBars + foldValidationBars)`. Surplus history extends the first training span. Later training spans may include earlier inner validation because they are all inside outer Train; no outer Validation/Test range enters the report or a fold.
- Malformed or out-of-domain plans are errors. Valid short histories are `NOT_ELIGIBLE`. A requirement above the JavaScript safe-integer bound is reported as null and cannot pass.
- The module is not called by the runner or admission path. It does not execute folds, calculate trade/return evidence, or produce a confirmation `PASS`.

## Required Action / Decision

1. P12c-2 should execute the declared Train-only folds, persist their window/protocol identity and evaluation evidence, and refuse a plan whose feasibility report is not `ELIGIBLE`.
2. P12d must freeze and justify minimum training/evaluation lengths, instrument/snapshot binding and protocol versions before admission. A caller-provided threshold of 1 is syntactically valid here but is not evidence of sufficient market history.
3. Keep outer Validation/Test unavailable to search, prompt construction and parameter tuning. P12e/P13 retain confirmation calculations and consumption ownership.

## Verification

`cargo test --locked --quiet`: 436 Rust tests pass (93 core, 341 desktop, 2 service smoke). `cargo check --locked --all-targets` passes. `cargo clippy --locked --all-targets` passes with five pre-existing warnings in unrelated files. Targeted rustfmt and `git diff --check` pass. No packaged Tauri UI rerun (no UI change); remote CI pending PR.

## Resolution (2026-09-25)

P12c-1 merged through [PR #121](https://github.com/yoyoCadence/AlphaFactorForge/pull/121) as `5069805`. P12c-2a now executes fixed candidates on the declared Train-only folds and returns versioned evidence; see [its handoff](2026-09-25-p12c2a-walk-forward-execution-v1.md). The original P12c-2 runtime/persistence action remains P12c-2b, and admission remains P12d.
