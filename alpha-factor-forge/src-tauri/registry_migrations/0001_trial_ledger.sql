-- P12b trial ledger registry, schema v1 (docs/trial-ledger-v1.md §10).
--
-- This database is NOT a workspace database. It lives outside every
-- workspace (spec §2.1) and has its own migration sequence, tracked in
-- `registry_migrations` by `research::trial_ledger`. Nothing here is ever
-- updated or deleted: every table below is append-only, enforced by the
-- triggers at the end. The single permitted UPDATE is the one-time pin of a
-- family's protocol (NULL -> value) when its first effective trial lands.
--
-- Table and write order follow the foreign keys:
-- families -> batches -> events -> receipts -> checkpoints (spec §8.2).

-- `registry_id`: 16 random bytes (hex), inserted with this migration and
-- never changed. It seeds the hash chain's genesis value (spec §7.1).
CREATE TABLE registry_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- One row per `trial-family-v1` family (spec §3). `protocol_json` is NULL
-- until the family's first effective event, then frozen.
CREATE TABLE trial_families (
    family_id     TEXT PRIMARY KEY,      -- trial-family-v1:<sha256(family_key)>
    family_key    TEXT NOT NULL,         -- canonical JSON
    protocol_json TEXT,                  -- canonical {"correction","testsPerTrial"}
    created_at    TEXT NOT NULL
);

-- Batch identity (spec §6.1). No counts: those are recomputed from events.
CREATE TABLE trial_batches (
    batch_id           TEXT PRIMARY KEY, -- trial-batch-v1:<sha256(familyId, sorted [key, eventId])>
    family_id          TEXT REFERENCES trial_families(family_id),  -- NULL = family_unknown
    members_json       TEXT NOT NULL,    -- canonical sorted [[idempotencyKey, eventId], ...]
    origin_registry_id TEXT NOT NULL,
    created_at         TEXT NOT NULL
);

-- One row per registration (spec §4). `payload_json` is the exact canonical
-- text `event_id` hashes; `seq`/`chain` place it on this registry's chain.
CREATE TABLE trial_events (
    event_id           TEXT PRIMARY KEY, -- trial-event-v1:<sha256(payload_json)>
    seq                INTEGER NOT NULL UNIQUE CHECK (seq >= 1),
    chain              TEXT NOT NULL,
    family_id          TEXT REFERENCES trial_families(family_id),  -- NULL = family_unknown
    idempotency_key    TEXT NOT NULL UNIQUE,
    kind               TEXT NOT NULL CHECK (kind IN
                           ('hypothesis','variant','diagnostic','benchmark','reproduction','legacy')),
    effective          INTEGER NOT NULL CHECK (effective IN (0, 1)),
    batch_id           TEXT NOT NULL REFERENCES trial_batches(batch_id),
    payload_json       TEXT NOT NULL,
    origin_registry_id TEXT NOT NULL,    -- provenance only; not part of identity (spec §4.3)
    registered_at      TEXT NOT NULL
);
CREATE INDEX idx_trial_events_family ON trial_events(family_id, effective);
CREATE INDEX idx_trial_events_batch ON trial_events(batch_id);

-- Historical receipts (spec §6.3). Audit only: never a count source and
-- never a precision-plan input, hence not named prior/planned.
CREATE TABLE batch_receipts (
    batch_id                TEXT NOT NULL REFERENCES trial_batches(batch_id),
    receipt_registry_id     TEXT NOT NULL,
    family_effective_before INTEGER NOT NULL CHECK (family_effective_before >= 0),
    batch_effective_trials  INTEGER NOT NULL CHECK (batch_effective_trials >= 0),
    PRIMARY KEY (batch_id, receipt_registry_id)
);

-- Verified chain checkpoints of OTHER registries, written only by imports
-- (spec §7.3, §8.2). P12b-1b fills this; the table exists from v1 so the
-- schema does not change between the two halves.
CREATE TABLE origin_checkpoints (
    origin_registry_id TEXT NOT NULL,
    origin_seq         INTEGER NOT NULL CHECK (origin_seq >= 1),
    event_id           TEXT NOT NULL REFERENCES trial_events(event_id),
    origin_chain       TEXT NOT NULL,
    PRIMARY KEY (origin_registry_id, origin_seq)
);

CREATE TABLE registry_imports (
    import_id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_registry_id TEXT NOT NULL,
    file_sha256        TEXT NOT NULL,
    head_seq           INTEGER NOT NULL,
    head_chain         TEXT NOT NULL,
    added_events       INTEGER NOT NULL,
    skipped_events     INTEGER NOT NULL,
    conflicts          INTEGER NOT NULL,
    imported_at        TEXT NOT NULL
);

-- Any row quarantines its family (spec §8.2 step 2); v1 has no release.
CREATE TABLE family_conflicts (
    conflict_id INTEGER PRIMARY KEY AUTOINCREMENT,
    family_id   TEXT NOT NULL,
    kind        TEXT NOT NULL,
    detail_json TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);

CREATE TABLE registry_conflicts (
    conflict_id        INTEGER PRIMARY KEY AUTOINCREMENT,
    origin_registry_id TEXT NOT NULL,
    detail_json        TEXT NOT NULL,
    recorded_at        TEXT NOT NULL
);

-- ---------------------------------------------------------- append-only

-- `registry_migrations` is created by the Rust opener before this file runs.
CREATE TRIGGER registry_migrations_no_update BEFORE UPDATE ON registry_migrations
BEGIN SELECT RAISE(ABORT, 'registry_migrations is append-only'); END;
CREATE TRIGGER registry_migrations_no_delete BEFORE DELETE ON registry_migrations
BEGIN SELECT RAISE(ABORT, 'registry_migrations is append-only'); END;

CREATE TRIGGER registry_meta_no_update BEFORE UPDATE ON registry_meta
BEGIN SELECT RAISE(ABORT, 'registry_meta is append-only'); END;
CREATE TRIGGER registry_meta_no_delete BEFORE DELETE ON registry_meta
BEGIN SELECT RAISE(ABORT, 'registry_meta is append-only'); END;

CREATE TRIGGER trial_families_pin_protocol_once BEFORE UPDATE ON trial_families
WHEN NOT (OLD.protocol_json IS NULL AND NEW.protocol_json IS NOT NULL
          AND NEW.family_id IS OLD.family_id
          AND NEW.family_key IS OLD.family_key
          AND NEW.created_at IS OLD.created_at)
BEGIN SELECT RAISE(ABORT, 'trial_families: only the one-time protocol pin may change'); END;
CREATE TRIGGER trial_families_no_delete BEFORE DELETE ON trial_families
BEGIN SELECT RAISE(ABORT, 'trial_families is append-only'); END;

CREATE TRIGGER trial_batches_no_update BEFORE UPDATE ON trial_batches
BEGIN SELECT RAISE(ABORT, 'trial_batches is append-only'); END;
CREATE TRIGGER trial_batches_no_delete BEFORE DELETE ON trial_batches
BEGIN SELECT RAISE(ABORT, 'trial_batches is append-only'); END;

CREATE TRIGGER trial_events_no_update BEFORE UPDATE ON trial_events
BEGIN SELECT RAISE(ABORT, 'trial_events is append-only'); END;
CREATE TRIGGER trial_events_no_delete BEFORE DELETE ON trial_events
BEGIN SELECT RAISE(ABORT, 'trial_events is append-only'); END;

CREATE TRIGGER batch_receipts_no_update BEFORE UPDATE ON batch_receipts
BEGIN SELECT RAISE(ABORT, 'batch_receipts is append-only'); END;
CREATE TRIGGER batch_receipts_no_delete BEFORE DELETE ON batch_receipts
BEGIN SELECT RAISE(ABORT, 'batch_receipts is append-only'); END;

CREATE TRIGGER origin_checkpoints_no_update BEFORE UPDATE ON origin_checkpoints
BEGIN SELECT RAISE(ABORT, 'origin_checkpoints is append-only'); END;
CREATE TRIGGER origin_checkpoints_no_delete BEFORE DELETE ON origin_checkpoints
BEGIN SELECT RAISE(ABORT, 'origin_checkpoints is append-only'); END;

CREATE TRIGGER registry_imports_no_update BEFORE UPDATE ON registry_imports
BEGIN SELECT RAISE(ABORT, 'registry_imports is append-only'); END;
CREATE TRIGGER registry_imports_no_delete BEFORE DELETE ON registry_imports
BEGIN SELECT RAISE(ABORT, 'registry_imports is append-only'); END;

CREATE TRIGGER family_conflicts_no_update BEFORE UPDATE ON family_conflicts
BEGIN SELECT RAISE(ABORT, 'family_conflicts is append-only'); END;
CREATE TRIGGER family_conflicts_no_delete BEFORE DELETE ON family_conflicts
BEGIN SELECT RAISE(ABORT, 'family_conflicts is append-only'); END;

CREATE TRIGGER registry_conflicts_no_update BEFORE UPDATE ON registry_conflicts
BEGIN SELECT RAISE(ABORT, 'registry_conflicts is append-only'); END;
CREATE TRIGGER registry_conflicts_no_delete BEFORE DELETE ON registry_conflicts
BEGIN SELECT RAISE(ABORT, 'registry_conflicts is append-only'); END;
