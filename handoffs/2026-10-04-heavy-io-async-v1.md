# Handoff: heavy native I/O off the UI thread

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/heavy-io-async`, from merged PR #153 (`d1bf8e2`)
Status: In Progress

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
converted here. The stub export_report still returns NotImplemented; no new
report format is added. Native operator acceptance stays separate.
