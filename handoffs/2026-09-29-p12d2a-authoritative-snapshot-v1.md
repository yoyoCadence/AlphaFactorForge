# Handoff: P12d-2a authoritative campaign snapshot resolution

Date: 2026-09-29
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12d2a-authoritative-snapshot` (from merged PR #124, `c787e8d`)
PR: [#125](https://github.com/yoyoCadence/AlphaFactorForge/pull/125) (ready for review; not merged)
Status: Implementation published for review; P12d-2 admission integration remains open.

## Summary

The Next queue is empty and P12d-2 is the next dependent P12 item. This
independently reviewable slice adds a read-only backend verifier for the P12d-1
frozen declaration's exact P06 snapshots. It cannot start a run or return a
qualification decision.

## Decisions and scope

- Resolve `snapshotId` exactly. Do not accept a newer snapshot for the same
  dataset or infer market semantics for a legacy dataset.
- Compare the declaration with the snapshot's exact instrument revision and
  recorded listing period, calendar, interval, dataset hash and full inclusive
  range. Require known listing metadata, clean coverage and the declared
  minimum total bars.
- Recompute the dataset identity over all current candles and apply P06 market
  data admission. Reconstruct the P06 snapshot content identity from the row
  and complete primary provenance set; reject rejected or superseded sources,
  demo/degraded status and missing cost/action confirmations.
- Return only observed row IDs, cost profile version and bar count for future
  transactional use. No schema, API command, runner caller, UI or dependency.

## Required Action / Decision

1. P12d-2b must freeze concrete strategy, execution and numeric cost inputs,
   persist the declaration and campaign/run links before enqueue, and repeat
   snapshot verification under an admission freshness fence. This read alone
   does not keep the database or registry current afterward.
2. Connect current P12b admission counts and unknown-history/quarantine
   blocking, P12a precision and P12c per-candidate feasibility/evidence.
   A passing snapshot check must never mean confirmation `PASS`.
3. The P12d-2b boundary must decide how to verify raw source artifact bytes
   and the actual cost profile against frozen execution settings. P12d-2a
   validates the P06 database identities and cost confirmation status only.
4. Keep P12d-2/P12d/P12 In Progress; P12e follows admission integration.

## Verification

- `cargo test --locked campaign_snapshot --quiet`: 4 new integration tests
  pass using the real P06 snapshot builder.
- `cargo test --locked --quiet`: 455 pass (101 library, 352 desktop, 2 service).
- `cargo check --locked --all-targets`: pass.
- `cargo clippy --locked --all-targets --quiet`: pass with the five existing
  warnings in backtest, score and file commands.
- Targeted rustfmt and `git diff --check`: pass.
- TypeScript, local Playwright, packaged desktop and a live campaign were not
  rerun; this slice changes only Rust backend verification and documentation.

## Publication (2026-09-29)

Implementation commit `84592d1` was pushed after a no-change rebase against
`origin/main`. The four targeted tests passed again. Chinese non-Draft PR #125
targets `main`; six remote checks started and remain the CI authority. No merge
was performed.
