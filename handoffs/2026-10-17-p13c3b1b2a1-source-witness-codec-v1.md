# Handoff: source witness codec and self-contained source anchors

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b1b2a1-source-witness-codec
Status: Merged and verified; graph integration follows separately.

## Summary

Implement frozen design #197's first pure foundation. Immutable canonical
source witnesses preserve original trial text and complete source claims.
Self-contained source anchors remove the need to fabricate empty historical
alpha/confirmation evidence when reconstructing an unadmitted source.
The anchored compatibility wrapper retains its existing exact checks.

## Changes and boundaries

- Closed original-text witness decoding, exact SHA/canonical UTF-8 and 8 MiB
  outer/inner limits; existing 4096-candidate and safe-integer domains.
- Immutable original prefix and complete restrictive trial evidence, exact
  binding comparison, generic archival protocol facts without eligibility.
- Shared original-only backend reconstruction, actual counts/full membership,
  independent of receipt audit totals and receiver/source availability.
- Independent Node four-event/two-batch fixtures, same-head protocol upgrade
  and audit-only byte variation; Rust and TypeScript byte/chain oracles.
- Original artifact/campaign/run/engine fields remain claims; no file/publisher
  authentication or current execution/owner/count/alpha authority.

No logical graph blob kind, schema/table, writer, import, invoke or runtime
observer switch. Existing two-test confirmation/report support and pure graph
inventory rules remain unchanged. No bootstrap/calibration/reserved seed run.

## Verification

`cargo test --locked`: 818 passed (280 library, 536 desktop, 2 service; one
ignored). `npm test`: 1239 passed / 79 files. Typecheck, build, all-target cargo
check and scoped rustfmt pass. Ten new pure codec/anchor tests and three backend
witness cases retain all eleven original reconstruction regressions. The three
new TypeScript cases independently verify original bytes and full nonempty chain.
Clippy retains only the five pre-existing warnings.

Four executable mutants (canonical outer requirement, outer byte cap, exact
protocol anchor, complete membership) were each caught by assertion failure;
original source bytes were restored before the full final suite. Deterministic
fixture regeneration retains SHA-256
`336acfd56d75251a8defb4b001d3867cc14ef4e913050f75e5c7a3792f5fff65`.
The source-removal/same-head upgrade test closes SQLite before removing its owned
temp tree; no owned historical-source temp directory remains. Four new local
Markdown links and diff whitespace checks pass. Delivery PR must record six
final-head green CI jobs, review/base checks, exact merge SHA and local main
tree equality.

## Next action

Only after this PR merges, implement planned 2a-2 checked witness host facts,
exact source/admission anchor dependencies, proof-root liveness and original
quarantine/HWM coverage parity. Preserve stats/binding orphan restrictions,
existing two-test/report/P12 policy and static diagnostics. Additive schema 7,
backend context/restoration and binding/transport activation remain later PRs.

[Contract](../docs/research-source-witness-v1.md),
[plan](../docs/plans/research-source-witness-storage-plan-v1.md).

## Resolution (2026-10-17 — merged codec/anchor foundation)

PR [#198](https://github.com/yoyoCadence/AlphaFactorForge/pull/198) verified head
`1a77be36c6086fd471abedbb52bf03d7ee8684bc` and merged as
`f15853df06f3c2c3cb7c1f4bb927fd60531ab416`. CI run `39039139282` passed all six
jobs: 818 Rust (one ignored), 1239 Vitest / 79 files and 98 first-pass E2E.
Native rendering, SQLite invokes, metrics windows and Rust event round trip
passed; exact hosted-browser-policy restoration and owned process/temp cleanup
passed. No review submissions or review threads remained. Clean local main
was fast-forwarded and its tree equaled the verified head; the PR body matched
the final receipt. 2a-2 may now start, keeping schema/runtime integration later.
