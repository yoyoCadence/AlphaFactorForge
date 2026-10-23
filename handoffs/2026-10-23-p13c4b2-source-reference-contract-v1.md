# Handoff: P13c-4b-2 source/reference contract

Date: 2026-10-23
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c4b2-source-reference-contract
Baseline: merged #210 (`443940d`).
Status: design/local verification complete; exact-head PR/CI delivery pending.

## Summary

Refine [4b-2](../docs/plans/research-source-reference-observation-v1.md) from
actual schema/runner/history/market/identity sources. Preserve original JSON
tokens, complete enumeration and row closure; separate claimed references,
semantic verification and later authenticated pinned input bytes.

## Decisions and remaining work

The first implementation is only a private bounded OriginalJson wrapper with
strict syntax/decoded-key uniqueness and byte/node/depth/key limits. Complete
row projection follows after concrete tuple/SQL refinement, then supported
semantic/digest adapters. New host digests bind exact original bytes, preserving
old strategy/data identities and parser policy. P06 lacks required original
cost/action/raw-source authority; missing evidence refuses without placeholders.
The parent 4b/4b-2/P13c-4 remain incomplete.

## Verification obligations

Check every new relative link/anchor, grounded schema names/fields and digest
framing; verify task order and Markdown-only scope. Source/runtime/schema/
dependencies/workflows remain #210's tree. Exact-head six-job CI, fresh base/
reviews/threads, expected-head merge, clean-main equality and PR receipt follow.
Only 4b-2a becomes Next after this design's verified delivery.

## Resolution (local design verification)

Twenty-one new-document relative links/anchors pass. Schema/field grounding,
original byte/numeric-policy/trial-prefix boundaries, task lifecycle/order and
five-file Markdown-only scope checks pass; git diff --check is clean. No source,
schema, dependency or workflow changes relative to merged #210. No new local
runtime test run is needed for this documentation-only change; the delivery PR
still requires all six exact-head CI jobs. No calibration or protected-data run.

The precise sourceEvidenceHash file framing, supported warmup/embargo adapters
and original cost/action authentication remain explicitly unavailable. The
design does not claim complete authenticated source assembly. Only the small
bounded original JSON implementation enters Next; parent obligations remain.
