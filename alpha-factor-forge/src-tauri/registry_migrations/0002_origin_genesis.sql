-- P12 audit R5 (docs/trial-ledger-v1.md §7.3, §8, §20): verified genesis
-- evidence for OTHER registries.
--
-- A workspace bound to an empty registry holds `seq = 0` and that registry's
-- genesis chain value. `origin_checkpoints` cannot express that prefix: its
-- rows need `origin_seq >= 1` and an event. A row here records that this
-- registry validated a complete export of the origin — directly, or carried
-- by another validated export — which proves the origin's empty prefix.
-- It never proves anything beyond seq 0; longer prefixes still need the exact
-- checkpoint. Like every registry table it is append-only.
CREATE TABLE origin_genesis (
    origin_registry_id TEXT PRIMARY KEY,
    recorded_at        TEXT NOT NULL
);

-- Backfill from evidence schema v1 already holds: the source of every
-- validated import, and every origin that has verified checkpoints.
INSERT OR IGNORE INTO origin_genesis (origin_registry_id, recorded_at)
SELECT source_registry_id, MIN(imported_at) FROM registry_imports
 WHERE source_registry_id <> (SELECT value FROM registry_meta WHERE key = 'registry_id')
 GROUP BY source_registry_id;
INSERT OR IGNORE INTO origin_genesis (origin_registry_id, recorded_at)
SELECT DISTINCT origin_registry_id, datetime('now') FROM origin_checkpoints;

CREATE TRIGGER origin_genesis_no_update BEFORE UPDATE ON origin_genesis
BEGIN SELECT RAISE(ABORT, 'origin_genesis is append-only'); END;
CREATE TRIGGER origin_genesis_no_delete BEFORE DELETE ON origin_genesis
BEGIN SELECT RAISE(ABORT, 'origin_genesis is append-only'); END;
