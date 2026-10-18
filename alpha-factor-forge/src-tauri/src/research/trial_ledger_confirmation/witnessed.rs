//! Strict schema-7 archival IO. All context is reconstructed in the caller's
//! transaction. Original witnesses never import trial rows or grant execution.
use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::{
    complete_witnessed_confirmation_graph, parse_source_witness,
    validate_witnessed_confirmation_graph, SourceWitnessObservation, SourceWitnessSnapshot,
};

struct Context {
    graph: GraphContext,
    witnesses: Vec<SourceWitnessObservation>,
}
impl Context {
    fn read(
        tx: &Transaction<'_>,
        registry: &str,
        i: &ConfirmationInventory,
    ) -> Result<Self, LedgerError> {
        write::capacity(i)?;
        let current = ledger::transfer::WitnessCurrentContext::new(tx, registry)?;
        let mut graph = ledger::alpha::observed_confirmation_context(tx)
            .map_err(|_| bad("invalid current alpha context"))?;
        let evidence = current.evidence();
        graph.quarantined_families = evidence.quarantined_families.iter().cloned().collect();
        graph.conflicted_origins = evidence.conflicted_origins.iter().cloned().collect();
        graph.family_tests = evidence.family_tests.clone();
        let mut witnesses = Vec::new();
        let mut prefixes = BTreeSet::new();
        for blob in i
            .blobs
            .iter()
            .filter(|b| b.kind() == BlobKind::SourceWitness)
        {
            let original =
                parse_source_witness(blob.text()).map_err(|_| bad("invalid original witness"))?;
            let (facts, coverage) = current.observe(&original)?;
            // Historical anchors are facts of these independently verified
            // original snapshots. Current incorporation is checked separately
            // for the complete snapshot, including every foreign-origin row.
            prefixes.insert(facts.anchor().prefix().clone());
            graph.witness_coverage.push(coverage);
            witnesses.push(facts);
        }
        graph.historical_prefixes = prefixes.into_iter().collect();
        // No generic facts are copied into legacy eligible historical_trials.
        // The strict witness host matches the exact original admission anchor.
        Ok(Self { graph, witnesses })
    }
    fn validate(
        &self,
        i: &ConfirmationInventory,
        complete: bool,
    ) -> Result<ValidatedGraph, LedgerError> {
        let result = if complete {
            complete_witnessed_confirmation_graph(i, &self.graph, &self.witnesses)
        } else {
            validate_witnessed_confirmation_graph(i, &self.graph, &self.witnesses)
        };
        result.map_err(|_| bad("invalid or incomplete witnessed graph"))
    }
    fn required_unknowns(&self) -> BTreeSet<String> {
        self.graph
            .witness_coverage
            .iter()
            .flat_map(|v| &v.unincorporated_origins)
            .filter(|v| self.graph.conflicted_origins.binary_search(v).is_err())
            .cloned()
            .collect()
    }
}
fn add_required_unknowns(i: &mut ConfirmationInventory, c: &Context) -> Result<(), LedgerError> {
    i.unknown_origins = i
        .unknown_origins
        .iter()
        .cloned()
        .chain(c.required_unknowns())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    write::capacity(i)
}

pub(in crate::research::trial_ledger) fn load_witnessed_in(
    tx: &Transaction<'_>,
    registry: &str,
) -> Result<ValidatedGraph, LedgerError> {
    let i = read_witnessed_inventory(tx)?;
    let context = Context::read(tx, registry, &i)?;
    let checked = context.validate(&i, false)?;
    projection::verify(tx, checked.inventory())?;
    Ok(checked)
}

pub(in crate::research::trial_ledger) fn append_witnessed_in(
    tx: &Transaction<'_>,
    registry: &str,
    input: &ConfirmationInventory,
) -> Result<ConfirmationWriteSummary, LedgerError> {
    let old = load_witnessed_in(tx, registry)?;
    let (mut retained, mut overlay) = write::union(old.inventory(), input)?;
    let context = Context::read(tx, registry, &retained)?;
    add_required_unknowns(&mut retained, &context)?;
    overlay.unknown_origins = retained.unknown_origins.clone();
    context.validate(&overlay, true)?;
    let proposed = context.validate(&retained, true)?;
    let summary = write::persist_in(tx, old.inventory(), proposed.inventory())?;
    if load_witnessed_in(tx, registry)?.inventory() != proposed.inventory() {
        return Err(bad("written witnessed graph differs"));
    }
    Ok(summary)
}

/// Unlike ordinary append, the old graph can lack its source proofs here.
/// Everything else must ALREADY be materialized; never auto-complete conflicts.
fn restore_in(
    tx: &Transaction<'_>,
    registry: &str,
    witnesses: &[SourceWitnessSnapshot],
) -> Result<ConfirmationWriteSummary, LedgerError> {
    let old = read_witnessed_inventory(tx)?;
    projection::verify(tx, &old)?;
    let input = ConfirmationInventory {
        blobs: witnesses
            .iter()
            .map(|w| {
                HistoryBlob::new(BlobKind::SourceWitness, w.text())
                    .map_err(|_| bad("invalid original witness"))
            })
            .collect::<Result<_, _>>()?,
        ..ConfirmationInventory::default()
    };
    let (mut proposed, _) = write::union(&old, &input)?;
    let context = Context::read(tx, registry, &proposed)?;
    add_required_unknowns(&mut proposed, &context)?;
    let proposed = context.validate(&proposed, false)?;
    let summary = write::persist_in(tx, &old, proposed.inventory())?;
    if load_witnessed_in(tx, registry)?.inventory() != proposed.inventory() {
        return Err(bad("restored witnessed graph differs"));
    }
    Ok(summary)
}

impl TrialLedger {
    pub fn read_witnessed_confirmation_history(&self) -> Result<ValidatedGraph, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let graph = load_witnessed_in(&tx, self.registry_id())?;
        tx.commit()?;
        Ok(graph)
    }
    pub fn append_witnessed_confirmation_history(
        &self,
        input: &ConfirmationInventory,
    ) -> Result<ConfirmationWriteSummary, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let summary = append_witnessed_in(&tx, self.registry_id(), input)?;
        tx.commit()?;
        Ok(summary)
    }
    pub fn restore_confirmation_source_witnesses(
        &self,
        witnesses: &[SourceWitnessSnapshot],
    ) -> Result<ConfirmationWriteSummary, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let summary = restore_in(&tx, self.registry_id(), witnesses)?;
        tx.commit()?;
        Ok(summary)
    }
    pub fn find_original_witnessed_confirmation_operations(
        &self,
        origin_registry_id: &str,
        reservation_key: &str,
    ) -> Result<ConfirmationLookup, LedgerError> {
        if !origin(origin_registry_id) || !origin(reservation_key) {
            return Err(bad("invalid original operation key"));
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let graph = load_witnessed_in(&tx, self.registry_id())?;
        let operation_ids = {
            let mut stmt=tx.prepare("SELECT operation_id FROM confirmation_operations WHERE origin_registry_id=?1 AND reservation_key=?2 ORDER BY operation_id")?;
            let ids = stmt
                .query_map(params![origin_registry_id, reservation_key], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            ids
        };
        tx.commit()?;
        Ok(ConfirmationLookup {
            graph,
            operation_ids,
        })
    }
}
