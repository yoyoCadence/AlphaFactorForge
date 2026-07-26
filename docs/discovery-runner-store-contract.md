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
  assessments can exist at most once per (run, strategy, dataset).
- **At most one non-terminal run**, enforced by a partial unique index on an
  expression that is identical for every `running`/`paused` row. Written with
  `OR` rather than `IN` because SQLite restricts index expressions.

Enforcing the single-run rule in the database rather than with a
check-then-act in Rust means a concurrent second start fails on the constraint
instead of racing.

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
the run to actually be `running`, and requires the queued pair to exist and to
agree with the committed summaries' identity — a result whose strategy/dataset
does not match the candidate's job rows belongs to a different candidate.

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

`complete_discovery_run` derives `best_strategy_id` instead of accepting it:
the highest FINITE-score gate passer of that run, ties resolved by candidate
index then strategy hash, null when nothing passed. A caller cannot record a
winner the stored assessments do not support.

## 5. Crash recovery

`recover_orphaned_runs` moves orphaned `running` runs to `paused` and their
in-flight `running` jobs back to `queued`. `done` rows are never touched
because they mean a complete atomic assessment already exists. CPU work is
never resumed automatically — the user must explicitly resume. The operation
is idempotent: a second pass reports zero changes.

## 6. Test discipline

The rollback claim is pinned by
`a_failure_after_the_writes_rolls_everything_back`, whose failure is triggered
AFTER the summaries and trades were written (the per-run uniqueness rule firing
on the record insert) and which asserts the surviving rows still hold the
PREVIOUS commit's values.

This distinction matters. The weaker shape — delete a job row, then assert
"nothing was written" — passes whether or not the rollback works, because that
guard fires before any write. It is kept separately and named
`a_broken_job_pair_is_rejected_before_anything_is_written` so it cannot be
mistaken for atomicity evidence. The rollback test was verified by mutation:
flipping the transaction's drop behaviour to `Commit` makes it fail, and made
the weaker test pass.

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
