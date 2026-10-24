# Handoff: bounded raw original digest observations

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b2c1-original-digests
Status: Locally verified; exact-head CI and delivery required

## Summary

From merged #219 (`8238fe7`), implement only sections 3-6 of the
[frozen original adapter contract](../docs/plans/research-original-semantic-adapters-v1.md).
The private graph child reads only a sealed complete SourceRowsObservation.
Its sole ancestor-facing constructor returns immutable original-byte digests;
borrowed read-only getters expose no mutation, caller parts/domain/subset,
serialization or source/registry conversion.

The original campaign is hashed directly. Fixed definition/execution/lineage
domains preserve original JSON bytes and trial IDs; the engine manifest includes
every candidate in index order using u64be index parts. SHA updates begin only
after checked whole-operation reference/shape/output/work preflight. Reused
config/hypothesis/engine bytes are charged on every appearance, including all
domain/count/length/index prefixes. The private inclusive ceiling is 33554432
hashed bytes; output has 3*N+2 fixed-width strings, at most 786560 text bytes.
No parsed JSON tree, full graph clone or concatenated payload is retained.

## Scope and remaining obligations

No runtime caller, SQL/IO/clock/hardware/worker access, schema/dependency/workflow
change, reservation/spend or source builder. These checksums preserve raw claims;
they authenticate no semantic execution, artifact/candle/action/cost original,
current owner or registry. Old/copied graph equality remains historical only.
Parent 4b-2c/4b-2/4b/P13c-4 stay incomplete. Next is the independently frozen
candidate/engine/hypothesis original-format projection and actual-producer
controls required by section 2, before their separate semantic code.

## Verification

Literal fixture vectors were copied from the already merged design, not generated
by the production implementation. Independent Python hashlib/struct recomputation
matches every graph digest (7938 work / 704 output bytes) and seven framing
controls, including whitespace, empty parts, order and leading-zero hex.

Real migrated file-backed graphs and isolated leases exercise original byte
preservation, exact field effects, failed members/all engines, 4096 candidates,
shared-hypothesis charging, arithmetic overflow/atomic prior usage, and exact /
one-over whole hashed-work limits while the retained graph remains valid. A
test-only thread-local SHA tripwire covers direct campaign and framed updates.
Caller transactions/reservations, rows/schema/total_changes/mutation_seq and
historical saved graphs are preserved. The unchanged real run_lineage/history
writer control also passes through the observer without synthetic per-candidate
hypothesis assumptions.

The initial eight-case focused run caught one test-setup error: the campaign
whitespace perturbation hit its append-only trigger. Only the test-owned pending
transaction now drops that named guard and rolls it back; production safeguards
are unchanged. Original initial failure is retained in this record. A ninth case
also challenges damaged private test copies for late reference/index/event and
candidate-count refusals before any SHA update.

Full verification, six restored assertion mutants, exact-head CI and merge
receipt will be appended after their completion.

## Resolution (focused checks and restored mutants)

All **91 focused confirmation-input cases** pass (nine new digest cases plus
82 earlier cases); actual writer shared-base control passes through the observer.
Six explicit source mutations each exited 101 with an assertion panic / FAILED,
never a compiler error: omit part lengths (43.59 s), omit part count (30.61 s),
decimal index bytes (43.19 s), first engine only (52.75 s), disable whole work
ceiling (88.58 s; SHA tripwire catches the first attempted update), and reset
work per domain (42.20 s). Each used its exact targeted test and an external
TEMP log. Driver finally restored exact original source bytes; independently
read-back SHA-256 is
`ac43081f6cc636ae46815d770f13e63e8e60cfbd51b58c0a4a614fd4312c035a`.

Full locked Rust is running in its own fresh TEMP/TMP root before Node checks,
avoiding the separately tracked existing PID/counter reuse issue. No preexisting
TEMP directory is cleared, and logs stay outside the owned verification root.

## Resolution (full local verification)

Fresh isolated full locked Rust passed on the first full run: **1040 + one
ignored** (297/741/2), 39.66 / 487.67 / 1.37 seconds. All-target check and Clippy
pass (39.60 / 39.57 seconds); only the five existing backtest/score/file-command
warnings. Scoped graph/new-module/test rustfmt passes. After all Rust work,
Vitest **1242 / 80 files**, TypeScript and production build pass. New evidence
links, private API / sole SHA-wrapper scope, independent frozen literals and
diff --check pass. No runtime/dependency/schema/workflow changes.

Logs are retained under normal TEMP as `aff-p13c4b2c1-{focused,rust,check,clippy,
frontend-test,typecheck,build}.log` and six `mutant-*.log` files. The exact owned
full-verification root was removed only after absolute parent/name checks and
complete traversal proving no reparse points: 42 regular files / 63 directories;
no existing TEMP root was deleted. Source SHA read-back still matches the exact
restored value above.

Fetch/fast-forward main, rebase, relevant post-rebase checks and explicit scoped
commit/push precede the draft PR. Require all six exact-head jobs, fresh base and
reviews, expected-head merge, GitHub read-back and clean main before starting
the next original-format projection design.
