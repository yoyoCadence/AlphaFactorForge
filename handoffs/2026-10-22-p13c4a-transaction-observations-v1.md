# Handoff: P13c-4a transaction-owned observations

Date: 2026-10-22
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4a-transaction-owned-observations
Baseline: merged #207 (`49bbcc0`).
Status: implementation/local checks complete; exact-head CI/PR delivery pending.

## Summary

Implement only the [agreed read-helper contract](../docs/plans/research-final-admission-implementation-v1.md#2-first-code-slice-p13c-4a-transaction-owned-observations).
Borrowed transaction readers let a future caller retain its snapshot and write
reservation. They do not authenticate physical ownership or grant admission.

## Implementation organization

Keep public pre-mutex validation and deferred transaction finish behavior. Share
private SQL observation bodies with the new internal borrowed entry points.
Only the new entry points check active/matching registry-meta identity: making
the old public wrappers call those new guards directly would change historical
error precedence for damaged combinations and broaden the narrow count fence.
The borrowed helpers validate supplied evidence and never lock the receiver,
start/end a transaction, write rows or call into workspace/control/file code.

## Verification obligations

Prove public/helper parity, uncommitted import visibility and complete rollback,
no nested receiver lock with a bounded worker, retained IMMEDIATE reservation,
inactive/wrong-registry refusal, unchanged caller state/total_changes and deferred
snapshot behavior. Retain strict whole binding coverage and narrow count-fence
semantics. Catch deliberate mutants by assertions and restore exact source.
Run focused/full checks, then exact-head CI, fresh reviews/base and guarded merge.

## Remaining scope

No runtime caller, alpha spend, schema, policy/calibration or protected execution
changes. P13c-4 stays incomplete. After delivery, freeze the concrete 4b input
observation contract before its implementation.

## Resolution (focused tests and mutants)

The internal readers share private SQL observation bodies with the public
wrappers; active/metadata guards do not alter public error precedence. The
first 12 real-SQLite cases passed (0.65 seconds). Additional assertions cover
malformed saved bindings/future prefixes; a thirteenth case checks an actual SQL
read error preserves the caller's pending schema and complete rollback.

Four deliberate mutations were caught: premature commit (caller lifetime),
nested mutex (bounded 10-second refusal; guard always released before join),
growth reported unchanged (pending import), and omitted transaction guards
(foreign fence must refuse before prefix comparison). The last mutation first
failed through `unwrap_err` on a returned prefix refusal; its check was tightened
to an explicit error-code assertion and the mutation rerun successfully. These
are intentional mutant failures, not production-suite failures. Each run restored
the exact original source bytes; final SHA-256 was
`5f62eb840bea675938caba81243575393e5908c8c544e5a87ce0ab6a2ab0000b`.
Full restored-source verification and PR delivery follow.

## Resolution (full Rust suite)

`cargo test --manifest-path src-tauri/Cargo.toml --locked` passed on restored
source: **949 = 297 library + 650 desktop + 2 service tests**, one existing
ignored library test. Suite times: 42.47 / 424.04 / 5.61 seconds; total command
509.38 seconds. All 13 new cases passed, including the actual SQL read error.
The large pre-existing 128,000-interval import boundary also passed. Test process
34536 left zero binding fixture or private registry-preflight directories.

A structural comparison against merged #207 confirmed the binding chain,
restrictive identities, alpha/provenance/v5 logic and count SQL/comparisons are
unchanged after removing only owned-transaction setup/finish and adjusting
borrowed references. Full ancillary checks and PR delivery are pending.

## Resolution (local verification complete)

All-targets locked check (30.94 seconds), Clippy (19.98 seconds; the same five
existing warnings), scoped rustfmt for the new test module, **1242 Vitest / 80
files**, typecheck and production build passed. The build exited 0; a cp950
console formatter then failed on Vite's checkmark while printing its summary.
The complete UTF-8 log confirms successful build; no product failure or rerun is
being hidden. The legacy binding module was not globally formatted.

P13c-4a is Done locally; only 4b-design is Next after delivery. The concrete
workspace/input API and bounds must be fixed before its code. Exact-head six-job
CI, fresh base/reviews/threads, expected-head merge and clean-main receipt follow.
