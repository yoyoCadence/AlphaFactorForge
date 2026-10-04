# Handoff: remaining Phase A DB commands on blocking workers

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/database-query-async`, from merged PR #155 (`448c7f4`)
Status: Complete locally; publishing for final-head CI and merge.

## Scope / agreed plan

The maintainer authorized continued task-board work, PR creation and checked
merges. DB-ASYNC-001 already defines the async / spawn_blocking approach.
PR #155 moved heavy I/O, but the remaining synchronous Phase A reads can still
block the command polling thread waiting for the same SQLite mutex.

1. Reuse the existing private database_task helper for all eight remaining DB
   handlers: run_migrations, get_datasets, save_strategy, get_strategies,
   get_backtest_results, get_backtest_result_detail, list_validation_records
   and get_validation_record. Keep init_database's immediate health response.
2. Preserve names, arguments, result/error types and repository transactions;
   connected mode continues to skip explicit migrations. Keep migration and
   strategy writes admitted across the await, following existing write handlers.
3. Reuse the controlled DB-lock responsiveness/error/panic tests and repository
   coverage. Run the appropriate Rust checks and existing frontend boundary
   checks, then gate merge on all six final-head CI jobs.

No schema, dependency, calculation, batching or pagination change. Other native
command groups and operator acceptance remain separately reviewable.

## Implementation / verification

All eight handlers now use the existing database_task worker; no DB mutex wait
remains on their command polling path. run_migrations still returns the same
connected-service skip message before DB work. Both writes hold an admission
guard through the await. init_database retains its immediate health response.

Full cargo test --locked passes: **585 Rust (170 library + 413 desktop + 2
service), 1 ignored**; library 9.59 s, desktop 19.32 s, service 3.61 s.
The existing controlled lock responsiveness/result/error/panic tests are reused.
All-target cargo check and clippy pass (five pre-existing warnings only).
The three typed-client/mock/campaign test files pass **17 tests**. Targeted
rustfmt and git diff --check pass. Frontend source was unchanged, so local
frontend build/full Vitest/native interaction were not repeated; all six CI
jobs must pass on the final pushed head before merge.
