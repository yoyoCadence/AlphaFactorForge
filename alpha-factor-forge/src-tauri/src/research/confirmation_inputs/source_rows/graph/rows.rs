// Fixed 117-column graph manifest. Every value follows its validity marker.
// Literal SQL only; no runtime query/column factory.

#[derive(Clone, Eq, PartialEq)]
struct JobRow {
    id: i64,
    discovery_run_id: i64,
    candidate_index: i64,
    strategy_id: i64,
    dataset_id: i64,
    segment: String,
    status: String,
    result_id: Option<i64>,
}

const JOBS_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("discovery_run_id")='integer' AND "discovery_run_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("discovery_run_id")='integer' AND "discovery_run_id">0 THEN "discovery_run_id" ELSE NULL END,
 CASE WHEN typeof("candidate_index")='integer' AND "candidate_index">=0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("candidate_index")='integer' AND "candidate_index">=0 THEN "candidate_index" ELSE NULL END,
 CASE WHEN typeof("strategy_id")='integer' AND "strategy_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("strategy_id")='integer' AND "strategy_id">0 THEN "strategy_id" ELSE NULL END,
 CASE WHEN typeof("dataset_id")='integer' AND "dataset_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("dataset_id")='integer' AND "dataset_id">0 THEN "dataset_id" ELSE NULL END,
 CASE WHEN typeof("segment")='text' AND length(CAST("segment" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("segment")='text' AND length(CAST("segment" AS BLOB))<=?2 THEN "segment" ELSE NULL END,
 CASE WHEN typeof("status")='text' AND length(CAST("status" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("status")='text' AND length(CAST("status" AS BLOB))<=?2 THEN "status" ELSE NULL END,
 CASE WHEN typeof("result_id")='null' OR (typeof("result_id")='integer' AND "result_id">0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("result_id")='null' OR (typeof("result_id")='integer' AND "result_id">0) THEN "result_id" ELSE NULL END
 FROM discovery_jobs WHERE discovery_run_id=?1 ORDER BY candidate_index, segment, id LIMIT 8193"#;

fn read_jobs(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<Vec<JobRow>> {
    let mut statement = tx.prepare(JOBS_SQL)?;
    let mut rows = statement.query(params![selector, MAX_TEXT_BYTES as i64])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        if records.len() == 8192 {
            return Err(invalid());
        } // sentinel, before payload
        records.push(JobRow {
            id: positive_cell(row, 0)?,
            discovery_run_id: positive_cell(row, 1)?,
            candidate_index: nonnegative_cell(row, 2)?,
            strategy_id: positive_cell(row, 3)?,
            dataset_id: positive_cell(row, 4)?,
            segment: text_cell(row, 5, budget, |s| matches!(s, "train" | "validation"))?,
            status: text_cell(row, 6, budget, |s| {
                matches!(s, "queued" | "running" | "done" | "failed" | "skipped")
            })?,
            result_id: optional_positive_cell(row, 7)?,
        });
    }
    Ok(records)
}

#[derive(Clone, Eq, PartialEq)]
struct AttemptRow {
    id: i64,
    attempt_key: String,
    hypothesis_id: i64,
    strategy_id: i64,
    dataset_id: i64,
    discovery_run_id: Option<i64>,
    candidate_index: Option<i64>,
    status: String,
    input_fingerprint_json: OriginalJson,
    engine_fingerprint_json: OriginalJson,
    outcome_json: Option<OriginalJson>,
    result_artifact_id: Option<i64>,
    trial_event_id: Option<String>,
}

const ATTEMPTS_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("attempt_key")='text' AND length(CAST("attempt_key" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("attempt_key")='text' AND length(CAST("attempt_key" AS BLOB))<=?2 THEN "attempt_key" ELSE NULL END,
 CASE WHEN typeof("hypothesis_id")='integer' AND "hypothesis_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("hypothesis_id")='integer' AND "hypothesis_id">0 THEN "hypothesis_id" ELSE NULL END,
 CASE WHEN typeof("strategy_id")='integer' AND "strategy_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("strategy_id")='integer' AND "strategy_id">0 THEN "strategy_id" ELSE NULL END,
 CASE WHEN typeof("dataset_id")='integer' AND "dataset_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("dataset_id")='integer' AND "dataset_id">0 THEN "dataset_id" ELSE NULL END,
 CASE WHEN typeof("discovery_run_id")='null' OR (typeof("discovery_run_id")='integer' AND "discovery_run_id">0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("discovery_run_id")='null' OR (typeof("discovery_run_id")='integer' AND "discovery_run_id">0) THEN "discovery_run_id" ELSE NULL END,
 CASE WHEN typeof("candidate_index")='null' OR (typeof("candidate_index")='integer' AND "candidate_index">=0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("candidate_index")='null' OR (typeof("candidate_index")='integer' AND "candidate_index">=0) THEN "candidate_index" ELSE NULL END,
 CASE WHEN typeof("status")='text' AND length(CAST("status" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("status")='text' AND length(CAST("status" AS BLOB))<=?2 THEN "status" ELSE NULL END,
 CASE WHEN typeof("input_fingerprint_json")='text' AND length(CAST("input_fingerprint_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("input_fingerprint_json")='text' AND length(CAST("input_fingerprint_json" AS BLOB))<=?3 THEN "input_fingerprint_json" ELSE NULL END,
 CASE WHEN typeof("engine_fingerprint_json")='text' AND length(CAST("engine_fingerprint_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("engine_fingerprint_json")='text' AND length(CAST("engine_fingerprint_json" AS BLOB))<=?3 THEN "engine_fingerprint_json" ELSE NULL END,
 CASE WHEN typeof("outcome_json")='null' OR (typeof("outcome_json")='text' AND length(CAST("outcome_json" AS BLOB))<=?3) THEN 1 ELSE 0 END,
 CASE WHEN typeof("outcome_json")='null' OR (typeof("outcome_json")='text' AND length(CAST("outcome_json" AS BLOB))<=?3) THEN "outcome_json" ELSE NULL END,
 CASE WHEN typeof("result_artifact_id")='null' OR (typeof("result_artifact_id")='integer' AND "result_artifact_id">0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("result_artifact_id")='null' OR (typeof("result_artifact_id")='integer' AND "result_artifact_id">0) THEN "result_artifact_id" ELSE NULL END,
 CASE WHEN typeof("trial_event_id")='null' OR (typeof("trial_event_id")='text' AND length(CAST("trial_event_id" AS BLOB))<=?2) THEN 1 ELSE 0 END,
 CASE WHEN typeof("trial_event_id")='null' OR (typeof("trial_event_id")='text' AND length(CAST("trial_event_id" AS BLOB))<=?2) THEN "trial_event_id" ELSE NULL END
 FROM research_attempts WHERE discovery_run_id=?1 ORDER BY candidate_index, id LIMIT 4097"#;

fn read_attempts(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<Vec<AttemptRow>> {
    let mut statement = tx.prepare(ATTEMPTS_SQL)?;
    let mut rows = statement.query(params![
        selector,
        MAX_TEXT_BYTES as i64,
        MAX_JSON_BYTES as i64
    ])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        if records.len() == 4096 {
            return Err(invalid());
        } // sentinel, before payload
        records.push(AttemptRow {
            id: positive_cell(row, 0)?,
            attempt_key: text_cell(row, 1, budget, version)?,
            hypothesis_id: positive_cell(row, 2)?,
            strategy_id: positive_cell(row, 3)?,
            dataset_id: positive_cell(row, 4)?,
            discovery_run_id: optional_positive_cell(row, 5)?,
            candidate_index: optional_nonnegative_cell(row, 6)?,
            status: text_cell(row, 7, budget, |s| {
                matches!(
                    s,
                    "submitted" | "running" | "completed" | "failed" | "skipped"
                )
            })?,
            input_fingerprint_json: json_cell(row, 8, budget)?,
            engine_fingerprint_json: json_cell(row, 9, budget)?,
            outcome_json: optional_json_cell(row, 10, budget)?,
            result_artifact_id: optional_positive_cell(row, 11)?,
            trial_event_id: optional_text_cell(row, 12, budget, |s| {
                s.strip_prefix("trial-event-v1:").is_some_and(hash)
            })?,
        });
    }
    Ok(records)
}

#[derive(Clone, Eq, PartialEq)]
struct StrategyRow {
    id: i64,
    r#type: String,
    original_definition_json: OriginalJson,
    dsl_json: Option<OriginalJson>,
    strategy_hash: String,
    parent_strategy_id: Option<i64>,
}

const STRATEGY_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("type")='text' AND length(CAST("type" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("type")='text' AND length(CAST("type" AS BLOB))<=?2 THEN "type" ELSE NULL END,
 CASE WHEN typeof("original_definition_json")='text' AND length(CAST("original_definition_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("original_definition_json")='text' AND length(CAST("original_definition_json" AS BLOB))<=?3 THEN "original_definition_json" ELSE NULL END,
 CASE WHEN typeof("dsl_json")='null' OR (typeof("dsl_json")='text' AND length(CAST("dsl_json" AS BLOB))<=?3) THEN 1 ELSE 0 END,
 CASE WHEN typeof("dsl_json")='null' OR (typeof("dsl_json")='text' AND length(CAST("dsl_json" AS BLOB))<=?3) THEN "dsl_json" ELSE NULL END,
 CASE WHEN typeof("strategy_hash")='text' AND length(CAST("strategy_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("strategy_hash")='text' AND length(CAST("strategy_hash" AS BLOB))<=?2 THEN "strategy_hash" ELSE NULL END,
 CASE WHEN typeof("parent_strategy_id")='null' OR (typeof("parent_strategy_id")='integer' AND "parent_strategy_id">0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("parent_strategy_id")='null' OR (typeof("parent_strategy_id")='integer' AND "parent_strategy_id">0) THEN "parent_strategy_id" ELSE NULL END
 FROM strategy_def WHERE id=?1 ORDER BY id LIMIT 2"#;

fn read_strategy(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<StrategyRow> {
    let mut statement = tx.prepare(STRATEGY_SQL)?;
    let mut rows = statement.query(params![
        selector,
        MAX_TEXT_BYTES as i64,
        MAX_JSON_BYTES as i64
    ])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = StrategyRow {
        id: positive_cell(row, 0)?,
        r#type: text_cell(row, 1, budget, |_| true)?,
        original_definition_json: json_cell(row, 2, budget)?,
        dsl_json: optional_json_cell(row, 3, budget)?,
        strategy_hash: text_cell(row, 4, budget, strategy_hash)?,
        parent_strategy_id: optional_positive_cell(row, 5)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}

#[derive(Clone, Eq, PartialEq)]
struct HypothesisRow {
    id: i64,
    hypothesis_hash: String,
    version: String,
    source: String,
    mechanism: String,
    applicability_json: OriginalJson,
    failure_modes: String,
    strategy_hash: String,
    strategy_id: Option<i64>,
    parent_strategy_id: Option<i64>,
    variation_kind: Option<String>,
    content_json: OriginalJson,
}

const HYPOTHESIS_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("hypothesis_hash")='text' AND length(CAST("hypothesis_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("hypothesis_hash")='text' AND length(CAST("hypothesis_hash" AS BLOB))<=?2 THEN "hypothesis_hash" ELSE NULL END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN "version" ELSE NULL END,
 CASE WHEN typeof("source")='text' AND length(CAST("source" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("source")='text' AND length(CAST("source" AS BLOB))<=?2 THEN "source" ELSE NULL END,
 CASE WHEN typeof("mechanism")='text' AND length(CAST("mechanism" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("mechanism")='text' AND length(CAST("mechanism" AS BLOB))<=?2 THEN "mechanism" ELSE NULL END,
 CASE WHEN typeof("applicability_json")='text' AND length(CAST("applicability_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("applicability_json")='text' AND length(CAST("applicability_json" AS BLOB))<=?3 THEN "applicability_json" ELSE NULL END,
 CASE WHEN typeof("failure_modes")='text' AND length(CAST("failure_modes" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("failure_modes")='text' AND length(CAST("failure_modes" AS BLOB))<=?2 THEN "failure_modes" ELSE NULL END,
 CASE WHEN typeof("strategy_hash")='text' AND length(CAST("strategy_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("strategy_hash")='text' AND length(CAST("strategy_hash" AS BLOB))<=?2 THEN "strategy_hash" ELSE NULL END,
 CASE WHEN typeof("strategy_id")='null' OR (typeof("strategy_id")='integer' AND "strategy_id">0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("strategy_id")='null' OR (typeof("strategy_id")='integer' AND "strategy_id">0) THEN "strategy_id" ELSE NULL END,
 CASE WHEN typeof("parent_strategy_id")='null' OR (typeof("parent_strategy_id")='integer' AND "parent_strategy_id">0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("parent_strategy_id")='null' OR (typeof("parent_strategy_id")='integer' AND "parent_strategy_id">0) THEN "parent_strategy_id" ELSE NULL END,
 CASE WHEN typeof("variation_kind")='null' OR (typeof("variation_kind")='text' AND length(CAST("variation_kind" AS BLOB))<=?2) THEN 1 ELSE 0 END,
 CASE WHEN typeof("variation_kind")='null' OR (typeof("variation_kind")='text' AND length(CAST("variation_kind" AS BLOB))<=?2) THEN "variation_kind" ELSE NULL END,
 CASE WHEN typeof("content_json")='text' AND length(CAST("content_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("content_json")='text' AND length(CAST("content_json" AS BLOB))<=?3 THEN "content_json" ELSE NULL END
 FROM hypotheses WHERE id=?1 ORDER BY id LIMIT 2"#;

fn read_hypothesis(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<HypothesisRow> {
    let mut statement = tx.prepare(HYPOTHESIS_SQL)?;
    let mut rows = statement.query(params![
        selector,
        MAX_TEXT_BYTES as i64,
        MAX_JSON_BYTES as i64
    ])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = HypothesisRow {
        id: positive_cell(row, 0)?,
        hypothesis_hash: text_cell(row, 1, budget, hash)?,
        version: text_cell(row, 2, budget, version)?,
        source: text_cell(row, 3, budget, |s| {
            matches!(s, "manual" | "discovery" | "ai")
        })?,
        mechanism: text_cell(row, 4, budget, |_| true)?,
        applicability_json: json_cell(row, 5, budget)?,
        failure_modes: text_cell(row, 6, budget, |_| true)?,
        strategy_hash: text_cell(row, 7, budget, strategy_hash)?,
        strategy_id: optional_positive_cell(row, 8)?,
        parent_strategy_id: optional_positive_cell(row, 9)?,
        variation_kind: optional_text_cell(row, 10, budget, |_| true)?,
        content_json: json_cell(row, 11, budget)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}

#[derive(Clone, Eq, PartialEq)]
struct ArtifactRow {
    id: i64,
    sha256: String,
    kind: String,
    byte_len: i64,
    relative_path: String,
}

const ARTIFACT_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("sha256")='text' AND length(CAST("sha256" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("sha256")='text' AND length(CAST("sha256" AS BLOB))<=?2 THEN "sha256" ELSE NULL END,
 CASE WHEN typeof("kind")='text' AND length(CAST("kind" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("kind")='text' AND length(CAST("kind" AS BLOB))<=?2 THEN "kind" ELSE NULL END,
 CASE WHEN typeof("byte_len")='integer' AND "byte_len">=0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("byte_len")='integer' AND "byte_len">=0 THEN "byte_len" ELSE NULL END,
 CASE WHEN typeof("relative_path")='text' AND length(CAST("relative_path" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("relative_path")='text' AND length(CAST("relative_path" AS BLOB))<=?2 THEN "relative_path" ELSE NULL END
 FROM research_artifacts WHERE id=?1 ORDER BY id LIMIT 2"#;

fn read_artifact(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<ArtifactRow> {
    let mut statement = tx.prepare(ARTIFACT_SQL)?;
    let mut rows = statement.query(params![selector, MAX_TEXT_BYTES as i64])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = ArtifactRow {
        id: positive_cell(row, 0)?,
        sha256: text_cell(row, 1, budget, hash)?,
        kind: text_cell(row, 2, budget, |_| true)?,
        byte_len: nonnegative_cell(row, 3)?,
        relative_path: text_cell(row, 4, budget, |_| true)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}

#[derive(Clone, Eq, PartialEq)]
struct SnapshotRow {
    id: i64,
    snapshot_id: String,
    version: String,
    instrument_row_id: i64,
    instrument_id: String,
    interval: String,
    dataset_id: i64,
    dataset_hash: String,
    price_basis: String,
    calendar_id: String,
    corporate_action_version: Option<String>,
    cost_profile_version: Option<String>,
    kind: String,
    status: String,
    as_of: i64,
    coverage_json: OriginalJson,
    content_json: OriginalJson,
}

const SNAPSHOT_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("snapshot_id")='text' AND length(CAST("snapshot_id" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("snapshot_id")='text' AND length(CAST("snapshot_id" AS BLOB))<=?2 THEN "snapshot_id" ELSE NULL END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN "version" ELSE NULL END,
 CASE WHEN typeof("instrument_row_id")='integer' AND "instrument_row_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("instrument_row_id")='integer' AND "instrument_row_id">0 THEN "instrument_row_id" ELSE NULL END,
 CASE WHEN typeof("instrument_id")='text' AND length(CAST("instrument_id" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("instrument_id")='text' AND length(CAST("instrument_id" AS BLOB))<=?2 THEN "instrument_id" ELSE NULL END,
 CASE WHEN typeof("interval")='text' AND length(CAST("interval" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("interval")='text' AND length(CAST("interval" AS BLOB))<=?2 THEN "interval" ELSE NULL END,
 CASE WHEN typeof("dataset_id")='integer' AND "dataset_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("dataset_id")='integer' AND "dataset_id">0 THEN "dataset_id" ELSE NULL END,
 CASE WHEN typeof("dataset_hash")='text' AND length(CAST("dataset_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("dataset_hash")='text' AND length(CAST("dataset_hash" AS BLOB))<=?2 THEN "dataset_hash" ELSE NULL END,
 CASE WHEN typeof("price_basis")='text' AND length(CAST("price_basis" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("price_basis")='text' AND length(CAST("price_basis" AS BLOB))<=?2 THEN "price_basis" ELSE NULL END,
 CASE WHEN typeof("calendar_id")='text' AND length(CAST("calendar_id" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("calendar_id")='text' AND length(CAST("calendar_id" AS BLOB))<=?2 THEN "calendar_id" ELSE NULL END,
 CASE WHEN typeof("corporate_action_version")='null' OR (typeof("corporate_action_version")='text' AND length(CAST("corporate_action_version" AS BLOB))<=?2) THEN 1 ELSE 0 END,
 CASE WHEN typeof("corporate_action_version")='null' OR (typeof("corporate_action_version")='text' AND length(CAST("corporate_action_version" AS BLOB))<=?2) THEN "corporate_action_version" ELSE NULL END,
 CASE WHEN typeof("cost_profile_version")='null' OR (typeof("cost_profile_version")='text' AND length(CAST("cost_profile_version" AS BLOB))<=?2) THEN 1 ELSE 0 END,
 CASE WHEN typeof("cost_profile_version")='null' OR (typeof("cost_profile_version")='text' AND length(CAST("cost_profile_version" AS BLOB))<=?2) THEN "cost_profile_version" ELSE NULL END,
 CASE WHEN typeof("kind")='text' AND length(CAST("kind" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("kind")='text' AND length(CAST("kind" AS BLOB))<=?2 THEN "kind" ELSE NULL END,
 CASE WHEN typeof("status")='text' AND length(CAST("status" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("status")='text' AND length(CAST("status" AS BLOB))<=?2 THEN "status" ELSE NULL END,
 CASE WHEN typeof("as_of")='integer' THEN 1 ELSE 0 END,
 CASE WHEN typeof("as_of")='integer' THEN "as_of" ELSE NULL END,
 CASE WHEN typeof("coverage_json")='text' AND length(CAST("coverage_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("coverage_json")='text' AND length(CAST("coverage_json" AS BLOB))<=?3 THEN "coverage_json" ELSE NULL END,
 CASE WHEN typeof("content_json")='text' AND length(CAST("content_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("content_json")='text' AND length(CAST("content_json" AS BLOB))<=?3 THEN "content_json" ELSE NULL END
 FROM market_snapshots WHERE snapshot_id=?1 ORDER BY id LIMIT 2"#;

fn read_snapshot(
    tx: &Transaction<'_>,
    selector: &str,
    budget: &mut ResourceBudget,
) -> AppResult<SnapshotRow> {
    let mut statement = tx.prepare(SNAPSHOT_SQL)?;
    let mut rows = statement.query(params![
        selector,
        MAX_TEXT_BYTES as i64,
        MAX_JSON_BYTES as i64
    ])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = SnapshotRow {
        id: positive_cell(row, 0)?,
        snapshot_id: text_cell(row, 1, budget, hash)?,
        version: text_cell(row, 2, budget, version)?,
        instrument_row_id: positive_cell(row, 3)?,
        instrument_id: text_cell(row, 4, budget, version)?,
        interval: text_cell(row, 5, budget, version)?,
        dataset_id: positive_cell(row, 6)?,
        dataset_hash: text_cell(row, 7, budget, dataset_hash)?,
        price_basis: text_cell(row, 8, budget, |s| {
            matches!(s, "raw" | "adjusted-split" | "adjusted-total-return")
        })?,
        calendar_id: text_cell(row, 9, budget, version)?,
        corporate_action_version: optional_text_cell(row, 10, budget, |_| true)?,
        cost_profile_version: optional_text_cell(row, 11, budget, |_| true)?,
        kind: text_cell(row, 12, budget, |s| {
            matches!(s, "demo" | "historical" | "forward-observed")
        })?,
        status: text_cell(row, 13, budget, |s| matches!(s, "ok" | "degraded"))?,
        as_of: integer_cell(row, 14)?,
        coverage_json: json_cell(row, 15, budget)?,
        content_json: json_cell(row, 16, budget)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}

#[derive(Clone, Eq, PartialEq)]
struct InstrumentRow {
    id: i64,
    instrument_id: String,
    revision: i64,
    content_hash: String,
    version: String,
    market: String,
    venue: String,
    symbol: String,
    base: String,
    quote: String,
    asset_type: String,
    session_calendar_id: String,
    timezone: String,
    lot_size_bits: Option<u64>,
    price_step_bits: Option<u64>,
    min_notional_bits: Option<u64>,
    listed_from: Option<i64>,
    delisted_at: Option<i64>,
    suspensions_json: OriginalJson,
    source_capabilities_json: OriginalJson,
    content_json: OriginalJson,
}

const INSTRUMENT_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("instrument_id")='text' AND length(CAST("instrument_id" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("instrument_id")='text' AND length(CAST("instrument_id" AS BLOB))<=?2 THEN "instrument_id" ELSE NULL END,
 CASE WHEN typeof("revision")='integer' AND "revision">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("revision")='integer' AND "revision">0 THEN "revision" ELSE NULL END,
 CASE WHEN typeof("content_hash")='text' AND length(CAST("content_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("content_hash")='text' AND length(CAST("content_hash" AS BLOB))<=?2 THEN "content_hash" ELSE NULL END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN "version" ELSE NULL END,
 CASE WHEN typeof("market")='text' AND length(CAST("market" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("market")='text' AND length(CAST("market" AS BLOB))<=?2 THEN "market" ELSE NULL END,
 CASE WHEN typeof("venue")='text' AND length(CAST("venue" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("venue")='text' AND length(CAST("venue" AS BLOB))<=?2 THEN "venue" ELSE NULL END,
 CASE WHEN typeof("symbol")='text' AND length(CAST("symbol" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("symbol")='text' AND length(CAST("symbol" AS BLOB))<=?2 THEN "symbol" ELSE NULL END,
 CASE WHEN typeof("base")='text' AND length(CAST("base" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("base")='text' AND length(CAST("base" AS BLOB))<=?2 THEN "base" ELSE NULL END,
 CASE WHEN typeof("quote")='text' AND length(CAST("quote" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("quote")='text' AND length(CAST("quote" AS BLOB))<=?2 THEN "quote" ELSE NULL END,
 CASE WHEN typeof("asset_type")='text' AND length(CAST("asset_type" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("asset_type")='text' AND length(CAST("asset_type" AS BLOB))<=?2 THEN "asset_type" ELSE NULL END,
 CASE WHEN typeof("session_calendar_id")='text' AND length(CAST("session_calendar_id" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("session_calendar_id")='text' AND length(CAST("session_calendar_id" AS BLOB))<=?2 THEN "session_calendar_id" ELSE NULL END,
 CASE WHEN typeof("timezone")='text' AND length(CAST("timezone" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("timezone")='text' AND length(CAST("timezone" AS BLOB))<=?2 THEN "timezone" ELSE NULL END,
 CASE WHEN typeof("lot_size")='null' OR (typeof("lot_size")='real') THEN 1 ELSE 0 END,
 CASE WHEN typeof("lot_size")='null' OR (typeof("lot_size")='real') THEN "lot_size" ELSE NULL END,
 CASE WHEN typeof("price_step")='null' OR (typeof("price_step")='real') THEN 1 ELSE 0 END,
 CASE WHEN typeof("price_step")='null' OR (typeof("price_step")='real') THEN "price_step" ELSE NULL END,
 CASE WHEN typeof("min_notional")='null' OR (typeof("min_notional")='real') THEN 1 ELSE 0 END,
 CASE WHEN typeof("min_notional")='null' OR (typeof("min_notional")='real') THEN "min_notional" ELSE NULL END,
 CASE WHEN typeof("listed_from")='null' OR (typeof("listed_from")='integer') THEN 1 ELSE 0 END,
 CASE WHEN typeof("listed_from")='null' OR (typeof("listed_from")='integer') THEN "listed_from" ELSE NULL END,
 CASE WHEN typeof("delisted_at")='null' OR (typeof("delisted_at")='integer') THEN 1 ELSE 0 END,
 CASE WHEN typeof("delisted_at")='null' OR (typeof("delisted_at")='integer') THEN "delisted_at" ELSE NULL END,
 CASE WHEN typeof("suspensions_json")='text' AND length(CAST("suspensions_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("suspensions_json")='text' AND length(CAST("suspensions_json" AS BLOB))<=?3 THEN "suspensions_json" ELSE NULL END,
 CASE WHEN typeof("source_capabilities_json")='text' AND length(CAST("source_capabilities_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("source_capabilities_json")='text' AND length(CAST("source_capabilities_json" AS BLOB))<=?3 THEN "source_capabilities_json" ELSE NULL END,
 CASE WHEN typeof("content_json")='text' AND length(CAST("content_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("content_json")='text' AND length(CAST("content_json" AS BLOB))<=?3 THEN "content_json" ELSE NULL END
 FROM market_instruments WHERE id=?1 ORDER BY id LIMIT 2"#;

fn read_instrument(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<InstrumentRow> {
    let mut statement = tx.prepare(INSTRUMENT_SQL)?;
    let mut rows = statement.query(params![
        selector,
        MAX_TEXT_BYTES as i64,
        MAX_JSON_BYTES as i64
    ])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = InstrumentRow {
        id: positive_cell(row, 0)?,
        instrument_id: text_cell(row, 1, budget, version)?,
        revision: positive_cell(row, 2)?,
        content_hash: text_cell(row, 3, budget, hash)?,
        version: text_cell(row, 4, budget, version)?,
        market: text_cell(row, 5, budget, |s| {
            matches!(s, "crypto" | "us-etf" | "tw-etf")
        })?,
        venue: text_cell(row, 6, budget, |_| true)?,
        symbol: text_cell(row, 7, budget, |_| true)?,
        base: text_cell(row, 8, budget, |_| true)?,
        quote: text_cell(row, 9, budget, |_| true)?,
        asset_type: text_cell(row, 10, budget, |s| matches!(s, "spot-crypto" | "etf"))?,
        session_calendar_id: text_cell(row, 11, budget, version)?,
        timezone: text_cell(row, 12, budget, |_| true)?,
        lot_size_bits: optional_real_cell(row, 13)?,
        price_step_bits: optional_real_cell(row, 14)?,
        min_notional_bits: optional_real_cell(row, 15)?,
        listed_from: optional_integer_cell(row, 16)?,
        delisted_at: optional_integer_cell(row, 17)?,
        suspensions_json: json_cell(row, 18, budget)?,
        source_capabilities_json: json_cell(row, 19, budget)?,
        content_json: json_cell(row, 20, budget)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}

#[derive(Clone, Eq, PartialEq)]
struct CalendarRow {
    id: i64,
    calendar_id: String,
    version: String,
    kind: String,
    timezone: String,
    content_hash: String,
    definition_json: OriginalJson,
}

const CALENDAR_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("calendar_id")='text' AND length(CAST("calendar_id" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("calendar_id")='text' AND length(CAST("calendar_id" AS BLOB))<=?2 THEN "calendar_id" ELSE NULL END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN "version" ELSE NULL END,
 CASE WHEN typeof("kind")='text' AND length(CAST("kind" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("kind")='text' AND length(CAST("kind" AS BLOB))<=?2 THEN "kind" ELSE NULL END,
 CASE WHEN typeof("timezone")='text' AND length(CAST("timezone" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("timezone")='text' AND length(CAST("timezone" AS BLOB))<=?2 THEN "timezone" ELSE NULL END,
 CASE WHEN typeof("content_hash")='text' AND length(CAST("content_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("content_hash")='text' AND length(CAST("content_hash" AS BLOB))<=?2 THEN "content_hash" ELSE NULL END,
 CASE WHEN typeof("definition_json")='text' AND length(CAST("definition_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("definition_json")='text' AND length(CAST("definition_json" AS BLOB))<=?3 THEN "definition_json" ELSE NULL END
 FROM market_calendars WHERE calendar_id=?1 ORDER BY id LIMIT 2"#;

fn read_calendar(
    tx: &Transaction<'_>,
    selector: &str,
    budget: &mut ResourceBudget,
) -> AppResult<CalendarRow> {
    let mut statement = tx.prepare(CALENDAR_SQL)?;
    let mut rows = statement.query(params![
        selector,
        MAX_TEXT_BYTES as i64,
        MAX_JSON_BYTES as i64
    ])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = CalendarRow {
        id: positive_cell(row, 0)?,
        calendar_id: text_cell(row, 1, budget, version)?,
        version: text_cell(row, 2, budget, version)?,
        kind: text_cell(row, 3, budget, |s| {
            matches!(s, "continuous" | "trading-days")
        })?,
        timezone: text_cell(row, 4, budget, |_| true)?,
        content_hash: text_cell(row, 5, budget, hash)?,
        definition_json: json_cell(row, 6, budget)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}

#[derive(Clone, Eq, PartialEq)]
struct DatasetRow {
    id: i64,
    exchange: String,
    symbol: String,
    interval: String,
    start_time: i64,
    end_time: i64,
    candle_count: i64,
    source: String,
    dataset_hash: String,
}

const DATASET_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("exchange")='text' AND length(CAST("exchange" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("exchange")='text' AND length(CAST("exchange" AS BLOB))<=?2 THEN "exchange" ELSE NULL END,
 CASE WHEN typeof("symbol")='text' AND length(CAST("symbol" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("symbol")='text' AND length(CAST("symbol" AS BLOB))<=?2 THEN "symbol" ELSE NULL END,
 CASE WHEN typeof("interval")='text' AND length(CAST("interval" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("interval")='text' AND length(CAST("interval" AS BLOB))<=?2 THEN "interval" ELSE NULL END,
 CASE WHEN typeof("start_time")='integer' THEN 1 ELSE 0 END,
 CASE WHEN typeof("start_time")='integer' THEN "start_time" ELSE NULL END,
 CASE WHEN typeof("end_time")='integer' THEN 1 ELSE 0 END,
 CASE WHEN typeof("end_time")='integer' THEN "end_time" ELSE NULL END,
 CASE WHEN typeof("candle_count")='integer' AND "candle_count">=0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("candle_count")='integer' AND "candle_count">=0 THEN "candle_count" ELSE NULL END,
 CASE WHEN typeof("source")='text' AND length(CAST("source" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("source")='text' AND length(CAST("source" AS BLOB))<=?2 THEN "source" ELSE NULL END,
 CASE WHEN typeof("dataset_hash")='text' AND length(CAST("dataset_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("dataset_hash")='text' AND length(CAST("dataset_hash" AS BLOB))<=?2 THEN "dataset_hash" ELSE NULL END
 FROM datasets WHERE id=?1 ORDER BY id LIMIT 2"#;

fn read_dataset(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<DatasetRow> {
    let mut statement = tx.prepare(DATASET_SQL)?;
    let mut rows = statement.query(params![selector, MAX_TEXT_BYTES as i64])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = DatasetRow {
        id: positive_cell(row, 0)?,
        exchange: text_cell(row, 1, budget, |_| true)?,
        symbol: text_cell(row, 2, budget, |_| true)?,
        interval: text_cell(row, 3, budget, version)?,
        start_time: integer_cell(row, 4)?,
        end_time: integer_cell(row, 5)?,
        candle_count: nonnegative_cell(row, 6)?,
        source: text_cell(row, 7, budget, |_| true)?,
        dataset_hash: text_cell(row, 8, budget, dataset_hash)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}

#[derive(Clone, Eq, PartialEq)]
struct SnapshotSourceRow {
    snapshot_row_id: i64,
    provenance_id: i64,
}

const SOURCES_SQL: &str = r#"SELECT
 CASE WHEN typeof("snapshot_row_id")='integer' AND "snapshot_row_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("snapshot_row_id")='integer' AND "snapshot_row_id">0 THEN "snapshot_row_id" ELSE NULL END,
 CASE WHEN typeof("provenance_id")='integer' AND "provenance_id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("provenance_id")='integer' AND "provenance_id">0 THEN "provenance_id" ELSE NULL END
 FROM market_snapshot_sources WHERE snapshot_row_id=?1 ORDER BY provenance_id LIMIT 1025"#;

fn read_sources(
    tx: &Transaction<'_>,
    selector: i64,
    _budget: &mut ResourceBudget,
) -> AppResult<Vec<SnapshotSourceRow>> {
    let mut statement = tx.prepare(SOURCES_SQL)?;
    let mut rows = statement.query(params![selector])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        if records.len() == 1024 {
            return Err(invalid());
        } // sentinel, before payload
        records.push(SnapshotSourceRow {
            snapshot_row_id: positive_cell(row, 0)?,
            provenance_id: positive_cell(row, 1)?,
        });
    }
    Ok(records)
}

#[derive(Clone, Eq, PartialEq)]
struct ProvenanceRow {
    id: i64,
    record_hash: String,
    version: String,
    instrument_id: String,
    interval: String,
    source: String,
    role: String,
    request_scope_json: OriginalJson,
    raw_sha256: String,
    raw_byte_len: i64,
    raw_artifact_path: String,
    raw_media_type: String,
    retrieved_at: String,
    available_at: Option<String>,
    accepted: i64,
    rejection_reason: Option<String>,
    revision_of: Option<i64>,
}

const PROVENANCE_SQL: &str = r#"SELECT
 CASE WHEN typeof("id")='integer' AND "id">0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("id")='integer' AND "id">0 THEN "id" ELSE NULL END,
 CASE WHEN typeof("record_hash")='text' AND length(CAST("record_hash" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("record_hash")='text' AND length(CAST("record_hash" AS BLOB))<=?2 THEN "record_hash" ELSE NULL END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("version")='text' AND length(CAST("version" AS BLOB))<=?2 THEN "version" ELSE NULL END,
 CASE WHEN typeof("instrument_id")='text' AND length(CAST("instrument_id" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("instrument_id")='text' AND length(CAST("instrument_id" AS BLOB))<=?2 THEN "instrument_id" ELSE NULL END,
 CASE WHEN typeof("interval")='text' AND length(CAST("interval" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("interval")='text' AND length(CAST("interval" AS BLOB))<=?2 THEN "interval" ELSE NULL END,
 CASE WHEN typeof("source")='text' AND length(CAST("source" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("source")='text' AND length(CAST("source" AS BLOB))<=?2 THEN "source" ELSE NULL END,
 CASE WHEN typeof("role")='text' AND length(CAST("role" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("role")='text' AND length(CAST("role" AS BLOB))<=?2 THEN "role" ELSE NULL END,
 CASE WHEN typeof("request_scope_json")='text' AND length(CAST("request_scope_json" AS BLOB))<=?3 THEN 1 ELSE 0 END,
 CASE WHEN typeof("request_scope_json")='text' AND length(CAST("request_scope_json" AS BLOB))<=?3 THEN "request_scope_json" ELSE NULL END,
 CASE WHEN typeof("raw_sha256")='text' AND length(CAST("raw_sha256" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("raw_sha256")='text' AND length(CAST("raw_sha256" AS BLOB))<=?2 THEN "raw_sha256" ELSE NULL END,
 CASE WHEN typeof("raw_byte_len")='integer' AND "raw_byte_len">=0 THEN 1 ELSE 0 END,
 CASE WHEN typeof("raw_byte_len")='integer' AND "raw_byte_len">=0 THEN "raw_byte_len" ELSE NULL END,
 CASE WHEN typeof("raw_artifact_path")='text' AND length(CAST("raw_artifact_path" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("raw_artifact_path")='text' AND length(CAST("raw_artifact_path" AS BLOB))<=?2 THEN "raw_artifact_path" ELSE NULL END,
 CASE WHEN typeof("raw_media_type")='text' AND length(CAST("raw_media_type" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("raw_media_type")='text' AND length(CAST("raw_media_type" AS BLOB))<=?2 THEN "raw_media_type" ELSE NULL END,
 CASE WHEN typeof("retrieved_at")='text' AND length(CAST("retrieved_at" AS BLOB))<=?2 THEN 1 ELSE 0 END,
 CASE WHEN typeof("retrieved_at")='text' AND length(CAST("retrieved_at" AS BLOB))<=?2 THEN "retrieved_at" ELSE NULL END,
 CASE WHEN typeof("available_at")='null' OR (typeof("available_at")='text' AND length(CAST("available_at" AS BLOB))<=?2) THEN 1 ELSE 0 END,
 CASE WHEN typeof("available_at")='null' OR (typeof("available_at")='text' AND length(CAST("available_at" AS BLOB))<=?2) THEN "available_at" ELSE NULL END,
 CASE WHEN typeof("accepted")='integer' AND "accepted" IN (0,1) THEN 1 ELSE 0 END,
 CASE WHEN typeof("accepted")='integer' AND "accepted" IN (0,1) THEN "accepted" ELSE NULL END,
 CASE WHEN typeof("rejection_reason")='null' OR (typeof("rejection_reason")='text' AND length(CAST("rejection_reason" AS BLOB))<=?2) THEN 1 ELSE 0 END,
 CASE WHEN typeof("rejection_reason")='null' OR (typeof("rejection_reason")='text' AND length(CAST("rejection_reason" AS BLOB))<=?2) THEN "rejection_reason" ELSE NULL END,
 CASE WHEN typeof("revision_of")='null' OR (typeof("revision_of")='integer' AND "revision_of">0) THEN 1 ELSE 0 END,
 CASE WHEN typeof("revision_of")='null' OR (typeof("revision_of")='integer' AND "revision_of">0) THEN "revision_of" ELSE NULL END
 FROM market_provenance WHERE id=?1 ORDER BY id LIMIT 2"#;

fn read_provenance(
    tx: &Transaction<'_>,
    selector: i64,
    budget: &mut ResourceBudget,
) -> AppResult<ProvenanceRow> {
    let mut statement = tx.prepare(PROVENANCE_SQL)?;
    let mut rows = statement.query(params![
        selector,
        MAX_TEXT_BYTES as i64,
        MAX_JSON_BYTES as i64
    ])?;
    let row = rows.next()?.ok_or_else(invalid)?;
    let record = ProvenanceRow {
        id: positive_cell(row, 0)?,
        record_hash: text_cell(row, 1, budget, hash)?,
        version: text_cell(row, 2, budget, version)?,
        instrument_id: text_cell(row, 3, budget, version)?,
        interval: text_cell(row, 4, budget, version)?,
        source: text_cell(row, 5, budget, |_| true)?,
        role: text_cell(row, 6, budget, |s| matches!(s, "primary" | "comparison"))?,
        request_scope_json: json_cell(row, 7, budget)?,
        raw_sha256: text_cell(row, 8, budget, hash)?,
        raw_byte_len: nonnegative_cell(row, 9)?,
        raw_artifact_path: text_cell(row, 10, budget, |_| true)?,
        raw_media_type: text_cell(row, 11, budget, |_| true)?,
        retrieved_at: text_cell(row, 12, budget, |_| true)?,
        available_at: optional_text_cell(row, 13, budget, |_| true)?,
        accepted: boolean_cell(row, 14)?,
        rejection_reason: optional_text_cell(row, 15, budget, |_| true)?,
        revision_of: optional_positive_cell(row, 16)?,
    };
    if rows.next()?.is_some() {
        return Err(invalid());
    }
    Ok(record)
}
