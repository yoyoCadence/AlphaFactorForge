# Handoff: P13b-2b complete alpha transfer

Date: 2026-10-16 (started 2026-10-15)
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13b2b-alpha-complete-transfer
PR: [#185](https://github.com/yoyoCadence/AlphaFactorForge/pull/185)
Status: Published as a draft; local verification passes; final-head CI follows on the PR.

## Summary

From merged #184 (`13649f4`), complete v4 JSON Lines archives now carry the
existing validated trial projection, all primary budgets, original reservation
bodies/IDs, every permanent conflict copy and legacy-alpha unknown origins.
One snapshot exports them; one IMMEDIATE transaction imports their preflighted
union. [Contract](../docs/research-alpha-transfer-v1.md).

The prior trial transfer implementation is shared through transaction-scoped
helpers. Those helpers neither reacquire the mutex nor commit their caller's
transaction. Complete import validates the source and proposed alpha union in
private in-memory databases before the destination's first write, then checks
the actual written inventory before its only commit. The audit receipt hashes
the complete outer file, rather than only the embedded trial projection.

## Important behavior

- Exact IDs deduplicate; origin/key/number/share/body never change. Independent
  number or operation collisions retain all claims and add permanent stops.
  Deterministic representatives avoid generating every possible pair, while
  previously recorded conflict IDs remain present if a representative changes.
- An existing primary budget is immutable. Other valid declarations stay in
  budget-conflict evidence; their own reservations validate against those
  declarations, remain readable and never produce an allocation. Alpha stops
  do not create ordinary trial quarantines or prevent exploration.
- A complete archive must carry every conflict's exact reservation-row
  references and every affected-family copy, not merely a valid conflict hash.
  An unblocked-history gap cannot be exported or accepted as complete.
- Migration 0005 adds append-only unknown-origin provenance, backfilled from
  prior trial imports/known origins. Legacy v3 imports record uncertainty in
  their own commit. Complete archives retain it transitively; changing format
  cannot turn unknown alpha history into a fresh budget or confirmation permit.
- Binding v4 preserves v3 identities plus mandatory sorted unique
  `alphaUnknownOrigins`. v1-v3 provenance stays unknown until verified adoption;
  only v4 can be written. Lost origin markers refuse adoption and keep the last
  good workspace binding JSON unchanged.
- Archival v4 import may preserve unknown provenance. Qualification-transfer
  import refuses v1-v3 and unknown source/destination provenance before writes.
  Alpha-family reads and the unwired reservation primitive also block amount
  allocation when provenance is unknown. A transport check grants no reveal.
- Existing v3 APIs remain trial-only compatibility paths. Their wire format
  and trial semantics stay intact; nonempty alpha export is still refused.

## Bounds and verification

File 128 MiB; inner trial projection 64 MiB (including its header); v4 header
4096 bytes; alpha line 1 MiB + 2048 bytes. Storage remains 10000 alpha rows /
16 MiB JSON with the existing per-document limits. Unknown origins have a
separate 10000-entry cap. Binding retains 4 MiB / 30000 alpha identities, plus
bounded unknown origins. Oversized source/union/conflict expansion is refused,
never truncated; no runtime/CI timeout was relaxed.

Local full suite: **668 Rust (180 library + 486 desktop + 2 service), 1 existing
ignored; 1184 Vitest / 73 files; typecheck/build pass.** Twenty added regressions
cover complete/repeated/two-hop transfer and reopen/exhaustion, independent
budgets/numbers/cross-family operations, representative changes, missing
conflict rows/references, corruption/gaps/rehashed malformed inputs, exact
capacity and conflict expansion before trial writes, two independent import
connections, injected early alpha and late provenance failures, legacy
laundering refusal, migration/backfill/append-only guards, strict v4 binding
compatibility and real workspace DB rollback preserving saved JSON.

All-target check/Clippy and final focused verification are recorded in OS temp
`aff-p13b2b-20261015/`; Clippy has only the five pre-existing warnings. Focused
rustfmt, relative links and diff checks pass. Normal calibration tests only
replay predeclared prefixes; no full final-seed run or statistical PASS.

Actual service compatibility smoke passes in an owned empty temp workspace:
new service writes binding v4/schema 5 and stops with exit 0. Preserved baseline
service SHA-256 `1bc1dcbf3ca37809cc4b4128d5c5ac3d73ba361ce5c94d6831cab850858809bf`
first refuses binding v4 with exit 1. After changing only that isolated empty
binding fixture to v3, it independently refuses schema 5 with exit 1. No
endpoint/process/temp workspace remains. See `transfer-downgrade-result.json`,
new/old/schema stderr, the bounded PowerShell harness and fixture adapter in
the evidence directory. No real workspace or per-user registry was modified.

## Remaining work

P13c must establish the frozen confirmation selection/batch, exposure identity
and period overlap, legacy manual exposure, prompt filtering, synchronized
final trial/P12a/alpha reservation transaction, execution recovery and native
acceptance contracts before any production caller. The local storage primitive
uses the current registry's origin: after a registry replacement, recovery must
look up the original frozen origin/key instead of requesting a new next slot.

P12e support remains its exact tested set. Real-market or other statistical
configurations are not made eligible here. A fabricated archive with recomputed
hashes cannot authenticate its publisher's unseen history, and joint rollback
of all local registry/workspace evidence remains the offline limitation.

## Resolution (2026-10-16 — continuation verification and delivery)

Resumed on the intended feature branch with all pending files in this slice.
Fetched `origin/main` and fast-forwarded local `main`; both remain at merged
#184 (`13649f4`). No additional implementation change was needed after checking
the shared transaction boundary, source/union preflight, original identities,
conflict references and legacy-provenance allocation/transfer barriers.

Fresh full verification: **668 Rust passed (180 library + 486 desktop + 2
service), 1 existing ignored; 1184 Vitest / 73 files; typecheck, build,
all-target cargo check and Clippy pass.** Clippy retains only the five existing
warnings. Vitest initially could not start esbuild inside the sandbox
(`spawn EPERM`); the same commands passed with escalation, without changing
timeouts or test settings. Logs: OS temp `aff-p13b2b-closeout-20261016/`.
The prior isolated service downgrade receipt was inspected and still records
new exit 0, separate old binding/schema refusals with exit 1 and cleanup.

Published as draft [PR #185](https://github.com/yoyoCadence/AlphaFactorForge/pull/185),
base `main`, head `feat/p13b2b-alpha-complete-transfer`; implementation commit
`7cfb98f0d57b54e2a72d81a82c35b06a30cdcaaf`. The connector refused creation
with `422 must be a collaborator`; the repository-prescribed GitHub CLI
fallback succeeded. The PR's head is the authority for subsequent documentation
delivery and final-head CI. P13c design starts after its merge.
No merge, production confirmation/reveal or new calibration run is performed
by this continuation.
