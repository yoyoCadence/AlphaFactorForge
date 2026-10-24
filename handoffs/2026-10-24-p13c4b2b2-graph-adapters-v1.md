# Handoff: P13c-4b-2b-2 graph adapters and fixture joins

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c4b2b2-graph-adapters
Baseline: merged #214 (`07cf11d`).
Status: resolved; PR #215 merged and clean main verified on 2026-10-24.

## Summary

Refine the remaining [complete graph contract](../docs/plans/research-source-graph-adapters-v1.md).
First code is a private bounded selector derived only from captured original
campaign/config roots. It reuses existing declaration/config/enumeration codecs,
preserves legacy numeric policies and rejects raw Cartesian expansion before
strategy/base-ID cloning exceeds its checked scratch payload limits.

The later graph slice has fixed immutable APIs, the existing 117-column manifest's
exhaustive Rust types, exact optional/marker domains, bounded query shapes and
complete candidate/reference checks. Its budget continues from roots; recheck
validates the whole fresh closure before field/member comparison. Unsupported
external parent lineage is refused rather than recursively expanding history.

## Decisions / Limits

Selection is independent of current hardware: valid declared maxConcurrency is
used only as a deterministic parser core count, null uses one, and candidate
identity invariance is required. Existing exploratory codecs/runner are unchanged.
Counted serializer output is scratch sizing only, never original identity/hash
material. Selection remains transient; final graph contains original rows only.

No semantic source builder, schema/dependency/workflow/source edits, file/candle
authentication, registry/spend, live owner permission, command/UI or execution.
Original floating-point execution, source authenticity and missing immutable
cost/action evidence remain unsupported. Parent tasks stay incomplete. After
verified delivery only 4b-2b-2a bounded selectors becomes Next; eleven-table graph
implementation follows separately, then semantic/file/final-owner work.

## Grounding / Independent Oracles

- The enumerator's exact stored strategy IDs are `strategy-v2:<64 hex>`, not
  bare SHA suffixes. Dataset IDs remain `dataset-content-v2:<64 hex>`. Preserve
  prefixes through selection/row observations; any pure-source suffix conversion
  belongs to a later semantic adapter. All eleven type maps reference the same
  117 columns already frozen, without adding tables or mutable summaries.
- Existing six enumeration fixture counts are 3/9/1/4/4/4, including prune,
  deduplication and reversed-base order. Saved expected manifests are the oracle;
  no adapter-under-test expected output generation is permitted.
- The campaign fixture uses dataset hash suffix b*64; enumeration fixtures use
  a*64. Clone only the test declaration's datasetHash for those joins and retain
  all committed inputs/expected candidate outputs. Do not weaken equality.
- An independent Python type-tagged canonical encoder (UTF-8 key order, u32be
  lengths, f64be safe integer encoding) reproduced the committed original campaign
  ID `2ffaed4e40e8cd81c1d8631f802c8b744876f83cfef29e2d7db01db5e5f26c45`.
  Changing only that hash to dataset-a yields
  `3d64d27d196dcb275b30a1d59a50ea3481199331cab4a04a2dda2b4fb0a47b9e`.
  Each canonical payload is 1151 bytes before the existing domain/zero prefix.
  This is a bounded declaration oracle, not calibration or execution.

## Verification / Delivery

Ground against actual root/config/enumeration/campaign/history/market codecs and
schema. Check type/optional domains, exact projection/parameter counts, independent
fixture identities/counts, relative links/anchors, lifecycle/order and Markdown-only
scope. Runtime source/schema/dependencies/workflows must equal merged #214's
six-job-verified 998 Rust (one ignored) / 1242 Vitest / 98 E2E baseline. No new
runtime test for this documentation-only scope. Fresh main/rebase, six exact-head
CI jobs, fresh reviews/threads and expected-head merge/clean-main receipt follow.

## Resolution (local contract verification)

Applied all ten actual migrations to an in-memory SQLite database and checked
every manifest column and nullable non-primary-key field: 117 distinct columns
across eleven tables (8/13/6/12/5/17/21/7/9/2/17). Verified six saved enumeration
counts/indexes and exact strategy-v2-prefixed IDs. The independently authored
campaign encoder reproduced the original known control and dataset-a variant,
each 1151 canonical bytes; committed fixtures remain unchanged.

Fifteen plan links/anchors and both checked handoff backlinks pass. Static review
checks raw-product-before-cloning order, no guessed f64 text bound, deterministic
performance-only concurrency, exact SQL parameter arity, full-fresh-closure error
priority and explicit unsupported parent/file/original-execution boundaries.
Source/schema/dependencies/workflows equal merged #214; git diff --check passes.
Only five Markdown files are in scope. No new runtime test for documentation only.
Fresh main/rebase and post-rebase scope/link/lifecycle checks precede publication.

## Resolution (PR #215 delivery)

[PR #215](https://github.com/yoyoCadence/AlphaFactorForge/pull/215) merged tested
head `352b523ed38cdec884bce4dba0a09db52632d940` into unchanged #214 base
`07cf11d13d073b1857c25451fa0edcf86553d28c`; merge is
`731aa5e93aaf036f6adc7641d74241833e0d4e60`. All six jobs in
[CI 40037779367](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/40037779367)
succeeded, with every checkout log matching head/base. Rust: 998 + one ignored
(297/699/2), 31.85 / 264.42 / 5.70 seconds. Vitest: 1242 / 80 files; E2E: 98
with no retries. Typecheck/build/production audit zero and native render/invoke/
metrics/event passed, including exact hosted policy restoration and owned cleanup.
No failed CI attempt was retried.

Fresh base/head were unchanged, review submissions/threads empty and PR mergeable.
The already-confirmed connector write limitation used the repository's gh
fallback. Ready-body update/transition and expected-head merge succeeded; GitHub
merged state/SHA were read back. Local main fast-forwarded, complete tree equaled
tested head and worktree was clean before the next branch. Final delivery body
was updated. Only bounded root selection code follows; parent obligations remain.
