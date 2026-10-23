# Handoff: P13c-4b-2a bounded original JSON

Date: 2026-10-23
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b2a-original-json
Baseline: merged #211 (`5d6d366`).
Status: resolved; PR #212 merged and clean main verified on 2026-10-23.

## Summary

Implement only [the frozen original JSON slice](../docs/plans/research-source-reference-observation-v1.md#2-first-code-slice-bounded-original-json):
private owned original bytes/node count, strict syntax/decoded-key uniqueness,
opaque numbers, 4 MiB / 65,536 nodes / depth 64 / 1,024 decoded-key bytes.
No source/schema authority, numeric reinterpretation, SQL/IO, dependency change
or runtime caller. Complete row/semantic assembly and pinning follow separately.

## Implementation decisions

Check size/UTF-8 first; a constant-memory quote/escape-aware scan bounds container
depth before RawValue's iterative internal scratch stack. Borrow raw tokens;
stream children through Map/Seq visitors, retaining only per-object decoded
keys. Validate string Unicode scalars; count every value with root depth 1.
Copy exact original bytes only on success. No Value/f64 parse, wire/default/
unchecked constructor, mutable accessor or content-bearing Debug/errors.

## Verification obligations

Original numeric audit literals/whitespace/escapes/Unicode, all roots and mixed
node counts; decoded duplicates at all depths; malformed syntax/UTF-8/surrogates;
exact/one-over byte/node/depth/key limits, flat and branching trees; deep input
refusal, quote-aware scanning and detached owned clones. Four deliberate mutants
must fail through explicit assertions and restore exact original bytes before
full locked Rust, all-targets check/Clippy/scoped format, Vitest/typecheck/build,
links/scope checks and six-job exact-head CI/guarded merge/clean-main receipt.

## Remaining scope

Parent 4b-2/4b/P13c-4 remain incomplete. No complete source builder, migration,
registry spend, physical-owner/unseen permission, ordinary runner activation,
command/provider/UI, statistical-policy change, calibration or reserved seed.

## Resolution (focused verification)

All 15 new cases pass (0.12 seconds). Tests retain all six existing original
numeric-audit definitions, opaque out-of-range numeric tokens, whitespace and
escapes without reinterpretation. Independent literal counts cover scalar,
mixed, flat and branching trees; exact/one-over caps cover both scalar and empty
container depth and multibyte/escaped key/text bytes. Decoded duplicates,
malformed syntax/UTF-8/surrogates, trailing input, deep bounded rejection and
owned-clone isolation pass. An initial test-declaration typo was caught by
rustfmt and corrected before the successful test run. Deliberate mutants and
restored-source full checks follow.

## Resolution (mutant verification)

All four deliberately broken versions failed their intended explicit assertions:
decoded duplicate (24.55 seconds), one-extra-node acceptance (21.72), one-extra
depth acceptance (18.52) and whitespace-trimmed original bytes (23.83). Each
isolated test ran exactly once and exited 101 with its expected assertion;
none failed compilation. Original bytes were restored after each run and in
finally. Source SHA-256:
`84de7f19aa373df069e1181196d45940953cfdd9d476f4537099aedb51828e4e`.
These intentional failures are mutation evidence, not product-suite failures.
Restored-source full checks follow.

## Resolution (full local verification)

Restored-source full locked Rust passed: **978 = 297 library + 679 desktop +
2 service**, one existing ignored test. Suite times 32.58 / 413.66 / 1.36 seconds;
command 491.69 seconds. All 15 new cases and existing large joint byte/record/
interval import boundaries passed. Desktop test PID 36020 exited and left zero
workspace-observation fixtures/private registry-preflight directories.

All-targets locked check (22.02 seconds), Clippy (32.45; the same five existing
warnings), scoped rustfmt for the new module/tests, 1242 Vitest / 80 files,
typecheck and production build pass. Initial sandboxed npm startup refused
esbuild spawn with EPERM before tests; the identical command under escalation
passed all three checks without source/dependency changes. The initial log is
retained in the exact owned verification root until guarded cleanup.

Static checks confirm exact restored hash, private original-byte-only API,
no Debug/wire/default/unchecked constructor, Value/f64 reconstruction, SQL/IO,
domain authority or runtime caller. Old history/config/manual parsers, schema,
dependencies and workflows are unchanged. 4b-2a is Done locally; only the agreed
4b-2b concrete SQL/tuple contract refinement enters Next after verified delivery.
Parent 4b-2/4b/P13c-4 remain incomplete. Fresh rebase/rechecks, six exact-head CI
jobs, reviews/threads and guarded merge/clean main follow.

Six added relative links/anchors, lifecycle/order and seven-file scope checks
pass; git diff --check is clean. The exact named verification root's 17 expected
regular files (including the initial npm sandbox log) were removed only after
canonical root/parent, no-reparse, restored source hash and successful receipt
checks. No owned fixture/process/verification directory remains.

## Resolution (PR #212 delivery)

[PR #212](https://github.com/yoyoCadence/AlphaFactorForge/pull/212) merged tested
head `b37c6c44818975aa228617098174f33f6955a708` into unchanged #211 base
`5d6d366be4bbcb6ac2522c70463c5ed4dd6a79e6`; merge is
`be699bc26abd9d8b496d1936cc5c4677c296b206`. CI
[39893573470](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/39893573470)
finished with six successful jobs and exact head/base checkout logs. Rust: 978
(297/679/2, one ignored), suite times 27.64 / 326.67 / 5.80 seconds. Vitest:
1242 / 80 files; typecheck/production audit zero/build passed. Native verified
render/SQLite/invoke/metrics/event, exact policy restoration and owned cleanup.

Initial E2E: 97 passed plus one flaky (2.1 minutes). zoom.spec.ts:27 waited
30 seconds for link#afs-fonts's settled marker at chartLayout.ts:24; its internal
retry passed. UI/helper/config/assertions/timeouts were unchanged. Only that
job was repeated on the same head after initial workflow completion: 98 passed
(1.3 minutes), no test retry/flake. Initial history and repeat remain in the PR.
This confirmation does not establish the root cause or fix font-settlement
timing; retain a separate Backlog investigation without reordering P13.

Fresh main/base/branch were unchanged/clean, reviews/threads empty. Ready
body/refs readback preceded expected-head merge. Local main fast-forwarded to
remote merge, complete tree equal to tested head and worktree clean. Final
Delivery body/merge SHA were read back and verified. Only 4b-2b SQL refinement
follows; parent 4b-2/4b/P13c-4 remain incomplete.
