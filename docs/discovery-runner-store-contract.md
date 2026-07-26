# Discovery run/job store (RUNNER-STORE-001)

Status: implemented as a pure SQLite store. No worker pool, no Tauri commands,
no events — those are RUNNER-EXEC-001. Nothing here executes a backtest, and
the hidden Test segment has no representation at all.

Authority: the PR #66 handoff Resolution
(`handoffs/2026-07-19-runner-001-design-proposal-v1.md`), sections D5 and D6.

Owning code:

| Concern | Location |
| --- | --- |
| Schema | `src-tauri/migrations/0003_discovery_runner.sql` |
| Store API | `src-tauri/src/db/discovery.rs` |
| Behaviour tests | `src-tauri/src/db/discovery_tests.rs` |

## 1. Migration 0003

`discovery_runs` and `discovery_jobs` were created schema-only in 0001 and had
no writer at any point before this slice, so 0003 is the first migration that
gives either table data-carrying structure.

- **`discovery_jobs.candidate_index`** plus
  `UNIQUE(discovery_run_id, candidate_index, segment)`. The scheduling unit is
  the candidate, whose Train and Validation rows are paired checkpoints; the
  index makes a retry or double-enqueue impossible rather than merely unlikely.
  The column's `DEFAULT 0` exists only because SQLite demands a default when
  ALTERing in a `NOT NULL` column.
- **`validation_records.discovery_run_id`** (nullable) plus a PARTIAL
  `UNIQUE(discovery_run_id, strategy_id, dataset_id) WHERE discovery_run_id IS
  NOT NULL`. Manual UI assessments keep a null run and stay unconstrained, so
  PERSIST-001's append-only behaviour is unchanged for them; runner
  assessments can exist at most once per (run, strategy, dataset). The
  reference is `ON DELETE RESTRICT`, not `SET NULL`: these rows are immutable
  audit evidence, and nulling the linkage would silently erase which run
  produced an assessment, so a run that produced records cannot be deleted.
- **At most one non-terminal run**, enforced by a partial unique index on an
  expression that is identical for every `running`/`paused` row. Written with
  `OR` rather than `IN` because SQLite restricts index expressions.

Enforcing the single-run rule in the database rather than with a
check-then-act in Rust means a concurrent second start fails on the constraint
instead of racing.

Each migration and its `schema_migrations` row commit in ONE transaction.
SQLite DDL is transactional, so without that a migration failing partway (0003
has several statements) would leave half a schema behind AND no version row —
and every retry would then die on "duplicate column name", leaving the
database permanently unupgradeable.

## 2. State machine

Run transitions are exactly D5's, and anything absent is refused:

```
idle    -> running
running -> paused | completed | failed | cancelled
paused  -> running | cancelled
```

Terminal states never resume. `idle` deliberately holds no global slot, so
drafting a run never blocks another one; the slot is taken on `running` and
released only by a terminal state.

`Segment` is a two-variant enum (`Train`, `Validation`). A Test job row is
unrepresentable — not rejected at runtime, but impossible to construct.

## 3. The atomic candidate commit

`commit_candidate_assessment` writes, in ONE transaction:

1. Train summary + trades
2. Validation summary + trades
3. the append-only validation record, carrying its `discovery_run_id`
4. BOTH job rows → `done`, each pointing at its own summary
5. run `progress_json`
6. `strategy_def.lifecycle`

Before writing it re-runs PERSIST-001's `validate_validation_bundle`, requires
the run to actually be `running`, and requires the queued pair to exist, to
agree with the committed summaries' identity — a result whose strategy/dataset
does not match the candidate's job rows belongs to a different candidate — and
to still be `queued` or `running`. That last check matters: without it a
late-arriving result could flip an already `failed` or `skipped` row to `done`
and wipe its `error_message`, destroying the evidence D5 requires a failure to
carry. `fail_candidate_jobs` and `skip_remaining_jobs` are likewise restricted
to unfinished rows, so no terminal state is ever overwritten.

This atomicity is not cosmetic. `status = 'done'` is the runner's ONLY
checkpoint, so a partially applied assessment would make a resumed run skip
work that never actually landed.

The lifecycle update is derived from `record.gate_passed` rather than accepted
as a parameter, so a caller cannot record a verdict that contradicts the stored
evidence. Per D6: a pass moves `candidate`/`rejected` to `validated`; a fail
moves only `candidate` to `rejected`, so a validated strategy is never demoted
by a later dataset or run failure — that evidence lives in its immutable
record instead.

## 4. Completion and promotion

`running -> completed` is deliberately ABSENT from the transition table, so
`transition_run` cannot reach it. Completion exists only as
`complete_discovery_run`, which derives `best_strategy_id` instead of
accepting it: the highest FINITE-score gate passer of that run, ties resolved
by candidate index then strategy hash, null when nothing passed. Leaving the
transition in place would have given callers a second path to "completed" that
silently skipped the derivation.

Completion also refuses while any job is still `queued` or `running`.
"Completed" is terminal and never resumes, so allowing it mid-queue would
freeze unfinished work behind it and derive the winner from a partial set of
assessments. Cancellation drains the queue via `skip_remaining_jobs` first.

## 5. Crash recovery

`recover_orphaned_runs` moves orphaned `running` runs to `paused` and their
in-flight `running` jobs back to `queued`. `done` rows are never touched
because they mean a complete atomic assessment already exists. CPU work is
never resumed automatically — the user must explicitly resume. The operation
is idempotent: a second pass reports zero changes.

## 6. Test discipline

Every guard in this module rejects BEFORE writing anything, so a test that
trips a guard and then asserts "nothing was written" passes whether or not the
rollback works. Such tests prove the guard, not atomicity, and are named to say
so (`a_broken_job_pair_is_rejected_before_anything_is_written`,
`re_committing_a_done_candidate_is_rejected_before_any_write`).

The single rollback proof is
`a_failure_at_the_last_write_rolls_back_jobs_lifecycle_and_progress`: it
injects its failure at the LAST write in the transaction (a trigger on the
progress update), so summaries, trades, the record, BOTH job rows, and the
lifecycle promotion must all be undone. It is mutation-verified — setting the
transaction's drop behaviour to `Commit` makes it fail.

That verification is not ceremony. The PR #74 review's job-status pre-check
moved the earliest rejection for a re-commit *earlier* than the record INSERT,
which silently downgraded the then-current rollback test into a guard test.
Only re-running the mutation caught it. A guard added upstream of a failure
point can quietly invalidate a test that depended on reaching it.

Because that pre-check now shields it, 0003's per-run assessment uniqueness
index is no longer reachable through the store API; it is asserted directly at
the schema level by `per_run_assessment_uniqueness_is_enforced_by_the_schema`,
since it remains the last line of defence if a job row is manipulated.

## 7. Deliberately out of scope

- Worker pool, dequeue loop, pause/resume/cancel commands, single-writer
  serialization, versioned events — RUNNER-EXEC-001.
- Typed frontend wrappers and progress UI — RUNNER-UI-001.
- Cross-run result reuse: current summaries are mutable UPSERT views whose key
  omits the split/engine contract, and trades are not an immutable execution
  cache (Resolution D5).

`db/discovery.rs` carries a module-level `#![allow(dead_code)]` because nothing
in a non-test build calls the store yet. It must be REMOVED when
RUNNER-EXEC-001 wires the commands, so genuinely dead code starts failing
again.
