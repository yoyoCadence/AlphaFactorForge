-- P13c-3a-2a: immutable archival schema only; checked graph IO is 3a-2b.
-- Exact joint limits: docs/plans/research-confirmation-storage-transfer-plan-v1.md.
-- REPLACE cannot alter an existing row even if recursive_triggers is OFF.

CREATE TABLE confirmation_batches (
    batch_id TEXT NOT NULL CHECK (typeof(batch_id) = 'text' AND length(batch_id) = 64 AND length(CAST(batch_id AS BLOB)) = 64 AND batch_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    family_id TEXT NOT NULL REFERENCES trial_families(family_id) DEFERRABLE INITIALLY DEFERRED CHECK (typeof(family_id) = 'text' AND length(family_id) = 80 AND length(CAST(family_id AS BLOB)) = 80 AND substr(family_id,1,16) = 'trial-family-v1:' AND substr(family_id,17) NOT GLOB '*[^0-9a-f]*'),
    stage TEXT NOT NULL CHECK (stage IN ('validation','test')),
    parent_batch_id TEXT CHECK (parent_batch_id IS NULL OR (typeof(parent_batch_id) = 'text' AND length(parent_batch_id) = 64 AND length(CAST(parent_batch_id AS BLOB)) = 64 AND parent_batch_id NOT GLOB '*[^0-9a-f]*')) REFERENCES confirmation_batches(batch_id) DEFERRABLE INITIALLY DEFERRED,
    selection_id TEXT CHECK (selection_id IS NULL OR (typeof(selection_id) = 'text' AND length(selection_id) = 64 AND length(CAST(selection_id AS BLOB)) = 64 AND selection_id NOT GLOB '*[^0-9a-f]*')) REFERENCES confirmation_selections(selection_id) DEFERRABLE INITIALLY DEFERRED,
    document_json TEXT NOT NULL CHECK (typeof(document_json) = 'text' AND length(CAST(document_json AS BLOB)) BETWEEN 1 AND 4194304),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text'),
    CHECK ((stage = 'validation' AND parent_batch_id IS NULL AND selection_id IS NULL) OR (stage = 'test' AND parent_batch_id IS NOT NULL AND selection_id IS NOT NULL))
);

CREATE TABLE confirmation_selections (
    selection_id TEXT NOT NULL CHECK (typeof(selection_id) = 'text' AND length(selection_id) = 64 AND length(CAST(selection_id AS BLOB)) = 64 AND selection_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    validation_batch_id TEXT NOT NULL CHECK (typeof(validation_batch_id) = 'text' AND length(validation_batch_id) = 64 AND length(CAST(validation_batch_id AS BLOB)) = 64 AND validation_batch_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_batches(batch_id) DEFERRABLE INITIALLY DEFERRED,
    sealed_receipt_id TEXT NOT NULL CHECK (typeof(sealed_receipt_id) = 'text' AND length(sealed_receipt_id) = 64 AND length(CAST(sealed_receipt_id AS BLOB)) = 64 AND sealed_receipt_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_events(event_id) DEFERRABLE INITIALLY DEFERRED,
    alpha_reservation_id TEXT NOT NULL CHECK (typeof(alpha_reservation_id) = 'text' AND length(alpha_reservation_id) = 64 AND length(CAST(alpha_reservation_id AS BLOB)) = 64 AND alpha_reservation_id NOT GLOB '*[^0-9a-f]*') REFERENCES alpha_reservations(reservation_id) DEFERRABLE INITIALLY DEFERRED,
    report_sha256 TEXT NOT NULL CHECK (typeof(report_sha256) = 'text' AND length(report_sha256) = 64 AND length(CAST(report_sha256 AS BLOB)) = 64 AND report_sha256 NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_blobs(blob_sha256) DEFERRABLE INITIALLY DEFERRED,
    document_json TEXT NOT NULL CHECK (typeof(document_json) = 'text' AND length(CAST(document_json AS BLOB)) BETWEEN 1 AND 4194304),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text')
);

CREATE TABLE confirmation_operations (
    operation_id TEXT NOT NULL CHECK (typeof(operation_id) = 'text' AND length(operation_id) = 64 AND length(CAST(operation_id AS BLOB)) = 64 AND operation_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    origin_registry_id TEXT NOT NULL CHECK (typeof(origin_registry_id) = 'text' AND length(origin_registry_id) = 32 AND length(CAST(origin_registry_id AS BLOB)) = 32 AND origin_registry_id NOT GLOB '*[^0-9a-f]*'),
    reservation_key TEXT NOT NULL CHECK (typeof(reservation_key) = 'text' AND length(reservation_key) = 32 AND length(CAST(reservation_key AS BLOB)) = 32 AND reservation_key NOT GLOB '*[^0-9a-f]*'),
    workspace_id TEXT NOT NULL CHECK (typeof(workspace_id) = 'text' AND length(workspace_id) = 32 AND length(CAST(workspace_id AS BLOB)) = 32 AND workspace_id NOT GLOB '*[^0-9a-f]*'),
    batch_id TEXT NOT NULL CHECK (typeof(batch_id) = 'text' AND length(batch_id) = 64 AND length(CAST(batch_id AS BLOB)) = 64 AND batch_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_batches(batch_id) DEFERRABLE INITIALLY DEFERRED,
    family_id TEXT NOT NULL REFERENCES trial_families(family_id) DEFERRABLE INITIALLY DEFERRED CHECK (typeof(family_id) = 'text' AND length(family_id) = 80 AND length(CAST(family_id AS BLOB)) = 80 AND substr(family_id,1,16) = 'trial-family-v1:' AND substr(family_id,17) NOT GLOB '*[^0-9a-f]*'),
    document_json TEXT NOT NULL CHECK (typeof(document_json) = 'text' AND length(CAST(document_json AS BLOB)) BETWEEN 1 AND 4194304),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text')
);

CREATE TABLE confirmation_exposures (
    exposure_id TEXT NOT NULL CHECK (typeof(exposure_id) = 'text' AND length(exposure_id) = 64 AND length(CAST(exposure_id AS BLOB)) = 64 AND exposure_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    instrument_id TEXT NOT NULL CHECK (typeof(instrument_id) = 'text' AND length(CAST(instrument_id AS BLOB)) BETWEEN 1 AND 256),
    kind TEXT NOT NULL CHECK (kind IN ('confirmation','manual','legacy_unknown')),
    operation_id TEXT CHECK (operation_id IS NULL OR (typeof(operation_id) = 'text' AND length(operation_id) = 64 AND length(CAST(operation_id AS BLOB)) = 64 AND operation_id NOT GLOB '*[^0-9a-f]*')) REFERENCES confirmation_operations(operation_id) DEFERRABLE INITIALLY DEFERRED,
    alpha_reservation_id TEXT CHECK (alpha_reservation_id IS NULL OR (typeof(alpha_reservation_id) = 'text' AND length(alpha_reservation_id) = 64 AND length(CAST(alpha_reservation_id AS BLOB)) = 64 AND alpha_reservation_id NOT GLOB '*[^0-9a-f]*')) REFERENCES alpha_reservations(reservation_id) DEFERRABLE INITIALLY DEFERRED,
    document_json TEXT NOT NULL CHECK (typeof(document_json) = 'text' AND length(CAST(document_json AS BLOB)) BETWEEN 1 AND 4194304),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text'),
    CHECK ((kind = 'confirmation' AND operation_id IS NOT NULL AND alpha_reservation_id IS NOT NULL) OR (kind <> 'confirmation' AND operation_id IS NULL AND alpha_reservation_id IS NULL))
);

CREATE TABLE confirmation_intervals (
    exposure_id TEXT NOT NULL CHECK (typeof(exposure_id) = 'text' AND length(exposure_id) = 64 AND length(CAST(exposure_id AS BLOB)) = 64 AND exposure_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_exposures(exposure_id) DEFERRABLE INITIALLY DEFERRED,
    interval_index INTEGER NOT NULL CHECK (typeof(interval_index) = 'integer' AND interval_index BETWEEN 0 AND 127),
    instrument_id TEXT NOT NULL CHECK (typeof(instrument_id) = 'text' AND length(CAST(instrument_id AS BLOB)) BETWEEN 1 AND 256),
    from_ms INTEGER NOT NULL CHECK (typeof(from_ms) = 'integer' AND from_ms >= 946684800000 AND from_ms < 4102444800000),
    to_exclusive_ms INTEGER NOT NULL CHECK (typeof(to_exclusive_ms) = 'integer' AND to_exclusive_ms > from_ms AND to_exclusive_ms <= 4102444800000),
    PRIMARY KEY (exposure_id,interval_index)
);

CREATE TABLE confirmation_admissions (
    admission_id TEXT NOT NULL CHECK (typeof(admission_id) = 'text' AND length(admission_id) = 64 AND length(CAST(admission_id AS BLOB)) = 64 AND admission_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    operation_id TEXT NOT NULL CHECK (typeof(operation_id) = 'text' AND length(operation_id) = 64 AND length(CAST(operation_id AS BLOB)) = 64 AND operation_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_operations(operation_id) DEFERRABLE INITIALLY DEFERRED,
    alpha_reservation_id TEXT NOT NULL CHECK (typeof(alpha_reservation_id) = 'text' AND length(alpha_reservation_id) = 64 AND length(CAST(alpha_reservation_id AS BLOB)) = 64 AND alpha_reservation_id NOT GLOB '*[^0-9a-f]*') REFERENCES alpha_reservations(reservation_id) DEFERRABLE INITIALLY DEFERRED,
    exposure_id TEXT NOT NULL CHECK (typeof(exposure_id) = 'text' AND length(exposure_id) = 64 AND length(CAST(exposure_id AS BLOB)) = 64 AND exposure_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_exposures(exposure_id) DEFERRABLE INITIALLY DEFERRED,
    binding_sha256 TEXT NOT NULL CHECK (typeof(binding_sha256) = 'text' AND length(binding_sha256) = 64 AND length(CAST(binding_sha256 AS BLOB)) = 64 AND binding_sha256 NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_blobs(blob_sha256) DEFERRABLE INITIALLY DEFERRED,
    document_json TEXT NOT NULL CHECK (typeof(document_json) = 'text' AND length(CAST(document_json AS BLOB)) BETWEEN 1 AND 4194304),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text')
);

CREATE TABLE confirmation_events (
    event_id TEXT NOT NULL CHECK (typeof(event_id) = 'text' AND length(event_id) = 64 AND length(CAST(event_id AS BLOB)) = 64 AND event_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    operation_id TEXT NOT NULL CHECK (typeof(operation_id) = 'text' AND length(operation_id) = 64 AND length(CAST(operation_id AS BLOB)) = 64 AND operation_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_operations(operation_id) DEFERRABLE INITIALLY DEFERRED,
    admission_id TEXT NOT NULL CHECK (typeof(admission_id) = 'text' AND length(admission_id) = 64 AND length(CAST(admission_id AS BLOB)) = 64 AND admission_id NOT GLOB '*[^0-9a-f]*') REFERENCES confirmation_admissions(admission_id) DEFERRABLE INITIALLY DEFERRED,
    kind TEXT NOT NULL CHECK (kind IN ('execution_started','sealed','interrupted_spent','failed_spent','cancelled_spent')),
    document_json TEXT NOT NULL CHECK (typeof(document_json) = 'text' AND length(CAST(document_json AS BLOB)) BETWEEN 1 AND 4194304),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text')
);

CREATE TABLE confirmation_blobs (
    blob_sha256 TEXT NOT NULL CHECK (typeof(blob_sha256) = 'text' AND length(blob_sha256) = 64 AND length(CAST(blob_sha256 AS BLOB)) = 64 AND blob_sha256 NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('statistics','binding')),
    byte_length INTEGER NOT NULL CHECK (typeof(byte_length) = 'integer' AND byte_length BETWEEN 1 AND 8388608),
    content_bytes BLOB NOT NULL CHECK (typeof(content_bytes) = 'blob' AND length(content_bytes) = byte_length AND (kind = 'binding' OR byte_length <= 4194304)),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text')
);

CREATE TABLE confirmation_conflicts (
    conflict_id TEXT NOT NULL CHECK (typeof(conflict_id) = 'text' AND length(conflict_id) = 64 AND length(CAST(conflict_id AS BLOB)) = 64 AND conflict_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    document_json TEXT NOT NULL CHECK (typeof(document_json) = 'text' AND length(CAST(document_json AS BLOB)) BETWEEN 1 AND 4194304),
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text')
);

CREATE TABLE confirmation_unknown_origins (
    origin_registry_id TEXT NOT NULL CHECK (typeof(origin_registry_id) = 'text' AND length(origin_registry_id) = 32 AND length(CAST(origin_registry_id AS BLOB)) = 32 AND origin_registry_id NOT GLOB '*[^0-9a-f]*') PRIMARY KEY,
    recorded_at TEXT NOT NULL CHECK (typeof(recorded_at) = 'text')
);

CREATE TRIGGER confirmation_batches_no_update BEFORE UPDATE ON confirmation_batches
BEGIN SELECT RAISE(ABORT, 'confirmation_batches is append-only'); END;
CREATE TRIGGER confirmation_batches_no_delete BEFORE DELETE ON confirmation_batches
BEGIN SELECT RAISE(ABORT, 'confirmation_batches is append-only'); END;
CREATE TRIGGER confirmation_batches_no_replacement BEFORE INSERT ON confirmation_batches
WHEN EXISTS (SELECT 1 FROM confirmation_batches WHERE batch_id = NEW.batch_id AND NOT (batch_id IS NEW.batch_id AND family_id IS NEW.family_id AND stage IS NEW.stage AND parent_batch_id IS NEW.parent_batch_id AND selection_id IS NEW.selection_id AND document_json IS NEW.document_json AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_batches is append-only'); END;
CREATE TRIGGER confirmation_batches_capacity BEFORE INSERT ON confirmation_batches
WHEN NOT EXISTS (SELECT 1 FROM confirmation_batches WHERE batch_id = NEW.batch_id) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(CAST(NEW.document_json AS BLOB)) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_selections_no_update BEFORE UPDATE ON confirmation_selections
BEGIN SELECT RAISE(ABORT, 'confirmation_selections is append-only'); END;
CREATE TRIGGER confirmation_selections_no_delete BEFORE DELETE ON confirmation_selections
BEGIN SELECT RAISE(ABORT, 'confirmation_selections is append-only'); END;
CREATE TRIGGER confirmation_selections_no_replacement BEFORE INSERT ON confirmation_selections
WHEN EXISTS (SELECT 1 FROM confirmation_selections WHERE selection_id = NEW.selection_id AND NOT (selection_id IS NEW.selection_id AND validation_batch_id IS NEW.validation_batch_id AND sealed_receipt_id IS NEW.sealed_receipt_id AND alpha_reservation_id IS NEW.alpha_reservation_id AND report_sha256 IS NEW.report_sha256 AND document_json IS NEW.document_json AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_selections is append-only'); END;
CREATE TRIGGER confirmation_selections_capacity BEFORE INSERT ON confirmation_selections
WHEN NOT EXISTS (SELECT 1 FROM confirmation_selections WHERE selection_id = NEW.selection_id) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(CAST(NEW.document_json AS BLOB)) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_operations_no_update BEFORE UPDATE ON confirmation_operations
BEGIN SELECT RAISE(ABORT, 'confirmation_operations is append-only'); END;
CREATE TRIGGER confirmation_operations_no_delete BEFORE DELETE ON confirmation_operations
BEGIN SELECT RAISE(ABORT, 'confirmation_operations is append-only'); END;
CREATE TRIGGER confirmation_operations_no_replacement BEFORE INSERT ON confirmation_operations
WHEN EXISTS (SELECT 1 FROM confirmation_operations WHERE operation_id = NEW.operation_id AND NOT (operation_id IS NEW.operation_id AND origin_registry_id IS NEW.origin_registry_id AND reservation_key IS NEW.reservation_key AND workspace_id IS NEW.workspace_id AND batch_id IS NEW.batch_id AND family_id IS NEW.family_id AND document_json IS NEW.document_json AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_operations is append-only'); END;
CREATE TRIGGER confirmation_operations_capacity BEFORE INSERT ON confirmation_operations
WHEN NOT EXISTS (SELECT 1 FROM confirmation_operations WHERE operation_id = NEW.operation_id) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(CAST(NEW.document_json AS BLOB)) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_exposures_no_update BEFORE UPDATE ON confirmation_exposures
BEGIN SELECT RAISE(ABORT, 'confirmation_exposures is append-only'); END;
CREATE TRIGGER confirmation_exposures_no_delete BEFORE DELETE ON confirmation_exposures
BEGIN SELECT RAISE(ABORT, 'confirmation_exposures is append-only'); END;
CREATE TRIGGER confirmation_exposures_no_replacement BEFORE INSERT ON confirmation_exposures
WHEN EXISTS (SELECT 1 FROM confirmation_exposures WHERE exposure_id = NEW.exposure_id AND NOT (exposure_id IS NEW.exposure_id AND instrument_id IS NEW.instrument_id AND kind IS NEW.kind AND operation_id IS NEW.operation_id AND alpha_reservation_id IS NEW.alpha_reservation_id AND document_json IS NEW.document_json AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_exposures is append-only'); END;
CREATE TRIGGER confirmation_exposures_capacity BEFORE INSERT ON confirmation_exposures
WHEN NOT EXISTS (SELECT 1 FROM confirmation_exposures WHERE exposure_id = NEW.exposure_id) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(CAST(NEW.document_json AS BLOB)) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_intervals_no_update BEFORE UPDATE ON confirmation_intervals
BEGIN SELECT RAISE(ABORT, 'confirmation_intervals is append-only'); END;
CREATE TRIGGER confirmation_intervals_no_delete BEFORE DELETE ON confirmation_intervals
BEGIN SELECT RAISE(ABORT, 'confirmation_intervals is append-only'); END;
CREATE TRIGGER confirmation_intervals_no_replacement BEFORE INSERT ON confirmation_intervals
WHEN EXISTS (SELECT 1 FROM confirmation_intervals WHERE exposure_id = NEW.exposure_id AND interval_index = NEW.interval_index AND NOT (exposure_id IS NEW.exposure_id AND interval_index IS NEW.interval_index AND instrument_id IS NEW.instrument_id AND from_ms IS NEW.from_ms AND to_exclusive_ms IS NEW.to_exclusive_ms))
BEGIN SELECT RAISE(ABORT, 'confirmation_intervals is append-only'); END;
CREATE TRIGGER confirmation_intervals_capacity BEFORE INSERT ON confirmation_intervals
WHEN NOT EXISTS (SELECT 1 FROM confirmation_intervals WHERE exposure_id = NEW.exposure_id AND interval_index = NEW.interval_index) AND ((SELECT COUNT(*) FROM confirmation_intervals) >= 128000)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_admissions_no_update BEFORE UPDATE ON confirmation_admissions
BEGIN SELECT RAISE(ABORT, 'confirmation_admissions is append-only'); END;
CREATE TRIGGER confirmation_admissions_no_delete BEFORE DELETE ON confirmation_admissions
BEGIN SELECT RAISE(ABORT, 'confirmation_admissions is append-only'); END;
CREATE TRIGGER confirmation_admissions_no_replacement BEFORE INSERT ON confirmation_admissions
WHEN EXISTS (SELECT 1 FROM confirmation_admissions WHERE admission_id = NEW.admission_id AND NOT (admission_id IS NEW.admission_id AND operation_id IS NEW.operation_id AND alpha_reservation_id IS NEW.alpha_reservation_id AND exposure_id IS NEW.exposure_id AND binding_sha256 IS NEW.binding_sha256 AND document_json IS NEW.document_json AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_admissions is append-only'); END;
CREATE TRIGGER confirmation_admissions_capacity BEFORE INSERT ON confirmation_admissions
WHEN NOT EXISTS (SELECT 1 FROM confirmation_admissions WHERE admission_id = NEW.admission_id) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(CAST(NEW.document_json AS BLOB)) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_events_no_update BEFORE UPDATE ON confirmation_events
BEGIN SELECT RAISE(ABORT, 'confirmation_events is append-only'); END;
CREATE TRIGGER confirmation_events_no_delete BEFORE DELETE ON confirmation_events
BEGIN SELECT RAISE(ABORT, 'confirmation_events is append-only'); END;
CREATE TRIGGER confirmation_events_no_replacement BEFORE INSERT ON confirmation_events
WHEN EXISTS (SELECT 1 FROM confirmation_events WHERE event_id = NEW.event_id AND NOT (event_id IS NEW.event_id AND operation_id IS NEW.operation_id AND admission_id IS NEW.admission_id AND kind IS NEW.kind AND document_json IS NEW.document_json AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_events is append-only'); END;
CREATE TRIGGER confirmation_events_capacity BEFORE INSERT ON confirmation_events
WHEN NOT EXISTS (SELECT 1 FROM confirmation_events WHERE event_id = NEW.event_id) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(CAST(NEW.document_json AS BLOB)) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_blobs_no_update BEFORE UPDATE ON confirmation_blobs
BEGIN SELECT RAISE(ABORT, 'confirmation_blobs is append-only'); END;
CREATE TRIGGER confirmation_blobs_no_delete BEFORE DELETE ON confirmation_blobs
BEGIN SELECT RAISE(ABORT, 'confirmation_blobs is append-only'); END;
CREATE TRIGGER confirmation_blobs_no_replacement BEFORE INSERT ON confirmation_blobs
WHEN EXISTS (SELECT 1 FROM confirmation_blobs WHERE blob_sha256 = NEW.blob_sha256 AND NOT (blob_sha256 IS NEW.blob_sha256 AND kind IS NEW.kind AND byte_length IS NEW.byte_length AND content_bytes IS NEW.content_bytes AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_blobs is append-only'); END;
CREATE TRIGGER confirmation_blobs_capacity BEFORE INSERT ON confirmation_blobs
WHEN NOT EXISTS (SELECT 1 FROM confirmation_blobs WHERE blob_sha256 = NEW.blob_sha256) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(NEW.content_bytes) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_conflicts_no_update BEFORE UPDATE ON confirmation_conflicts
BEGIN SELECT RAISE(ABORT, 'confirmation_conflicts is append-only'); END;
CREATE TRIGGER confirmation_conflicts_no_delete BEFORE DELETE ON confirmation_conflicts
BEGIN SELECT RAISE(ABORT, 'confirmation_conflicts is append-only'); END;
CREATE TRIGGER confirmation_conflicts_no_replacement BEFORE INSERT ON confirmation_conflicts
WHEN EXISTS (SELECT 1 FROM confirmation_conflicts WHERE conflict_id = NEW.conflict_id AND NOT (conflict_id IS NEW.conflict_id AND document_json IS NEW.document_json AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_conflicts is append-only'); END;
CREATE TRIGGER confirmation_conflicts_capacity BEFORE INSERT ON confirmation_conflicts
WHEN NOT EXISTS (SELECT 1 FROM confirmation_conflicts WHERE conflict_id = NEW.conflict_id) AND (((SELECT COUNT(*) FROM confirmation_batches) + (SELECT COUNT(*) FROM confirmation_selections) + (SELECT COUNT(*) FROM confirmation_operations) + (SELECT COUNT(*) FROM confirmation_exposures) + (SELECT COUNT(*) FROM confirmation_admissions) + (SELECT COUNT(*) FROM confirmation_events) + (SELECT COUNT(*) FROM confirmation_blobs) + (SELECT COUNT(*) FROM confirmation_conflicts)) >= 10000 OR ((SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_batches) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_selections) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_operations) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_exposures) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_admissions) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_events) + (SELECT COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs) + (SELECT COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM confirmation_conflicts)) + length(CAST(NEW.document_json AS BLOB)) > 67108864)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE TRIGGER confirmation_unknown_origins_no_update BEFORE UPDATE ON confirmation_unknown_origins
BEGIN SELECT RAISE(ABORT, 'confirmation_unknown_origins is append-only'); END;
CREATE TRIGGER confirmation_unknown_origins_no_delete BEFORE DELETE ON confirmation_unknown_origins
BEGIN SELECT RAISE(ABORT, 'confirmation_unknown_origins is append-only'); END;
CREATE TRIGGER confirmation_unknown_origins_no_replacement BEFORE INSERT ON confirmation_unknown_origins
WHEN EXISTS (SELECT 1 FROM confirmation_unknown_origins WHERE origin_registry_id = NEW.origin_registry_id AND NOT (origin_registry_id IS NEW.origin_registry_id AND recorded_at IS NEW.recorded_at))
BEGIN SELECT RAISE(ABORT, 'confirmation_unknown_origins is append-only'); END;
CREATE TRIGGER confirmation_unknown_origins_capacity BEFORE INSERT ON confirmation_unknown_origins
WHEN NOT EXISTS (SELECT 1 FROM confirmation_unknown_origins WHERE origin_registry_id = NEW.origin_registry_id) AND ((SELECT COUNT(*) FROM confirmation_unknown_origins) >= 10000)
BEGIN SELECT RAISE(ABORT, 'confirmation history capacity exceeded'); END;

CREATE INDEX idx_confirmation_operations_origin_key ON confirmation_operations(origin_registry_id,reservation_key);
CREATE INDEX idx_confirmation_operations_family ON confirmation_operations(family_id);
CREATE INDEX idx_confirmation_exposures_operation ON confirmation_exposures(operation_id);
CREATE INDEX idx_confirmation_intervals_support ON confirmation_intervals(instrument_id,from_ms,to_exclusive_ms);
CREATE INDEX idx_confirmation_admissions_operation ON confirmation_admissions(operation_id);
CREATE INDEX idx_confirmation_events_operation_kind ON confirmation_events(operation_id,kind);
CREATE INDEX idx_confirmation_batches_family ON confirmation_batches(family_id);

-- Protect the new birth value from REPLACE's implicit-delete path too.
CREATE TRIGGER confirmation_birth_no_replacement BEFORE INSERT ON registry_meta
WHEN NEW.key = 'confirmation_history_birth' AND EXISTS (
    SELECT 1 FROM registry_meta WHERE key = NEW.key AND value IS NOT NEW.value
)
BEGIN SELECT RAISE(ABORT, 'confirmation birth is immutable'); END;

-- Only genuinely fresh registries received the immutable birth key in 0001.
INSERT INTO confirmation_unknown_origins (origin_registry_id,recorded_at)
SELECT origin,strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM (
    SELECT value AS origin FROM registry_meta WHERE key = 'registry_id'
      AND NOT EXISTS (SELECT 1 FROM registry_meta WHERE key = 'confirmation_history_birth' AND value = '0006_confirmation_history')
    UNION SELECT source_registry_id FROM registry_imports
    UNION SELECT origin_registry_id FROM origin_genesis
    UNION SELECT origin_registry_id FROM origin_checkpoints
);
