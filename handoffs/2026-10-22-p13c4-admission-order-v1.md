# Handoff: P13c-4 final-admission implementation order

Date: 2026-10-22
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c4-admission-implementation-order
Baseline: merged #206 (`9f36cbb`).
Status: design/local verification complete; documentation-only PR/CI delivery follows.

## Summary

Complete v5 transport is delivered. Existing binding/fence readers finish their
own transactions and alpha reservation commits independently; neither can bridge
the future final owner/count/share/exposure fence. The new plan records bounded
read helpers first, then backend input observation, actual-share preparation,
private atomic registry admission and registry-first workspace projection.

## Required action

After this design's verified PR merges, promote only **P13c-4a transaction-owned
observations**. Add internal active borrowed-transaction binding/count-fence
readers, preserve every old public validation/check/finish behavior and retain
whole witnessed binding coverage. No reservation, workspace migration, runtime
caller or admission capability belongs in that PR. Exact acceptance and later
contract-freezing obligations are in the [plan](../docs/plans/research-final-admission-implementation-v1.md).

## Review notes

An identity-matching private copy or a consistent deferred read is not actual
registry ownership or live authorization. Public snapshot rows and pure frozen
admission documents cannot mint private capabilities. The final path must use
actual owner/epoch/mutation observation and actual receiver IMMEDIATE under the
workspace guard, then one registry commit before workspace projection.

Statistical policy and managed/manual exposure mediation remain unavailable
production authorizations. Test-only positive fixtures cannot create release
authority. Parent P13c-4 stays incomplete through all integration/crash/contention
acceptance; protected execution, automatic recovery, commands/providers/UI and
new calibration remain outside this design.

## Verification

Source grounding, document links, task ordering and diff/source-scope checks are
being recorded. No source/schema/dependency/fixture/statistical change is planned.
The inherited code baseline has 936 Rust (one ignored), 1242 Vitest / 80 files
and 98 first-attempt E2E from #206. This documentation PR still requires six
exact-head CI jobs, fresh reviews/base and guarded merge before 4a begins.

## Resolution (local verification)

The plan was checked against actual binding/fence, alpha reservation, workspace
ownership/mutation version, snapshot resolver, pure admission and exploratory
runner code. Public pre-mutex validation and whole-binding versus narrower count
fence semantics are explicit. Inactive/identity-mismatched transaction refusal
does not claim physical ownership; positive tests cannot mint release authority.

All 12 relative links in the two new documents resolve, including parent section
anchors. Parent/task backlinks and the first-code Next entry point to the exact
contract. Five changed files are Markdown only; source, schema, dependencies,
workflows, fixtures and policy are unchanged. `git diff --check` passes. No
redundant local source test run is attributed to this design; #206's exact code
baseline and this PR's required six-job CI remain separately identified.

P13c-4-design is Done locally; only 4a becomes Next after delivery. The parent
P13c-4 stays Backlog/incomplete until its integration/crash/contention criteria
are actually fulfilled. Final PR records fresh-main rebase, exact-head CI,
reviews/threads, expected-head merge and clean-main receipt, then 4a begins.
