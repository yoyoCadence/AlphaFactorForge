-- 0008_market_foundation — P06 market data foundation (plan §3.3 "Market",
-- "Provenance" and "Snapshot" groups; docs/market-contract.md,
-- docs/market-foundation-v1.md).
--
-- Six append-only tables beside the existing market data. None of them
-- replaces anything: `datasets`/`candles` (0001) keep their columns and their
-- meaning, and a dataset that has no snapshot row stays LEGACY — importable,
-- backtestable, and unable to acquire new qualification on its own, exactly
-- as the contract requires. Nothing here rewrites `dataset-content-v2`.
--
-- * `market_calendars`: one row per calendar VERSION. The id carries its
--   version (`crypto-24x7-v1`), so a corrected calendar is a new id and no
--   past result silently changes meaning.
-- * `market_instruments`: one row per instrument REVISION, content-hashed.
--   Re-registering the same content is the same row; a change adds revision
--   n+1 and leaves n intact for whatever already referenced it.
-- * `market_provenance`: one immutable row per retrieval, including the ones
--   that were REJECTED (kept so the rejection can be replayed). A revision
--   points at what it revises; it never overwrites it, so the chain is the
--   history. Raw bytes live in the content-addressed artifact store and are
--   referenced by checksum, never inlined here.
-- * `market_snapshots` (+ `market_snapshot_sources`): what research and paper
--   are allowed to read — a dataset bound to an instrument revision, a
--   calendar version, a price basis, and the provenance it came from. A
--   snapshot only exists when its coverage audit found nothing blocking.
-- * `market_quality_events`: the structured evidence (gaps, duplicates,
--   unit changes, source conflicts, unverified corporate actions ...). It is
--   append-only and is what a blocked range is reported from.
--
-- Triggers make immutability mechanical rather than a matter of discipline.

CREATE TABLE market_calendars (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    calendar_id      TEXT    NOT NULL UNIQUE,
    version          TEXT    NOT NULL,
    kind             TEXT    NOT NULL CHECK (kind IN ('continuous','trading-days')),
    timezone         TEXT    NOT NULL,
    content_hash     TEXT    NOT NULL UNIQUE,
    definition_json  TEXT    NOT NULL,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TRIGGER market_calendars_are_immutable
BEFORE UPDATE ON market_calendars
BEGIN
    SELECT RAISE(ABORT, 'a calendar version is immutable; register a new calendar id');
END;

CREATE TRIGGER market_calendars_are_kept
BEFORE DELETE ON market_calendars
BEGIN
    SELECT RAISE(ABORT, 'calendars are never deleted');
END;

CREATE TABLE market_instruments (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    instrument_id         TEXT    NOT NULL,
    revision              INTEGER NOT NULL CHECK (revision >= 1),
    content_hash          TEXT    NOT NULL UNIQUE,
    version               TEXT    NOT NULL,
    market                TEXT    NOT NULL CHECK (market IN ('crypto','us-etf','tw-etf')),
    venue                 TEXT    NOT NULL,
    symbol                TEXT    NOT NULL,
    base                  TEXT    NOT NULL,
    quote                 TEXT    NOT NULL,
    asset_type            TEXT    NOT NULL CHECK (asset_type IN ('spot-crypto','etf')),
    session_calendar_id   TEXT    NOT NULL REFERENCES market_calendars(calendar_id),
    timezone              TEXT    NOT NULL,
    -- NULL means "not reported by any source yet". An instrument without a
    -- trading specification must not reach paper trading (P19/P20).
    lot_size              REAL,
    price_step            REAL,
    min_notional          REAL,
    listed_from           INTEGER,
    delisted_at           INTEGER,
    suspensions_json      TEXT    NOT NULL,
    source_capabilities_json TEXT NOT NULL,
    content_json          TEXT    NOT NULL,
    created_at            TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instrument_id, revision)
);

CREATE INDEX idx_market_instruments_id ON market_instruments(instrument_id, revision);

CREATE TRIGGER market_instruments_are_immutable
BEFORE UPDATE ON market_instruments
BEGIN
    SELECT RAISE(ABORT, 'an instrument revision is immutable; register a new revision');
END;

CREATE TRIGGER market_instruments_are_kept
BEFORE DELETE ON market_instruments
BEGIN
    SELECT RAISE(ABORT, 'instrument revisions are never deleted');
END;

CREATE TABLE market_provenance (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    record_hash         TEXT    NOT NULL UNIQUE,
    version             TEXT    NOT NULL,
    instrument_id       TEXT    NOT NULL,
    interval            TEXT    NOT NULL,
    source              TEXT    NOT NULL,
    -- A `comparison` source is independent evidence only: it can never be a
    -- component of a snapshot, and it never backfills the primary source.
    role                TEXT    NOT NULL CHECK (role IN ('primary','comparison')),
    request_scope_json  TEXT    NOT NULL,
    raw_sha256          TEXT    NOT NULL,
    raw_byte_len        INTEGER NOT NULL CHECK (raw_byte_len >= 0),
    raw_artifact_path   TEXT    NOT NULL,
    raw_media_type      TEXT    NOT NULL,
    retrieved_at        TEXT    NOT NULL,
    -- When the data could FIRST have been observed. NULL means unknown, and
    -- unknown availability can never support a forward-observed snapshot.
    available_at        TEXT,
    accepted            INTEGER NOT NULL CHECK (accepted IN (0,1)),
    rejection_reason    TEXT,
    revision_of         INTEGER REFERENCES market_provenance(id),
    created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
    CHECK ((accepted = 1 AND rejection_reason IS NULL)
        OR (accepted = 0 AND rejection_reason IS NOT NULL))
);

CREATE INDEX idx_market_provenance_series ON market_provenance(instrument_id, interval, source);
CREATE INDEX idx_market_provenance_revision ON market_provenance(revision_of);

CREATE TRIGGER market_provenance_is_immutable
BEFORE UPDATE ON market_provenance
BEGIN
    SELECT RAISE(ABORT, 'a provenance record is immutable; record a revision instead');
END;

CREATE TRIGGER market_provenance_is_kept
BEFORE DELETE ON market_provenance
BEGIN
    SELECT RAISE(ABORT, 'provenance records are never deleted');
END;

CREATE TABLE market_snapshots (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id               TEXT    NOT NULL UNIQUE,
    version                   TEXT    NOT NULL,
    instrument_row_id         INTEGER NOT NULL REFERENCES market_instruments(id),
    instrument_id             TEXT    NOT NULL,
    interval                  TEXT    NOT NULL,
    dataset_id                INTEGER NOT NULL REFERENCES datasets(id),
    dataset_hash              TEXT    NOT NULL,
    price_basis               TEXT    NOT NULL
                              CHECK (price_basis IN ('raw','adjusted-split','adjusted-total-return')),
    calendar_id               TEXT    NOT NULL REFERENCES market_calendars(calendar_id),
    -- NULL = not verified. The snapshot is then DEGRADED and cannot support
    -- qualification, which is the contract's fail-closed default.
    corporate_action_version  TEXT,
    cost_profile_version      TEXT,
    kind                      TEXT    NOT NULL CHECK (kind IN ('demo','historical','forward-observed')),
    status                    TEXT    NOT NULL CHECK (status IN ('ok','degraded')),
    as_of                     INTEGER NOT NULL,
    coverage_json             TEXT    NOT NULL,
    content_json              TEXT    NOT NULL,
    created_at                TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_market_snapshots_dataset ON market_snapshots(dataset_id);
CREATE INDEX idx_market_snapshots_instrument ON market_snapshots(instrument_id, interval);

CREATE TRIGGER market_snapshots_are_immutable
BEFORE UPDATE ON market_snapshots
BEGIN
    SELECT RAISE(ABORT, 'a snapshot is immutable; build a new one');
END;

CREATE TRIGGER market_snapshots_are_kept
BEFORE DELETE ON market_snapshots
BEGIN
    SELECT RAISE(ABORT, 'snapshots are never deleted');
END;

CREATE TABLE market_snapshot_sources (
    snapshot_row_id  INTEGER NOT NULL REFERENCES market_snapshots(id),
    provenance_id    INTEGER NOT NULL REFERENCES market_provenance(id),
    PRIMARY KEY (snapshot_row_id, provenance_id)
);

CREATE TRIGGER market_snapshot_sources_are_immutable
BEFORE UPDATE ON market_snapshot_sources
BEGIN
    SELECT RAISE(ABORT, 'a snapshot''s sources are immutable');
END;

CREATE TRIGGER market_snapshot_sources_are_kept
BEFORE DELETE ON market_snapshot_sources
BEGIN
    SELECT RAISE(ABORT, 'snapshot sources are never deleted');
END;

CREATE TABLE market_quality_events (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    version        TEXT    NOT NULL,
    instrument_id  TEXT    NOT NULL,
    interval       TEXT    NOT NULL,
    code           TEXT    NOT NULL,
    severity       TEXT    NOT NULL CHECK (severity IN ('blocking','degraded','info')),
    action         TEXT    NOT NULL,
    range_start    INTEGER,
    range_end      INTEGER,
    bar_count      INTEGER,
    dataset_id     INTEGER REFERENCES datasets(id),
    provenance_id  INTEGER REFERENCES market_provenance(id),
    snapshot_row_id INTEGER REFERENCES market_snapshots(id),
    detail_json    TEXT    NOT NULL,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_market_quality_events_series ON market_quality_events(instrument_id, interval, id);
CREATE INDEX idx_market_quality_events_dataset ON market_quality_events(dataset_id, id);

CREATE TRIGGER market_quality_events_are_immutable
BEFORE UPDATE ON market_quality_events
BEGIN
    SELECT RAISE(ABORT, 'quality events are immutable');
END;

CREATE TRIGGER market_quality_events_are_kept
BEFORE DELETE ON market_quality_events
BEGIN
    SELECT RAISE(ABORT, 'quality events are never deleted');
END;
