-- 0006_request_effects — P03b acceptance fixes (R1, R2).
--
-- * `request_outcomes` (R1): the IMMUTABLE first outcome of a mutating
--   command, keyed by `request_id`, written inside the same transaction as
--   the domain change that decides it. Stages:
--
--     begun     the command started changing state (a run row was created,
--               a paused run was switched to running) but has not yet been
--               accepted — a crash here leaves exactly this row;
--     accepted  the command completed its admission; `outcome_json` is the
--               result the caller was told (`{"runId": n}`);
--     rejected  the command failed after it had begun; `outcome_json` holds
--               the error message the caller was told.
--
--   Only `begun` may be superseded (by `accepted` or `rejected`); the other
--   two never change. A receipt in `command_requests` is a copy of this row
--   and may fail to be written; the row cannot be missing if the change
--   committed, so a retry replays the FIRST outcome from here and never runs
--   the command again. A command that fails before changing anything has no
--   row: its retry is its first execution.
--
-- * `runtime_state.mutation_seq` (R2): a state version bumped inside every
--   domain write transaction (`ownership::write_transaction`). Snapshot reads
--   and `events.read` both return it, so a reader following the event ledger
--   from a cursor can tell that the database moved without a ledger row
--   arriving (an append that failed) and must take a fresh snapshot. Ledger
--   appends, request receipts, and heartbeats do NOT bump it: they describe
--   the state, they are not the state.

CREATE TABLE request_outcomes (
    request_id    TEXT    PRIMARY KEY,
    run_id        INTEGER NOT NULL REFERENCES discovery_runs(id) ON DELETE CASCADE,
    command       TEXT    NOT NULL,
    stage         TEXT    NOT NULL CHECK (stage IN ('begun','accepted','rejected')),
    outcome_json  TEXT,
    epoch         INTEGER NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    CHECK ((stage = 'begun' AND outcome_json IS NULL) OR (stage <> 'begun' AND outcome_json IS NOT NULL))
);
CREATE INDEX idx_request_outcomes_run ON request_outcomes(run_id);

CREATE TABLE runtime_state (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    mutation_seq  INTEGER NOT NULL CHECK (mutation_seq >= 0)
);
INSERT INTO runtime_state (id, mutation_seq) VALUES (1, 0);
