-- Trial-only imports cannot establish complete alpha history. Preserve this
-- uncertainty through new archives, reopen and rollback-aware bindings.
CREATE TABLE alpha_unknown_origins (
    origin_registry_id TEXT PRIMARY KEY CHECK (
        length(origin_registry_id) = 32 AND origin_registry_id NOT GLOB '*[^0-9a-f]*'
    ),
    recorded_at TEXT NOT NULL
);
CREATE TRIGGER alpha_unknown_origins_no_update BEFORE UPDATE ON alpha_unknown_origins
BEGIN SELECT RAISE(ABORT, 'alpha_unknown_origins is append-only'); END;
CREATE TRIGGER alpha_unknown_origins_no_delete BEFORE DELETE ON alpha_unknown_origins
BEGIN SELECT RAISE(ABORT, 'alpha_unknown_origins is append-only'); END;
CREATE TRIGGER alpha_unknown_origins_capacity BEFORE INSERT ON alpha_unknown_origins
WHEN NOT EXISTS (SELECT 1 FROM alpha_unknown_origins WHERE origin_registry_id = NEW.origin_registry_id)
 AND (SELECT COUNT(*) FROM alpha_unknown_origins) >= 10000
BEGIN SELECT RAISE(ABORT, 'alpha unknown-origin limit exceeded'); END;

INSERT INTO alpha_unknown_origins (origin_registry_id, recorded_at)
SELECT origin, strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM (
    SELECT source_registry_id AS origin FROM registry_imports
    UNION SELECT origin_registry_id FROM origin_genesis
    UNION SELECT origin_registry_id FROM origin_checkpoints
);
