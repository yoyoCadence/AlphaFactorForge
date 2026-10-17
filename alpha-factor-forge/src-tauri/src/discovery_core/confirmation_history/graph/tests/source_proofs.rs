use super::*;

fn strict_fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../../fixtures/rs-core/research-confirmation-witness-graph-v1.json"
    ))
    .unwrap()
}
fn witness_fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../../fixtures/rs-core/research-source-witness-v1.json"
    ))
    .unwrap()
}
fn observation(case: usize) -> SourceWitnessObservation {
    let f = witness_fixture();
    let c = &f["cases"][case];
    let a = &c["anchor"];
    let anchor = SourceTrialAnchor::from_host_facts(
        HistoricalPrefix {
            registry_id: a["registryId"].as_str().unwrap().into(),
            seq: a["seq"].as_u64().unwrap(),
            chain_head: a["chainHead"].as_str().unwrap().into(),
        },
        vec![],
        vec![],
        serde_json::from_value(a["evidence"]["familyTests"].clone()).unwrap(),
    )
    .unwrap();
    let source: RegisteredSource = serde_json::from_value(f["registeredSource"].clone()).unwrap();
    let trial = HistoricalTrialObservation {
        registry_id: anchor.prefix().registry_id.clone(),
        seq: anchor.prefix().seq,
        chain_head: anchor.prefix().chain_head.clone(),
        trial_batch_id: format!("trial-batch-v1:{}", source.source.trial_batch_id),
        family_id: source.source.family_id.clone(),
        family_effective_trials: 4,
        registered_batch_effective_trials: 2,
        tests_per_trial: c["expected"]["testsPerTrial"].as_u64().unwrap(),
        complete_candidates: 2,
    };
    SourceWitnessObservation::from_host_facts(
        parse_source_witness(c["witnessText"].as_str().unwrap()).unwrap(),
        anchor,
        trial,
    )
    .unwrap()
}
fn strict_setup() -> (
    ConfirmationInventory,
    GraphContext,
    Vec<SourceWitnessObservation>,
) {
    let f = strict_fixture();
    let r = &f["records"];
    let mut i = ConfirmationInventory::default();
    for (names, section) in [
        (vec!["validation", "test"], &mut i.batches),
        (vec!["selection"], &mut i.selections),
        (
            vec!["validationOperation", "testOperation"],
            &mut i.operations,
        ),
        (vec!["validationExposure", "testExposure"], &mut i.exposures),
        (
            vec!["validationAdmission", "testAdmission"],
            &mut i.admissions,
        ),
        (
            vec!["validationStart", "validationSeal", "testStart", "testSeal"],
            &mut i.events,
        ),
    ] {
        for name in names {
            section.push(r[name].to_string());
        }
    }
    for b in f["blobs"].as_array().unwrap() {
        let kind = match b["kind"].as_str().unwrap() {
            "binding" => BlobKind::Binding,
            "source_witness" => BlobKind::SourceWitness,
            _ => BlobKind::Statistics,
        };
        let blob = HistoryBlob::new(kind, b["text"].as_str().unwrap()).unwrap();
        assert_eq!(blob.sha256(), b["sha256"]);
        i.blobs.push(blob);
    }
    let w = observation(0);
    let c = GraphContext {
        registered_sources: vec![w.witness().source_claims().clone()],
        allocations: vec![HostAllocation {
            family_id: w.trial().family_id.clone(),
            declaration: parse_alpha_allocation(&f["allocationDeclaration"]).unwrap(),
        }],
        reservations: ["validation", "test"]
            .iter()
            .map(|k| parse_alpha_reservation(&f["reservations"][k].to_string()).unwrap())
            .collect(),
        historical_prefixes: vec![w.anchor().prefix().clone()],
        historical_trials: vec![w.trial().clone()],
        family_tests: w.anchor().family_tests().clone(),
        witness_coverage: vec![HostWitnessCoverage {
            witness_sha256: w.witness().sha256().into(),
            unincorporated_origins: vec![],
        }],
        ..GraphContext::default()
    };
    (i, c, vec![w])
}
fn only_validation() -> (
    ConfirmationInventory,
    GraphContext,
    Vec<SourceWitnessObservation>,
) {
    let (mut i, c, w) = strict_setup();
    i.batches.truncate(1);
    i.selections.clear();
    i.operations.truncate(1);
    i.exposures.truncate(1);
    i.admissions.truncate(1);
    i.events.truncate(2);
    i.blobs.pop();
    (i, c, w)
}
fn roots(i: &ConfirmationInventory) -> ConfirmationInventory {
    ConfirmationInventory {
        blobs: i
            .blobs
            .iter()
            .filter(|b| b.kind() == BlobKind::SourceWitness)
            .cloned()
            .collect(),
        ..ConfirmationInventory::default()
    }
}
fn strict_bad(i: &ConfirmationInventory, c: &GraphContext, w: &[SourceWitnessObservation]) {
    let e = validate_witnessed_confirmation_graph(i, c, w).unwrap_err();
    assert!(!e.to_string().contains("PROTECTED_SENTINEL"));
}
fn add_witness(
    i: &mut ConfirmationInventory,
    c: &mut GraphContext,
    w: &mut Vec<SourceWitnessObservation>,
    next: SourceWitnessObservation,
) {
    c.witness_coverage.push(HostWitnessCoverage {
        witness_sha256: next.witness().sha256().into(),
        unincorporated_origins: vec![],
    });
    i.blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, next.witness().text()).unwrap());
    w.push(next);
}

#[test]
fn strict_actual_source_graph_retains_full_test_counts_and_legacy_entries_refuse_new_kind() {
    let (i, c, w) = strict_setup();
    let g = validate_witnessed_confirmation_graph(&i, &c, &w).unwrap();
    assert!(!g.is_restricted());
    assert_eq!(g.inventory().admissions.len(), 2);
    assert!(complete_confirmation_graph(&i, &c).is_err());
    let n = nodes(&i, Some(&WitnessHost::new(&i, &c, &w).unwrap())).unwrap();
    for a in &i.admissions {
        let id = doc(a)["id"].as_str().unwrap().to_string();
        assert!(n[&key(Kind::Admission, &id)]
            .deps
            .contains(&key(Kind::Blob, w[0].witness().sha256())));
        assert_eq!(doc(a)["body"]["precisionProof"]["familyTests"], 8);
    }
    assert_eq!(
        doc(&i.batches[1])["body"]["candidates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (legacy_i, legacy_c) = val_only();
    strict_bad(&legacy_i, &legacy_c, &[]);
}

#[test]
fn freeze_only_sources_and_standalone_roots_need_proof_but_no_invented_binding() {
    let (i, mut c, w) = strict_setup();
    c.historical_trials.clear();
    c.registered_sources.clear();
    let mut root = roots(&i);
    validate_witnessed_confirmation_graph(&root, &c, &w).unwrap();
    root.batches.push(i.batches[0].clone());
    validate_witnessed_confirmation_graph(&root, &c, &w).unwrap();
    strict_bad(&root, &c, &[]);
    root.blobs.clear();
    strict_bad(&root, &c, &w);
    strict_bad(&root, &c, &[]);
}

#[test]
fn new_roots_never_legalize_orphan_stats_or_binding_blobs() {
    let (i, c, w) = strict_setup();
    for blob in i
        .blobs
        .iter()
        .filter(|b| b.kind() != BlobKind::SourceWitness)
    {
        let mut root = roots(&i);
        root.blobs.push(blob.clone());
        strict_bad(&root, &c, &w);
    }
}

#[test]
fn no_same_prefix_or_unknown_laundering_of_missing_or_unchecked_source_bytes() {
    let (i, c, w) = strict_setup();
    let mut root = roots(&i);
    let other = observation(2);
    strict_bad(&root, &c, &[other]);
    let opaque =
        freeze_source_witness("PROTECTED_SENTINEL\n", w[0].witness().source_claims()).unwrap();
    root.blobs[0] = HistoryBlob::new(BlobKind::SourceWitness, opaque.text()).unwrap();
    root.unknown_origins
        .push(w[0].anchor().prefix().registry_id.clone());
    strict_bad(&root, &c, &w);
    strict_bad(&root, &c, &[]);
}

#[test]
fn same_head_protocol_roots_select_exact_original_eligible_anchor() {
    let (mut i, mut c, mut w) = strict_setup();
    c.family_tests.insert(w[0].trial().family_id.clone(), 3);
    add_witness(&mut i, &mut c, &mut w, observation(1));
    validate_witnessed_confirmation_graph(&i, &c, &w).unwrap();
    i.blobs.retain(|b| b.sha256() != w[0].witness().sha256());
    w.remove(0);
    strict_bad(&i, &c, &w);
    let root = roots(&i);
    validate_witnessed_confirmation_graph(&root, &c, &w).unwrap();
    c.historical_trials = vec![w[0].trial().clone()];
    strict_bad(&root, &c, &w); // unchanged eligible host pool still requires two tests
}

#[test]
fn equal_audit_facts_coalesce_and_conflicting_same_anchor_counts_never_win_by_order() {
    let (mut i, mut c, mut w) = strict_setup();
    add_witness(&mut i, &mut c, &mut w, observation(2));
    w.push(w[0].clone());
    let before = validate_witnessed_confirmation_graph(&i, &c, &w)
        .unwrap()
        .inventory()
        .clone();
    w.reverse();
    assert_eq!(
        validate_witnessed_confirmation_graph(&i, &c, &w)
            .unwrap()
            .inventory(),
        &before
    );
    let mut trial = w[0].trial().clone();
    trial.family_effective_trials = 5;
    let changed = SourceWitnessObservation::from_host_facts(
        w[0].witness().clone(),
        w[0].anchor().clone(),
        trial,
    )
    .unwrap();
    w.push(changed);
    strict_bad(&i, &c, &w);
    w.reverse();
    strict_bad(&i, &c, &w);
}

#[test]
fn original_source_key_cannot_hide_unequal_campaign_engine_or_candidate_metadata() {
    let (i, c, w) = strict_setup();
    for kind in 0..3 {
        let mut source = w[0].witness().source_claims().clone();
        match kind {
            0 => source.source.campaign_document_hash = "f".repeat(64),
            1 => source.source.engine_manifest_hash = "f".repeat(64),
            _ => source.candidates[0].result_artifact_hash = "f".repeat(64),
        }
        let snapshot = freeze_source_witness(w[0].witness().trial_export_text(), &source).unwrap();
        let next = SourceWitnessObservation::from_host_facts(
            snapshot,
            w[0].anchor().clone(),
            w[0].trial().clone(),
        )
        .unwrap();
        let mut root = roots(&i);
        let mut proofs = w.clone();
        let mut c = c.clone();
        add_witness(&mut root, &mut c, &mut proofs, next);
        strict_bad(&root, &c, &proofs);
        proofs.reverse();
        strict_bad(&root, &c, &proofs);
    }
    let mut c = c;
    c.registered_sources[0].candidates.pop();
    strict_bad(&i, &c, &w);
}

#[test]
fn actual_quarantine_covers_current_hwm_without_changing_original_anchor_or_current_map() {
    let (i, mut c, w) = only_validation();
    let family = w[0].trial().family_id.clone();
    c.family_tests.insert(family.clone(), 1);
    strict_bad(&i, &c, &w);
    c.quarantined_families = vec![family.clone()];
    assert!(validate_witnessed_confirmation_graph(&i, &c, &w)
        .unwrap()
        .is_restricted());
    assert_eq!(c.family_tests.get(&family), Some(&1));
    c.quarantined_families = vec![format!("trial-family-v1:{}", "f".repeat(64))];
    strict_bad(&i, &c, &w);
    c.quarantined_families = vec![family];
    let mut i = i;
    let pos = i
        .blobs
        .iter()
        .position(|b| b.kind() == BlobKind::Binding)
        .unwrap();
    let old = i.blobs[pos].sha256().to_string();
    let mut binding = doc(i.blobs[pos].text());
    binding["evidence"]["familyTests"][w[0].trial().family_id.clone()] = json!(3);
    i.blobs[pos] = HistoryBlob::new(BlobKind::Binding, &binding.to_string()).unwrap();
    let new = i.blobs[pos].sha256().to_string();
    // The existing node helper updates admission/start identities faithfully.
    change_admission(&mut i, |a| {
        a["registryObservation"]["bindingSha256"] = json!(new)
    });
    assert_ne!(old, new);
    strict_bad(&i, &c, &w);
}

#[test]
fn unincorporated_roots_need_actual_matching_stops_and_cannot_restore_lost_restrictions() {
    let (i, mut c, w) = strict_setup();
    let mut root = roots(&i);
    c.historical_prefixes.clear();
    c.historical_trials.clear();
    strict_bad(&root, &c, &w);
    root.unknown_origins = vec!["f".repeat(32)];
    strict_bad(&root, &c, &w);
    root.unknown_origins = vec![w[0].anchor().prefix().registry_id.clone()];
    c.witness_coverage[0].unincorporated_origins = root.unknown_origins.clone();
    assert!(validate_witnessed_confirmation_graph(&root, &c, &w)
        .unwrap()
        .is_restricted());
    let a = SourceTrialAnchor::from_host_facts(
        w[0].anchor().prefix().clone(),
        vec![w[0].trial().family_id.clone()],
        vec![],
        w[0].anchor().family_tests().clone(),
    )
    .unwrap();
    let changed =
        SourceWitnessObservation::from_host_facts(w[0].witness().clone(), a, w[0].trial().clone())
            .unwrap();
    strict_bad(&root, &c, &[changed]);
}

#[test]
fn observation_constructor_checks_all_fact_links_but_retains_generic_noneffective_sources() {
    let w = observation(0);
    for change in 0..10 {
        let mut t = w.trial().clone();
        match change {
            0 => t.registry_id = "f".repeat(32),
            1 => t.seq += 1,
            2 => t.chain_head = "f".repeat(64),
            3 => t.family_id = format!("trial-family-v1:{}", "f".repeat(64)),
            4 => t.trial_batch_id = format!("trial-batch-v1:{}", "f".repeat(64)),
            5 => t.complete_candidates = 1,
            6 => t.family_effective_trials = PRECISION_MAX_COUNT + 1,
            7 => t.tests_per_trial = 3,
            8 => t.family_effective_trials = 0,
            _ => t.registered_batch_effective_trials = 3,
        }
        assert!(SourceWitnessObservation::from_host_facts(
            w.witness().clone(),
            w.anchor().clone(),
            t
        )
        .is_err());
    }
    let mut t = w.trial().clone();
    t.registered_batch_effective_trials = 0;
    assert!(
        SourceWitnessObservation::from_host_facts(w.witness().clone(), w.anchor().clone(), t)
            .is_ok()
    );
}

#[test]
fn material_admission_edge_matches_every_original_anchor_component() {
    let (i, c, w) = strict_setup();
    let h = WitnessHost::new(&i, &c, &w).unwrap();
    let b = i.blobs.iter().find_map(|v| v.binding.as_ref()).unwrap();
    for change in 0..6 {
        let mut b = b.clone();
        match change {
            0 => b.registry_id = "f".repeat(32),
            1 => b.seq += 1,
            2 => b.chain_head = "f".repeat(64),
            3 => b
                .evidence
                .quarantined_families
                .push(w[0].trial().family_id.clone()),
            4 => b.evidence.conflicted_origins.push("f".repeat(32)),
            _ => {
                b.evidence
                    .family_tests
                    .insert(w[0].trial().family_id.clone(), 3);
            }
        }
        let mut deps = BTreeSet::new();
        assert!(h
            .source_dependencies(
                &mut deps,
                &serde_json::to_value(&w[0].witness().source_claims().source).unwrap(),
                Some(&b)
            )
            .is_err());
    }
}

#[test]
fn audit_duplicate_roots_do_not_multiply_material_admission_edges() {
    let (mut i, mut c, mut w) = strict_setup();
    add_witness(&mut i, &mut c, &mut w, observation(2));
    let host = WitnessHost::new(&i, &c, &w).unwrap();
    let n = nodes(&i, Some(&host)).unwrap();
    let expected = w.iter().map(|v| v.witness().sha256()).min().unwrap();
    for (k, node) in n.iter().filter(|(k, _)| k.0 == Kind::Admission) {
        let proofs: Vec<_> = node
            .deps
            .iter()
            .filter(|d| {
                w.iter()
                    .any(|w| d.0 == Kind::Blob && d.1 == w.witness().sha256())
            })
            .collect();
        assert_eq!(proofs, vec![&key(Kind::Blob, expected)], "{k:?}");
    }
}

#[test]
fn source_witness_roots_share_the_existing_joint_row_capacity() {
    let (i, c, w) = strict_setup();
    let mut root = roots(&i);
    root.exposures = (0..MAX_HISTORY_ROWS - 1).map(|n| {
        freeze_manual_exposure(&json!({"contractVersion":EXPOSURE_VERSION,"originRegistryId":"2".repeat(32),"reservationKey":format!("{n:032x}"),
            "instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":946684800000u64,"toExclusiveMs":946684800001u64}],
            "claim":{"kind":"manual","accessKind":"raw","afterSealId":null}}).to_string()).unwrap().document().to_string()
    }).collect();
    let valid = validate_witnessed_confirmation_graph(&root, &c, &w).unwrap();
    assert_eq!(
        valid.inventory().exposures.len() + valid.inventory().blobs.len(),
        MAX_HISTORY_ROWS
    );
    root.exposures.push(root.exposures[0].clone());
    assert_eq!(
        validate_witnessed_confirmation_graph(&root, &c, &w).unwrap_err(),
        HistoryRecordError::Capacity
    );
}

#[test]
fn complete_witness_coverage_cannot_be_inferred_from_an_equal_local_prefix() {
    let (i, mut c, w) = strict_setup();
    let mut root = roots(&i);
    c.witness_coverage.clear();
    strict_bad(&root, &c, &w);
    let foreign = "f".repeat(32);
    c.witness_coverage.push(HostWitnessCoverage {
        witness_sha256: w[0].witness().sha256().into(),
        unincorporated_origins: vec![foreign.clone()],
    });
    strict_bad(&root, &c, &w); // header prefix still matches; foreign rows do not
    root.unknown_origins = vec![w[0].anchor().prefix().registry_id.clone()];
    strict_bad(&root, &c, &w);
    root.unknown_origins = vec![foreign.clone()];
    assert!(validate_witnessed_confirmation_graph(&root, &c, &w)
        .unwrap()
        .is_restricted());
    c.witness_coverage.push(HostWitnessCoverage {
        witness_sha256: w[0].witness().sha256().into(),
        unincorporated_origins: vec![],
    });
    strict_bad(&root, &c, &w); // unequal duplicate coverage cannot win by order
    c.witness_coverage.pop();
    c.witness_coverage[0].unincorporated_origins.push(foreign);
    strict_bad(&root, &c, &w);
}
