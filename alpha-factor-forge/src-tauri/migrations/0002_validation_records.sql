-- 0002_validation_records — PERSIST-001 (PR #64 handoff Resolution, revised
-- Option C). Immutable, append-only decision audit snapshots: one row per
-- completed validation assessment. `backtest_summary` remains the
-- "current/latest" materialized view (its key upserts); this table keeps every
-- historical judgment and has NO update/delete path in v1.
--
-- `record_json` is a self-contained `validation-record-v1` snapshot (embargo
-- derivation, split plan, Train/Validation metric snapshots, full benchmark
-- record incl. the Random Entry distribution, full GateVerdict, ScoreBreakdown
-- or null on gate fail, and contract versions). The Rust command validates the
-- JSON and finite score first; the CHECKs below are the second line of defense.

CREATE TABLE validation_records (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_id    INTEGER NOT NULL REFERENCES strategy_def(id),
    dataset_id     INTEGER NOT NULL REFERENCES datasets(id),
    record_version TEXT    NOT NULL,
    gate_passed    INTEGER NOT NULL CHECK (gate_passed IN (0, 1)),
    score          REAL,
    record_json    TEXT    NOT NULL,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    -- D3 invariant (Resolution addendum): gate fail => no score; pass => score.
    CHECK (
        (gate_passed = 0 AND score IS NULL)
        OR
        (gate_passed = 1 AND score IS NOT NULL)
    )
);

CREATE INDEX idx_validation_records_identity
    ON validation_records(strategy_id, dataset_id, created_at);
