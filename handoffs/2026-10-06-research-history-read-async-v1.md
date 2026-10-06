# Handoff: research-history reads off the polling thread

Date: 2026-10-06
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/research-history-read-async`, from merged PR #162 (`ac9ea41`)
PR: #163
Status: Resolved; PR #163 merged after all six final-head CI jobs passed.

## Scope / implementation plan

Continuation of DB-ASYNC-001 under the user's standing instruction to keep
completing tasks, open PRs and merge checked ones. PR #162 (DB-ASYNC-001d) was
verified merged before this slice started; its Resolution is appended to
[its handoff](2026-10-05-campaign-read-async-v1.md).

An inventory of `#[tauri::command]` handlers on `ac9ea41` found these
synchronous handlers that still touch SQLite or the workspace files:

- P05 research history (`research_commands.rs`): `list_research_attempts`,
  `list_hypotheses`, `list_unreferenced_artifacts`, `get_research_attempt`.
  The last two also walk/read the artifact store after the DB lock is dropped.
- Discovery progress (`discovery_commands.rs`): `get_discovery_progress`,
  `get_active_discovery_run` — DB lock in embedded mode, an HTTP round trip
  to the service in connect mode. Left for the next slice (DB-ASYNC-001f).

`get_workspace_info` / `get_host_status` read only in-memory host state plus
one `is_file` stat, and the AI/secret/report stubs do no I/O; they are not in
scope.

This slice (DB-ASYNC-001e) covers the four research-history reads only:

1. Make them async; SQLite work runs through the existing commands-module
   `database_task` (same mutex, same blocking worker, same error mapping).
2. Artifact work (directory walk, read + checksum) runs on a new private
   `artifact_task` blocking helper that holds no DB lock. The DB mutex is still
   released before any artifact file is touched, as before.
3. Keep command names, invoke keys (`filter`, `limit`, `id`), DTOs, the
   100-default / 500-max page clamp, the `null` for a missing attempt and the
   `resultError` texts unchanged. The artifact-to-result mapping moves
   verbatim into a private `read_result` function.

No schema, dependency, query, history-contract, frontend or other
command-group change. DB-ASYNC-001 stays open.

## Implementation / verification

- `research_commands.rs` only. `rustfmt --check` on the file passes; its
  pre-existing drift sat entirely in the rewritten handlers, so no untouched
  line changed.
- Three new controlled tests in the module:
  - `artifact_work_does_not_run_on_the_command_polling_thread` — the operation
    blocks on a test-held gate, so an inline call cannot yield on the first
    poll; finite 5 s test deadline. **Mutation check:** replacing the helper
    body with an inline `operation()` makes this test fail after 5.02 s; the
    file was restored and re-verified.
  - `a_panicking_artifact_worker_returns_an_explicit_command_error` —
    `research artifact task failed: …`, mirroring the DB worker test.
  - `artifact_results_and_failures_are_reported_unchanged` — no artifact →
    `(null, null)`; valid JSON crosses the worker unchanged; invalid JSON →
    `artifact is not valid JSON: …`; a file altered after storing → the
    checksum error; the unreferenced walk crosses the worker unchanged.
- The SQLite half reuses the existing `database_task` regressions (busy-DB
  first-poll yield, repository error propagation, worker panic) and the
  research-history repository tests.
- Full `cargo test --locked`: **588 passed (170 library + 416 desktop +
  2 service), 1 ignored** (baseline 585 + the 3 new tests).
  `cargo check --locked --all-targets` passes.
- Frontend unchanged: `npm run typecheck`, **1098 Vitest / 67 files** and
  `npm run build` pass; bundle unchanged (worker chunk 17.83 kB). No typed
  client or E2E spec references these commands' behaviour beyond the
  unchanged invoke keys in `src/tauri-client/commands.ts`.

Not claimed: a native-window responsiveness measurement for these commands,
or any change to the research-history contract. The final head must pass all
six CI jobs (including native-smoke and e2e) before merge.

## Next recommended step

DB-ASYNC-001f: `get_discovery_progress` and `get_active_discovery_run` — both
polled during runs — move to `spawn_blocking` for the embedded DB read and the
connect-mode proxy round trip, preserving the `discovery-progress-v1` JSON in
both modes. After that, re-run the command inventory and decide whether the
DB-ASYNC-001 parent can close.

## Resolution (2026-10-06)

[PR #163](https://github.com/yoyoCadence/AlphaFactorForge/pull/163) merged as
`201b434dc6646ca71e7e8fb7a34d30f870dcd56f` (2026-10-06T13:24:55Z), pinned to the
verified head with `--match-head-commit`. All six jobs (typecheck, test,
build, cargo-check, native-smoke, e2e) in
[run 37469573739](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37469573739)
passed on final head `7402e9ff1e6e0a16ce9d429a4ad2e8e321dc9274`; the PR had no
reviews or comments. A later `cargo clippy --all-targets` run (DB-ASYNC-001f)
reports no warning in this module. Recorded by the next slice
([handoff](2026-10-06-discovery-progress-read-async-v1.md)).
