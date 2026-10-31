//! Original pure producers accept declared scalars; only the integrated control
//! below may obtain VerifiedDataset from the real unchanged loader.
use super::*;
use crate::research::trial_ledger::projection_controls as controls;
use alpha_factor_forge::discovery_core::split::plan_validation_split;

#[test]
fn original_trial_all_41_candidate_identities_hashes_and_nullable_reports() {
    const { assert!(usize::BITS >= 64, "frozen original target is 64-bit") };
    let v = controls::oracle();
    let mut count = 0;
    for case in v["cases"].as_array().unwrap() {
        let raw: Value = serde_json::from_str(case["configJson"].as_str().unwrap()).unwrap();
        let saved = raw.clone();
        let config = parse_discovery_config(&raw, 8.0).unwrap();
        let plan = enumerate_candidates(&config).unwrap();
        let plan_before = plan.clone();
        let expected = case["candidates"].as_array().unwrap();
        assert_eq!(plan.candidates.len(), expected.len(), "{}", case["id"]);
        if case["id"] == "v1-shared-dedup" {
            assert_eq!(plan.counts.raw, 6);
            assert_eq!(plan.counts.final_unique, 4);
            assert_eq!(plan.counts.duplicates, 2);
        }
        let total = usize::try_from(case["declaredCandleCount"].as_u64().unwrap()).unwrap();
        let hashes = candidate_split_hashes(&config, total).unwrap();
        assert_eq!(hashes.len(), expected.len());
        let engine = serde_json::to_string(&execution::engine_fingerprint(&config)).unwrap();
        assert_eq!(engine, case["engineFingerprintJson"]);
        assert_eq!(
            sha256_hex(&canonical_json(&raw).unwrap()),
            case["configHash"]
        );
        assert_eq!(
            sha256_hex(case["configJson"].as_str().unwrap().as_bytes()),
            case["configRawSha256"]
        );
        for (candidate, literal) in plan.candidates.iter().zip(expected) {
            count += 1;
            assert_eq!(candidate.index, literal["index"]);
            assert_eq!(candidate.strategy_hash, literal["strategyHash"]);
            assert_eq!(
                serde_json::to_string(&candidate.strategy).unwrap(),
                literal["definitionJson"]
            );
            let seeds = serde_json::to_string(&candidate.seeds).unwrap();
            assert_eq!(seeds, literal["seedsJson"]);
            assert_eq!(sha256_hex(seeds.as_bytes()), literal["seedsHash"]);
            assert_eq!(
                sha256_hex(engine.as_bytes()),
                literal["engineFingerprintHash"]
            );
            let identity = execution::trial_split_identity(&config, candidate, total);
            assert_eq!(
                serde_json::to_string(&identity).unwrap(),
                literal["identityJson"]
            );
            let payload = json!({"version":"trial-split-v1", "identity":identity});
            assert_eq!(
                serde_json::to_string(&payload).unwrap(),
                literal["splitIdentityPayloadJson"]
            );
            assert_eq!(split_hash_for(&identity).unwrap(), literal["splitHash"]);
            assert_eq!(
                hashes[&candidate.index],
                (
                    candidate.strategy_hash.clone(),
                    literal["splitHash"].as_str().unwrap().into()
                )
            );
            let report = &literal["candidateWalkForwardReport"];
            match config.walk_forward {
                None => assert_eq!(report, &json!({"status":"not_applicable"})),
                Some(input) => {
                    let result = candidate_walk_forward_report(&config, candidate, total, input);
                    if report["status"] == "ok" {
                        assert_eq!(
                            serde_json::to_value(result.unwrap()).unwrap(),
                            report["report"]
                        );
                    } else {
                        let error = result.unwrap_err().to_string();
                        if report["phase"] == "candidate_lookback" {
                            assert!(
                                error.contains("below the candidate signal lookback"),
                                "{error}"
                            );
                        }
                    }
                }
            }
            assert_eq!(
                execution::trial_split_identity(&config, candidate, total),
                identity
            );
        }
        assert_eq!(plan, plan_before);
        assert_eq!(raw, saved);
    }
    assert_eq!(count, 41);
}

#[test]
fn original_trial_all_nine_split_primitive_boundaries_match_literals() {
    let v = controls::oracle();
    let all = v["primitiveSplitControls"].as_array().unwrap();
    assert_eq!(all.len(), 9);
    for c in all {
        let total = c["totalBars"].as_i64().unwrap();
        let embargo = c["embargoBars"].as_i64().unwrap();
        let result = plan_validation_split(total, embargo);
        if c["expectedStatus"] == "ok" {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                c["expectedPlan"]
            );
        } else {
            assert!(result.is_err());
        }
    }
}

fn real_market(f: &mut controls::Harness) -> (String, String) {
    use crate::market::{
        provenance, registry,
        snapshot::{self, SnapshotOutcome},
    };
    let source: Value = serde_json::from_str(include_str!(
        "../../../fixtures/research/original-market-source-projections-v1.json"
    ))
    .unwrap();
    let registry_data: Value = serde_json::from_str(include_str!(
        "../../../fixtures/research/original-market-registry-projections-v1.json"
    ))
    .unwrap();
    registry::register_calendar(
        f.conn(),
        &serde_json::from_str(registry_data["calendars"][0]["inputJson"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    registry::register_instrument(
        f.conn(),
        &serde_json::from_str(
            registry_data["instruments"][0]["inputJson"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let d = &source["datasets"][0];
    let m = &d["metadata"];
    let dataset = repositories::Dataset {
        id: None,
        exchange: m["exchange"].as_str().unwrap().into(),
        symbol: m["symbol"].as_str().unwrap().into(),
        interval: m["interval"].as_str().unwrap().into(),
        start_time: m["startTime"].as_i64().unwrap(),
        end_time: m["endTime"].as_i64().unwrap(),
        candle_count: 4,
        source: m["source"].as_str().unwrap().into(),
        dataset_hash: m["datasetHash"].as_str().unwrap().into(),
    };
    let candles: Vec<repositories::Candle> = serde_json::from_value(d["candles"].clone()).unwrap();
    assert_eq!(
        repositories::import_dataset_with_candles(f.conn_mut(), &dataset, &candles).unwrap(),
        1
    );
    let store = ArtifactStore::in_data_dir(&f.workspace);
    for step in source["provenanceSequence"].as_array().unwrap() {
        let c = source["provenance"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == step["input"])
            .unwrap();
        let input = serde_json::from_str(c["inputJson"].as_str().unwrap()).unwrap();
        assert_eq!(
            provenance::record_raw(
                f.conn(),
                &store,
                &input,
                &hex::decode(c["rawHex"].as_str().unwrap()).unwrap()
            )
            .unwrap(),
            (
                step["expectedRow"].as_i64().unwrap(),
                step["created"].as_bool().unwrap()
            )
        );
    }
    let literal = &source["snapshots"][0];
    let request = serde_json::from_str(literal["requestJson"].as_str().unwrap()).unwrap();
    let result = snapshot::build_snapshot(f.conn_mut(), &request).unwrap();
    let snapshot = match result {
        SnapshotOutcome::Created(s) => s,
        other => panic!("fresh original snapshot: {other:?}"),
    };
    assert_eq!(snapshot.snapshot_id, literal["expected"]["snapshotId"]);
    (snapshot.instrument_id, snapshot.snapshot_id)
}

#[test]
fn original_trial_real_four_bar_runner_registers_exact_links_and_preserves_caller() {
    use crate::research::trial_ledger::{write_workspace_binding, BindingCheck};
    let mut f = controls::Harness::new();
    let snapshot = real_market(&mut f);
    let workspace_id = crate::db::runtime_ledger::workspace_id(f.conn()).unwrap();
    let case = controls::real_workspace_case(&controls::named("v1-market-small"), &workspace_id);
    assert_ne!(workspace_id, "trial-projection-workspace");
    let raw: Value = serde_json::from_str(case["configJson"].as_str().unwrap()).unwrap();
    let config = parse_discovery_config(&raw, 8.0).unwrap();
    let (dataset, candles) = load_verified_dataset(f.conn(), &config).unwrap();
    assert_eq!(dataset.candle_count, 4);
    assert_eq!(candles.len(), 4);
    assert_eq!(dataset.content_hash, config.dataset.content_hash);
    assert_eq!(snapshot.1, case["batchInput"]["events"][0]["snapshotId"]);
    let plan = enumerate_candidates(&config).unwrap();
    let scheduled = plan
        .candidates
        .into_iter()
        .map(|candidate| {
            let dto = candidate_strategy_def(&candidate).unwrap();
            let strategy_id =
                repositories::get_or_insert_verified_runner_strategy(f.conn(), Some(f.epoch), &dto)
                    .unwrap();
            ScheduledCandidate {
                candidate,
                strategy_id,
            }
        })
        .collect::<Vec<_>>();
    let run_id = discovery::create_discovery_run(
        f.conn(),
        Some(f.epoch),
        "original trial metadata control",
        case["configJson"].as_str().unwrap(),
    )
    .unwrap();
    let lineage = run_lineage(&config, &raw, run_id, &dataset, &scheduled, Some(f.epoch)).unwrap();
    for ((hypothesis, attempt), literal) in
        lineage.iter().zip(case["candidates"].as_array().unwrap())
    {
        assert_eq!(hypothesis.hash().unwrap(), literal["hypothesisHash"]);
        assert_eq!(
            attempt.input_fingerprint["strategyHash"],
            literal["strategyHash"]
        );
        assert_eq!(
            serde_json::to_string(&attempt.input_fingerprint["seeds"]).unwrap(),
            literal["seedsJson"]
        );
        assert_eq!(
            sha256_hex(&canonical_json(&attempt.engine_fingerprint).unwrap()),
            literal["engineFingerprintHash"]
        );
    }
    let missing_binding_workspace = controls::image(f.conn());
    let missing_binding_registry = controls::ledger_image(f.ledger());
    let missing_binding_artifacts = f.artifacts();
    let request = case["batchInput"]["events"][0]["origin"]["requestId"]
        .as_str()
        .unwrap();
    let missing = register_lineage_with(
        f.ledger(),
        f.conn(),
        &workspace_id,
        request,
        &config,
        &dataset,
        &lineage,
        Some(snapshot.clone()),
    )
    .err()
    .expect("missing original workspace binding");
    assert!(missing.to_string().contains("workspace binding is absent"));
    assert_eq!(controls::ledger_image(f.ledger()), missing_binding_registry);
    f.assert_workspace(&missing_binding_workspace, &missing_binding_artifacts);
    let initial = match f.ledger().check_binding(None).unwrap() {
        BindingCheck::Current(head) => head,
        other => panic!("fresh binding: {other:?}"),
    };
    write_workspace_binding(f.conn(), &initial).unwrap();
    let before_pending = controls::image(f.conn());
    f.conn().execute_batch("SAVEPOINT caller_pending; INSERT INTO app_settings(key,value_json) VALUES('original-trial-caller','{\"pending\":true}')").unwrap();
    let workspace = controls::image(f.conn());
    let artifacts = f.artifacts();
    let before_registry = controls::ledger_image(f.ledger());
    let request = case["batchInput"]["events"][0]["origin"]["requestId"]
        .as_str()
        .unwrap();
    let mut wrong = lineage.clone();
    wrong[2].1.candidate_index = Some(99);
    assert!(register_lineage_with(
        f.ledger(),
        f.conn(),
        &workspace_id,
        request,
        &config,
        &dataset,
        &wrong,
        Some(snapshot.clone())
    )
    .is_err());
    assert_eq!(controls::ledger_image(f.ledger()), before_registry);
    f.assert_workspace(&workspace, &artifacts);
    for defect in ["strategy", "seeds", "index"] {
        let mut wrong = lineage.clone();
        match defect {
            "strategy" => {
                wrong[2].1.input_fingerprint["strategyHash"] = json!("changed-original-claim")
            }
            "seeds" => {
                wrong[2]
                    .1
                    .input_fingerprint
                    .as_object_mut()
                    .unwrap()
                    .remove("seeds");
            }
            "index" => wrong[2].1.candidate_index = None,
            _ => unreachable!(),
        }
        assert!(
            register_lineage_with(
                f.ledger(),
                f.conn(),
                &workspace_id,
                request,
                &config,
                &dataset,
                &wrong,
                Some(snapshot.clone())
            )
            .is_err(),
            "{defect}"
        );
        assert_eq!(controls::ledger_image(f.ledger()), before_registry);
        f.assert_workspace(&workspace, &artifacts);
    }
    let registered = register_lineage_with(
        f.ledger(),
        f.conn(),
        &workspace_id,
        request,
        &config,
        &dataset,
        &lineage,
        Some(snapshot.clone()),
    )
    .unwrap();
    assert_eq!(
        json!(registered.event_ids),
        case["expectedBatch"]["returnedEventIdsInputOrder"]
    );
    assert_eq!(registered.batch_id, case["expectedBatch"]["batchId"]);
    let actual_head = match f.ledger().check_binding(None).unwrap() {
        BindingCheck::Current(head) => head,
        other => panic!("actual current head: {other:?}"),
    };
    assert_eq!(registered.head, actual_head);
    assert_eq!(
        registered.admission,
        f.ledger()
            .read_admission_count(&registered.batch_id)
            .unwrap()
    );
    f.assert_workspace(&workspace, &artifacts);
    let replay = f
        .ledger()
        .register_batch(&controls::input(&case["batchInput"]))
        .unwrap();
    assert!(replay.replayed);
    // Inspect the original durable rows using the real unchanged replay output.
    controls::stored(f.ledger(), &case["expectedBatch"], &replay);
    let ledger_after = controls::ledger_image(f.ledger());
    controls::fresh_delta(&before_registry, &ledger_after, &case);
    let again = register_lineage_with(
        f.ledger(),
        f.conn(),
        &workspace_id,
        request,
        &config,
        &dataset,
        &lineage,
        Some(snapshot),
    )
    .unwrap();
    assert_eq!(again.event_ids, registered.event_ids);
    assert_eq!(again.batch_id, registered.batch_id);
    assert_eq!(again.head, registered.head);
    assert_eq!(again.admission, registered.admission);
    assert_eq!(controls::ledger_image(f.ledger()), ledger_after);
    f.assert_workspace(&workspace, &artifacts);
    let saved =
        history::register_run_lineage_with_events(f.conn(), &lineage, Some(&registered.event_ids))
            .unwrap();
    assert_eq!(saved.len(), 3);
    for (index, (_, attempt_id)) in saved.iter().enumerate() {
        let (candidate_index, event): (i64, String) = f
            .conn()
            .query_row(
                "SELECT candidate_index,trial_event_id FROM research_attempts WHERE id=?",
                [attempt_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(candidate_index, index as i64);
        assert_eq!(event, registered.event_ids[index]);
    }
    assert!(!f.conn().is_autocommit());
    assert_eq!(
        crate::db::runtime_ledger::workspace_id(f.conn()).unwrap(),
        workspace_id
    );
    assert_eq!(f.artifacts(), artifacts);
    f.conn()
        .execute_batch("ROLLBACK TO caller_pending; RELEASE caller_pending")
        .unwrap();
    controls::assert_rollback(&before_pending, &controls::image(f.conn()));
    assert_eq!(controls::ledger_image(f.ledger()), ledger_after);
}
