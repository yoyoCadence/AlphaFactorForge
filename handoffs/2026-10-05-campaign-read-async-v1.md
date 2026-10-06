# Handoff: campaign read commands off the polling thread

Date: 2026-10-05
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/campaign-read-async`, from merged PR #161 (`880b668`)
Status: Resolved; PR #162 merged after all six final-head CI jobs passed.

## Scope / implementation plan

The user requested continuation after the previous agent exhausted its tokens.
Existing handoffs record authorization for task-board work, PR creation and
checked merges. PR #161 was verified and merged before starting this slice.
DB-ASYNC-001 already specifies async Tauri commands with blocking DB workers;
its Phase A helper and controlled lock/error/panic regressions are available.

1. Expose the existing `database_task` only to sibling command modules and
   reuse it for the four remaining synchronous campaign reads:
   `list_market_instruments`, `list_market_snapshots`,
   `list_research_campaigns` and `get_campaign_admission`.
2. Keep command names, camelCase invoke keys, return/error types, the snapshot
   limit and field projection, and the single SQLite mutex unchanged. Existing
   async preview/freeze/start commands and write admission remain unchanged.
3. Reuse the controlled locked-DB polling and error/panic regressions, campaign
   repository coverage and typed-client argument tests. Check all Rust targets,
   full Rust tests, frontend typecheck/build and final-head six-green CI.

No schema, dependency, query semantics, campaign qualification, pagination or
other command-group change. DB-ASYNC-001 remains open for the other command
groups. This is not native campaign/operator acceptance or P13 implementation.

## Implementation / verification

All four reads are async and call the existing `database_task`. It is exposed
only within the commands module (`pub(super)`); its lock, execution and error
handling are unchanged. The 500-snapshot limit, dataset field projection,
repository queries and `runId` invoke key are preserved. Existing campaign
preview/freeze/start commands were not modified.

- 35 focused Rust command/runtime tests pass, including the controlled busy-DB
  first-poll yield, repository error propagation and explicit worker-panic error.
- Full `cargo test --locked`: **585 passed (170 library + 413 desktop + 2
  service), 1 ignored**. Desktop suite 20.47 s; service suite 5.59 s.
- `cargo check --locked --all-targets` passes (20.21 s).
- 21 existing typed-client/mock tests in three files pass; all **1098 Vitest
  tests / 67 files** pass. No test assertions or specs were edited.
- Production build includes a passing TypeScript check; bundle names/sizes
  match the baseline, including the 17.83 kB worker chunk.
- Targeted campaign-command rustfmt and `git diff --check` pass. Re-fetch found
  main unchanged at `880b668`. Frontend tests/build need escalation because
  the sandbox denies the existing esbuild child startup.

This reuses the already-controlled blocking boundary; it does not claim a new
native command responsiveness or operator campaign acceptance run. The existing
full E2E and native bridge lanes must pass on the pushed head. The other
discovery/research/runtime readers remain separate DB-ASYNC-001 slices; P09
authenticated acceptance, P12 native campaign acceptance, P13 and numeric
versioning retain their existing states. Record final-head CI in the PR and
append merge evidence to this handoff in the next tracked update.

## Resolution (2026-10-06)

[PR #162](https://github.com/yoyoCadence/AlphaFactorForge/pull/162) merged as
`ac9ea4178edd11576bbd346f2a4264cb543bcd9a` (2026-10-05T13:42:46Z). All six jobs
(typecheck, test, build, cargo-check, native-smoke, e2e) in
[run 37317872403](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37317872403)
passed on final head `02a0538c166ff17a220fc520820f449c282c9c13`; the PR has no
review comments. Recorded by the next slice (DB-ASYNC-001e,
[handoff](2026-10-06-research-history-read-async-v1.md)) instead of a
docs-only push to the merged branch.
