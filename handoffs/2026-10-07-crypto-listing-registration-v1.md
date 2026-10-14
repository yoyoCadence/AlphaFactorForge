# Handoff: CAMPAIGN-LISTING-001 repair

Date: 2026-10-07 (Asia/Taipei)
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/crypto-listing-registration`
Baseline: `77c1bda6ffb57365b2b9003dd89f3a155123a93e`
PR: Delivery tracked on branch `fix/crypto-listing-registration`.
Status: Implementation and controlled native acceptance complete; final-head CI/merge pending.

## Summary

The #126–145 acceptance review reproduced a formal Binance BTC snapshot whose
immutable instrument has no listing date. Native campaign Add correctly refused
it, and there was no supported registration entry. The accepted repair adds the
bounded service CLI in [the contract](../docs/crypto-listing-registration-v1.md).
It preserves source evidence in a new instrument revision; re-fetch builds a new
snapshot. Old rows, snapshot identity and missing-listing refusal remain intact.

## Required action and scope

- Strict versioned settings and bounded byte-preserving, operator-reviewed
  Binance source evidence are validated before workspace acquisition.
- Register through the existing owner epoch and transaction, preserving other
  instrument metadata and rejecting contradictory known listing assertions.
- Verify exact retry, immutability, ownership, bounds and source integrity in
  Rust tests; validate actual CLI fetch and native campaign UI/reopen.
- Publish one scoped repair PR, verify its final-head CI and merge under the
  maintainer's existing authorization.

No schema, credentials, automatic listing inference, UI metadata editor or
numeric-policy rollout belongs to this repair. A source checksum proves byte
integrity; it does not authenticate an operator's listing assertion.

## Verification

Pending implementation checks and controlled live/native acceptance. The
source used is the official USDT announcement linked in the contract, with a
short reviewed capture and explicit UTC conversion rather than a candle-derived
date. The native harness must clean its owned processes and temporary paths.

## Resolution

Pending. Append implementation, exact verification, PR/head and remaining
acceptance limits when acted on; preserve the original review evidence.

### Resumed implementation and verification — 2026-10-15

Maintainer approved continuing this slice, publication and merge. Preserved
the existing draft as `ebe390a` after rebasing onto merged PR #178 (`9490ec4`).
Earlier local acceptance notes/task edits were backed up before rebase; their
latest tracked versions now come from main. No schema or frontend change.

Passed: all eight new Rust cases; complete 614 Rust tests (170 + 442 + 2,
one existing ignored), 1,183 Vitest, typecheck and production/native builds.
The first native harness attempt downloaded correctly but Windows PowerShell 5
returned a null process exit code. Retaining the process handle fixes that
harness issue without treating a missing exit code as success.

The second full native run passes: official January 2025 BTCUSDT archive,
744/744 bars; append revision 2; exact retry returns the same row/hash;
re-fetch pins the new revision with unchanged dataset identity. Old snapshot
Add/backend-preview refusal remains. Actual native preview/freeze/start yields
one completed candidate and a persisted ELIGIBLE admission (not research PASS).
After stopping/restarting the app, its higher ownership epoch reads identical
campaign/admission/progress/attempt/artifact data. Both screenshots inspected;
no runtime/console errors. Cleanup JSON confirms zero owned apps/WebViews/CDP
listeners and all three disposable data/profile directories removed.

Evidence: OS temp `aff-crypto-evidence-20261015-1792021299277/`;
`native-campaign-2/` contains
inputs, CLI receipts, initial/reopen JSON/screenshots and cleanup report.
Public source cross-check: the [English Binance announcement](https://www.binance.com/en/support/announcement/detail/115001094331)
supports the retained original excerpt and the stated UTC+8 conversion; the
Russian route now redirects to the announcement index. The source remains an
operator-reviewed capture, not an authenticated full HTTP response.

Local mock Campaign E2E twice passed 4/5; the first case stopped at initial
page.goto after 30 seconds, before any UI assertion. No frontend source or
assertion changed. A 90-second diagnostic passes all five unchanged cases
(first case 53.5 seconds), with trace retained. These attempts are not
reported as a passing suite at the default 30-second limit. Normal-limit
exact-head full CI remains required before merge.

All-target cargo check and Clippy also pass, with only the five existing
warnings in unchanged modules. O3's controlled BTC flow is verified; P12
parent acceptance review, O1/O2/O4 and later P13 execution remain separate.
