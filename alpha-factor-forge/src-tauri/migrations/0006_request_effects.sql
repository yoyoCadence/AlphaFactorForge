-- 0006_request_effects — P03b acceptance fixes (R1, R2).
--
-- * `request_effects` (R1): the durable link between a mutating command's
--   `request_id` and the run it changed, written INSIDE the same transaction
--   as the domain change itself (run creation, cancel, pause, resume). The
--   receipt in `command_requests` is written afterwards and can fail; this
--   row cannot be missing if the change committed. A retry that finds a
--   `pending` receipt therefore looks here first: an effect means "the change
--   happened, recover the receipt from it, execute nothing"; no effect means
--   "the change never committed, this is the first execution". One row per
--   request (PRIMARY KEY), so a second execution of the same request inside
--   the same transaction as its effect is impossible.
--
-- * `runtime_state.mutation_seq` (R2): a state version bumped inside every
--   domain write transaction (`ownership::write_transaction`). Snapshot reads
--   and `events.read` both return it, so a reader following the event ledger
--   from a cursor can tell that the database moved without a ledger row
--   arriving (an append that failed) and must take a fresh snapshot. Ledger
--   appends, request receipts, and heartbeats do NOT bump it: they describe
--   the state, they are not the state.

CREATE TABLE request_effects (
    request_id  TEXT    PRIMARY KEY,
    run_id      INTEGER NOT NULL REFERENCES discovery_runs(id) ON DELETE CASCADE,
    command     TEXT    NOT NULL,
    epoch       INTEGER NOT NULL,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_request_effects_run ON request_effects(run_id);

CREATE TABLE runtime_state (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    mutation_seq  INTEGER NOT NULL CHECK (mutation_seq >= 0)
);
INSERT INTO runtime_state (id, mutation_seq) VALUES (1, 0);
