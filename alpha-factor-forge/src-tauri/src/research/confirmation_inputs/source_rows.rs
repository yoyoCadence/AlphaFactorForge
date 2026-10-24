//! Dormant exact root observations. These are incomplete consistency evidence,
//! never authenticated source inputs or a live workspace/registry capability.

use rusqlite::{params, types::ValueRef, Row, Transaction};

use super::original_json::OriginalJson;
use super::{capture_workspace_in, recheck_workspace_in, ExpectedWorkspace, WorkspaceObservation};
use crate::error::{AppError, AppResult};

mod selection;

const MAX_TEXT_BYTES: usize = 4096;
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOTAL_TEXT_BYTES: usize = 32 * 1024 * 1024;
const MAX_TOTAL_JSON_NODES: usize = 262_144;
const FIXED_TEXT_BYTES: usize = 128; // locator + actual workspace/holder IDs

// Each logical cell has a validity marker followed by its guarded value.
// Invalid/oversized payloads never cross SQLite's projection boundary.
const CAMPAIGN_SQL: &str = "SELECT
 CASE WHEN typeof(campaign_id)='text' AND length(CAST(campaign_id AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(campaign_id)='text' AND length(CAST(campaign_id AS BLOB))<=?2 THEN campaign_id ELSE NULL END,
 CASE WHEN typeof(version)='text' AND length(CAST(version AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(version)='text' AND length(CAST(version AS BLOB))<=?2 THEN version ELSE NULL END,
 CASE WHEN typeof(document_json)='text' AND length(CAST(document_json AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof(document_json)='text' AND length(CAST(document_json AS BLOB))<=?3 THEN document_json ELSE NULL END
 FROM research_campaigns WHERE campaign_id=?1 ORDER BY campaign_id LIMIT 2";

const ADMISSION_SQL: &str = "SELECT
 CASE WHEN typeof(id)='integer' AND id>0 THEN 1 ELSE 0 END,
 CASE WHEN typeof(id)='integer' AND id>0 THEN id ELSE NULL END,
 CASE WHEN typeof(discovery_run_id)='integer' AND discovery_run_id>0 THEN 1 ELSE 0 END,
 CASE WHEN typeof(discovery_run_id)='integer' AND discovery_run_id>0 THEN discovery_run_id ELSE NULL END,
 CASE WHEN typeof(campaign_id)='text' AND length(CAST(campaign_id AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(campaign_id)='text' AND length(CAST(campaign_id AS BLOB))<=?2 THEN campaign_id ELSE NULL END,
 CASE WHEN typeof(instrument_id)='text' AND length(CAST(instrument_id AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(instrument_id)='text' AND length(CAST(instrument_id AS BLOB))<=?2 THEN instrument_id ELSE NULL END,
 CASE WHEN typeof(batch_id)='text' AND length(CAST(batch_id AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(batch_id)='text' AND length(CAST(batch_id AS BLOB))<=?2 THEN batch_id ELSE NULL END,
 CASE WHEN typeof(status)='text' AND length(CAST(status AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(status)='text' AND length(CAST(status AS BLOB))<=?2 THEN status ELSE NULL END,
 CASE WHEN typeof(report_version)='text' AND length(CAST(report_version AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(report_version)='text' AND length(CAST(report_version AS BLOB))<=?2 THEN report_version ELSE NULL END,
 CASE WHEN typeof(report_json)='text' AND length(CAST(report_json AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof(report_json)='text' AND length(CAST(report_json AS BLOB))<=?3 THEN report_json ELSE NULL END,
 CASE WHEN typeof(fee_pct)='real' THEN 1 ELSE 0 END,
 CASE WHEN typeof(fee_pct)='real' THEN fee_pct ELSE NULL END,
 CASE WHEN typeof(slip_pct)='real' THEN 1 ELSE 0 END,
 CASE WHEN typeof(slip_pct)='real' THEN slip_pct ELSE NULL END
 FROM campaign_run_admissions WHERE discovery_run_id=?1 ORDER BY id LIMIT 2";

const RUN_SQL: &str = "SELECT
 CASE WHEN typeof(id)='integer' AND id>0 THEN 1 ELSE 0 END,
 CASE WHEN typeof(id)='integer' AND id>0 THEN id ELSE NULL END,
 CASE WHEN typeof(status)='text' AND length(CAST(status AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof(status)='text' AND length(CAST(status AS BLOB))<=?2 THEN status ELSE NULL END,
 CASE WHEN typeof(config_json)='text' AND length(CAST(config_json AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof(config_json)='text' AND length(CAST(config_json AS BLOB))<=?3 THEN config_json ELSE NULL END
 FROM discovery_runs WHERE id=?1 ORDER BY id LIMIT 2";

fn invalid() -> AppError {
    AppError::Other("source_rows_invalid".into())
}

fn changed() -> AppError {
    AppError::Other("source_rows_changed".into())
}

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn version(value: &str) -> bool {
    !value.is_empty()
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct SourceLocator {
    campaign_id: String,
    run_id: i64,
}

impl SourceLocator {
    pub(super) fn new(campaign_id: &str, run_id: i64) -> AppResult<Self> {
        if !hash(campaign_id) || run_id <= 0 {
            return Err(invalid());
        }
        Ok(Self {
            campaign_id: campaign_id.into(),
            run_id,
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
struct CampaignRoot {
    campaign_id: String,
    version: String,
    document_json: OriginalJson,
}

#[derive(Clone, Eq, PartialEq)]
struct AdmissionRoot {
    id: i64,
    discovery_run_id: i64,
    campaign_id: String,
    instrument_id: String,
    batch_id: String,
    status: String,
    report_version: String,
    report_json: OriginalJson,
    fee_pct_bits: u64,
    slip_pct_bits: u64,
}

#[derive(Clone, Eq, PartialEq)]
struct RunRoot {
    id: i64,
    status: String,
    config_json: OriginalJson,
}

#[derive(Clone, Eq, PartialEq)]
struct ResourceUsage {
    text_bytes: usize,
    json_nodes: usize,
}

struct ResourceBudget {
    used: ResourceUsage,
}

impl ResourceBudget {
    fn new() -> AppResult<Self> {
        let mut budget = Self {
            used: ResourceUsage {
                text_bytes: 0,
                json_nodes: 0,
            },
        };
        budget.charge(FIXED_TEXT_BYTES, 0)?;
        Ok(budget)
    }

    fn check_bytes(&self, bytes: usize) -> AppResult<()> {
        match self.used.text_bytes.checked_add(bytes) {
            Some(total) if total <= MAX_TOTAL_TEXT_BYTES => Ok(()),
            _ => Err(invalid()),
        }
    }

    fn charge(&mut self, bytes: usize, nodes: usize) -> AppResult<()> {
        self.check_bytes(bytes)?;
        let total_nodes = self
            .used
            .json_nodes
            .checked_add(nodes)
            .ok_or_else(invalid)?;
        if total_nodes > MAX_TOTAL_JSON_NODES {
            return Err(invalid());
        }
        self.used.text_bytes += bytes; // checked before either counter changes
        self.used.json_nodes = total_nodes;
        Ok(())
    }

    fn json(&mut self, raw: &[u8]) -> AppResult<OriginalJson> {
        self.check_bytes(raw.len())?;
        let remaining = MAX_TOTAL_JSON_NODES
            .checked_sub(self.used.json_nodes)
            .ok_or_else(invalid)?;
        // The parser clamps the remainder and checks it before its sole copy.
        let json = OriginalJson::parse_with_node_limit(raw, remaining).map_err(|_| invalid())?;
        self.charge(raw.len(), json.node_count())?;
        Ok(json)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct RootRowsObservation {
    workspace: WorkspaceObservation,
    locator: SourceLocator,
    campaign: CampaignRoot,
    admission: AdmissionRoot,
    run: RunRoot,
    usage: ResourceUsage,
}

fn marked_value<'a>(row: &'a Row<'_>, logical: usize) -> AppResult<ValueRef<'a>> {
    if !matches!(row.get_ref(2 * logical)?, ValueRef::Integer(1)) {
        return Err(invalid());
    }
    Ok(row.get_ref(2 * logical + 1)?)
}

fn text_cell(
    row: &Row<'_>,
    logical: usize,
    budget: &mut ResourceBudget,
    domain: fn(&str) -> bool,
) -> AppResult<String> {
    let ValueRef::Text(raw) = marked_value(row, logical)? else {
        return Err(invalid());
    };
    if raw.len() > MAX_TEXT_BYTES {
        return Err(invalid());
    }
    budget.check_bytes(raw.len())?;
    let text = std::str::from_utf8(raw).map_err(|_| invalid())?;
    if !domain(text) {
        return Err(invalid());
    }
    budget.charge(raw.len(), 0)?;
    Ok(text.to_owned())
}

fn json_cell(
    row: &Row<'_>,
    logical: usize,
    budget: &mut ResourceBudget,
) -> AppResult<OriginalJson> {
    let ValueRef::Text(raw) = marked_value(row, logical)? else {
        return Err(invalid());
    };
    budget.json(raw)
}

fn positive_cell(row: &Row<'_>, logical: usize) -> AppResult<i64> {
    match marked_value(row, logical)? {
        ValueRef::Integer(value) if value > 0 => Ok(value),
        _ => Err(invalid()),
    }
}

fn real_cell(row: &Row<'_>, logical: usize) -> AppResult<u64> {
    match marked_value(row, logical)? {
        ValueRef::Real(value) if value.is_finite() => Ok(value.to_bits()),
        _ => Err(invalid()),
    }
}

fn read_roots(
    tx: &Transaction<'_>,
    locator: &SourceLocator,
    budget: &mut ResourceBudget,
) -> AppResult<(CampaignRoot, AdmissionRoot, RunRoot)> {
    let campaign = {
        let mut statement = tx.prepare(CAMPAIGN_SQL)?;
        let mut rows = statement.query(params![
            locator.campaign_id,
            MAX_TEXT_BYTES as i64,
            MAX_JSON_BYTES as i64
        ])?;
        let row = rows.next()?.ok_or_else(invalid)?;
        let root = CampaignRoot {
            campaign_id: text_cell(row, 0, budget, hash)?,
            version: text_cell(row, 1, budget, version)?,
            document_json: json_cell(row, 2, budget)?,
        };
        if rows.next()?.is_some() {
            return Err(invalid());
        }
        root
    };
    let admission = {
        let mut statement = tx.prepare(ADMISSION_SQL)?;
        let mut rows = statement.query(params![
            locator.run_id,
            MAX_TEXT_BYTES as i64,
            MAX_JSON_BYTES as i64
        ])?;
        let row = rows.next()?.ok_or_else(invalid)?;
        let root = AdmissionRoot {
            id: positive_cell(row, 0)?,
            discovery_run_id: positive_cell(row, 1)?,
            campaign_id: text_cell(row, 2, budget, hash)?,
            instrument_id: text_cell(row, 3, budget, |s| {
                !s.is_empty() && s.len() <= 256 && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
            })?,
            batch_id: text_cell(row, 4, budget, |s| {
                s.strip_prefix("trial-batch-v1:").is_some_and(hash)
            })?,
            status: text_cell(row, 5, budget, |s| matches!(s, "ELIGIBLE" | "NOT_ELIGIBLE"))?,
            report_version: text_cell(row, 6, budget, version)?,
            report_json: json_cell(row, 7, budget)?,
            fee_pct_bits: real_cell(row, 8)?,
            slip_pct_bits: real_cell(row, 9)?,
        };
        if rows.next()?.is_some() {
            return Err(invalid());
        }
        root
    };
    let run = {
        let mut statement = tx.prepare(RUN_SQL)?;
        let mut rows = statement.query(params![
            locator.run_id,
            MAX_TEXT_BYTES as i64,
            MAX_JSON_BYTES as i64
        ])?;
        let row = rows.next()?.ok_or_else(invalid)?;
        let root = RunRoot {
            id: positive_cell(row, 0)?,
            status: text_cell(row, 1, budget, |s| {
                matches!(
                    s,
                    "idle" | "running" | "paused" | "completed" | "failed" | "cancelled"
                )
            })?,
            config_json: json_cell(row, 2, budget)?,
        };
        if rows.next()?.is_some() {
            return Err(invalid());
        }
        root
    };
    if campaign.campaign_id != locator.campaign_id
        || admission.campaign_id != locator.campaign_id
        || admission.discovery_run_id != locator.run_id
        || run.id != locator.run_id
    {
        return Err(invalid());
    }
    Ok((campaign, admission, run))
}

pub(super) fn capture_root_rows_in(
    tx: &Transaction<'_>,
    expected: &ExpectedWorkspace,
    locator: &SourceLocator,
) -> AppResult<RootRowsObservation> {
    let mut budget = ResourceBudget::new()?;
    let workspace = capture_workspace_in(tx, expected)?;
    let (campaign, admission, run) = read_roots(tx, locator, &mut budget)?;
    Ok(RootRowsObservation {
        workspace,
        locator: locator.clone(),
        campaign,
        admission,
        run,
        usage: budget.used,
    })
}

pub(super) fn recheck_root_rows_in(
    tx: &Transaction<'_>,
    expected: &ExpectedWorkspace,
    saved: &RootRowsObservation,
) -> AppResult<()> {
    let mut budget = ResourceBudget::new()?;
    recheck_workspace_in(tx, expected, &saved.workspace)?;
    let (campaign, admission, run) = read_roots(tx, &saved.locator, &mut budget)?;
    if campaign != saved.campaign
        || admission != saved.admission
        || run != saved.run
        || budget.used != saved.usage
    {
        return Err(changed());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
