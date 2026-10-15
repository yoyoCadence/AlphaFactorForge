-- P13b-1: append-only alpha evidence in the external trial registry.
-- This is storage only; these rows are not confirmation admission permits.
CREATE TABLE alpha_family_budgets (
    family_id TEXT PRIMARY KEY REFERENCES trial_families(family_id),
    allocation_id TEXT NOT NULL,
    document_json TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);

CREATE TABLE alpha_reservations (
    reservation_id TEXT PRIMARY KEY,
    family_id TEXT NOT NULL REFERENCES alpha_family_budgets(family_id),
    record_json TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);
CREATE INDEX idx_alpha_reservations_family ON alpha_reservations(family_id);

-- One content-addressed conflict may stop both families of a reassigned
-- source operation. Neither confirmation number nor operation key is UNIQUE:
-- future imports must retain independently allocated conflicting records.
CREATE TABLE alpha_conflicts (
    conflict_id TEXT NOT NULL,
    family_id TEXT NOT NULL REFERENCES alpha_family_budgets(family_id),
    detail_json TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY (conflict_id, family_id)
);

CREATE TRIGGER alpha_family_budgets_no_update BEFORE UPDATE ON alpha_family_budgets
BEGIN SELECT RAISE(ABORT, 'alpha_family_budgets is append-only'); END;
CREATE TRIGGER alpha_family_budgets_no_delete BEFORE DELETE ON alpha_family_budgets
BEGIN SELECT RAISE(ABORT, 'alpha_family_budgets is append-only'); END;
CREATE TRIGGER alpha_reservations_no_update BEFORE UPDATE ON alpha_reservations
BEGIN SELECT RAISE(ABORT, 'alpha_reservations is append-only'); END;
CREATE TRIGGER alpha_reservations_no_delete BEFORE DELETE ON alpha_reservations
BEGIN SELECT RAISE(ABORT, 'alpha_reservations is append-only'); END;
CREATE TRIGGER alpha_conflicts_no_update BEFORE UPDATE ON alpha_conflicts
BEGIN SELECT RAISE(ABORT, 'alpha_conflicts is append-only'); END;
CREATE TRIGGER alpha_conflicts_no_delete BEFORE DELETE ON alpha_conflicts
BEGIN SELECT RAISE(ABORT, 'alpha_conflicts is append-only'); END;
