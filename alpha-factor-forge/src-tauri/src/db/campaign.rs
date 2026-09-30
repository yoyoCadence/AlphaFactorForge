//! P12d-2c: frozen campaign declarations and the admission decision of each
//! campaign-bound run (migration 0010, docs/research-campaign-declaration-v1.md).
//!
//! Writes happen only as the workspace owner: `freeze_and_store_campaign`
//! (P12d-2d, its own write transaction) or inside the enqueue transaction
//! (`discovery::start_discovery_run_for_campaign`). Reads re-freeze the stored
//! declaration: a row is never trusted just because it is in the database.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

use alpha_factor_forge::discovery_core::campaign::{
    freeze_campaign, FrozenCampaignDeclaration, CAMPAIGN_DECLARATION_VERSION,
};

use crate::error::{AppError, AppResult};
use crate::research::campaign_admission::{
    CampaignAdmissionReport, CampaignAdmissionStatus, CAMPAIGN_ADMISSION_VERSION,
};
use crate::research::canonical_json;
use crate::research::history::RunLineage;

/// What the enqueue transaction stores for a campaign-bound run.
pub struct CampaignAdmissionRecord<'a> {
    pub campaign: &'a FrozenCampaignDeclaration,
    pub report: &'a CampaignAdmissionReport,
    /// The run's frozen costs (maintainer decision 2026-09-29).
    pub fee_pct: f64,
    pub slip_pct: f64,
}

/// A stored decision whose declaration was re-validated on read. Serialized
/// for the read-only decision view; list the declaration with `list_campaigns`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredCampaignAdmission {
    pub campaign_id: String,
    pub instrument_id: String,
    pub batch_id: String,
    pub status: String,
    pub report: Value,
    pub fee_pct: f64,
    pub slip_pct: f64,
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Other(format!("campaign admission record: {}", message.into()))
}

fn status_code(status: CampaignAdmissionStatus) -> &'static str {
    match status {
        CampaignAdmissionStatus::Eligible => "ELIGIBLE",
        CampaignAdmissionStatus::NotEligible => "NOT_ELIGIBLE",
    }
}

/// Store a frozen declaration once, inside the caller's transaction. The same
/// campaign stored again must present the identical canonical document.
fn store_campaign(conn: &Connection, campaign: &FrozenCampaignDeclaration) -> AppResult<()> {
    let campaign_id = campaign.campaign_id();
    let document = String::from_utf8(canonical_json(campaign.document())?)
        .map_err(|_| invalid("campaign document is not UTF-8"))?;
    let stored: Option<String> = conn
        .query_row(
            "SELECT document_json FROM research_campaigns WHERE campaign_id = ?1",
            [campaign_id],
            |row| row.get(0),
        )
        .optional()?;
    match stored {
        Some(existing) if existing != document => Err(invalid(format!(
            "stored campaign {campaign_id} does not match its declaration"
        ))),
        Some(_) => Ok(()),
        None => {
            conn.execute(
                "INSERT INTO research_campaigns (campaign_id, version, document_json)
                 VALUES (?1, ?2, ?3)",
                params![campaign_id, CAMPAIGN_DECLARATION_VERSION, document],
            )?;
            Ok(())
        }
    }
}

/// P12d-2d: freeze a declaration and store it as the workspace owner
/// (maintainer decision 2026-09-30: freezing saves; runs start from the saved
/// list). Validation is `freeze_campaign`'s, never the caller's. Storing the
/// same document again is a no-op that returns the same ID.
pub fn freeze_and_store_campaign(
    conn: &Connection,
    epoch: Option<i64>,
    declaration: &Value,
) -> AppResult<String> {
    let frozen =
        freeze_campaign(declaration).map_err(|error| AppError::Other(error.to_string()))?;
    let tx = super::ownership::write_transaction(conn, epoch)?;
    store_campaign(&tx, &frozen)?;
    tx.commit()?;
    Ok(frozen.campaign_id().to_string())
}

/// A stored campaign, re-frozen from its row, or `None` when not stored.
pub fn get_campaign(
    conn: &Connection,
    campaign_id: &str,
) -> AppResult<Option<FrozenCampaignDeclaration>> {
    let document: Option<String> = conn
        .query_row(
            "SELECT document_json FROM research_campaigns WHERE campaign_id = ?1",
            [campaign_id],
            |row| row.get(0),
        )
        .optional()?;
    document
        .map(|document| refreeze(campaign_id, &document))
        .transpose()
}

fn refreeze(campaign_id: &str, document: &str) -> AppResult<FrozenCampaignDeclaration> {
    let campaign = freeze_campaign(&serde_json::from_str(document)?)
        .map_err(|error| invalid(error.to_string()))?;
    if campaign.campaign_id() != campaign_id {
        return Err(invalid(format!(
            "stored campaign {campaign_id} no longer re-freezes to its identity"
        )));
    }
    Ok(campaign)
}

/// One run started for a stored campaign and its decision.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignRunSummary {
    pub run_id: i64,
    pub instrument_id: String,
    pub status: String,
    pub created_at: String,
}

/// A stored campaign for the authoring list: its re-validated document and
/// the runs already started for it (one per instrument per run).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignSummary {
    pub campaign_id: String,
    pub version: String,
    pub created_at: String,
    pub document: Value,
    pub runs: Vec<CampaignRunSummary>,
}

/// Every stored campaign, newest first. A row that no longer re-freezes to
/// its ID is an error, not silently skipped.
pub fn list_campaigns(conn: &Connection) -> AppResult<Vec<CampaignSummary>> {
    let rows: Vec<(String, String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT campaign_id, version, document_json, created_at FROM research_campaigns
              ORDER BY created_at DESC, campaign_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let mut runs_stmt = conn.prepare(
        "SELECT discovery_run_id, instrument_id, status, created_at FROM campaign_run_admissions
          WHERE campaign_id = ?1 ORDER BY discovery_run_id",
    )?;
    rows.into_iter()
        .map(|(campaign_id, version, document, created_at)| {
            let frozen = refreeze(&campaign_id, &document)?;
            let runs = runs_stmt
                .query_map([&campaign_id], |row| {
                    Ok(CampaignRunSummary {
                        run_id: row.get(0)?,
                        instrument_id: row.get(1)?,
                        status: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                })?
                .collect::<Result<_, _>>()?;
            Ok(CampaignSummary {
                campaign_id,
                version,
                created_at,
                document: frozen.document().clone(),
                runs,
            })
        })
        .collect()
}

/// Store the declaration once and append the run's decision, inside the
/// caller's transaction. The decision must describe exactly the candidates
/// this enqueue freezes (by index and strategy hash): a missing or
/// substituted candidate is refused, never recorded.
pub(crate) fn record_campaign_admission(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    lineage: &RunLineage,
    record: &CampaignAdmissionRecord<'_>,
) -> AppResult<()> {
    let campaign_id = record.campaign.campaign_id();
    let report = record.report;
    if report.campaign_id != campaign_id || report.contract_version != CAMPAIGN_ADMISSION_VERSION {
        return Err(invalid(
            "the decision was made for another campaign or contract",
        ));
    }
    if !record.fee_pct.is_finite() || !record.slip_pct.is_finite() {
        return Err(invalid("frozen costs must be finite"));
    }
    let admitted: BTreeSet<(i64, &str)> = report
        .walk_forward
        .iter()
        .map(|candidate| (candidate.candidate_index, candidate.strategy_hash.as_str()))
        .collect();
    let queued = lineage
        .iter()
        .map(|(_, attempt)| {
            let index = attempt
                .candidate_index
                .ok_or_else(|| invalid("a queued attempt has no candidate index"))?;
            let strategy = attempt
                .input_fingerprint
                .get("strategyHash")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("a queued attempt has no strategy hash"))?;
            Ok((index, strategy))
        })
        .collect::<AppResult<BTreeSet<_>>>()?;
    if admitted.len() != report.walk_forward.len() || admitted != queued {
        return Err(invalid(
            "the decision's candidates differ from the candidates being queued",
        ));
    }

    store_campaign(conn, record.campaign)?;
    conn.execute(
        "INSERT INTO campaign_run_admissions
            (discovery_run_id, campaign_id, instrument_id, batch_id, status,
             report_version, report_json, fee_pct, slip_pct, epoch)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            run_id,
            campaign_id,
            report.instrument_id,
            report.batch_id,
            status_code(report.status),
            report.contract_version,
            serde_json::to_string(report)?,
            record.fee_pct,
            record.slip_pct,
            epoch,
        ],
    )?;
    Ok(())
}

/// campaign_id, document_json, instrument_id, batch_id, status, report_json,
/// fee_pct, slip_pct.
type AdmissionRow = (String, String, String, String, String, String, f64, f64);

/// The decision of a campaign-bound run, or `None` for any other run. The
/// declaration is re-frozen and must reproduce its stored identity.
pub fn get_campaign_admission(
    conn: &Connection,
    run_id: i64,
) -> AppResult<Option<StoredCampaignAdmission>> {
    let row: Option<AdmissionRow> = conn
        .query_row(
            "SELECT a.campaign_id, c.document_json, a.instrument_id, a.batch_id, a.status,
                    a.report_json, a.fee_pct, a.slip_pct
               FROM campaign_run_admissions a
               JOIN research_campaigns c ON c.campaign_id = a.campaign_id
              WHERE a.discovery_run_id = ?1",
            [run_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .optional()?;
    let Some((campaign_id, document, instrument_id, batch_id, status, report, fee, slip)) = row
    else {
        return Ok(None);
    };
    refreeze(&campaign_id, &document)?;
    Ok(Some(StoredCampaignAdmission {
        campaign_id,
        instrument_id,
        batch_id,
        status,
        report: serde_json::from_str(&report)?,
        fee_pct: fee,
        slip_pct: slip,
    }))
}
