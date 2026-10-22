//! Complete archival union. Private preflight precedes the first receiver write;
//! provenance checks grant no qualification, execution or workspace authority.
use super::super::{storage, ConfirmationWriteSummary};
use super::{decode::ValidatedWitnessed, load_witnessed_in, LedgerError, TrialLedger};
use crate::research::trial_ledger::{self as ledger, AlphaTransferSummary};
use alpha_factor_forge::discovery_core::confirmation_history::ValidatedGraph;
use rusqlite::{Transaction, TransactionBehavior};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitnessedTransferSummary {
    pub alpha: AlphaTransferSummary,
    pub confirmation: ConfirmationWriteSummary,
    pub confirmation_unknown_origin_count: usize,
    pub trial_history_known: bool,
    pub restricted: bool,
}
struct Proposed {
    summary: WitnessedTransferSummary,
    graph: ValidatedGraph,
    head: (u64, String),
    trial_evidence: ledger::LedgerEvidence,
    alpha_evidence: ledger::AlphaEvidence,
    alpha_unknown_origins: BTreeSet<String>,
}

fn require_source(source: &ValidatedWitnessed) -> Result<(), LedgerError> {
    if !source.trial_known() {
        return Err(LedgerError::TrialHistoryUnknown);
    }
    if !source.alpha().history_known() {
        return Err(LedgerError::AlphaHistoryUnknown);
    }
    if !source.graph().inventory().unknown_origins.is_empty() {
        return Err(LedgerError::ConfirmationHistoryUnknown);
    }
    Ok(())
}
fn current_provenance(
    tx: &Transaction<'_>,
    registry: &str,
    graph: &ValidatedGraph,
    qualification: bool,
) -> Result<bool, LedgerError> {
    let known = storage::trial_history_known_in(tx, registry, graph.inventory())?;
    if qualification && !known {
        return Err(LedgerError::TrialHistoryUnknown);
    }
    if qualification && !ledger::alpha::unknown_origins(tx)?.is_empty() {
        return Err(LedgerError::AlphaHistoryUnknown);
    }
    if qualification && !graph.inventory().unknown_origins.is_empty() {
        return Err(LedgerError::ConfirmationHistoryUnknown);
    }
    Ok(known)
}

/// Identical caller-owned apply/final checks in disposable and real transactions.
/// The outer caller is solely responsible for the one real commit.
fn apply_in(
    receiver: &TrialLedger,
    tx: &Transaction<'_>,
    source: &ValidatedWitnessed,
    qualification: bool,
) -> Result<Proposed, LedgerError> {
    load_witnessed_in(tx, receiver.registry_id())?;
    let alpha = source
        .alpha()
        .merge_in(receiver, tx, source.file_sha256())?;
    let confirmation = ledger::confirmation::append_witnessed_in(
        tx,
        receiver.registry_id(),
        source.graph().inventory(),
    )?;
    let graph = load_witnessed_in(tx, receiver.registry_id())?;
    if tx.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err(LedgerError::StateInvalid(
            "complete witnessed union has missing SQL dependencies".into(),
        ));
    }
    let trial_history_known =
        current_provenance(tx, receiver.registry_id(), &graph, qualification)?;
    let summary = WitnessedTransferSummary {
        alpha,
        confirmation,
        confirmation_unknown_origin_count: graph.inventory().unknown_origins.len(),
        trial_history_known,
        restricted: graph.is_restricted() || !trial_history_known,
    };
    Ok(Proposed {
        summary,
        graph,
        head: ledger::head(tx, receiver.registry_id())?,
        trial_evidence: ledger::binding::observed_evidence(tx)?,
        alpha_evidence: ledger::alpha::observed_identities(tx)?,
        alpha_unknown_origins: ledger::alpha::unknown_origins(tx)?,
    })
}
impl TrialLedger {
    /// Complete archival evidence, including valid unknown/conflicted history.
    /// Does not adopt workspace bindings or permit a reveal/execution/recovery.
    pub fn import_witnessed_json_lines(
        &self,
        bytes: &[u8],
    ) -> Result<WitnessedTransferSummary, LedgerError> {
        self.import_witnessed(bytes, false)
    }
    /// Coverage-only gate, distinct from actual qualification/admission. Known
    /// fully accounted conflicts remain restrictive even after this succeeds.
    pub fn import_witnessed_qualification_json_lines(
        &self,
        bytes: &[u8],
    ) -> Result<WitnessedTransferSummary, LedgerError> {
        self.import_witnessed(bytes, true)
    }
    fn import_witnessed(
        &self,
        bytes: &[u8],
        qualification: bool,
    ) -> Result<WitnessedTransferSummary, LedgerError> {
        // Complete original source validation cannot borrow receiver rows or
        // wait for its mutex. No destination mutation occurs on parse failure.
        let source = ValidatedWitnessed::parse(bytes)?;
        if qualification {
            require_source(&source)?;
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = load_witnessed_in(&tx, self.registry_id())?;
        if qualification {
            current_provenance(&tx, self.registry_id(), &current, true)?;
        }
        let expected = ledger::preflight::with_snapshot(&tx, |private| {
            apply_in(self, private, &source, qualification)
        })?;
        // with_snapshot has already rolled back, closed and cleaned its owned
        // staging database. This is the receiver's FIRST possible write path.
        let actual = apply_in(self, &tx, &source, qualification)?;
        if actual.summary != expected.summary
            || actual.graph.inventory() != expected.graph.inventory()
            || actual.head != expected.head
            || actual.trial_evidence != expected.trial_evidence
            || actual.alpha_evidence != expected.alpha_evidence
            || actual.alpha_unknown_origins != expected.alpha_unknown_origins
        {
            return Err(LedgerError::StateInvalid(
                "written complete union differs from private preflight".into(),
            ));
        }
        tx.commit()?;
        Ok(actual.summary)
    }
}
