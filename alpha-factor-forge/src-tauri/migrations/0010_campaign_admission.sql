-- P12d-2c (docs/research-campaign-declaration-v1.md): frozen campaign
-- declarations and the admission decision of each campaign-bound run.
--
-- A declaration row is the validated, canonical document whose re-freeze
-- must reproduce `campaign_id`; readers re-validate instead of trusting it.
-- A decision row is written in the same owner-checked transaction that
-- queues the run's jobs, so a queued campaign run always has exactly one.
-- `NOT_ELIGIBLE` runs still execute (maintainer decision 2026-09-29); the
-- decision only gates later confirmation. Both tables are append-only.

CREATE TABLE research_campaigns (
    campaign_id   TEXT PRIMARY KEY CHECK (length(campaign_id) = 64),
    version       TEXT NOT NULL,
    document_json TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE campaign_run_admissions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    discovery_run_id INTEGER NOT NULL UNIQUE
                     REFERENCES discovery_runs(id) ON DELETE RESTRICT,
    campaign_id      TEXT NOT NULL REFERENCES research_campaigns(campaign_id),
    instrument_id    TEXT NOT NULL,
    batch_id         TEXT NOT NULL,
    status           TEXT NOT NULL CHECK (status IN ('ELIGIBLE', 'NOT_ELIGIBLE')),
    report_version   TEXT NOT NULL,
    -- research-campaign-admission-v1 report: reasons, snapshot observation,
    -- the AdmissionSnapshot a later fence compares, precision, walk-forward.
    report_json      TEXT NOT NULL,
    -- The run's frozen costs: P06 cost-profile-v1 carries no numbers, so these
    -- are not cross-checked against the snapshot (recorded limitation).
    fee_pct          REAL NOT NULL,
    slip_pct         REAL NOT NULL,
    epoch            INTEGER,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_campaign_run_admissions_campaign
    ON campaign_run_admissions(campaign_id, instrument_id);

CREATE TRIGGER research_campaigns_no_update BEFORE UPDATE ON research_campaigns
BEGIN SELECT RAISE(ABORT, 'research_campaigns is append-only'); END;
CREATE TRIGGER research_campaigns_no_delete BEFORE DELETE ON research_campaigns
BEGIN SELECT RAISE(ABORT, 'research_campaigns is append-only'); END;
CREATE TRIGGER campaign_run_admissions_no_update BEFORE UPDATE ON campaign_run_admissions
BEGIN SELECT RAISE(ABORT, 'campaign_run_admissions is append-only'); END;
CREATE TRIGGER campaign_run_admissions_no_delete BEFORE DELETE ON campaign_run_admissions
BEGIN SELECT RAISE(ABORT, 'campaign_run_admissions is append-only'); END;
