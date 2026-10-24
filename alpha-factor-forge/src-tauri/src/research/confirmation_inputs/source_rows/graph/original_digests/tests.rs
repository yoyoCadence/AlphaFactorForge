use super::super::tests::{capture, damage, fixture, large_family, locator};
use super::*;
use crate::research::confirmation_inputs::{
    source_rows::graph::{capture_source_rows_in, recheck_source_rows_in},
    tests::{snapshot, unchanged, writer_busy},
};
use rusqlite::TransactionBehavior;
use serde_json::Value;

fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../../../fixtures/research/raw-original-digests-v1.json"
    ))
    .unwrap()
}

fn refused<T>(result: AppResult<T>) {
    assert!(matches!(result, Err(AppError::Other(ref code)) if code == "original_digests_invalid"));
}

fn width(value: &str) {
    assert_eq!(value.len(), 64);
    assert!(value
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
}

fn output_bytes(observed: &RawDigestObservation) -> usize {
    observed.campaign_document_hash().len()
        + observed.engine_manifest_hash().len()
        + observed
            .candidates()
            .iter()
            .map(|c| c.definition_hash().len() + c.execution_hash().len() + c.lineage_hash().len())
            .sum::<usize>()
}

#[test]
fn captured_graph_matches_all_independent_original_byte_vectors() {
    let mut f = fixture();
    let rows = capture(&mut f);
    let saved = rows.clone();
    let prepared = preflight(&rows).unwrap();
    let expected = oracle();
    assert_eq!(
        prepared.work_bytes,
        expected["workBytes"].as_u64().unwrap() as usize
    );
    assert_eq!(prepared.output_text_bytes, 704);
    let observed = observe_raw_digests(&rows).unwrap();
    assert_eq!(
        observed.campaign_document_hash(),
        expected["campaignDocumentHash"]
    );
    assert_eq!(
        observed.engine_manifest_hash(),
        expected["engineManifestHash"]
    );
    assert_eq!(observed.candidates().len(), 3);
    for (candidate, expected) in observed
        .candidates()
        .iter()
        .zip(expected["candidates"].as_array().unwrap())
    {
        assert_eq!(candidate.index(), expected["index"].as_i64().unwrap());
        assert_eq!(candidate.definition_hash(), expected["definitionHash"]);
        assert_eq!(candidate.execution_hash(), expected["executionHash"]);
        assert_eq!(candidate.lineage_hash(), expected["lineageHash"]);
        for digest in [
            candidate.definition_hash(),
            candidate.execution_hash(),
            candidate.lineage_hash(),
        ] {
            width(digest);
        }
    }
    width(observed.campaign_document_hash());
    width(observed.engine_manifest_hash());
    assert_eq!(
        output_bytes(&observed),
        expected["outputTextBytes"].as_u64().unwrap() as usize
    );
    assert!(observed == observe_raw_digests(&rows).unwrap());
    assert!(observed == observed.clone());
    assert!(saved == rows);
    assert_eq!(
        rows.strategies[0].original_definition_json.as_bytes(),
        b" \n{\"n\":0.0036944444444444438,\"huge\":1e999}\t"
    );
}

#[test]
fn independent_framing_controls_keep_counts_lengths_order_whitespace_and_zeroes() {
    for control in oracle()["framingControls"].as_array().unwrap() {
        let domain = match control["domain"].as_str().unwrap() {
            "definition" => Domain::Definition,
            "execution" => Domain::Execution,
            _ => panic!("unknown control domain"),
        };
        let parts: Vec<_> = control["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().as_bytes())
            .collect();
        assert_eq!(frame(domain, &parts).unwrap(), control["hash"]);
        let mut work = HashWork { bytes: 0 };
        work.frame(domain, &parts.iter().map(|p| p.len()).collect::<Vec<_>>())
            .unwrap();
        assert_eq!(work.bytes, control["bytes"].as_u64().unwrap() as usize);
    }
}

#[test]
fn pending_original_field_changes_affect_only_their_exact_digest_inputs() {
    let mut f = fixture();
    damage(&mut f);
    let baseline = observe_raw_digests(&capture(&mut f)).unwrap();
    let expected = f.expected.clone();
    for (sql, campaign, manifest, definitions, executions, lineages) in [
        ("UPDATE research_campaigns SET document_json=' '||document_json", true, false, vec![], vec![], vec![]),
        ("UPDATE strategy_def SET original_definition_json=' '||original_definition_json WHERE id=100", false, false, vec![0], vec![], vec![]),
        ("UPDATE discovery_runs SET config_json=' '||config_json", false, false, vec![], vec![0,1,2], vec![]),
        ("UPDATE hypotheses SET content_json=' '||content_json", false, false, vec![], vec![], vec![0,1,2]),
        ("UPDATE research_attempts SET input_fingerprint_json=' '||input_fingerprint_json WHERE id=400", false, false, vec![], vec![], vec![0]),
        ("UPDATE research_attempts SET engine_fingerprint_json=' '||engine_fingerprint_json WHERE id=400", false, true, vec![], vec![0], vec![0]),
        ("UPDATE research_attempts SET trial_event_id='trial-event-v1:0000000000000000000000000000000000000000000000000000000000000004' WHERE id=400", false, false, vec![], vec![], vec![0]),
        // The last engine also enters the manifest, rather than only the first.
        ("UPDATE research_attempts SET engine_fingerprint_json=' { } ' WHERE id=402", false, true, vec![], vec![2], vec![2]),
    ] {
        let tx = f.connection().transaction().unwrap();
        // Test-owned pending damage only; rollback restores the append-only guard.
        if campaign {
            tx.execute_batch("DROP TRIGGER research_campaigns_no_update").unwrap();
        }
        tx.execute_batch(sql).unwrap();
        let rows = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
        let saved = rows.clone();
        let observed = unchanged(&tx, || observe_raw_digests(&rows)).unwrap();
        assert!(saved == rows);
        assert_eq!(observed.campaign_document_hash() != baseline.campaign_document_hash(), campaign, "{sql}");
        assert_eq!(observed.engine_manifest_hash() != baseline.engine_manifest_hash(), manifest, "{sql}");
        for i in 0..3 {
            let a = &observed.candidates()[i];
            let b = &baseline.candidates()[i];
            assert_eq!(a.definition_hash() != b.definition_hash(), definitions.contains(&i), "{sql}: {i}");
            assert_eq!(a.execution_hash() != b.execution_hash(), executions.contains(&i), "{sql}: {i}");
            assert_eq!(a.lineage_hash() != b.lineage_hash(), lineages.contains(&i), "{sql}: {i}");
        }
        tx.rollback().unwrap();
    }
}

#[test]
fn failed_members_and_all_engines_remain_in_complete_manifest() {
    let mut f = fixture();
    damage(&mut f);
    let baseline = observe_raw_digests(&capture(&mut f)).unwrap();
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    tx.execute_batch("UPDATE discovery_jobs SET status='failed' WHERE candidate_index=1; UPDATE research_attempts SET status='failed' WHERE candidate_index=1").unwrap();
    let rows = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    let failed = observe_raw_digests(&rows).unwrap();
    assert_eq!(failed.candidates().len(), 3);
    assert!(failed == baseline); // status is captured/rechecked, not a raw digest part
    tx.execute_batch(
        "UPDATE research_attempts SET engine_fingerprint_json='[1e999]' WHERE candidate_index=1",
    )
    .unwrap();
    let changed =
        observe_raw_digests(&capture_source_rows_in(&tx, &expected, &locator()).unwrap()).unwrap();
    assert_ne!(
        changed.engine_manifest_hash(),
        baseline.engine_manifest_hash()
    );
    assert_ne!(
        changed.candidates()[1].execution_hash(),
        baseline.candidates()[1].execution_hash()
    );
    assert_eq!(
        changed.candidates()[0].execution_hash(),
        baseline.candidates()[0].execution_hash()
    );
    tx.rollback().unwrap();
}

#[test]
fn whole_work_arithmetic_is_inclusive_checked_and_atomic_with_prior_usage() {
    let mut work = HashWork { bytes: 7 };
    work.add(33_554_425).unwrap();
    assert_eq!(work.bytes, 33_554_432);
    refused(work.add(1));
    assert_eq!(work.bytes, 33_554_432);
    refused(work.add(usize::MAX));
    assert_eq!(work.bytes, 33_554_432);
    let mut work = HashWork { bytes: 19 };
    refused(work.frame(Domain::Lineage, &[usize::MAX]));
    assert_eq!(work.bytes, 19);
    refused(work.frame(Domain::Execution, &[33_554_432]));
    assert_eq!(work.bytes, 19);
    work.frame(Domain::Definition, &[2]).unwrap();
    assert_eq!(work.bytes, 64);
}

struct DenySha;
impl DenySha {
    fn new() -> Self {
        DENY_SHA.with(|deny| assert!(!deny.replace(true)));
        Self
    }
}
impl Drop for DenySha {
    fn drop(&mut self) {
        DENY_SHA.with(|deny| deny.set(false));
    }
}

#[test]
fn real_captured_exact_and_one_over_whole_work_preflight_before_first_sha() {
    let mut f = fixture();
    damage(&mut f);
    let other = f.independent();
    let expected = f.expected.clone();
    let before = snapshot(f.connection());
    let tx = f
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    // Independently derived from 7938: each of three engine byte strings is
    // charged in execution, lineage and manifest; the one shared hypothesis is
    // charged for each candidate, and candidate-zero definition once.
    let engine = format!("\"{}\"", "x".repeat(2_499_998));
    let hypothesis = format!("\"{}\"", "y".repeat(3_682_211));
    tx.execute(
        "UPDATE research_attempts SET engine_fingerprint_json=?1",
        [&engine],
    )
    .unwrap();
    tx.execute("UPDATE hypotheses SET content_json=?1", [&hypothesis])
        .unwrap();
    tx.execute_batch("UPDATE strategy_def SET original_definition_json='  '||original_definition_json WHERE id=100").unwrap();
    let rows = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    assert!(rows.usage.text_bytes < 33_554_432);
    assert_eq!(preflight(&rows).unwrap().work_bytes, 33_554_432);
    let exact = unchanged(&tx, || observe_raw_digests(&rows)).unwrap();
    assert_eq!(exact.candidates().len(), 3);
    writer_busy(&other);
    tx.execute_batch("UPDATE strategy_def SET original_definition_json=' '||original_definition_json WHERE id=100").unwrap();
    let rows = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    assert!(rows.usage.text_bytes < 33_554_432);
    let updates = SHA_UPDATES.with(|count| count.get());
    {
        let _deny = DenySha::new();
        refused(unchanged(&tx, || observe_raw_digests(&rows)));
    }
    assert_eq!(SHA_UPDATES.with(|count| count.get()), updates);
    writer_busy(&other);
    assert!(!tx.is_autocommit());
    tx.rollback().unwrap();
    assert_eq!(snapshot(f.connection()), before);
    other.execute_batch("BEGIN IMMEDIATE; ROLLBACK").unwrap();
}

#[test]
fn actual_4096_candidate_graph_has_exact_bounded_output_and_all_indexes() {
    let mut f = fixture();
    damage(&mut f);
    large_family(&mut f);
    let rows = capture(&mut f);
    assert_eq!(rows.jobs.len(), 8192);
    let prepared = preflight(&rows).unwrap();
    assert_eq!(prepared.output_text_bytes, 786_560);
    let observed = observe_raw_digests(&rows).unwrap();
    assert_eq!(observed.candidates().len(), 4096);
    assert_eq!(output_bytes(&observed), 786_560);
    for (i, c) in observed.candidates().iter().enumerate() {
        assert_eq!(c.index(), i as i64);
        width(c.definition_hash());
        width(c.execution_hash());
        width(c.lineage_hash());
    }
    width(observed.engine_manifest_hash());
}

#[test]
fn historical_observation_does_not_recheck_owner_or_release_caller_reservation() {
    let mut f = fixture();
    let saved = capture(&mut f);
    let raw = observe_raw_digests(&saved).unwrap();
    let expected = f.expected.clone();
    let other = f.independent();
    let baseline = snapshot(f.connection());
    let tx = f
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute_batch("UPDATE runtime_state SET mutation_seq=mutation_seq+1 WHERE id=1")
        .unwrap();
    let historical = unchanged(&tx, || observe_raw_digests(&saved)).unwrap();
    assert!(historical == raw);
    assert!(recheck_source_rows_in(&tx, &expected, &saved).is_err());
    writer_busy(&other);
    tx.rollback().unwrap();
    assert_eq!(snapshot(f.connection()), baseline);
    // A saved copy remains just bytes even after its fixture/lease disappears.
    drop(other);
    drop(f);
    assert!(observe_raw_digests(&saved).unwrap() == raw);
}

#[test]
fn invalid_complete_shape_is_refused_before_any_hash_update() {
    let mut f = fixture();
    let rows = capture(&mut f);
    let mut malformed = Vec::new();
    let mut changed = rows.clone();
    changed.attempts.clear();
    malformed.push(changed);
    let mut changed = rows.clone();
    changed.attempts.resize(4097, rows.attempts[0].clone());
    malformed.push(changed);
    let mut changed = rows.clone();
    changed.attempts[2].candidate_index = Some(1);
    malformed.push(changed);
    let mut changed = rows.clone();
    changed.attempts[2].candidate_index = None;
    malformed.push(changed);
    let mut changed = rows.clone();
    changed.attempts[2].trial_event_id = None;
    malformed.push(changed);
    let mut changed = rows.clone();
    changed.strategies.pop();
    malformed.push(changed);
    let mut changed = rows.clone();
    changed.hypotheses.clear();
    malformed.push(changed);
    // Test descendants can damage copies, but production gets no unchecked
    // constructor or mutation API. Late-member failures must precede SHA too.
    let updates = SHA_UPDATES.with(|count| count.get());
    let _deny = DenySha::new();
    for changed in &malformed {
        refused(observe_raw_digests(changed));
    }
    assert_eq!(SHA_UPDATES.with(|count| count.get()), updates);
}
