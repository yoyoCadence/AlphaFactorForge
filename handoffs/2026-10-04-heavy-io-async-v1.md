# Handoff: heavy native I/O off the UI thread

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/heavy-io-async`, from merged PR #153 (`d1bf8e2`)
Status: Resolved; PR #155 merged after all six final-head CI jobs passed.

## Scope / agreed task

The maintainer requested continued autonomous task-board implementation,
PRs and merges. DB-ASYNC-001 already specifies async Tauri commands with
`spawn_blocking`, one SQLite coordinator boundary and a controlled slow-operation
responsiveness test. FU-5 completed campaign preview; this is the next bounded
slice for get_candles/import_candles/save_backtest_result/save_validation_record
and save_report. Invoke names/arguments/results remain the same; no schema,
batching, pagination or computation changes.

## Implementation plan

1. Take the current shared DB handle on the command's async path, then acquire
   its existing mutex and call unchanged repository functions in a blocking
   worker. Preserve repository errors; report worker-join failures explicitly.
2. Keep each mutating DB command admitted across its await, following the
   existing discovery-command pattern, so background hand-over drains it.
   Validation stays before its write transaction, in the worker.
3. Resolve the downloads path and validate the filename as before; put directory
   creation and the existing exclusive report writer in a blocking worker.
4. Use a held DB lock and a controlled worker to prove the command future
   yields; cover worker errors and preserved repository results. Reuse existing
   transaction/file/typed-client tests; verify full checks and final-head CI.

## Remaining scope

Lighter sync DB lists/settings/migrations and other command groups are not
converted here. They can still wait on the shared mutex when a worker is busy;
DB-ASYNC-001c will convert the remaining Phase A DB commands before closing
that residual polling-thread contention. The stub export_report still returns NotImplemented; no new
report format is added. Native operator acceptance stays separate.

## Implementation / initial verification

The five commands now use async Tauri handlers; repository validation,
transactions and exclusive report writing run in blocking workers. The DB
worker acquires the same shared mutex and returns repository errors unchanged;
worker panics become explicit command errors. Each mutating DB handler holds
the existing admission guard through completion, following discovery commands,
so service hand-over cannot pass its drain while a write is admitted.

Three tests pass: with SQLite deliberately locked, the first command poll
returns Pending before the lock is released; repository results/SQLite errors
survive the worker boundary; controlled worker panic returns a join error.
The slow test has a finite test-only 5 s deadline and releases the lock before
asserting, so an inline-lock regression fails without deadlocking the suite.
After rebase onto merged PR #154 (`7891218`), full verification passes:
**1083 Vitest / 65 files; 585 Rust (170 library + 413 desktop + 2 service),
1 ignored**. Typecheck, production build and all-target check pass. A new
clippy suggestion in the initial test helper was corrected to `Waker::noop`
(supported by the declared Rust 1.89 minimum); focused tests and clippy were
rerun. No frontend source, repository transaction, schema or dependency changed.
Local rendered/native interaction checks were not rerun; six CI jobs gate merge.

## PR / CI

[PR #155](https://github.com/yoyoCadence/AlphaFactorForge/pull/155), base main,
head `fix/heavy-io-async`, implementation `418ab68`, verification `947e3a8`.
The PR records final-head CI and merge status; all six jobs must pass before
the authorized merge.

## Resolution

PR #155 merged as `448c7f4274063f086b7cd3100f417f56a434edf7` after all six
jobs in [run 37214012035](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37214012035)
passed on final head `b2d6ecf9951df025eedfd1c4f4a4cbbacdb8f97c`.
DB-ASYNC-001c continues the remaining Phase A mutex waits using the same helper;
DB-ASYNC-001 remains open for separately reviewable command groups.
