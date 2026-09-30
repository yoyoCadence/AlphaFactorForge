# Handoff: P12d-2d-2 campaign authoring UI

Date: 2026-09-30
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12d2d2-campaign-ui` (from merged PR #130, `8269c7e`)
Status: Implemented and verified locally; preparing a draft PR.

## Summary

PR #130 was reviewed after merge: its owner-checked freeze/list/start commands,
typed client and six CI jobs were accepted with no blocking finding. This slice
connects them to a campaign authoring panel and the DEV-only Playwright seam.
No schema, Rust command or runner change is included.

## What changed

- `CampaignPanel` lets an author choose previously created snapshots for one
  or more distinct instruments. The exact snapshot and matching instrument
  revision supply the dataset/listing bindings. The author enters an explicit
  sample policy and rationale per instrument and can edit P12a sampling.
  Invalid or incomplete drafts fail before preview; any edit invalidates the
  previous preview. Freeze requires every instrument's latest backend preview
  to resolve and verifies that the returned ID matches the preview.
- Freezing saves through the PR #130 command. The saved list offers one start
  per instrument, using a locally validated `discovery-config-v3` envelope
  whose fold declaration exactly matches the frozen campaign. Runs use the
  current params strategy's fee/slippage, a visible seed and holding allowance.
  The backend still freezes costs and re-verifies snapshots at start. One active
  discovery job blocks another start. If decision/list reading fails after a
  successful start, the UI preserves and reports that run ID.
- The decision view shows `ELIGIBLE`/`NOT_ELIGIBLE`, reasons, precision,
  snapshot and frozen costs. It never calls either status `PASS`.
- `dataClient` now routes the `campaigns` client through its DEV-only mock. The
  mock offers two BTC snapshots for one dataset and one ETH snapshot, plus
  bounded failure/empty-list scenarios. A saved campaign whose snapshot later
  ages out of the 500-row authoring list can still start by resolving its unique
  dataset hash, interval and range from the workspace dataset list.

## Verification

- PR #130 merged as `8269c7e`; all six GitHub CI jobs passed in
  [run 36714893574](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36714893574).
- `npm run typecheck`, `npm run build`: pass.
- `npm test`: **986 passed / 56 files** (984 baseline + 2 builder tests).
- Playwright campaign specs: author two instruments → preview → freeze → start
  separately → read positive/negative decisions; stale/unresolved preview
  blocked; empty source list explained; saved campaign starts after snapshot
  leaves the bounded list. Four pass.
- Full Playwright suite: **82/82 passed**, including the new list-aging
  regression.
- Playwright CLI opened the `?mock=1` page, inspected the panel snapshot and
  console (only the pre-existing favicon 404), and captured the expanded panel.
  The layout is readable with the current theme.

## Remaining acceptance

- Native desktop and connected-service smoke with actual service-created
  snapshot(s): preview the exact ID, freeze, start one instrument, reopen its
  decision; repeat with two instruments. The browser mock cannot prove P06
  snapshot content, SQLite owner semantics or ledger counts; PR #130 Rust
  coverage owns those lower layers.
- Only snapshots already produced by the CLI adapters can be authored here.
  Fetch/import and snapshot creation in the UI are separate work.
- P12e/P13 still own confirmation and the atomic last-fence scheduling step;
  admission `ELIGIBLE` is only feasibility.
