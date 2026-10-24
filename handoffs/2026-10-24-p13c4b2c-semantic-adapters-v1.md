# Handoff: P13c-4b-2c original adapters and digest design

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c4b2c-semantic-adapters
Baseline: corrected merged #217 (`d88312e`).
Status: resolved; PR #219 merged and clean main verified. No semantic/runtime code in this design.

## Summary

The [design](../docs/plans/research-original-semantic-adapters-v1.md) grounds
campaign/config/candidate/input/engine/shared-base hypothesis, warmup/split,
trial, result-file and market original-format boundaries in their actual writers.
It preserves the unresolved numeric compatibility policy and missing action/
cost/source/file proof. No RegisteredSource builder can ship from row claims.

The first bounded code slice is only a private raw digest observation over the
sealed complete SQL graph: exact original-byte SHA/H framing, every candidate
engine in index order, no caller part/subset/domain factory or authority. A
checked whole-operation 32 MiB hash-work preflight counts repeated shared bytes
and every framing prefix before the first SHA update, preventing 4 MiB fields
being hashed thousands of times. Output text is independently bounded by
(3*N+2)*64 <=786560 bytes; no whole graph clone/Value tree is retained.

## Independent Evidence

Python hashlib/struct u64be, importing no project implementation, generated the
committed corrected graph's expected 11 digest strings before code. Original
campaign bytes=1061; total hash work=7938 bytes; digest output text=704. The
6102-byte/184-node graph's uninterpreted/unsuitable claims remain framing controls,
not an admitted or executed source. Independent one-part/two-part, ordering,
whitespace and empty-part controls freeze length/count/domain framing. Exact
values are recorded in the design; later tests must not generate their own
expected values via production hashing/serialization.

## Required Action / Delivery

Complete vector/source/link/scope/lifecycle checks, then one documentation PR
with six exact-head CI jobs, fresh base/reviews, expected-head merge and clean
main verification. Only 4b-2c-1 raw digest code enters Next. Before further
candidate/engine/hypothesis semantic code, freeze corresponding original-format
fixtures and actual-producer positive controls. Keep file/candle evidence and
actual-owner/registry final admission separate. No statistical/seed activity.

## Verification

Read parent source/hypothesis/numeric contracts; actual run_lineage, strategy and
result writers, history registration, engine and split/embargo/DSL functions,
market calendar/instrument/snapshot/provenance writers and trial input shapes.
No display DTO fallback or current-engine reconstruction is accepted as original
evidence. This branch changes documentation only; runtime/schema/dependencies/
workflows remain corrected #217's six-job-verified 1031 Rust + one ignored,
1242 Vitest / 80 files and 98 E2E baseline. No local runtime rerun for Markdown.

All graph digest/work/output values were independently recomputed and matched
the plan. Seven framing controls include a leading-zero SHA to preserve 64-hex
width. Local evidence links, Markdown-only scope, lifecycle and diff --check
pass. The temporary oracle generator was removed; no production serializer or
reader generated expected values. Six explicit first-code mutants are frozen.

## Resolution (PR #219 delivery)

[PR #219](https://github.com/yoyoCadence/AlphaFactorForge/pull/219) merged tested
head `85f69b6a78a102140e1022acc2c4cd5fc1d76105` against unchanged #217 base
`d88312ef0dc93fe1788e3e7e75b4a7eee8546533`; merge is
`8238fe76956439c182be6478f502269ffd9eb0e8`. All six jobs in
[CI 40081272256](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/40081272256)
passed on that exact head/base: 1031 Rust + one ignored (297/732/2),
16.37 / 232.39 / 5.65 seconds; Vitest 1242/80; E2E 98 without retries.
Typecheck/build/production audit zero/native render-invoke-metrics-event pass,
including exact hosted-policy restoration and owned cleanup. No CI failure or
rerun. Fresh base unchanged; reviews/threads empty and PR mergeable.

Repository-prescribed gh fallback completed ready/body/expected-head merge;
GitHub merged state/SHA read back. Local main fast-forwarded, full tree equaled
tested head and worktree was clean before the raw-digest branch. Final delivery
body updated. Only the frozen private raw-digest observer follows; all original
semantic/file/registry/owner obligations remain incomplete.
