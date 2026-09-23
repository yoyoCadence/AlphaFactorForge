# Handoff: P12b trial-ledger specification (review request)

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p12b-trial-ledger-spec` (from main `a56f12e`, PR #115 / P12a merged)
PR: [#116](https://github.com/yoyoCadence/AlphaFactorForge/pull/116) (ready for review; merge is maintainer-owned)
Status: P12b specification accepted at `41626c5` after re-review; implementation remains open. P12 stays In Progress.

## Summary

Following the maintainer's 2026-09-23 decisions (second screenshot on PR #115 and the
[acceptance review follow-up](2026-09-23-pr115-acceptance-review-v1.md)), this PR adds
[`docs/trial-ledger-v1.md`](../docs/trial-ledger-v1.md): the rules that will make trial counts
impossible to reset. It is documentation only — no code, migration, command, or runtime change.

The PR also commits the reviewer's uncommitted re-review records from the PR #115 worktree
(the Resolution appended to the acceptance review handoff and its `tasks.md` line), unchanged.

## What the spec decides

- **Location:** one SQLite registry at `%LOCALAPPDATA%\com.alphafactorforge.evidence\`, a sibling
  of (never inside) any workspace; containment check both ways; `AFF_REGISTRY_DIR` for isolated tests;
  refused on OneDrive/network volumes. Own migration sequence and `registry_id`; no workspace lease —
  SQLite's writer lock plus idempotency keys handle concurrent workspaces.
- **Family:** `trial-family-v1` keyed only by the P06 `instrumentId`. Interval, dates, source,
  dataset hash, workspace and protocol are deliberately excluded so none of them can open a zero-count
  family. Protocol (`holm`, `testsPerTrial`) is pinned on the family at its first effective trial;
  a mismatch is rejected. No snapshot → `family_unknown` → no qualification. Multi-instrument trials
  rejected in v1.
- **Kinds:** hypothesis / variant / diagnostic / legacy count; only whitelisted benchmarks and
  fully identity-matched reproductions are recorded without adding effective trials. Kind and
  effectiveness are inside the content-hashed event, so relabeling conflicts with the idempotency key.
- **Ordering:** registry COMMIT before the workspace enqueue transaction. The idempotency key uses the
  P03b command `requestId` (P05 `attempt_key` cannot be used because `run_id` does not exist yet);
  orphaned registrations still count; claim requires a non-null `trial_event_id`.
- **Rollback/missing:** each workspace keeps a watermark (`registryId`, event counts); a lower count,
  a missing registry or a replaced registry without a matching import stops qualification.
- **Restore/import:** `trial-ledger-export-v1` with checksums; union by `eventId`; identity conflicts
  quarantine the family; never overwrite or take `max(count)`.
- **Legacy:** backfill existing P05 attempts as effective `legacy` trials; pre-P05 records that
  cannot be counted mark the family `legacy_trials_unknown`.
- **Acceptance:** 20 isolated-workspace cases (A1–A20) and a split into P12b-1 (registry module) and
  P12b-2 (workspace/runner wiring).

## Required Action / Decision

Please confirm or change the six choices in spec §15 before implementation:

1. Family = single instrument only (coarser and more conservative than "instrument/research
   scope/protocol").
2. Cross-venue alias table in v1, or defer to multi-asset work.
3. Diagnostics always count in v1 (no exemption mechanism).
4. Pre-P05 history: stop qualification for affected families, or accept a recorded one-time
   conservative estimate.
5. Quarantine release: none in v1 (new contract version only), or a manual release that leaves an event.
6. Registry path under `%LOCALAPPDATA%` (not roaming).

## Review Notes

- Self-check found and fixed one contradiction during drafting: the first draft keyed registrations by
  P05 `attempt_key`, which contains a `run_id` assigned only after registration; the spec now uses the
  enqueue command's `requestId`.
- Referenced columns and constants were checked against the code: `market_snapshots.instrument_id`
  and `dataset_id` (migration 0008), `research_attempts.attempt_key`/`dataset_id` (0007), the four
  `DETERMINISTIC_BENCHMARK_IDS`, the P03b `requestId` semantics, and `default_data_dir` resolution
  (`%APPDATA%\com.alphafactorforge.desktop`).
- The spec does not cover statistical tests, alpha allocation, or Validation/Test consumption
  (P12e-1..3 and P13, now listed in `tasks.md`).

## Verification

Documentation only. No build or test suite was rerun; no source file changed. Markdown links in the
new files point to existing paths.

## Resolution

2026-09-23 — The [PR #116 acceptance review](2026-09-23-pr116-acceptance-review-v1.md) found four
blocking specification contradictions (R1 export without batches, R2 count watermarks not proving
inclusion, R3 reproduction identity missing from the payload, R4 batch retry bypassing conflict
checks). All four are corrected in `docs/trial-ledger-v1.md` on this branch (details in the review's
Resolution); acceptance cases A21–A30 were added and the P12b-1/P12b-2 split updated. The §15 policy
questions remain open. Still documentation only.

## Re-review and policy decision (2026-09-23)

The re-review of `af1a7c7` closed R1–R4 but found R5: a historical batch receipt can understate the current family size if another batch registers before a crashed request retries admission. The six policy questions are now answered in spec §15: instrument-only v1 family; defer cross-venue aliases; all diagnostics count; unknown pre-P05 history blocks qualification; no manual conflict release in v1; local AppData registry accepted. These local documentation edits do not approve PR #116 or start P12b implementation. See the [acceptance review](2026-09-23-pr116-acceptance-review-v1.md).

## Resolution — R5 (2026-09-23)

R5 is corrected in `docs/trial-ledger-v1.md` §5/§6.1/§6.3/§6.4/§11: `register_batch` returns a
current `admissionCount` (same registry transaction as the registration or verified replay) separately
from the historical receipt; only the count can build a P12a plan; a freshness fence re-reads before any
`ELIGIBLE` decision is acted on. Acceptance cases A31–A32 added. Details in the
[review Resolution](2026-09-23-pr116-acceptance-review-v1.md). Still documentation only; awaiting re-review.

## Acceptance re-review (2026-09-23)

The independent [PR #116 review](2026-09-23-pr116-acceptance-review-v1.md) closed R5 on `41626c5`, with no new blocking specification finding. A31/A32's numerical thresholds were recomputed independently and all six CI jobs passed on the reviewed head. The specification is accepted; P12b-1/P12b-2 implementation and its behavioral acceptance have not started.
