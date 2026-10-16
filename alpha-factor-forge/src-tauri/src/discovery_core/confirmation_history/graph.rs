//! Closed, bounded archival graph validation. Host observations are evidence,
//! never an authenticated database snapshot or an execution permit.
use super::admission::{genesis, BindingObservation, BudgetIdentity};
use super::*;
use crate::discovery_core::{
    alpha_allocation::{alpha_allocation_id, parse_alpha_allocation, AlphaAllocationDeclaration},
    confirmation_contracts::{
        footprints_overlap, parse_operation, parse_selection, parse_test_batch,
        parse_validation_batch, validate_registered_source, verify_confirmation_report,
        FrozenSelection, RegisteredSource, SealedValidationInput,
    },
};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_HISTORY_ROWS: usize = 10000;
pub const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_HISTORY_INTERVALS: usize = 128000;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConfirmationInventory {
    pub batches: Vec<String>,
    pub selections: Vec<String>,
    pub operations: Vec<String>,
    pub exposures: Vec<String>,
    pub admissions: Vec<String>,
    pub events: Vec<String>,
    pub conflicts: Vec<String>,
    pub blobs: Vec<HistoryBlob>,
    pub unknown_origins: Vec<String>,
}
impl ConfirmationInventory {
    fn sections(&self) -> [&Vec<String>; 7] {
        [
            &self.batches,
            &self.selections,
            &self.operations,
            &self.exposures,
            &self.admissions,
            &self.events,
            &self.conflicts,
        ]
    }
    fn capacity(&self) -> Result<(), HistoryRecordError> {
        let rows = self.sections().iter().map(|v| v.len()).sum::<usize>() + self.blobs.len();
        require(
            rows <= MAX_HISTORY_ROWS && self.unknown_origins.len() <= 10000,
            HistoryRecordError::Capacity,
        )?;
        let bytes = self
            .sections()
            .iter()
            .flat_map(|v| v.iter())
            .map(|v| v.len())
            .chain(self.blobs.iter().map(|v| v.text().len()))
            .try_fold(0usize, |n, v| n.checked_add(v))
            .ok_or(HistoryRecordError::Capacity)?;
        require(bytes <= MAX_HISTORY_BYTES, HistoryRecordError::Capacity)?;
        require(
            ordered(&self.unknown_origins) && self.unknown_origins.iter().all(|v| hex(v, 32)),
            HistoryRecordError::Reference,
        )
    }
}
#[derive(Clone, Debug)]
pub struct HostAllocation {
    pub family_id: String,
    pub declaration: AlphaAllocationDeclaration,
}
#[derive(Clone, Debug)]
pub struct HostConflict {
    pub id: String,
    pub family_ids: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct HistoricalPrefix {
    pub registry_id: String,
    pub seq: u64,
    pub chain_head: String,
}
/// Complete independent observations from the future transactional host.
#[derive(Clone, Debug, Default)]
pub struct GraphContext {
    pub registered_sources: Vec<RegisteredSource>,
    pub allocations: Vec<HostAllocation>,
    pub reservations: Vec<AlphaReservation>,
    pub alpha_conflicts: Vec<HostConflict>,
    pub alpha_unknown_origins: Vec<String>,
    pub quarantined_families: Vec<String>,
    pub conflicted_origins: Vec<String>,
    pub family_tests: BTreeMap<String, u64>,
    pub historical_prefixes: Vec<HistoricalPrefix>,
    pub historical_trials: Vec<HistoricalTrialObservation>,
}
/// Archival consistency only. No release, recovery or engine entry point.
#[derive(Clone, Debug)]
pub struct ValidatedGraph {
    inventory: ConfirmationInventory,
    restricted: bool,
}
impl ValidatedGraph {
    pub fn inventory(&self) -> &ConfirmationInventory {
        &self.inventory
    }
    /// Global archival conflict/provenance flags. False does not clear any
    /// occupied time support and is never a qualification or execution grant.
    pub fn is_restricted(&self) -> bool {
        self.restricted
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Kind {
    Batch,
    Selection,
    Operation,
    Exposure,
    Admission,
    Event,
    Conflict,
    Blob,
}
type Key = (Kind, String);
struct Node {
    raw: String,
    body: Value,
    deps: BTreeSet<Key>,
}
fn field<'a>(v: &'a Value, name: &str) -> Result<&'a str, HistoryRecordError> {
    v.get(name)
        .and_then(Value::as_str)
        .ok_or(HistoryRecordError::Dependency)
}
fn key(k: Kind, id: &str) -> Key {
    (k, id.into())
}
fn dep(set: &mut BTreeSet<Key>, k: Kind, v: &Value, name: &str) -> Result<(), HistoryRecordError> {
    set.insert(key(k, field(v, name)?));
    Ok(())
}
/// Iterative Kahn traversal also covers binding pre-state edges. No recursive
/// evaluation whose call stack depends on an imported graph's depth.
fn topological(nodes: &BTreeMap<Key, Node>) -> Result<Vec<Key>, HistoryRecordError> {
    let mut pending = BTreeMap::new();
    let mut children: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
    for (k, n) in nodes {
        pending.insert(k.clone(), n.deps.len());
        for d in &n.deps {
            require(nodes.contains_key(d), HistoryRecordError::Dependency)?;
            children.entry(d.clone()).or_default().push(k.clone());
        }
    }
    let mut ready: VecDeque<Key> = pending
        .iter()
        .filter(|(_, v)| **v == 0)
        .map(|(k, _)| k.clone())
        .collect();
    let mut order = Vec::new();
    while let Some(k) = ready.pop_front() {
        if let Some(next) = children.get(&k) {
            for c in next {
                let count = pending.get_mut(c).ok_or(HistoryRecordError::Dependency)?;
                *count -= 1;
                if *count == 0 {
                    ready.push_back(c.clone());
                }
            }
        }
        order.push(k);
    }
    require(order.len() == nodes.len(), HistoryRecordError::Cycle)?;
    Ok(order)
}
type TrialObservationKey = (HistoricalPrefix, String, String);
struct Host<'a> {
    context: &'a GraphContext,
    sources: BTreeMap<String, &'a RegisteredSource>,
    prefixes: BTreeSet<HistoricalPrefix>,
    trials: BTreeMap<TrialObservationKey, &'a HistoricalTrialObservation>,
    allocations: BTreeMap<BudgetIdentity, &'a AlphaAllocationDeclaration>,
    reservations: BTreeMap<String, &'a AlphaReservation>,
    conflicts: BTreeMap<String, &'a HostConflict>,
}
impl<'a> Host<'a> {
    fn new(c: &'a GraphContext) -> Result<Self, HistoryRecordError> {
        require(
            c.allocations.len() + c.reservations.len() + c.alpha_conflicts.len() <= 30000
                && c.registered_sources.len() <= 10000
                && c.historical_prefixes.len() <= 10000
                && c.historical_trials.len() <= 10000
                && c.family_tests.len() <= 10000,
            HistoryRecordError::Capacity,
        )?;
        for ids in [&c.alpha_unknown_origins, &c.conflicted_origins] {
            require(
                ids.len() <= 10000 && ordered(ids) && ids.iter().all(|v| hex(v, 32)),
                HistoryRecordError::Reference,
            )?;
        }
        require(
            c.quarantined_families.len() <= 10000
                && ordered(&c.quarantined_families)
                && c.quarantined_families.iter().all(|v| admission::family(v))
                && c.family_tests
                    .iter()
                    .all(|(k, v)| admission::family(k) && (1..=PRECISION_MAX_COUNT).contains(v)),
            HistoryRecordError::Binding,
        )?;
        let mut allocations = BTreeMap::new();
        let mut reservations = BTreeMap::new();
        let mut conflicts = BTreeMap::new();
        let mut sources = BTreeMap::new();
        for source in &c.registered_sources {
            validate_registered_source(source).map_err(|_| HistoryRecordError::Binding)?;
            let key = serde_json::to_value(&source.source)
                .map_err(|_| HistoryRecordError::Binding)?
                .to_string();
            require(
                sources.insert(key, source).is_none(),
                HistoryRecordError::Identity,
            )?;
        }
        for a in &c.allocations {
            require(admission::family(&a.family_id), HistoryRecordError::Binding)?;
            parse_alpha_allocation(&a.declaration.document())
                .map_err(|_| HistoryRecordError::Binding)?;
            let id =
                alpha_allocation_id(&a.declaration).map_err(|_| HistoryRecordError::Binding)?;
            require(
                allocations
                    .insert(
                        BudgetIdentity {
                            family_id: a.family_id.clone(),
                            allocation_id: id,
                        },
                        &a.declaration,
                    )
                    .is_none(),
                HistoryRecordError::Identity,
            )?;
        }
        for a in &c.reservations {
            require(
                allocations.contains_key(&BudgetIdentity {
                    family_id: a.family_id().into(),
                    allocation_id: a.allocation_id().into(),
                }),
                HistoryRecordError::Binding,
            )?;
            require(
                reservations.insert(a.reservation_id().into(), a).is_none(),
                HistoryRecordError::Identity,
            )?;
        }
        for a in &c.alpha_conflicts {
            require(
                hex(&a.id, 64)
                    && !a.family_ids.is_empty()
                    && a.family_ids.len() <= 2
                    && ordered(&a.family_ids)
                    && a.family_ids.iter().all(|v| admission::family(v)),
                HistoryRecordError::Binding,
            )?;
            require(
                conflicts.insert(a.id.clone(), a).is_none(),
                HistoryRecordError::Identity,
            )?;
        }
        let mut prefixes = BTreeSet::new();
        for p in &c.historical_prefixes {
            require(
                hex(&p.registry_id, 32)
                    && hex(&p.chain_head, 64)
                    && p.seq <= PRECISION_MAX_COUNT
                    && (p.seq != 0 || p.chain_head == genesis(&p.registry_id))
                    && prefixes.insert(p.clone()),
                HistoryRecordError::Binding,
            )?;
        }
        let mut trials = BTreeMap::new();
        for t in &c.historical_trials {
            require(
                prefixes.contains(&HistoricalPrefix {
                    registry_id: t.registry_id.clone(),
                    seq: t.seq,
                    chain_head: t.chain_head.clone(),
                }) && admission::family(&t.family_id)
                    && t.trial_batch_id
                        .strip_prefix("trial-batch-v1:")
                        .is_some_and(|v| hex(v, 64))
                    && t.family_effective_trials <= PRECISION_MAX_COUNT
                    && t.family_effective_trials >= t.registered_batch_effective_trials
                    && t.registered_batch_effective_trials >= t.complete_candidates
                    && (1..=4096).contains(&t.complete_candidates)
                    && t.tests_per_trial == 2,
                HistoryRecordError::Binding,
            )?;
            let prefix = HistoricalPrefix {
                registry_id: t.registry_id.clone(),
                seq: t.seq,
                chain_head: t.chain_head.clone(),
            };
            require(
                trials
                    .insert((prefix, t.trial_batch_id.clone(), t.family_id.clone()), t)
                    .is_none(),
                HistoryRecordError::Identity,
            )?;
        }
        Ok(Self {
            context: c,
            sources,
            prefixes,
            trials,
            allocations,
            reservations,
            conflicts,
        })
    }
    fn binding(&self, b: &BindingObservation) -> Result<(), HistoryRecordError> {
        let c = self.context;
        require(
            self.prefixes.contains(&HistoricalPrefix {
                registry_id: b.registry_id.clone(),
                seq: b.seq,
                chain_head: b.chain_head.clone(),
            }) && b
                .evidence
                .quarantined_families
                .iter()
                .all(|v| c.quarantined_families.binary_search(v).is_ok())
                && b.evidence
                    .conflicted_origins
                    .iter()
                    .all(|v| c.conflicted_origins.binary_search(v).is_ok())
                && b.evidence
                    .family_tests
                    .iter()
                    .all(|(k, v)| c.family_tests.get(k).is_some_and(|n| n >= v))
                && b.alpha
                    .budgets
                    .iter()
                    .all(|v| self.allocations.contains_key(v))
                && b.alpha
                    .reservations
                    .iter()
                    .all(|v| self.reservations.contains_key(v))
                && b.alpha
                    .conflicts
                    .iter()
                    .all(|v| self.conflicts.contains_key(v))
                && b.alpha_unknown_origins
                    .iter()
                    .all(|v| c.alpha_unknown_origins.binary_search(v).is_ok()),
            HistoryRecordError::Dependency,
        )
    }
    fn reservation(&self, id: &str) -> Result<&'a AlphaReservation, HistoryRecordError> {
        self.reservations
            .get(id)
            .copied()
            .ok_or(HistoryRecordError::Dependency)
    }
}
#[derive(Default)]
struct Checked {
    batches: BTreeMap<String, FrozenBatch>,
    selections: BTreeMap<String, FrozenSelection>,
    operations: BTreeMap<String, FrozenOperation>,
    exposures: BTreeMap<String, ExposureRecord>,
    admissions: BTreeMap<String, AdmissionRecord>,
    events: BTreeMap<String, EventRecord>,
    conflicts: BTreeMap<String, ConflictDeclaration>,
    blobs: BTreeMap<String, HistoryBlob>,
}
fn get<'a, T>(map: &'a BTreeMap<String, T>, id: &str) -> Result<&'a T, HistoryRecordError> {
    map.get(id).ok_or(HistoryRecordError::Dependency)
}
impl Checked {
    fn operation_batch(
        &self,
        op: &str,
    ) -> Result<(&FrozenOperation, &FrozenBatch), HistoryRecordError> {
        let op = get(&self.operations, op)?;
        Ok((op, get(&self.batches, op.confirmation_batch_id())?))
    }
    fn check(&mut self, k: &Key, n: &Node, h: &Host<'_>) -> Result<(), HistoryRecordError> {
        let v = &n.body;
        let id = &k.1;
        match k.0 {
            Kind::Batch => {
                let stage = v.get("stage").ok_or(HistoryRecordError::Dependency)?;
                let b = if field(stage, "kind")? == "validation" {
                    let source = v.get("source").ok_or(HistoryRecordError::Dependency)?;
                    let source = h
                        .sources
                        .get(&source.to_string())
                        .ok_or(HistoryRecordError::Dependency)?;
                    parse_validation_batch(&n.raw, source)
                } else {
                    parse_test_batch(
                        &n.raw,
                        get(&self.batches, field(stage, "validationBatchId")?)?,
                        get(&self.selections, field(stage, "selectionId")?)?,
                    )
                }
                .map_err(|_| HistoryRecordError::Binding)?;
                self.batches.insert(id.clone(), b);
            }
            Kind::Operation => {
                let op = parse_operation(
                    &n.raw,
                    get(&self.batches, field(v, "confirmationBatchId")?)?,
                )
                .map_err(|_| HistoryRecordError::Binding)?;
                self.operations.insert(id.clone(), op);
            }
            Kind::Exposure => {
                let claim = v.get("claim").ok_or(HistoryRecordError::Dependency)?;
                let e = if field(claim, "kind")? == "confirmation" {
                    let (op, b) = self.operation_batch(field(claim, "operationId")?)?;
                    parse_confirmation_exposure(
                        &n.raw,
                        b,
                        op,
                        h.reservation(field(claim, "alphaReservationId")?)?,
                    )?
                } else {
                    parse_manual_exposure(&n.raw)?
                };
                if let ExposureClaim::Manual {
                    after_seal_id: Some(seal),
                    ..
                } = &e.body.claim
                {
                    let s = get(&self.events, seal)?;
                    require(
                        matches!(s.body.event, ExecutionEvent::Sealed { .. }),
                        HistoryRecordError::Dependency,
                    )?;
                    let a = get(&self.admissions, &s.body.admission_id)?;
                    let support = get(&self.exposures, &a.body.exposure_id)?;
                    require(
                        support.body.instrument_id == e.body.instrument_id
                            && e.body.intervals.iter().all(|v| {
                                support.body.intervals.iter().any(|s| {
                                    s.from_ms() <= v.from_ms()
                                        && s.to_exclusive_ms() >= v.to_exclusive_ms()
                                })
                            }),
                        HistoryRecordError::Binding,
                    )?;
                }
                self.exposures.insert(id.clone(), e);
            }
            Kind::Admission => {
                let (op, b) = self.operation_batch(field(v, "operationId")?)?;
                let r = h.reservation(field(v, "alphaReservationId")?)?;
                let observation = v
                    .get("registryObservation")
                    .ok_or(HistoryRecordError::Dependency)?;
                let blob = get(&self.blobs, field(observation, "bindingSha256")?)?;
                let binding = blob.binding.as_ref().ok_or(HistoryRecordError::Blob)?;
                h.binding(binding)?;
                let prefix = HistoricalPrefix {
                    registry_id: binding.registry_id.clone(),
                    seq: binding.seq,
                    chain_head: binding.chain_head.clone(),
                };
                let trial = h
                    .trials
                    .get(&(
                        prefix,
                        format!("trial-batch-v1:{}", b.source().trial_batch_id),
                        b.source().family_id.clone(),
                    ))
                    .ok_or(HistoryRecordError::Dependency)?;
                let source_key = serde_json::to_value(b.source())
                    .map_err(|_| HistoryRecordError::Binding)?
                    .to_string();
                let source = h
                    .sources
                    .get(&source_key)
                    .ok_or(HistoryRecordError::Dependency)?;
                require(
                    source.candidates.len() as u64 == trial.complete_candidates,
                    HistoryRecordError::Binding,
                )?;
                let allocation = h
                    .allocations
                    .get(&BudgetIdentity {
                        family_id: b.source().family_id.clone(),
                        allocation_id: b.allocation_id().into(),
                    })
                    .ok_or(HistoryRecordError::Dependency)?;
                // A pre-state conflicting allocation/reservation is not a successful admission.
                require(
                    !binding
                        .alpha
                        .conflicts
                        .iter()
                        .any(|id| h.conflicts[id].family_ids.contains(&b.source().family_id)),
                    HistoryRecordError::Binding,
                )?;
                let a = parse_admission(
                    &n.raw,
                    &AdmissionInputs {
                        batch: b,
                        operation: op,
                        reservation: r,
                        exposure: get(&self.exposures, field(v, "exposureId")?)?,
                        binding: blob,
                        trial,
                        allocation,
                    },
                )?;
                self.admissions.insert(id.clone(), a);
            }
            Kind::Event => {
                let (op, b) = self.operation_batch(field(v, "operationId")?)?;
                let e = parse_confirmation_event(&n.raw, b, op)?;
                let a = get(&self.admissions, &e.body.admission_id)?;
                require(a.body.operation_id == op.id(), HistoryRecordError::Binding)?;
                match &e.body.event {
                    ExecutionEvent::ExecutionStarted {
                        workspace_id,
                        ownership_epoch,
                        ..
                    } => require(
                        workspace_id == &a.body.workspace_observation.workspace_id
                            && *ownership_epoch == a.body.workspace_observation.ownership_epoch,
                        HistoryRecordError::Binding,
                    )?,
                    ExecutionEvent::Sealed {
                        execution_started_id,
                        report_sha256,
                        ..
                    } => {
                        self.started(execution_started_id, &e)?;
                        let report = get(&self.blobs, report_sha256)?;
                        require(
                            report.kind() == BlobKind::Statistics,
                            HistoryRecordError::Blob,
                        )?;
                        verify_confirmation_report(
                            report.text(),
                            b,
                            h.reservation(&a.body.alpha_reservation_id)?,
                            a.family_tests(),
                        )
                        .map_err(|_| HistoryRecordError::Proof)?;
                    }
                    ExecutionEvent::InterruptedSpent {
                        execution_started_id,
                        ..
                    }
                    | ExecutionEvent::FailedSpent {
                        execution_started_id,
                        ..
                    }
                    | ExecutionEvent::CancelledSpent {
                        execution_started_id,
                        ..
                    } => {
                        if let Some(start) = execution_started_id {
                            self.started(start, &e)?;
                        }
                    }
                }
                self.events.insert(id.clone(), e);
            }
            Kind::Selection => {
                let s = get(&self.events, field(v, "sealedReceiptId")?)?;
                let ExecutionEvent::Sealed { report_sha256, .. } = &s.body.event else {
                    return Err(HistoryRecordError::Dependency);
                };
                let a = get(&self.admissions, &s.body.admission_id)?;
                require(
                    a.body.alpha_reservation_id == field(v, "alphaReservationId")?,
                    HistoryRecordError::Binding,
                )?;
                let (_, b) = self.operation_batch(s.operation_id())?;
                let selection = parse_selection(
                    &n.raw,
                    b,
                    &SealedValidationInput {
                        receipt_id: s.id(),
                        reservation: h.reservation(&a.body.alpha_reservation_id)?,
                        report_json: get(&self.blobs, report_sha256)?.text(),
                    },
                )
                .map_err(|_| HistoryRecordError::Binding)?;
                self.selections.insert(id.clone(), selection);
            }
            Kind::Conflict => {
                let c = parse_conflict_declaration(&n.raw)?;
                self.verify_conflict(&c)?;
                self.conflicts.insert(id.clone(), c);
            }
            Kind::Blob => {}
        }
        Ok(())
    }
    fn started(&self, id: &str, e: &EventRecord) -> Result<(), HistoryRecordError> {
        let s = get(&self.events, id)?;
        require(
            matches!(s.body.event, ExecutionEvent::ExecutionStarted { .. })
                && s.body.operation_id == e.body.operation_id
                && s.body.admission_id == e.body.admission_id,
            HistoryRecordError::Binding,
        )
    }
}

fn nodes(i: &ConfirmationInventory) -> Result<BTreeMap<Key, Node>, HistoryRecordError> {
    let mut nodes = BTreeMap::new();
    for (kind, section) in [
        Kind::Batch,
        Kind::Selection,
        Kind::Operation,
        Kind::Exposure,
        Kind::Admission,
        Kind::Event,
        Kind::Conflict,
    ]
    .into_iter()
    .zip(i.sections())
    {
        for raw in section {
            let stored: Stored<Value> = read(raw)?;
            reference(&stored.id, 64)?;
            require(
                nodes
                    .insert(
                        key(kind, &stored.id),
                        Node {
                            raw: raw.clone(),
                            body: stored.body,
                            deps: BTreeSet::new(),
                        },
                    )
                    .is_none(),
                HistoryRecordError::Identity,
            )?;
        }
    }
    for blob in &i.blobs {
        require(
            nodes
                .insert(
                    key(Kind::Blob, blob.sha256()),
                    Node {
                        raw: String::new(),
                        body: Value::Null,
                        deps: BTreeSet::new(),
                    },
                )
                .is_none(),
            HistoryRecordError::Identity,
        )?;
    }
    let claims: BTreeSet<Key> = nodes.keys().cloned().collect();
    let blobs: BTreeMap<_, _> = i.blobs.iter().map(|b| (b.sha256(), b)).collect();
    for (k, n) in &mut nodes {
        let v = &n.body;
        let d = &mut n.deps;
        match k.0 {
            Kind::Batch => {
                let stage = v.get("stage").ok_or(HistoryRecordError::Dependency)?;
                if field(stage, "kind")? == "test" {
                    dep(d, Kind::Batch, stage, "validationBatchId")?;
                    dep(d, Kind::Selection, stage, "selectionId")?;
                }
            }
            Kind::Operation => dep(d, Kind::Batch, v, "confirmationBatchId")?,
            Kind::Exposure => {
                let c = v.get("claim").ok_or(HistoryRecordError::Dependency)?;
                if field(c, "kind")? == "confirmation" {
                    dep(d, Kind::Operation, c, "operationId")?;
                } else if let Some(id) = c.get("afterSealId").and_then(Value::as_str) {
                    d.insert(key(Kind::Event, id));
                }
            }
            Kind::Admission => {
                dep(d, Kind::Operation, v, "operationId")?;
                dep(d, Kind::Exposure, v, "exposureId")?;
                dep(
                    d,
                    Kind::Blob,
                    v.get("registryObservation")
                        .ok_or(HistoryRecordError::Dependency)?,
                    "bindingSha256",
                )?;
            }
            Kind::Event => {
                dep(d, Kind::Operation, v, "operationId")?;
                dep(d, Kind::Admission, v, "admissionId")?;
                let event = v.get("event").ok_or(HistoryRecordError::Dependency)?;
                if let Some(id) = event.get("executionStartedId").and_then(Value::as_str) {
                    d.insert(key(Kind::Event, id));
                }
                if field(event, "kind")? == "sealed" {
                    dep(d, Kind::Blob, event, "reportSha256")?;
                }
            }
            Kind::Selection => {
                dep(d, Kind::Batch, v, "validationBatchId")?;
                dep(d, Kind::Event, v, "sealedReceiptId")?;
                dep(d, Kind::Blob, v, "reportSha256")?;
            }
            Kind::Conflict => {
                for id in v
                    .get("claimIds")
                    .and_then(Value::as_array)
                    .ok_or(HistoryRecordError::Dependency)?
                {
                    let id = id.as_str().ok_or(HistoryRecordError::Dependency)?;
                    let matches: Vec<_> = [
                        Kind::Operation,
                        Kind::Exposure,
                        Kind::Admission,
                        Kind::Event,
                    ]
                    .into_iter()
                    .map(|k| key(k, id))
                    .filter(|k| claims.contains(k))
                    .collect();
                    require(matches.len() == 1, HistoryRecordError::Dependency)?;
                    d.insert(matches[0].clone());
                }
            }
            Kind::Blob => {
                let blob = blobs.get(k.1.as_str()).ok_or(HistoryRecordError::Blob)?;
                if let Some(b) = &blob.binding {
                    for (section, kind) in b.confirmation.sections().into_iter().zip([
                        Kind::Batch,
                        Kind::Selection,
                        Kind::Operation,
                        Kind::Exposure,
                        Kind::Admission,
                        Kind::Event,
                        Kind::Blob,
                        Kind::Conflict,
                    ]) {
                        for id in section {
                            d.insert(key(kind, id));
                        }
                    }
                    require(
                        b.confirmation
                            .unknown_origins
                            .iter()
                            .all(|v| i.unknown_origins.binary_search(v).is_ok()),
                        HistoryRecordError::Dependency,
                    )?;
                }
            }
        }
    }
    Ok(nodes)
}

fn run(
    i: &ConfirmationInventory,
    c: &GraphContext,
    materialized: bool,
) -> Result<ValidatedGraph, HistoryRecordError> {
    i.capacity()?;
    let h = Host::new(c)?;
    let nodes = nodes(i)?;
    let order = topological(&nodes)?;
    let mut checked = Checked::default();
    for blob in &i.blobs {
        checked.blobs.insert(blob.sha256().into(), blob.clone());
        if let Some(b) = &blob.binding {
            h.binding(b)?;
        }
    }
    for k in order {
        checked.check(&k, &nodes[&k], &h)?;
    }
    require(
        checked
            .exposures
            .values()
            .map(|e| e.body.intervals.len())
            .sum::<usize>()
            <= MAX_HISTORY_INTERVALS,
        HistoryRecordError::Capacity,
    )?;
    let admitted: BTreeSet<_> = checked
        .admissions
        .values()
        .map(|a| a.body.exposure_id.as_str())
        .collect();
    for e in checked
        .exposures
        .values()
        .filter(|e| e.operation_id().is_some())
    {
        require(admitted.contains(e.id()), HistoryRecordError::Dependency)?;
    }
    let used: BTreeSet<_> = nodes
        .iter()
        .filter(|(k, _)| matches!(k.0, Kind::Admission | Kind::Event))
        .flat_map(|(_, n)| n.deps.iter())
        .filter(|k| k.0 == Kind::Blob)
        .map(|k| k.1.clone())
        .collect();
    require(used.len() == i.blobs.len(), HistoryRecordError::Dependency)?;
    let required = checked.required_conflicts()?;
    let mut inventory = i.clone();
    for conflict in required.into_values() {
        if !checked.conflicts.contains_key(conflict.id()) {
            require(!materialized, HistoryRecordError::Conflict)?;
            inventory.conflicts.push(
                serde_json::to_string(&conflict.document())
                    .map_err(|_| HistoryRecordError::Malformed)?,
            );
        }
    }
    inventory.capacity()?;
    for section in [
        &mut inventory.batches,
        &mut inventory.selections,
        &mut inventory.operations,
        &mut inventory.exposures,
        &mut inventory.admissions,
        &mut inventory.events,
        &mut inventory.conflicts,
    ] {
        section.sort_by_cached_key(|raw| {
            serde_json::from_str::<Stored<Value>>(raw)
                .expect("checked record")
                .id
        });
    }
    inventory.blobs.sort_by(|a, b| a.sha256().cmp(b.sha256()));
    let restricted = !inventory.conflicts.is_empty()
        || !inventory.unknown_origins.is_empty()
        || !c.alpha_conflicts.is_empty()
        || !c.alpha_unknown_origins.is_empty()
        || !c.quarantined_families.is_empty()
        || !c.conflicted_origins.is_empty();
    Ok(ValidatedGraph {
        inventory,
        restricted,
    })
}
pub fn complete_confirmation_graph(
    i: &ConfirmationInventory,
    c: &GraphContext,
) -> Result<ValidatedGraph, HistoryRecordError> {
    run(i, c, false)
}
pub fn validate_confirmation_graph(
    i: &ConfirmationInventory,
    c: &GraphContext,
) -> Result<ValidatedGraph, HistoryRecordError> {
    run(i, c, true)
}

impl Checked {
    fn exemption(&self, e: &ExposureRecord) -> Option<&str> {
        if let ExposureClaim::Manual {
            after_seal_id: Some(id),
            ..
        } = &e.body.claim
        {
            let seal = self.events.get(id)?;
            Some(
                &self
                    .admissions
                    .get(&seal.body.admission_id)?
                    .body
                    .exposure_id,
            )
        } else {
            None
        }
    }
    fn overlap_kind(&self, a: &ExposureRecord, b: &ExposureRecord) -> Option<ConflictKind> {
        if !footprints_overlap(&a.footprint(), &b.footprint()) {
            return None;
        }
        match (a.operation_id(), b.operation_id()) {
            (Some(x), Some(y)) if x != y => Some(ConflictKind::ExposureOverlap),
            (Some(_), None) if self.exemption(b) != Some(a.id()) => {
                Some(ConflictKind::ManualOverlap)
            }
            (None, Some(_)) if self.exemption(a) != Some(b.id()) => {
                Some(ConflictKind::ManualOverlap)
            }
            _ => None,
        }
    }
    fn origin_key(&self, id: &str) -> Result<(&str, &str), HistoryRecordError> {
        if let Some(o) = self.operations.get(id) {
            return Ok((o.origin_registry_id(), o.reservation_key()));
        }
        let e = get(&self.exposures, id)?;
        require(e.operation_id().is_none(), HistoryRecordError::Conflict)?;
        Ok((e.origin_registry_id(), e.reservation_key()))
    }
    fn claim_operation(&self, id: &str) -> Result<&str, HistoryRecordError> {
        if let Some(a) = self.admissions.get(id) {
            Ok(&a.body.operation_id)
        } else if let Some(e) = self.events.get(id) {
            Ok(&e.body.operation_id)
        } else {
            Err(HistoryRecordError::Conflict)
        }
    }
    fn targets(&self, id: &str) -> Result<(Option<String>, String), HistoryRecordError> {
        if let Some(e) = self.exposures.get(id) {
            return Ok((
                if let Some(op) = e.operation_id() {
                    Some(self.operation_batch(op)?.1.source().family_id.clone())
                } else {
                    None
                },
                e.body.instrument_id.clone(),
            ));
        }
        let op = if self.operations.contains_key(id) {
            id
        } else {
            self.claim_operation(id)?
        };
        let b = self.operation_batch(op)?.1;
        Ok((
            Some(b.source().family_id.clone()),
            b.source().instrument_id.clone(),
        ))
    }
    fn conflict(
        &self,
        kind: ConflictKind,
        a: &str,
        b: &str,
    ) -> Result<ConflictDeclaration, HistoryRecordError> {
        let mut claims = vec![a.to_string(), b.to_string()];
        claims.sort();
        let targets = [self.targets(a)?, self.targets(b)?];
        checked_conflict(ConflictBody {
            contract_version: CONFLICT_VERSION.into(),
            kind,
            claim_ids: claims,
            affected_family_ids: targets
                .iter()
                .filter_map(|t| t.0.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            affected_instrument_ids: targets
                .iter()
                .map(|t| t.1.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
    }
    fn verify_conflict(&self, c: &ConflictDeclaration) -> Result<(), HistoryRecordError> {
        let (a, b) = (&c.body.claim_ids[0], &c.body.claim_ids[1]);
        let relation = match c.body.kind {
            ConflictKind::OriginKey => self.origin_key(a)? == self.origin_key(b)?,
            ConflictKind::Admission => {
                get(&self.admissions, a)?.body.operation_id
                    == get(&self.admissions, b)?.body.operation_id
            }
            ConflictKind::ExposureOverlap | ConflictKind::ManualOverlap => {
                self.overlap_kind(get(&self.exposures, a)?, get(&self.exposures, b)?)
                    == Some(c.body.kind)
            }
            ConflictKind::ExecutionStarted | ConflictKind::Seal | ConflictKind::Terminal => {
                let (a, b) = (get(&self.events, a)?, get(&self.events, b)?);
                a.operation_id() == b.operation_id()
                    && match c.body.kind {
                        ConflictKind::ExecutionStarted => is_start(a) && is_start(b),
                        ConflictKind::Seal => is_seal(a) && is_seal(b),
                        _ => {
                            (is_terminal(a) && is_terminal(b))
                                || (is_terminal(a) && is_seal(b))
                                || (is_terminal(b) && is_seal(a))
                                || (is_unstarted_terminal(a) && is_start(b))
                                || (is_unstarted_terminal(b) && is_start(a))
                        }
                    }
            }
        };
        require(
            relation && self.conflict(c.body.kind, a, b)? == *c,
            HistoryRecordError::Conflict,
        )
    }
    fn required_conflicts(
        &self,
    ) -> Result<BTreeMap<String, ConflictDeclaration>, HistoryRecordError> {
        let mut out = BTreeMap::new();
        let mut add = |kind, a: &str, b: &str| -> Result<(), HistoryRecordError> {
            let c = self.conflict(kind, a, b)?;
            out.insert(c.id.clone(), c);
            require(out.len() <= MAX_HISTORY_ROWS, HistoryRecordError::Capacity)
        };
        let mut keys: BTreeMap<(&str, &str), Vec<&str>> = BTreeMap::new();
        for o in self.operations.values() {
            keys.entry((o.origin_registry_id(), o.reservation_key()))
                .or_default()
                .push(o.id());
        }
        for e in self
            .exposures
            .values()
            .filter(|e| e.operation_id().is_none())
        {
            keys.entry((e.origin_registry_id(), e.reservation_key()))
                .or_default()
                .push(e.id());
        }
        for group in keys.values_mut() {
            group.sort();
            for id in group.iter().skip(1) {
                add(ConflictKind::OriginKey, group[0], id)?;
            }
        }
        let mut admissions: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for a in self.admissions.values() {
            admissions
                .entry(&a.body.operation_id)
                .or_default()
                .push(a.id());
        }
        for group in admissions.values() {
            for id in group.iter().skip(1) {
                add(ConflictKind::Admission, group[0], id)?;
            }
        }
        let mut events: BTreeMap<&str, Vec<&EventRecord>> = BTreeMap::new();
        for e in self.events.values() {
            events.entry(e.operation_id()).or_default().push(e);
        }
        for group in events.values() {
            for (kind, filter) in [
                (
                    ConflictKind::ExecutionStarted,
                    is_start as fn(&EventRecord) -> bool,
                ),
                (ConflictKind::Seal, is_seal),
                (ConflictKind::Terminal, is_terminal),
            ] {
                let selected: Vec<_> = group.iter().filter(|e| filter(e)).collect();
                if let Some(first) = selected.first() {
                    for other in selected.iter().skip(1) {
                        add(kind, first.id(), other.id())?;
                    }
                }
            }
            let terminals: Vec<_> = group.iter().filter(|e| is_terminal(e)).collect();
            let seals: Vec<_> = group.iter().filter(|e| is_seal(e)).collect();
            // Two stars cover all claims without a quadratic pair expansion.
            if let (Some(t), Some(s)) = (terminals.first(), seals.first()) {
                for x in &seals {
                    add(ConflictKind::Terminal, t.id(), x.id())?;
                }
                for x in &terminals {
                    add(ConflictKind::Terminal, s.id(), x.id())?;
                }
            }
            let unstarted: Vec<_> = terminals
                .iter()
                .filter(|e| is_unstarted_terminal(e))
                .collect();
            let starts: Vec<_> = group.iter().filter(|e| is_start(e)).collect();
            if let (Some(t), Some(s)) = (unstarted.first(), starts.first()) {
                for x in &starts {
                    add(ConflictKind::Terminal, t.id(), x.id())?;
                }
                for x in &unstarted {
                    add(ConflictKind::Terminal, s.id(), x.id())?;
                }
            }
        }
        // Intersections are not transitive. Every edge below has two intervals
        // simultaneously active; an A/B/C chain never invents A/C evidence.
        let mut intervals: BTreeMap<&str, Vec<(u64, u64, &str)>> = BTreeMap::new();
        for e in self.exposures.values() {
            for v in &e.body.intervals {
                intervals.entry(&e.body.instrument_id).or_default().push((
                    v.from_ms(),
                    v.to_exclusive_ms(),
                    e.id(),
                ));
            }
        }
        for rows in intervals.values_mut() {
            rows.sort();
            let mut confirm = Active::default();
            let mut manual = Active::default();
            let mut expiry: BTreeSet<(u64, &str, bool, String)> = BTreeSet::new();
            for &(from, to, id) in rows.iter() {
                while expiry.first().is_some_and(|v| v.0 <= from) {
                    let (_, id, is_confirm, group) = expiry.pop_first().expect("active expiry");
                    if is_confirm {
                        confirm.remove(&group, id);
                    } else {
                        manual.remove(&group, id);
                    }
                }
                let e = &self.exposures[id];
                let group;
                if let Some(op) = e.operation_id() {
                    if let Some(other) = confirm.excluding_group(op) {
                        add(ConflictKind::ExposureOverlap, id, &other)?;
                    }
                    if let Some(other) = manual.excluding_group(id) {
                        add(ConflictKind::ManualOverlap, id, &other)?;
                    }
                    group = op.to_string();
                    confirm.insert(&group, id);
                } else {
                    let exempt = self.exemption(e).unwrap_or("");
                    if let Some(other) = confirm.excluding_member(exempt) {
                        add(ConflictKind::ManualOverlap, id, &other)?;
                    }
                    group = exempt.to_string();
                    manual.insert(&group, id);
                }
                expiry.insert((to, id, e.operation_id().is_some(), group));
            }
        }
        Ok(out)
    }
}
fn is_start(e: &EventRecord) -> bool {
    matches!(e.body.event, ExecutionEvent::ExecutionStarted { .. })
}
fn is_seal(e: &EventRecord) -> bool {
    matches!(e.body.event, ExecutionEvent::Sealed { .. })
}
fn is_terminal(e: &EventRecord) -> bool {
    !is_start(e) && !is_seal(e)
}
fn is_unstarted_terminal(e: &EventRecord) -> bool {
    matches!(
        e.body.event,
        ExecutionEvent::InterruptedSpent {
            execution_started_id: None,
            ..
        } | ExecutionEvent::FailedSpent {
            execution_started_id: None,
            ..
        } | ExecutionEvent::CancelledSpent {
            execution_started_id: None,
            ..
        }
    )
}
/// Group representatives keep sweep work logarithmic even when thousands of
/// overlapping records share one operation or one manual exemption.
#[derive(Default)]
struct Active {
    groups: BTreeMap<String, BTreeSet<String>>,
    first: BTreeSet<(String, String)>,
}
impl Active {
    fn insert(&mut self, g: &str, id: &str) {
        let members = self.groups.entry(g.into()).or_default();
        if let Some(old) = members.first() {
            self.first.remove(&(old.clone(), g.into()));
        }
        members.insert(id.into());
        self.first
            .insert((members.first().expect("inserted member").clone(), g.into()));
    }
    fn remove(&mut self, g: &str, id: &str) {
        if let Some(members) = self.groups.get_mut(g) {
            if let Some(old) = members.first() {
                self.first.remove(&(old.clone(), g.into()));
            }
            members.remove(id);
            if let Some(next) = members.first() {
                self.first.insert((next.clone(), g.into()));
            } else {
                self.groups.remove(g);
            }
        }
    }
    fn excluding_group(&self, g: &str) -> Option<String> {
        self.first
            .iter()
            .take(2)
            .find(|(_, group)| group != g)
            .map(|(id, _)| id.clone())
    }
    fn excluding_member(&self, id: &str) -> Option<String> {
        let (first, group) = self.first.first()?;
        if first != id {
            return Some(first.clone());
        }
        self.groups[group]
            .iter()
            .nth(1)
            .cloned()
            .into_iter()
            .chain(self.first.iter().nth(1).map(|(id, _)| id.clone()))
            .min()
    }
}

#[cfg(test)]
mod tests;
