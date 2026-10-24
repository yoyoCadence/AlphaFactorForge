# Handoff: correct source graph hypothesis grounding

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c4b2b-hypothesis-contract
Baseline: merged #216 (`4193eba`).
Status: resolved; PR #218 merged and clean main verified. Draft #217 remains pending corrected implementation/evidence.

## Summary

While grounding the next semantic adapter, found that the earlier #215 graph
contract and #217 synthetic fixture did not match the actual runner. run_lineage
uses a shared base hypothesis, strategy-doc-v1 and NULL strategy_id; history's
registration preserves those values and deduplicates by original content.
#217 required per-candidate strategy-v2 plus nonnull ID. That was a design error,
not an existing runner defect. Its successful tests were insufficient evidence.

The [focused correction](../docs/plans/research-source-hypothesis-grounding-v1.md)
freezes the actual domains/nullability/sharing, independent fixture revision and
a cfg(test)-only seam through the actual unchanged run_lineage/history writer.
All candidate strategy identities, eleven-table closure, 117 cells, limits and
transaction boundaries remain fixed. No schema/identity/runtime fix is needed.

## Required Action

Deliver this documentation PR first. Then rebase #217, update its fixture/reader,
add actual-writer positive coverage and two compatibility mutants, repeat the
four original mutants/full checks/six-job CI, and only then merge the graph.
Do not skip the real writer control or claim original green CI as compatibility.
Continue the original semantic design task only after corrected graph delivery.

## Verification

Read actual run_lineage, HypothesisDraft content/hash, register_hypothesis,
register_run_lineage_with_events and strategy registration. There is no hidden
assignment of hypothesis.strategy_id or replacement of its base hash.
This branch changes Markdown only; runtime remains merged #216. Link/scope/
lifecycle and final CI delivery evidence follow below.

Local link checks pass for all four evidence documents; static assertions confirm
the actual writer's strategy-doc-v1/None and unchanged hypothesis registration.
git diff --check and Markdown-only scope pass. No local runtime rerun for docs.

## Resolution (PR #218 delivery)

[PR #218](https://github.com/yoyoCadence/AlphaFactorForge/pull/218) merged tested
head `c4912c985f6f1b5bdba611cb77c8fb7f0a192f8c` against unchanged #216 base
`4193ebab3e1e8d8f25b92819191021d17966337a`; merge is
`f3a9d12c128a32a12a81a5775c99399da40eaf05`. All six jobs in
[CI 40062332415](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/40062332415)
passed on that exact head/base: 1011 Rust + one ignored (297/712/2),
16.39 / 219.93 / 5.64 seconds; Vitest 1242 / 80 files; E2E 98 without retries.
Typecheck/build/production audit zero/native render-invoke-metrics-event pass,
with exact hosted-policy restoration and owned process/directory cleanup.
No failed CI attempt/retry. Review submissions/threads empty; fresh base unchanged.

Expected-head merge completed through the repository's established gh fallback;
GitHub merged state read back. Local main fast-forwarded, full tree matched the
tested head and worktree was clean. Final PR body updated. Rebased #217 onto
this correction; tasks.md conflict kept main's In Progress graph status instead
of the uncorrected feature's premature Done. No runtime conflict occurred.
