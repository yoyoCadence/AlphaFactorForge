# Handoff: strategy-save mock parity

Date: 2026-10-05
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/mock-strategy-upsert`, from merged PR #156 (`863adaf7`)
Status: Resolved; PR #158 merged after six green CI jobs.

## Scope / agreed plan

The maintainer authorized continued task-board implementation, PRs and checked
merges. TEST-MOCK-PARITY-001 specifies the mock's same-hash strategy UPSERT.
The real repository updates name/source/updated_at, preserving the row id and
all other fields, particularly the validation-owned lifecycle. The current
mock always appends a new row/id, masking native library behavior in E2E.

1. Keep existing definition hash/type verification before any mock write.
2. Find a strategy by its verified hash; refresh name/source and return its
   stored id, retaining all immutable metadata/definition/lifecycle. Insert a
   new row only for a new hash. The DTO exposes no timestamps, so no timestamp
   field or ordering redesign is introduced.
3. Add behavioral regressions for repeated saves, rename/source updates,
   canonical-equivalent raw definition preservation, immutable metadata and
   lifecycle, rejection without writes, and distinct hashes remaining separate.
   Reuse the existing library E2E; run focused/full Vitest, typecheck/build and
   final-head CI before merge.

No Rust, schema, dependency, hash algorithm or rendered UI change. Detached
strategy reads and other mock behavior are outside this bounded task.

## Implementation / verification

After unchanged hash/type verification, the mock finds the existing hash,
updates name/source and returns its id. New hashes retain the existing insert
path. Four new tests cover repeated same-hash saves for both validated/rejected
lifecycle, canonical-equivalent but differently ordered JSON retaining the
original text and auxiliary metadata, forged/type-mismatched input preserving
the stored row, and distinct hashes keeping separate rows.

Focused mock/db tests pass **12 tests**. Full Vitest passes **1087 tests / 65
files**; typecheck and production build pass. The production JS asset remains
index-C57TOz7R.js (353.31 kB), consistent with this dev-only mock change.
Rust and the real SQLite UPSERT/lifecycle repository tests are unchanged; their
last full baseline passes **585 Rust / 1 ignored**. The existing library E2E
and full native/unit/E2E gates run in final-head CI before the authorized merge.
No local rendered interaction or new E2E test is claimed here.

## Rebase / publish verification (2026-10-05)

Rebased onto #157's checked merge `e8627065`. The only conflicts were two
task-board insertions; both native and mock records were retained with no
markers. Focused mock/DB tests pass 12/12 and typecheck passes again. Full
1087 Vitest/build evidence above remains scoped to the same product change;
six final-head CI jobs, including the now-real native bridge lane, gate merge.

## Resolution (2026-10-05)

[PR #158](https://github.com/yoyoCadence/AlphaFactorForge/pull/158) merged as
`2c2362bb66f48670cf9e1defd09d2badbd3a3677` after all six jobs passed on
head `f7232a8b1292c3664a3d2af276cbf55e79a83507`,
[run 37297346065](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37297346065).
The full unit, library E2E and real native bridge lanes passed; this closes
TEST-MOCK-PARITY-001 within its strategy-save scope.
