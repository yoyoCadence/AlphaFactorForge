-- 0004_workspace_ownership — P03a (docs/research-runtime-contract.md §1,
-- `ownership-lease-v1`).
--
-- One row per workspace database records WHO currently owns it and the
-- ownership EPOCH. The row is not the lock: the OS-level exclusive lock on
-- `ownership.lock` (taken before this database is even opened) is what keeps
-- two hosts apart. The row is what a writer checks inside its own transaction
-- (`WHERE epoch = ?`) so that a worker still reporting under a previous epoch
-- can never write a result, and what a connected reader shows as "who / how
-- alive" — heartbeat staleness is advisory only and never authorises a
-- take-over (§1.4).
--
-- `epoch = 0` means the workspace has never been owned. Every acquisition
-- increments it; it never decreases. `heartbeat_seq` is a plain counter the
-- owner bumps every heartbeat, so a reader judges liveness by "did the
-- counter move during MY monotonic interval" rather than by comparing wall
-- clocks across processes (§1.4, 時鐘來源).

CREATE TABLE workspace_ownership (
    id                 INTEGER PRIMARY KEY CHECK (id = 1),
    epoch              INTEGER NOT NULL CHECK (epoch >= 0),
    holder_kind        TEXT    NOT NULL,   -- none | desktop-embedded | service (contract §1.1)
    holder_instance_id TEXT    NOT NULL,   -- random per acquisition; '' while unowned
    pid                INTEGER NOT NULL,   -- diagnostic only, never an ownership proof
    acquired_at        TEXT    NOT NULL,
    heartbeat_at       TEXT    NOT NULL,   -- wall clock, display only
    heartbeat_seq      INTEGER NOT NULL DEFAULT 0 CHECK (heartbeat_seq >= 0)
);

INSERT INTO workspace_ownership
    (id, epoch, holder_kind, holder_instance_id, pid, acquired_at, heartbeat_at, heartbeat_seq)
VALUES
    (1, 0, 'none', '', 0, datetime('now'), datetime('now'), 0);
