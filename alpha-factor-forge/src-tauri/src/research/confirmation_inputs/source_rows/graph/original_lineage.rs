//! Bounded preparation and whole-family historical lineage agreement.
//! The graph stays borrowed; only the declared JSON and one run plan materialize.

use alpha_factor_forge::discovery_core::{
    config::ResolvedDiscoveryConfig, embargo::EmbargoDerivation, enumerate::CandidatePlan,
};
use serde_json::Value;

use super::super::selection::{materialize, prepare_run_plan, PreparedRunPlan};
use super::{OriginalJson, SourceRowsObservation, MAX_TOTAL_JSON_NODES, MAX_TOTAL_TEXT_BYTES};
use crate::error::{AppError, AppResult};

mod semantics;
mod warmup;

fn invalid() -> AppError {
    AppError::Other("original_lineage_invalid".into())
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Work {
    bytes: usize,
    nodes: usize,
}

impl Work {
    fn add(&mut self, weight: usize, bytes: usize, nodes: usize) -> AppResult<()> {
        let bytes = weight
            .checked_mul(bytes)
            .and_then(|n| self.bytes.checked_add(n))
            .ok_or_else(invalid)?;
        let nodes = weight
            .checked_mul(nodes)
            .and_then(|n| self.nodes.checked_add(n))
            .ok_or_else(invalid)?;
        if bytes > MAX_TOTAL_TEXT_BYTES || nodes > MAX_TOTAL_JSON_NODES {
            return Err(invalid());
        }
        *self = Self { bytes, nodes };
        Ok(())
    }

    fn document(&mut self, value: &OriginalJson, weight: usize) -> AppResult<()> {
        self.add(weight, value.as_bytes().len(), value.node_count())
    }
}

/// Counts all future stored-document parses before any Value materialization.
fn parse_work(rows: &SourceRowsObservation) -> AppResult<Work> {
    if rows.attempts.is_empty()
        || rows.attempts.len() > 4096
        || rows.strategies.is_empty()
        || rows.strategies.len() > 4096
        || rows.hypotheses.is_empty()
        || rows.hypotheses.len() > 4096
    {
        return Err(invalid());
    }
    let mut work = Work { bytes: 0, nodes: 0 };
    work.document(&rows.roots.run.config_json, 2)?;
    for row in &rows.strategies {
        work.document(&row.original_definition_json, 1)?;
        if let Some(dsl) = &row.dsl_json {
            work.document(dsl, 1)?;
        }
    }
    for row in &rows.attempts {
        work.document(&row.input_fingerprint_json, 1)?;
        work.document(&row.engine_fingerprint_json, 1)?;
    }
    for row in &rows.hypotheses {
        work.document(&row.content_json, 1)?;
        work.document(&row.applicability_json, 1)?;
    }
    Ok(work)
}

pub(in crate::research::confirmation_inputs) struct StrategyDocuments {
    definition: Value,
    dsl: Option<Value>,
}

impl StrategyDocuments {
    pub(in crate::research::confirmation_inputs) fn definition(&self) -> &Value {
        &self.definition
    }
    pub(in crate::research::confirmation_inputs) fn dsl(&self) -> Option<&Value> {
        self.dsl.as_ref()
    }
}

pub(in crate::research::confirmation_inputs) struct AttemptDocuments {
    input: Value,
    engine: Value,
}

impl AttemptDocuments {
    pub(in crate::research::confirmation_inputs) fn input(&self) -> &Value {
        &self.input
    }
    pub(in crate::research::confirmation_inputs) fn engine(&self) -> &Value {
        &self.engine
    }
}

pub(in crate::research::confirmation_inputs) struct HypothesisDocuments {
    content: Value,
    applicability: Value,
}

impl HypothesisDocuments {
    pub(in crate::research::confirmation_inputs) fn content(&self) -> &Value {
        &self.content
    }
    pub(in crate::research::confirmation_inputs) fn applicability(&self) -> &Value {
        &self.applicability
    }
}

/// Transient parsed claims with no constructor besides the sealed-graph entry.
pub(in crate::research::confirmation_inputs) struct PreparedOriginalLineage<'a> {
    rows: &'a SourceRowsObservation,
    run: PreparedRunPlan,
    strategies: Vec<StrategyDocuments>,
    attempts: Vec<AttemptDocuments>,
    hypotheses: Vec<HypothesisDocuments>,
    parse: Work,
    combined: Work,
}

impl PreparedOriginalLineage<'_> {
    pub(in crate::research::confirmation_inputs) fn rows(&self) -> &SourceRowsObservation {
        self.rows
    }
    pub(in crate::research::confirmation_inputs) fn raw_config(&self) -> &Value {
        self.run.raw_config()
    }
    pub(in crate::research::confirmation_inputs) fn config(&self) -> &ResolvedDiscoveryConfig {
        self.run.config()
    }
    pub(in crate::research::confirmation_inputs) fn plan(&self) -> &CandidatePlan {
        self.run.plan()
    }
    pub(in crate::research::confirmation_inputs) fn strategies(&self) -> &[StrategyDocuments] {
        &self.strategies
    }
    pub(in crate::research::confirmation_inputs) fn attempts(&self) -> &[AttemptDocuments] {
        &self.attempts
    }
    pub(in crate::research::confirmation_inputs) fn hypotheses(&self) -> &[HypothesisDocuments] {
        &self.hypotheses
    }
    pub(in crate::research::confirmation_inputs) fn parse_work(&self) -> (usize, usize) {
        (self.parse.bytes, self.parse.nodes)
    }
    pub(in crate::research::confirmation_inputs) fn expansion_work(&self) -> (usize, usize) {
        self.run.expansion()
    }
    pub(in crate::research::confirmation_inputs) fn combined_work(&self) -> (usize, usize) {
        (self.combined.bytes, self.combined.nodes)
    }
}

pub(in crate::research::confirmation_inputs) fn prepare_original_lineage(
    rows: &SourceRowsObservation,
) -> AppResult<PreparedOriginalLineage<'_>> {
    let parse = parse_work(rows)?;
    let run = prepare_run_plan(
        &rows.roots.run.config_json,
        &rows.dataset.dataset_hash,
        parse.bytes,
        parse.nodes,
    )
    .map_err(|_| invalid())?;
    let (bytes, nodes) = run.expansion();
    let mut combined = parse;
    combined.add(1, bytes, nodes)?;
    let mut strategies = Vec::with_capacity(rows.strategies.len());
    for row in &rows.strategies {
        strategies.push(StrategyDocuments {
            definition: materialize(&row.original_definition_json).map_err(|_| invalid())?,
            dsl: row
                .dsl_json
                .as_ref()
                .map(materialize)
                .transpose()
                .map_err(|_| invalid())?,
        });
    }
    let mut attempts = Vec::with_capacity(rows.attempts.len());
    for row in &rows.attempts {
        attempts.push(AttemptDocuments {
            input: materialize(&row.input_fingerprint_json).map_err(|_| invalid())?,
            engine: materialize(&row.engine_fingerprint_json).map_err(|_| invalid())?,
        });
    }
    let mut hypotheses = Vec::with_capacity(rows.hypotheses.len());
    for row in &rows.hypotheses {
        hypotheses.push(HypothesisDocuments {
            content: materialize(&row.content_json).map_err(|_| invalid())?,
            applicability: materialize(&row.applicability_json).map_err(|_| invalid())?,
        });
    }
    Ok(PreparedOriginalLineage {
        rows,
        run,
        strategies,
        attempts,
        hypotheses,
        parse,
        combined,
    })
}

/// Complete historical lineage agreement; no source/admission capability.
#[derive(Clone, Eq, PartialEq)]
pub(in crate::research::confirmation_inputs) struct OriginalLineageObservation {
    config_hash: String,
    candidates: Vec<CandidateLineageObservation>,
}

#[derive(Clone, Eq, PartialEq)]
pub(in crate::research::confirmation_inputs) struct CandidateLineageObservation {
    index: i64,
    strategy_id: i64,
    strategy_hash: String,
    hypothesis_id: i64,
    hypothesis_hash: String,
    base_id: String,
}

impl OriginalLineageObservation {
    pub(in crate::research::confirmation_inputs) fn config_hash(&self) -> &str {
        &self.config_hash
    }
    pub(in crate::research::confirmation_inputs) fn candidates(
        &self,
    ) -> &[CandidateLineageObservation] {
        &self.candidates
    }
}

impl CandidateLineageObservation {
    pub(in crate::research::confirmation_inputs) fn index(&self) -> i64 {
        self.index
    }
    pub(in crate::research::confirmation_inputs) fn strategy_id(&self) -> i64 {
        self.strategy_id
    }
    pub(in crate::research::confirmation_inputs) fn strategy_hash(&self) -> &str {
        &self.strategy_hash
    }
    pub(in crate::research::confirmation_inputs) fn hypothesis_id(&self) -> i64 {
        self.hypothesis_id
    }
    pub(in crate::research::confirmation_inputs) fn hypothesis_hash(&self) -> &str {
        &self.hypothesis_hash
    }
    pub(in crate::research::confirmation_inputs) fn base_id(&self) -> &str {
        &self.base_id
    }
}

pub(in crate::research::confirmation_inputs) fn observe_original_lineage(
    rows: &SourceRowsObservation,
) -> AppResult<OriginalLineageObservation> {
    let prepared = prepare_original_lineage(rows)?;
    semantics::observe(&prepared)
}

/// Whole historical warmup, with no source/admission capability.
#[derive(Clone, Eq, PartialEq)]
pub(in crate::research::confirmation_inputs) struct OriginalWarmupObservation {
    lineage: OriginalLineageObservation,
    derivations: Vec<EmbargoDerivation>,
}
impl OriginalWarmupObservation {
    pub(in crate::research::confirmation_inputs) fn lineage(&self) -> &OriginalLineageObservation {
        &self.lineage
    }
    pub(in crate::research::confirmation_inputs) fn derivations(&self) -> &[EmbargoDerivation] {
        &self.derivations
    }
}
pub(in crate::research::confirmation_inputs) fn observe_original_warmup(
    rows: &SourceRowsObservation,
) -> AppResult<OriginalWarmupObservation> {
    warmup::observe(rows)
}

#[cfg(test)]
#[path = "original_lineage/tests.rs"]
mod tests;
