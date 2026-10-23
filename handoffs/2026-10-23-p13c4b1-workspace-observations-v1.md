# Handoff: P13c-4b-1 workspace observations

Date: 2026-10-23
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b1-workspace-observations
Baseline: merged #209 (`4e51a8f`).
Status: implementation/local checks complete; exact-head PR/CI delivery pending.

## Summary

Implement only the [frozen metadata contract](../docs/plans/research-workspace-input-observation-v1.md#2-first-code-slice-private-workspace-observations):
internal ExpectedWorkspace/WorkspaceObservation, active borrowed SQL reads,
positive actual owner tuple, mutation comparison and bounded original metadata.
Read/compare only; no finish, write, locks, IO, registry or runtime caller.

## Verification obligations

Real migrated file-backed SQLite with isolated OS leases; capture/recheck,
pending mutation visibility/full rollback, independent writer reservation,
historical deferred snapshot versus fresh state, each owner/identity/version
change, ignored diagnostics, malformed/missing/type/byte-bound/read errors and
unchanged caller activity/total_changes/all data. Matching copies remain
consistency-only. Preserve old public helpers. Explicitly catch omitted owner,
omitted mutation, inactive and premature-finish mutants; restore exact bytes
before full checks, exact-head CI, fresh reviews/base and guarded merge.

## Remaining scope

Parent 4b/P13c-4 remain incomplete. Complete source/reference and bounded pinned
input contracts follow separately. No schema, source/artifact reader switch,
spend, physical-owner/unseen capability, policy/mediation authorization,
command/provider/UI, execution, calibration or reserved-seed experiment.

## Resolution (focused implementation verification)

The new module is private to research; old public ownership/identity helpers are
unchanged. Metadata SQL checks types/byte bounds before bounded raw-byte copies;
strict JSON/ASCII identity validation maps invalid UTF-8 TEXT to the same static
invalid refusal. Actual owner and mutation comparisons follow the frozen order.
No runtime caller, registry, write, transaction finish or physical-owner guard.

Fourteen real-SQLite tests passed (1.16 seconds), covering actual isolated OS
leases, pending state/full rollback, retained writer reservation across success/
refusal/SQL errors, historical deferred equality, every owner/version/identity
change, diagnostic-only updates, malformed/BLOB/invalid-UTF-8/missing/NULL rows,
256/257-byte JSON, escaping/case and unchanged legacy reader behavior. Snapshots
retain raw typed cells (including invalid text bytes), all rows and schema, plus
total_changes/activity checks. Matching metadata in a separately leased copy
remains explicitly consistency-only. Four deliberate mutants/full checks follow.

## Resolution (mutant verification)

Four deliberate mutations failed through their intended explicit assertions:
omitted owner comparison (34.22 seconds), omitted mutation comparison (35.11),
omitted active-transaction check (21.28) and premature commit (33.17). Each
isolated test exited 101 for the expected assertion, without compiler failure.
The harness restored the exact original bytes after every run and in finally;
source SHA-256 is
`7aa9168acd5f785da4a2808bf4f45bd60b1b53dbbb53cf0cde2597a92c1962c6`.
These intentional failures do not represent product-suite failures. Full
restored-source checks and delivery follow.

## Resolution (full Rust verification)

Restored-source `cargo test --manifest-path src-tauri/Cargo.toml --locked`
passed: **963 = 297 library + 664 desktop + 2 service**, one existing ignored
library test. Suite times: 28.58 / 378.22 / 1.29 seconds; command 448.80 seconds.
All 14 new cases and the old large import boundaries passed. Process 29576 left
zero workspace-observation fixtures or private registry-preflight directories.

Static boundary checks confirm exact restored hash and no runtime caller,
write/finish/nested-transaction/lock/file IO or serde/default constructor in the
private module. Ancillary checks and guarded PR delivery follow.

## Resolution (local checks complete)

All-targets locked check (32.14 seconds), Clippy (29.05; the same five existing
warnings), scoped rustfmt for the new module/tests, 1242 Vitest / 80 files,
typecheck and production build passed. No legacy/global format change. Only
new private reader/test source and the single module declaration changed;
existing public readers, schema, dependencies, commands and runtime stay intact.

4b-1 is Done locally; only 4b-2-design becomes Next after verified delivery.
Actual source/reference membership, strict original rows and exact original-byte
hash derivations must be frozen before that implementation; pinning remains the
later 4b-3 contract. Parent 4b/P13c-4 stay incomplete. Fresh main/rebase/rechecks,
six exact-head CI jobs, reviews/threads and guarded merge/clean main follow.

Seven added relative links/anchors and task lifecycle/order checks pass;
`git diff --check` is clean. The expected scope is the private module/test file,
one module declaration and four contract/task/handoff Markdown files.
