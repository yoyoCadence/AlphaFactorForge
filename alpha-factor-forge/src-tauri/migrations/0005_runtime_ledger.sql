-- 0005_runtime_ledger — P03b (docs/research-runtime-contract.md §2 / §3,
-- `research-command-v1` / `research-event-v1`).
--
-- Two ledgers and one identity:
--
-- * `command_requests` makes mutating commands idempotent by `request_id`
--   (§2). A request is RESERVED (status `pending`) in its own transaction
--   before the command executes and COMPLETED afterwards, so a retry can
--   never start a second piece of work: it either replays the stored
--   outcome or, while the first attempt is still pending (or died before
--   completing), is told so. `payload_hash` lets a reused id with a different
--   command/payload be rejected as `DuplicateRequest`. Rows are kept for at
--   least 24 hours (§2); the runtime purges older ones.
--
-- * `runtime_events` is the persistent event ledger (§3). `event_id` is an
--   AUTOINCREMENT rowid, so it is monotonic and never reused across
--   restarts (SQLite's `sqlite_sequence` remembers the high-water mark). Each
--   row records the ownership `epoch` it was written under, the entity it is
--   about, the inner payload's own contract version, the channel the host
--   publishes it on, and the payload. Events are appended AFTER the write
--   they describe committed (commit-then-emit is unchanged); a reader that
--   reconnects takes a snapshot first and then reads `event_id > cursor`.
--
-- * `workspace_id` in `app_settings`: the stable identity every command
--   envelope must name (§2). Random on first migration, never derived from
--   the path, so moving the directory does not change it.

CREATE TABLE command_requests (
    request_id    TEXT    PRIMARY KEY,
    workspace_id  TEXT    NOT NULL,
    command       TEXT    NOT NULL,
    payload_hash  TEXT    NOT NULL,
    epoch         INTEGER NOT NULL,
    status        TEXT    NOT NULL CHECK (status IN ('pending','succeeded','failed')),
    result_json   TEXT,
    error_json    TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    completed_at  TEXT,
    CHECK (
        (status = 'pending'   AND result_json IS NULL     AND error_json IS NULL) OR
        (status = 'succeeded' AND result_json IS NOT NULL AND error_json IS NULL) OR
        (status = 'failed'    AND result_json IS NULL     AND error_json IS NOT NULL)
    )
);
CREATE INDEX idx_command_requests_created ON command_requests(created_at);

CREATE TABLE runtime_events (
    event_id      INTEGER PRIMARY KEY AUTOINCREMENT,
    epoch         INTEGER NOT NULL,
    entity_kind   TEXT    NOT NULL,
    entity_id     TEXT    NOT NULL,
    event_version TEXT    NOT NULL,
    channel       TEXT    NOT NULL,
    committed_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    payload_json  TEXT    NOT NULL
);
CREATE INDEX idx_runtime_events_entity ON runtime_events(entity_kind, entity_id, event_id);

INSERT INTO app_settings (key, value_json)
SELECT 'workspace_id', json_quote(lower(hex(randomblob(16))))
WHERE NOT EXISTS (SELECT 1 FROM app_settings WHERE key = 'workspace_id');
