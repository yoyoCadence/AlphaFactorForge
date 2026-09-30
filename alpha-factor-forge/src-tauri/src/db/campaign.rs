//! P12d-2c: frozen campaign declarations and the admission decision of each
//! campaign-bound run (migration 0010, docs/research-campaign-declaration-v1.md).
//!
//! Writes happen only inside the caller's owner-checked enqueue transaction
//! (`discovery::start_discovery_run_for_campaign`). Reads re-freeze the stored
//! declaration: a row is never trusted just because it is in the database.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};
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

/// A stored decision with its re-validated declaration.
// First runtime reader: P13 confirmation scheduling (and a campaign status
// view); until then only tests read decisions back.
#[allow(dead_code)]
#[derive(Debug)]
pub struct StoredCampaignAdmission {
    pub campaign: FrozenCampaignDeclaration,
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

    let document = String::from_utf8(canonical_json(record.campaign.document())?)
        .map_err(|_| invalid("campaign document is not UTF-8"))?;
    let stored: Option<String> = conn
        .query_row(
            "SELECT document_json FROM research_campaigns WHERE campaign_id = ?1",
            [campaign_id],
            |row| row.get(0),
        )
        .optional()?;
    match stored {
        Some(existing) if existing != document => {
            return Err(invalid(format!(
                "stored campaign {campaign_id} does not match its declaration"
            )))
        }
        Some(_) => {}
        None => {
            conn.execute(
                "INSERT INTO research_campaigns (campaign_id, version, document_json)
                 VALUES (?1, ?2, ?3)",
                params![campaign_id, CAMPAIGN_DECLARATION_VERSION, document],
            )?;
        }
    }
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
#[allow(dead_code)] // see `StoredCampaignAdmission`
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
    let campaign = freeze_campaign(&serde_json::from_str(&document)?)
        .map_err(|error| invalid(error.to_string()))?;
    if campaign.campaign_id() != campaign_id {
        return Err(invalid(format!(
            "stored campaign {campaign_id} no longer re-freezes to its identity"
        )));
    }
    Ok(Some(StoredCampaignAdmission {
        campaign,
        instrument_id,
        batch_id,
        status,
        report: serde_json::from_str(&report)?,
        fee_pct: fee,
        slip_pct: slip,
    }))
}
