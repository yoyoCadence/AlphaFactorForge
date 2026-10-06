# Handoff: discovery progress reads off the polling thread (closes DB-ASYNC-001)

Date: 2026-10-06
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/discovery-progress-read-async`, from merged PR #163 (`201b434`)
PR: opened from this branch
Status: Complete locally; final-head six-green CI gates merge.

## Scope / implementation plan

Continuation of DB-ASYNC-001 under the user's standing instruction to keep
completing tasks, open PRs and merge checked ones. PR #163 (DB-ASYNC-001e) was
verified and merged first; its Resolution is appended to
[its handoff](2026-10-06-research-history-read-async-v1.md), which named this
slice as the next step.

`get_discovery_progress` and `get_active_discovery_run` were the last
synchronous command handlers doing blocking I/O. The window polls them while a
run is active (`DiscoveryPanel`, `CampaignPanel`), exactly when the runner
takes the same SQLite mutex for its commits; in connect mode each call is a
loopback HTTP round trip to the service. Both ran on the command polling
thread.

1. Make both handlers async. In both host modes the read runs on a new private
   `read_task` blocking helper in `discovery_commands.rs`, which reuses that
   module's existing `join_error` text (`discovery command task failed: …`).
2. Keep command names, the `runId` invoke key, the mode-agnostic
   `discovery-progress-v1` JSON (`null` when no run is active), runner and
   proxy methods, and error texts unchanged.
3. Leave the existing start/pause/resume/cancel handlers as they are (already
   async) — no reformatting of their pre-existing rustfmt drift.

No schema, dependency, runner, proxy, contract or frontend change.

## Implementation / verification

- `discovery_commands.rs` only. Three new tests in a new module test block,
  using a real migrated in-memory DB and a `DiscoveryRunner`:
  - `a_progress_read_does_not_block_the_command_polling_thread` — with the DB
    mutex held (as a committing run holds it) the first poll must be Pending;
    finite 5 s test deadline, lock released before asserting.
  - `reader_results_and_errors_cross_the_worker_unchanged` — no active run is
    still JSON `null`; a missing run is exactly `discovery run 404 not found`,
    not a worker-join error.
  - `a_panicking_reader_returns_an_explicit_command_error`.
  **Mutation check:** replacing the helper body with an inline `operation()`
  fails the yield test (5 s timeout) and the panic test; restored and
  re-verified.
- Full `cargo test --locked`: **591 passed (170 library + 419 desktop +
  2 service), 1 ignored** (585 before DB-ASYNC-001e, + 3 there, + 3 here).
  `cargo check --locked --all-targets` passes.
- `cargo clippy --locked --all-targets`: no warning in
  `discovery_commands.rs` or `research_commands.rs`; the five reported
  warnings are pre-existing in `discovery_core/backtest.rs`,
  `discovery_core/score.rs` and `commands/file_commands.rs` (not touched).
- rustfmt: every hunk `rustfmt` would change in this file is pre-existing
  drift on `main` (CI-RUSTFMT-001); the new lines produce none.
- Frontend unchanged: `npm run typecheck`, **1098 Vitest / 67 files** and
  `npm run build` pass; bundle unchanged (worker chunk 17.83 kB).

Not claimed: a native-window responsiveness measurement. The final head must
pass all six CI jobs (including native-smoke and e2e) before merge.

## Command inventory after this slice (DB-ASYNC-001 closure)

Every `#[tauri::command]` lives under `src-tauri/src/commands/`. After this
slice, the only synchronous handlers are:

| Handler | Why it can stay synchronous |
| --- | --- |
| `init_database` | Returns a constant string; migrations already ran at startup (`run_migrations` is async). |
| `validate_strategy_dsl` | Pure, bounded validation of one DSL document; no I/O. |
| `generate_strategy_dsl`, `save_ai_api_key`, `get_ai_api_key_status`, `delete_ai_api_key`, `test_ai_connection` | `NotImplemented` stubs (Phase C). Their real keychain/provider work must be async when implemented. |
| `export_report` | `NotImplemented` stub; report writing is the async `save_report`. |
| `get_workspace_info` | Copies in-memory host state; the host-mode mutex is only held briefly (the hand-over swaps in `Switching` and releases it). |
| `get_host_status` | Same host-mode read plus one `current_exe`/`is_file` stat. |

All SQLite, artifact-file, report-file and service round-trip work now runs
behind async commands on blocking workers: DB-ASYNC-001 FU-5 (campaign
preview), 001b (bulk candles/import, result/validation saves, report writing,
with the controlled held-lock responsiveness test), 001c (remaining Phase A
DB handlers), 001d (campaign reads, PR #162), 001e (research history and
artifacts, PR #163) and 001f (this slice). The parent's acceptance — async
commands plus blocking workers, one SQLite coordinator boundary, a controlled
slow-operation responsiveness test — is met, so DB-ASYNC-001 moves to Done.
Pagination/batching was explicitly "only after" this and is not started; a
native-window responsiveness measurement remains operator acceptance, not a
claim here.

## Next recommended step

Pick the next eligible Backlog item in task-board order. Small, independent
candidates in the same "Performance, documentation, and tooling debt" group:
`DB-MIGRATION-DIAGNOSTIC-001` (P3, migration existence-query error handling)
and `CI-RUSTFMT-001` (P3, format drift + CI check), or the P2
`PERF-CHART-COMPUTE-001` / `PERF-CHART-BRIDGE-001` items.
