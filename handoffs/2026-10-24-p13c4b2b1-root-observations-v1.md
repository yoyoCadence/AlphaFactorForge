# Handoff: P13c-4b-2b-1 bounded root observations

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b2b1-root-observations
Baseline: merged #213 (`9fc320d`).
Status: resolved; PR #214 merged and clean main verified on 2026-10-24.

## Summary

Implement the [frozen root/budget slice](../docs/plans/research-source-sql-projection-v1.md#1-first-slice-and-private-apis).
Private immutable source locators and observations retain the exact campaign,
admission and run records. Fixed SQL emits marker/value pairs in manifest order,
guards types and payload size, refuses missing/duplicate roots and checks exact
links. Workspace capture/recheck retains priority and the caller keeps its
transaction, pending state and writer reservation.

One budget accounts for the fixed 128 ID bytes and all retained TEXT/JSON bytes
and nodes. OriginalJson now clamps its node limit to the remaining joint budget
before its only byte copy; the original parse API/limits remain unchanged.
Finite REAL cells retain bits, original JSON numbers remain opaque, and unknown
version syntax or negative historical costs confer no semantic suitability.

## Boundaries / Remaining Work

No runtime caller, public/wire constructor, mutable accessor, Debug/serde/default,
RegisteredSource conversion, generic SQL factory, schema/dependency/workflow
change, registry/IO/spend/finish or activation. Shared test fixture helpers have
parent-only visibility under cfg(test); no existing metadata reader changed.
Root observations remain incomplete source evidence. Full graph membership,
supported original adapters, file/candle authentication and final actual-owner
fence are still required; parent 4b-2b/4b-2/4b/P13c-4 remain incomplete.

## Verification

- Initial focused suite: 49 pass (14 workspace, 16 JSON, 19 root/budget), including
  all 15 existing JSON cases. Migrated file-backed fixtures use isolated OS
  leases; raw typed row/schema snapshots and total_changes/activity checks cover
  pending writes/full rollback and success/refusal/SQL-error writer contention.
- Cases cover every retained field, diagnostics, owner/version changes, missing
  and duplicate roots, SQL types/NULL/UTF-8, domains, finite REAL bits, exact and
  one-over text/JSON byte/node/depth/key caps, joint remainders/overflow, deferred
  historical equality and matching copies without physical-owner authority.
- Test-only thread-local copy accounting proves joint node refusal occurs before
  OriginalJson byte ownership. The focused copy-order test passes separately.
- Four explicit mutants are assertion-caught (exit 101, one failed runtime test
  each): omitted uniqueness, ignored shared node remainder, omitted recheck
  comparison and premature caller COMMIT. Each restores exact original bytes.
  Restored source_rows SHA-256:
  `bcfe08b32f0b4309abd568fae90062f38b05aea92565c2e24d1ff80d87da69d7`.
- Local logs: TEMP `aff-p13c4b2b1-{focused,copy-order,rust,frontend}.log` and
  `aff-p13c4b2b1-mutant-{uniqueness,node-remainder,recheck-comparison,caller-finish}.log`.
- Initial frontend attempt hit sandbox esbuild spawn EPERM before tests ran;
  identical-command escalation passed after full locked Rust completed.

## Resolution (local verification)

Full `cargo test --locked`: 998 pass (297 library / 699 desktop / 2 service),
one existing ignored test; suite times 10.10 / 83.83 / 5.73 seconds. All-targets
locked check and Clippy pass, with the same five pre-existing warnings outside
this slice. Scoped rustfmt checks pass. Vitest: 1242 / 80 files; TypeScript and
production build pass. Static caller/boundary review, restored source hash,
relative links and git diff checks pass. No test failure was hidden by retry;
the initial frontend error was a sandbox child-process restriction.

The temporary mutation driver was removed after all four assertion receipts and
byte-for-byte restoration. Full Rust/format/check/Clippy ran on restored source.
Only the required complete-graph adapter/fixture refinement becomes Next after
verified delivery; all parent obligations remain open.

## Resolution (PR #214 delivery)

[PR #214](https://github.com/yoyoCadence/AlphaFactorForge/pull/214) merged tested
head `9bfbad41c128732d70727c2d73501e25d1526813` into unchanged #213 base
`9fc320d85f8942fd045b7d1979f0097d624af7dc`; merge is
`07cf11d13d073b1857c25451fa0edcf86553d28c`. All six jobs in
[CI 40035230223](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/40035230223)+succeeded, and each checkout log matched this head/base. Rust: 998 + one ignored
(297/699/2), 32.16 / 231.31 / 5.70 seconds; Vitest 1242 / 80 files. E2E passed
98 with no retries. Typecheck/build/production audit zero passed; native smoke
verified render/SQLite invokes/metrics/event, exact policy restoration and owned
process/directory cleanup. No failed CI attempt was retried.

Connector creation failed with 422 `must be a collaborator`; gh fallback created
the draft. Fresh main/head were unchanged, reviews/threads empty and PR mergeable.
Ready-body update, ready transition and expected-head guarded merge succeeded.
GitHub merged state/SHA were read back; local main fast-forwarded, full tree
equaled tested head and worktree was clean before the next branch. The final
delivery body was updated. Only complete-graph adapter/fixture refinement follows.

## Next Step

After full locked Rust, all-target check/Clippy, scoped format, frontend checks,
fresh base/rebase and exact-head six-job CI, merge with the expected head and
verify clean main. Then refine the already-required complete graph adapters and
fixture joins before their code. The 117-column manifest and joint/member caps
remain fixed; supported root selection/enumeration must detect consistent job
and attempt deletion. No missing cost/action/source evidence may be invented.
