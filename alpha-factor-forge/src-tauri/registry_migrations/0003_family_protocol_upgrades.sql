-- P12e-0 (docs/trial-ledger-v1.md §3, §22): a family's hypothesis-test count
-- may only rise.
--
-- `trial_families.protocol_json` keeps the protocol pinned by the family's
-- first effective trial (0001 allows that single NULL -> value change). A
-- batch declaring MORE tests per trial appends a row here in its registration
-- transaction; a batch declaring fewer is refused. The family's effective
-- `testsPerTrial` is the largest of the pin and every row here, so the P12a
-- family size m can only grow — never reset by a protocol change. Imports
-- union these rows. Every row's `tests_per_trial` exceeds the family's pin.
CREATE TABLE family_protocol_upgrades (
    family_id       TEXT    NOT NULL REFERENCES trial_families(family_id),
    tests_per_trial INTEGER NOT NULL CHECK (tests_per_trial >= 2),
    protocol_json   TEXT    NOT NULL,   -- canonical {"correction","testsPerTrial"}
    recorded_at     TEXT    NOT NULL,
    PRIMARY KEY (family_id, tests_per_trial)
);

CREATE TRIGGER family_protocol_upgrades_no_update BEFORE UPDATE ON family_protocol_upgrades
BEGIN SELECT RAISE(ABORT, 'family_protocol_upgrades is append-only'); END;
CREATE TRIGGER family_protocol_upgrades_no_delete BEFORE DELETE ON family_protocol_upgrades
BEGIN SELECT RAISE(ABORT, 'family_protocol_upgrades is append-only'); END;
