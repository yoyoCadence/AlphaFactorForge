# Handoff: P13c-3 exposure/receipt storage and complete transport design

Date: 2026-10-16
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c3-exposure-storage-contract
Base: merged #189 (`331a8807a3c2ef2c23a167d8dc143ad2320de604`)
Status: Merged as PR #190; P13c-3a-1a implements the first pure record slice.

## Summary

The [storage/transport contract](../docs/plans/research-confirmation-storage-transfer-plan-v1.md)
specifies exact new documents, ten append-only tables, explicit row/byte limits,
legacy birth/unknown rules, binding v5 and complete archive v5 before SQL work.
No production source, schema, dependency or statistic changes in this PR.

Exposure references operation/alpha; admission binds the exposure; started
and sealed events follow admission. Selection follows a Validation seal and
Test follows that selection. This avoids circular content hashes while
retaining complete commit-time dependency checks. Original statistics and
binding bytes stay in checked blobs; existing float reports are not rewritten.

The contract explicitly maps the P13c-1 bare trial batch digest to its existing
`trial-batch-v1:` SQL key without rehashing. P12a uses the complete original
registered-batch/current-family count even for a selected Test subset. Its
new receipt projection contains only the existing integer/enum decision fields.

All original operations, overlapping independent claims and contradictory
admission/start/seal/terminal evidence survive import. Derived permanent stops
remain separate from ordinary trial exploration. Manual-after-seal access is
strictly distinguished from unmediated or legacy-unknown exposure.

New registry birth metadata distinguishes actual schema-6 creation from legacy
migration. Existing origins and all old imports retain unknown confirmation
history; this cannot be cleared by adoption or complete re-export. This is
separate from alpha unknown history and manual interval uncertainty. Fresh
registry history still does not prove a workspace's raw/manual data unseen.

## Ordered follow-ups

P13c-3a-1a pure exposure/event/conflict records is Next after this design merges.
P13c-3a-1b completes admission/proof/blob/graph validation, then P13c-3a-2
storage, P13c-3b-1 binding and P13c-3b-2 complete atomic transfer are
separate tasks/PRs in that order. P13c-4 admission and P13c-5 dormant execution
remain later. The exact result artifact byte format must be specified before
execution; stored artifact hash/length do not claim files exist.

Each PR still needs the unchanged six final-head CI jobs before merge. User
has authorized continuing the task/PR/CI/merge loop. Production activation,
real-market statistical policy and manual mediation gates remain binding;
no final calibration-seed run is authorized or performed.

## Verification

Reviewed against actual registry migrations 0001–0005, alpha storage/transfer
helpers, binding v4, current admission-count/P12a producer and P13c-1/2 codecs.
Checked one-way Validation-to-Test dependencies, same-operation shared-key
handling, original-origin replacement/replay, conflicting unions and all
registry/workspace/start/seal commit boundaries. Relative links, exact-table
inventory, capacity arithmetic and an independent topological/cycle example
passed: ten relative links, ten table definitions, capacity ordering and a
16-node dependency example plus a deliberately circular variant. Baseline #189 already has six green jobs and
708 Rust / 1197 Vitest / 98 E2E plus real native smoke; this design PR also
waits for its own unchanged six-job CI.

## Resolution (2026-10-16)

[PR #190](https://github.com/yoyoCadence/AlphaFactorForge/pull/190) merged as
`f1e0e63602e0694c6ef4203bc4056dbd06deb2bc` after six successful jobs on
final head `2c51cddce9d328145ef9032e94d3117e3920ea00` in
[CI run 38922137615](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38922137615).
CI: 708 Rust tests (one ignored), 1197 Vitest, 98 E2E and real native smoke
with process/temp cleanup. The merged tree matches the checked head.
The authorized continuous loop proceeds from this merged baseline with
[P13c-3a-1a single-record codecs](../docs/research-confirmation-records-v1.md).
The next pure graph slice, then storage/binding/transport, retain their order;
single parsed records and conflict declarations grant no runtime authority.
