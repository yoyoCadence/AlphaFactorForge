//! Complete declared SQL closure, still only historical consistency evidence.
//! No file/source authentication or authority escapes these private observations.

use std::collections::BTreeSet;

use super::selection::{derive_selection, SourceSelection};
use super::*;

include!("graph/rows.rs");

#[derive(Clone, Eq, PartialEq)]
pub(in crate::research::confirmation_inputs) struct SourceRowsObservation {
    roots: RootRowsObservation,
    jobs: Vec<JobRow>,
    attempts: Vec<AttemptRow>,
    strategies: Vec<StrategyRow>,
    hypotheses: Vec<HypothesisRow>,
    artifacts: Vec<ArtifactRow>,
    snapshot: SnapshotRow,
    instrument: InstrumentRow,
    calendar: CalendarRow,
    dataset: DatasetRow,
    sources: Vec<SnapshotSourceRow>,
    provenance: Vec<ProvenanceRow>,
    usage: ResourceUsage,
}

fn strategy_hash(s: &str) -> bool {
    s.strip_prefix("strategy-v2:").is_some_and(hash)
}

fn hypothesis_strategy_hash(s: &str) -> bool {
    strategy_hash(s) || s.strip_prefix("strategy-doc-v1:").is_some_and(hash)
}

fn dataset_hash(s: &str) -> bool {
    s.strip_prefix("dataset-content-v2:").is_some_and(hash)
}

fn integer_cell(row: &Row<'_>, logical: usize) -> AppResult<i64> {
    match marked_value(row, logical)? {
        ValueRef::Integer(value) => Ok(value),
        _ => Err(invalid()),
    }
}

fn nonnegative_cell(row: &Row<'_>, logical: usize) -> AppResult<i64> {
    let value = integer_cell(row, logical)?;
    if value < 0 {
        return Err(invalid());
    }
    Ok(value)
}

fn boolean_cell(row: &Row<'_>, logical: usize) -> AppResult<i64> {
    let value = integer_cell(row, logical)?;
    if !matches!(value, 0 | 1) {
        return Err(invalid());
    }
    Ok(value)
}

fn optional_positive_cell(row: &Row<'_>, logical: usize) -> AppResult<Option<i64>> {
    if matches!(marked_value(row, logical)?, ValueRef::Null) {
        return Ok(None);
    }
    positive_cell(row, logical).map(Some)
}

fn optional_nonnegative_cell(row: &Row<'_>, logical: usize) -> AppResult<Option<i64>> {
    if matches!(marked_value(row, logical)?, ValueRef::Null) {
        return Ok(None);
    }
    nonnegative_cell(row, logical).map(Some)
}

fn optional_integer_cell(row: &Row<'_>, logical: usize) -> AppResult<Option<i64>> {
    if matches!(marked_value(row, logical)?, ValueRef::Null) {
        return Ok(None);
    }
    integer_cell(row, logical).map(Some)
}

fn optional_real_cell(row: &Row<'_>, logical: usize) -> AppResult<Option<u64>> {
    if matches!(marked_value(row, logical)?, ValueRef::Null) {
        return Ok(None);
    }
    real_cell(row, logical).map(Some)
}

fn optional_text_cell(
    row: &Row<'_>,
    logical: usize,
    budget: &mut ResourceBudget,
    domain: fn(&str) -> bool,
) -> AppResult<Option<String>> {
    if matches!(marked_value(row, logical)?, ValueRef::Null) {
        return Ok(None);
    }
    text_cell(row, logical, budget, domain).map(Some)
}

fn optional_json_cell(
    row: &Row<'_>,
    logical: usize,
    budget: &mut ResourceBudget,
) -> AppResult<Option<OriginalJson>> {
    if matches!(marked_value(row, logical)?, ValueRef::Null) {
        return Ok(None);
    }
    json_cell(row, logical, budget).map(Some)
}

fn reference(set: &mut BTreeSet<i64>, id: i64, cap: usize) -> AppResult<()> {
    if !set.contains(&id) && set.len() == cap {
        return Err(invalid());
    }
    set.insert(id);
    Ok(())
}

fn validate_membership(rows: &SourceRowsObservation, selection: &SourceSelection) -> AppResult<()> {
    let candidates = selection.candidates();
    if rows.jobs.len() != candidates.len() * 2 || rows.attempts.len() != candidates.len() {
        return Err(invalid());
    }
    let run = rows.roots.locator.run_id;
    let mut job_ids = BTreeSet::new();
    let mut attempt_ids = BTreeSet::new();
    let mut event_ids = BTreeSet::new();
    let strategy_ids: BTreeSet<i64> = rows.strategies.iter().map(|s| s.id).collect();
    for (i, candidate) in candidates.iter().enumerate() {
        let attempt = &rows.attempts[i];
        let train = &rows.jobs[2 * i];
        let validation = &rows.jobs[2 * i + 1];
        if attempt.discovery_run_id != Some(run)
            || attempt.candidate_index != Some(candidate.index())
            || attempt.dataset_id != selection.dataset_id()
            || attempt.attempt_key != format!("run:{run}:candidate:{}", candidate.index())
            || !attempt_ids.insert(attempt.id)
            || (attempt.status == "completed" && attempt.result_artifact_id.is_none())
        {
            return Err(invalid());
        }
        let event = attempt.trial_event_id.as_deref().ok_or_else(invalid)?;
        if !event_ids.insert(event) {
            return Err(invalid());
        }
        for (job, segment) in [(train, "train"), (validation, "validation")] {
            if job.discovery_run_id != run
                || job.candidate_index != candidate.index()
                || job.dataset_id != selection.dataset_id()
                || job.strategy_id != attempt.strategy_id
                || job.segment != segment
                || !job_ids.insert(job.id)
            {
                return Err(invalid());
            }
        }
        let strategy = rows
            .strategies
            .iter()
            .find(|s| s.id == attempt.strategy_id)
            .ok_or_else(invalid)?;
        let hypothesis = rows
            .hypotheses
            .iter()
            .find(|h| h.id == attempt.hypothesis_id)
            .ok_or_else(invalid)?;
        if strategy.strategy_hash != candidate.strategy_hash()
            || hypothesis
                .strategy_id
                .is_some_and(|id| id != attempt.strategy_id)
        {
            return Err(invalid());
        }
    }
    // The bounded direct strategy set is the only supported parent closure.
    if rows
        .strategies
        .iter()
        .filter_map(|s| s.parent_strategy_id)
        .chain(rows.hypotheses.iter().filter_map(|h| h.parent_strategy_id))
        .any(|id| !strategy_ids.contains(&id))
    {
        return Err(invalid());
    }
    let snapshot = &rows.snapshot;
    if snapshot.snapshot_id != selection.snapshot_id()
        || snapshot.instrument_id != rows.roots.admission.instrument_id
        || snapshot.interval != selection.interval()
        || snapshot.dataset_id != selection.dataset_id()
        || snapshot.dataset_hash != selection.dataset_hash()
        || rows.instrument.id != snapshot.instrument_row_id
        || rows.instrument.instrument_id != snapshot.instrument_id
        || rows.instrument.session_calendar_id != snapshot.calendar_id
        || rows.calendar.calendar_id != snapshot.calendar_id
        || rows.dataset.id != selection.dataset_id()
        || rows.dataset.dataset_hash != selection.dataset_hash()
        || rows.dataset.interval != selection.interval()
    {
        return Err(invalid());
    }
    let mut source_ids = BTreeSet::new();
    for source in &rows.sources {
        if source.snapshot_row_id != snapshot.id || !source_ids.insert(source.provenance_id) {
            return Err(invalid());
        }
        let provenance = rows
            .provenance
            .iter()
            .find(|p| p.id == source.provenance_id)
            .ok_or_else(invalid)?;
        if provenance.instrument_id != snapshot.instrument_id
            || provenance.interval != snapshot.interval
            || (provenance.accepted == 1) != provenance.rejection_reason.is_none()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(in crate::research::confirmation_inputs) fn capture_source_rows_in(
    tx: &Transaction<'_>,
    expected: &ExpectedWorkspace,
    locator: &SourceLocator,
) -> AppResult<SourceRowsObservation> {
    let roots = capture_root_rows_in(tx, expected, locator)?;
    let selection = derive_selection(&roots)?;
    let mut budget = ResourceBudget {
        used: roots.usage.clone(),
    };
    let jobs = read_jobs(tx, locator.run_id, &mut budget)?;
    let attempts = read_attempts(tx, locator.run_id, &mut budget)?;
    let mut strategy_ids = BTreeSet::new();
    let mut hypothesis_ids = BTreeSet::new();
    let mut artifact_ids = BTreeSet::new();
    for id in jobs
        .iter()
        .map(|j| j.strategy_id)
        .chain(attempts.iter().map(|a| a.strategy_id))
    {
        reference(&mut strategy_ids, id, 4096)?;
    }
    for a in &attempts {
        reference(&mut hypothesis_ids, a.hypothesis_id, 4096)?;
        if let Some(id) = a.result_artifact_id {
            reference(&mut artifact_ids, id, 4096)?;
        }
    }
    let mut strategies = Vec::new();
    for id in strategy_ids {
        let row = read_strategy(tx, id, &mut budget)?;
        if row.id != id {
            return Err(invalid());
        }
        strategies.push(row);
    }
    let mut hypotheses = Vec::new();
    for id in hypothesis_ids {
        let row = read_hypothesis(tx, id, &mut budget)?;
        if row.id != id {
            return Err(invalid());
        }
        hypotheses.push(row);
    }
    let mut artifacts = Vec::new();
    for id in artifact_ids {
        let row = read_artifact(tx, id, &mut budget)?;
        if row.id != id {
            return Err(invalid());
        }
        artifacts.push(row);
    }
    let snapshot = read_snapshot(tx, selection.snapshot_id(), &mut budget)?;
    let instrument = read_instrument(tx, snapshot.instrument_row_id, &mut budget)?;
    let calendar = read_calendar(tx, &snapshot.calendar_id, &mut budget)?;
    let dataset = read_dataset(tx, snapshot.dataset_id, &mut budget)?;
    let sources = read_sources(tx, snapshot.id, &mut budget)?;
    let mut provenance_ids = BTreeSet::new();
    for source in &sources {
        reference(&mut provenance_ids, source.provenance_id, 1024)?;
    }
    let mut provenance = Vec::new();
    for id in provenance_ids {
        let row = read_provenance(tx, id, &mut budget)?;
        if row.id != id {
            return Err(invalid());
        }
        provenance.push(row);
    }
    let observation = SourceRowsObservation {
        roots,
        jobs,
        attempts,
        strategies,
        hypotheses,
        artifacts,
        snapshot,
        instrument,
        calendar,
        dataset,
        sources,
        provenance,
        usage: budget.used,
    };
    validate_membership(&observation, &selection)?;
    Ok(observation)
}

pub(in crate::research::confirmation_inputs) fn recheck_source_rows_in(
    tx: &Transaction<'_>,
    expected: &ExpectedWorkspace,
    saved: &SourceRowsObservation,
) -> AppResult<()> {
    recheck_workspace_in(tx, expected, &saved.roots.workspace)?;
    let fresh = capture_source_rows_in(tx, expected, &saved.roots.locator)?;
    if fresh != *saved {
        return Err(changed());
    }
    Ok(())
}

#[cfg(test)]
#[path = "graph/tests.rs"]
mod tests;
