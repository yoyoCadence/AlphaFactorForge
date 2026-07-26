-- 0003_discovery_runner — RUNNER-STORE-001 (PR #66 handoff Resolution D5/D6).
--
-- `discovery_runs` and `discovery_jobs` were created schema-only in 0001 and
-- have never had a writer, so this migration is the first time either table
-- carries data. It adds exactly the structure D5 requires and nothing else:
-- no worker pool, no events, no commands.

-- ---------- 1. Candidate identity on jobs ----------
-- A job row belongs to one candidate assessment. The scheduling unit is the
-- candidate, whose Train and Validation rows are paired checkpoints that
-- transition together, so (run, candidate, segment) must be unique: without
-- it a retry or a double-enqueue could silently create a second job for work
-- that already committed.
--
-- The DEFAULT exists only because SQLite requires one when ALTERing in a NOT
-- NULL column. `discovery_jobs` is provably empty (no INSERT for it exists
-- anywhere in the codebase at 0002), and the repository always supplies a
-- real enumeration index.
ALTER TABLE discovery_jobs ADD COLUMN candidate_index INTEGER NOT NULL DEFAULT 0;

CREATE UNIQUE INDEX idx_jobs_run_candidate_segment
    ON discovery_jobs(discovery_run_id, candidate_index, segment);

-- ---------- 2. Run linkage on validation records ----------
-- Nullable by design: manual UI assessments have no run and stay unconstrained
-- (PERSIST-001 keeps this table append-only for them). Runner-produced records
-- carry their run, and at most ONE assessment may exist per
-- (run, strategy, dataset) — a second one would mean the same candidate was
-- assessed twice in the same run, which the candidate transaction must not be
-- able to do even under retry.
ALTER TABLE validation_records ADD COLUMN discovery_run_id INTEGER
    REFERENCES discovery_runs(id) ON DELETE SET NULL;

CREATE UNIQUE INDEX idx_validation_records_run_assessment
    ON validation_records(discovery_run_id, strategy_id, dataset_id)
    WHERE discovery_run_id IS NOT NULL;

-- ---------- 3. At most one non-terminal run ----------
-- D5: v1 permits only one `running` or `paused` run globally; a paused run
-- must resume or cancel before a new one starts. The indexed expression is
-- the same for every non-terminal row, so the second such row collides.
--
-- Written with OR rather than IN because SQLite restricts what may appear in
-- an index expression. `status` is a plain column, so the expression stays
-- deterministic.
CREATE UNIQUE INDEX idx_discovery_runs_single_active
    ON discovery_runs((status = 'running' OR status = 'paused'))
    WHERE status = 'running' OR status = 'paused';

-- Dequeue path: the runner scans its own run's queued jobs in candidate order.
CREATE INDEX idx_jobs_run_status ON discovery_jobs(discovery_run_id, status, candidate_index);
