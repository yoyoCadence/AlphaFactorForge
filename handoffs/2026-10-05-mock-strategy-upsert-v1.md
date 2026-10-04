# Handoff: strategy-save mock parity

Date: 2026-10-05
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/mock-strategy-upsert`, from merged PR #156 (`863adaf7`)
Status: Complete locally; waiting for the preceding PR #157 before publishing.

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
