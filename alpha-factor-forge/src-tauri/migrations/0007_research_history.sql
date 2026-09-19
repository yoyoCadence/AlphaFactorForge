-- 0007_research_history — P05 full research history (plan §3.3 "Hypothesis"
-- and "Research attempt" groups; docs/research-history-v1.md).
--
-- Three append-only tables beside the existing run records. None of them
-- replaces anything: `backtest_summary`/`backtest_trades` stay the LATEST
-- result projection (upserted per strategy/dataset/segment, as before), and
-- `validation_records` stays the append-only decision audit. What was
-- missing is the record of every attempt as it was submitted — the frozen
-- hypothesis it tested, the exact inputs and engine, what became of it —
-- and the complete result kept immutably even when the projection is later
-- overwritten by a re-run.
--
-- * `hypotheses`: registered BEFORE execution and never edited. Identity is
--   the content hash, so a repeated registration of the same hypothesis is
--   the same row (ABC-05: a repeated request adds nothing).
-- * `research_artifacts`: content-addressed references to immutable files in
--   the workspace's `artifacts/` directory (`<sha256[0..2]>/<sha256>.json`),
--   staged, hashed, renamed into place, and only then referenced here. A
--   referenced file is never deleted; unreferenced files are identifiable.
-- * `research_attempts`: one row per candidate submission (`attempt_key` is
--   its idempotency key, `run:<run>:candidate:<index>` for discovery). The
--   frozen columns (what was submitted, against what, on which engine) never
--   change; only the status moves forward, and a terminal status is final.
--
-- Triggers make the rules mechanical rather than a matter of discipline.

CREATE TABLE hypotheses (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    hypothesis_hash     TEXT    NOT NULL UNIQUE,
    version             TEXT    NOT NULL,
    source              TEXT    NOT NULL CHECK (source IN ('manual','discovery','ai')),
    mechanism           TEXT    NOT NULL,
    applicability_json  TEXT    NOT NULL,
    failure_modes       TEXT    NOT NULL,
    strategy_hash       TEXT    NOT NULL,
    strategy_id         INTEGER REFERENCES strategy_def(id),
    parent_strategy_id  INTEGER REFERENCES strategy_def(id),
    variation_kind      TEXT,
    content_json        TEXT    NOT NULL,
    created_at          TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TRIGGER hypotheses_are_immutable
BEFORE UPDATE ON hypotheses
BEGIN
    SELECT RAISE(ABORT, 'hypotheses are immutable');
END;

CREATE TRIGGER hypotheses_are_kept
BEFORE DELETE ON hypotheses
BEGIN
    SELECT RAISE(ABORT, 'hypotheses are never deleted');
END;

CREATE TABLE research_artifacts (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    sha256         TEXT    NOT NULL UNIQUE,
    kind           TEXT    NOT NULL,
    byte_len       INTEGER NOT NULL CHECK (byte_len >= 0),
    relative_path  TEXT    NOT NULL UNIQUE,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TRIGGER research_artifacts_are_immutable
BEFORE UPDATE ON research_artifacts
BEGIN
    SELECT RAISE(ABORT, 'research artifacts are immutable');
END;

CREATE TRIGGER research_artifacts_are_kept
BEFORE DELETE ON research_artifacts
BEGIN
    SELECT RAISE(ABORT, 'research artifacts are never deleted');
END;

CREATE TABLE research_attempts (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_key              TEXT    NOT NULL UNIQUE,
    hypothesis_id            INTEGER NOT NULL REFERENCES hypotheses(id),
    strategy_id              INTEGER NOT NULL REFERENCES strategy_def(id),
    dataset_id               INTEGER NOT NULL REFERENCES datasets(id),
    discovery_run_id         INTEGER REFERENCES discovery_runs(id),
    candidate_index          INTEGER,
    status                   TEXT    NOT NULL
                             CHECK (status IN ('submitted','running','completed','failed','skipped')),
    input_fingerprint_json   TEXT    NOT NULL,
    engine_fingerprint_json  TEXT    NOT NULL,
    outcome_json             TEXT,
    result_artifact_id       INTEGER REFERENCES research_artifacts(id),
    epoch                    INTEGER,
    submitted_at             TEXT    NOT NULL DEFAULT (datetime('now')),
    finished_at              TEXT
);

CREATE INDEX idx_research_attempts_run ON research_attempts(discovery_run_id, candidate_index);
CREATE INDEX idx_research_attempts_strategy ON research_attempts(strategy_id, dataset_id);
CREATE INDEX idx_research_attempts_hypothesis ON research_attempts(hypothesis_id);

-- The frozen columns never change, a terminal status is final, and a
-- completed attempt keeps its artifact.
CREATE TRIGGER research_attempts_are_frozen
BEFORE UPDATE ON research_attempts
WHEN NEW.attempt_key IS NOT OLD.attempt_key
  OR NEW.hypothesis_id IS NOT OLD.hypothesis_id
  OR NEW.strategy_id IS NOT OLD.strategy_id
  OR NEW.dataset_id IS NOT OLD.dataset_id
  OR NEW.discovery_run_id IS NOT OLD.discovery_run_id
  OR NEW.candidate_index IS NOT OLD.candidate_index
  OR NEW.input_fingerprint_json IS NOT OLD.input_fingerprint_json
  OR NEW.engine_fingerprint_json IS NOT OLD.engine_fingerprint_json
  OR NEW.submitted_at IS NOT OLD.submitted_at
  OR OLD.status IN ('completed','failed','skipped')
BEGIN
    SELECT RAISE(ABORT, 'research attempt is frozen');
END;

CREATE TRIGGER research_attempts_are_kept
BEFORE DELETE ON research_attempts
BEGIN
    SELECT RAISE(ABORT, 'research attempts are never deleted');
END;
