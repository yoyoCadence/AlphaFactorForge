use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::{
    freeze_source_witness, parse_source_witness, SourceWitnessObservation,
};

#[test]
fn independent_node_source_witnesses_reconstruct_complete_original_facts() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/rs-core/research-source-witness-v1.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let witness = parse_source_witness(case["witnessText"].as_str().unwrap()).unwrap();
        let observation = reconstruct_source_witness(&witness).unwrap();
        let checked = SourceWitnessObservation::from_host_facts(
            witness.clone(),
            observation.anchor().clone(),
            observation.trial().clone(),
        )
        .unwrap();
        assert_eq!(checked.witness().sha256(), witness.sha256());
        assert_eq!(checked.trial(), observation.trial());
        let binding = parse_binding_snapshot(case["bindingJson"].as_str().unwrap()).unwrap();
        let anchored = reconstruct_historical_source(
            witness.trial_export_text().as_bytes(),
            &binding,
            witness.source_claims(),
        )
        .unwrap();
        assert_eq!(observation.archive_sha256(), case["archiveSha256"]);
        assert_eq!(observation.source_claims(), witness.source_claims());
        assert_eq!(observation.anchor(), anchored.anchor());
        assert!(observation.anchor().matches_binding(&binding));
        let trial = observation.trial();
        assert_eq!(
            Some(trial.family_effective_trials),
            case["expected"]["familyEffectiveTrials"].as_u64()
        );
        assert_eq!(
            Some(trial.registered_batch_effective_trials),
            case["expected"]["registeredBatchEffectiveTrials"].as_u64()
        );
        assert_eq!(
            Some(trial.complete_candidates),
            case["expected"]["completeCandidates"].as_u64()
        );
        assert_eq!(
            Some(trial.tests_per_trial),
            case["expected"]["testsPerTrial"].as_u64()
        );
        assert_eq!(trial.seq, 4);
        assert_eq!(trial.chain_head, case["anchor"]["chainHead"]);
    }
}

#[test]
fn source_witness_survives_source_removal_and_same_head_protocol_upgrade() {
    let f = fixture();
    let original = freeze_source_witness(std::str::from_utf8(&f.raw).unwrap(), &f.source).unwrap();
    let original_anchor = reconstruct_source_witness(&original)
        .unwrap()
        .anchor()
        .clone();
    register(
        &f.ledger,
        f.source
            .candidates
            .iter()
            .map(|c| event("original", c.candidate_index, &c.strategy_hash))
            .collect(),
        3,
    );
    let upgraded_raw = f.ledger.export_trial_projection_for_test().unwrap();
    let upgraded =
        freeze_source_witness(std::str::from_utf8(&upgraded_raw).unwrap(), &f.source).unwrap();
    let upgraded_observation = reconstruct_source_witness(&upgraded).unwrap();
    assert_eq!(
        original_anchor.prefix(),
        upgraded_observation.anchor().prefix()
    );
    assert_ne!(&original_anchor, upgraded_observation.anchor());
    assert_eq!(upgraded_observation.trial().tests_per_trial, 3);
    assert!(!original_anchor.matches_binding(&binding(&f.ledger)));
    drop(f); // closes/removes the actual source; no receiver supplies old facts.
    assert_eq!(
        reconstruct_source_witness(&original).unwrap().anchor(),
        &original_anchor
    );
    assert_eq!(
        reconstruct_source_witness(&original)
            .unwrap()
            .trial()
            .tests_per_trial,
        2
    );
}

#[test]
fn shape_only_witness_cannot_supply_invalid_or_incomplete_trial_facts() {
    let f = fixture();
    let opaque = freeze_source_witness("PROTECTED_SENTINEL\n", &f.source).unwrap();
    let error = reconstruct_source_witness(&opaque).unwrap_err();
    assert_eq!(error.code(), "registry_state_invalid");
    assert!(!error.to_string().contains("PROTECTED_SENTINEL"));
    for tamper in 0..3 {
        let mut source = f.source.clone();
        // Each independent tamper preserves source-domain validity.
        match tamper {
            0 => source.candidates[0].strategy_hash = format!("strategy-v2:{}", "f".repeat(64)),
            1 => source.candidates[1].candidate_index += 1,
            2 => source.source.workspace_id = "f".repeat(32),
            _ => unreachable!(),
        }
        let witness = freeze_source_witness(std::str::from_utf8(&f.raw).unwrap(), &source).unwrap();
        assert!(reconstruct_source_witness(&witness).is_err());
    }
    let mut subset = f.source.clone();
    subset.candidates.pop();
    let witness = freeze_source_witness(std::str::from_utf8(&f.raw).unwrap(), &subset).unwrap();
    assert!(reconstruct_source_witness(&witness).is_err());
    let mut changed_claim = f.source.clone();
    changed_claim.candidates[0].result_artifact_hash = "f".repeat(64);
    let witness =
        freeze_source_witness(std::str::from_utf8(&f.raw).unwrap(), &changed_claim).unwrap();
    // Artifact bytes are absent from trial records: retain the claim, do not
    // pretend this reconstruction authenticated publisher files or grants use.
    assert_eq!(
        reconstruct_source_witness(&witness)
            .unwrap()
            .source_claims(),
        &changed_claim
    );
}
