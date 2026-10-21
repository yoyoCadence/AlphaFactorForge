//! Real complete source validation, including rehashed semantic corruption.
use super::*;
use crate::research::trial_ledger as ledger;

fn installed() -> (Dirs, TrialLedger) {
    let (dirs, source, mut input, context) = fixture();
    let proof = witnessed::original(&source, &context);
    input
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, proof.text()).unwrap());
    let from = json(&input.exposures[0]).unwrap()["body"]["intervals"][0]["fromMs"]
        .as_u64()
        .unwrap();
    input.exposures.push(cases::manual(
        source.registry_id(),
        'b',
        "raw",
        None,
        from,
        from + 1,
    ));
    input.unknown_origins.push("e".repeat(32));
    source
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    (dirs, source)
}
fn parts(bytes: &[u8]) -> (Value, Vec<Value>) {
    let text = std::str::from_utf8(bytes).unwrap();
    let (h, b) = text.split_once('\n').unwrap();
    (
        json(h).unwrap(),
        b.lines().map(|r| json(r).unwrap()).collect(),
    )
}
fn wire(mut h: Value, rows: &[Value]) -> Vec<u8> {
    for (name, kind) in [
        ("batches", "confirmationBatch"),
        ("selections", "confirmationSelection"),
        ("operations", "confirmationOperation"),
        ("exposures", "confirmationExposure"),
        ("admissions", "confirmationAdmission"),
        ("events", "confirmationEvent"),
        ("conflicts", "confirmationConflict"),
        ("blobs", "confirmationBlob"),
        ("unknownOrigins", "confirmationUnknownOrigin"),
    ] {
        h["counts"][name] = json!(rows.iter().filter(|r| r["type"] == kind).count());
    }
    let body: String = rows
        .iter()
        .map(|r| format!("{}\n", ledger::canonical_text(r).unwrap()))
        .collect();
    h["bodySha256"] = json!(ledger::sha256_hex(body.as_bytes()));
    format!("{}\n{body}", ledger::canonical_text(&h).unwrap()).into_bytes()
}
fn bad(bytes: &[u8]) {
    assert_eq!(
        TrialLedger::validate_witnessed_archive(bytes)
            .err()
            .map(|e| e.code()),
        Some("import_integrity_failed")
    );
}
fn snapshot(source: &TrialLedger) -> (String, u64) {
    let conn = source.lock().unwrap();
    let mut tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    tables.retain(|t| !t.starts_with("sqlite_"));
    let mut all = String::new();
    for table in tables {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let n = stmt.column_count();
        let rows: Vec<Vec<SqlValue>> = stmt
            .query_map([], |r| (0..n).map(|i| r.get(i)).collect())
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        all.push_str(&format!("{table}:{rows:?}\n"));
    }
    (
        all,
        conn.query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap(),
    )
}
#[test]
fn full_real_source_validates_three_original_blob_kinds_and_all_stops_without_writes() {
    let (dirs, source) = installed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let before = snapshot(&source);
    TrialLedger::validate_witnessed_archive(&bytes).unwrap();
    let (h, _) = parts(&bytes);
    for (_, n) in h["counts"].as_object().unwrap() {
        assert!(n.as_u64().unwrap() > 0);
    }
    assert!(source
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    assert_eq!(snapshot(&source), before);
    drop(source);
    let reopened = dirs.open();
    TrialLedger::validate_witnessed_archive(&reopened.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert_eq!(snapshot(&reopened).0, before.0);
}
#[test]
fn rehashed_missing_dependencies_or_materialized_conflicts_are_refused_without_repairs() {
    let (_dirs, source) = installed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let (h, rows) = parts(&bytes);
    let before = snapshot(&source);
    for kind in [
        "confirmationBatch",
        "confirmationSelection",
        "confirmationOperation",
        "confirmationExposure",
        "confirmationAdmission",
        "confirmationEvent",
        "confirmationConflict",
    ] {
        let mut r = rows.clone();
        let n = r.iter().position(|v| v["type"] == kind).unwrap();
        r.remove(n);
        bad(&wire(h.clone(), &r));
    }
    for kind in ["binding", "statistics", "source_witness"] {
        let mut r = rows.clone();
        let n = r
            .iter()
            .position(|v| v["type"] == "confirmationBlob" && v["kind"] == kind)
            .unwrap();
        r.remove(n);
        bad(&wire(h.clone(), &r));
    }
    let mut r = rows.clone();
    let record = r
        .iter_mut()
        .find(|r| r["type"] == "confirmationOperation")
        .unwrap();
    record["record"]["body"]["hidden"] = json!(true);
    bad(&wire(h.clone(), &r));
    assert_eq!(snapshot(&source), before);
    TrialLedger::validate_witnessed_archive(&bytes).unwrap();
}
#[test]
fn original_nested_duplicate_keys_and_closed_documents_are_checked_before_normalization() {
    let (_dirs, source) = installed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let (h, rows) = parts(&bytes);
    let mut lines: Vec<String> = rows
        .iter()
        .map(|r| ledger::canonical_text(r).unwrap())
        .collect();
    let n = rows
        .iter()
        .position(|r| r["type"] == "confirmationOperation")
        .unwrap();
    let body = &rows[n]["record"]["body"];
    let key = body.as_object().unwrap().keys().next().unwrap();
    let duplicate = format!("\"{key}\":{},", ledger::canonical_text(&body[key]).unwrap());
    let at = lines[n].find("\"body\":{").unwrap() + 8;
    lines[n].insert_str(at, &duplicate);
    let body = format!("{}\n", lines.join("\n"));
    let mut h = h;
    h["bodySha256"] = json!(ledger::sha256_hex(body.as_bytes()));
    bad(format!("{}\n{body}", ledger::canonical_text(&h).unwrap()).as_bytes());
}
#[test]
fn exact_source_context_cannot_borrow_missing_protocols_or_events_from_a_receiver() {
    let (_dirs, source) = installed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let (h, rows) = parts(&bytes);
    // A different fully valid empty nested source cannot stand in for the
    // current source, even when every outer/nested header/digest is rebuilt.
    let dirs = Dirs::new();
    let empty = dirs.open();
    let (_, empty_rows) = parts(&empty.export_witnessed_json_lines().unwrap());
    let mut r = rows.clone();
    r[0] = empty_rows[0].clone();
    let mut head = h.clone();
    head["registryId"] = json!(empty.registry_id());
    bad(&wire(head, &r));
    // Remove a required source proof while the live original still has it.
    let mut r = rows;
    r.retain(|r| r["kind"] != "source_witness");
    bad(&wire(h, &r));
    TrialLedger::validate_witnessed_archive(&bytes).unwrap();
}
#[test]
fn empty_manual_and_unknown_sources_validate_but_legacy_frames_do_not_gain_v5_authority() {
    let dirs = Dirs::new();
    let source = dirs.open();
    let v3 = source.export_json_lines().unwrap();
    let v4 = source.export_complete_json_lines().unwrap();
    bad(&v3);
    bad(&v4);
    TrialLedger::validate_witnessed_archive(&source.export_witnessed_json_lines().unwrap())
        .unwrap();
    source
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            exposures: vec![cases::manual(
                source.registry_id(),
                'a',
                "chart",
                None,
                946684800000,
                946684800001,
            )],
            unknown_origins: vec!["e".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    let before = snapshot(&source);
    TrialLedger::validate_witnessed_archive(&source.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert_eq!(snapshot(&source), before);
}

#[test]
fn same_head_upgrades_survive_source_staging_without_rewriting_original_witnesses() {
    let (_dirs, source, mut input, ctx) = fixture();
    let old = witnessed::original(&source, &ctx);
    input
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, old.text()).unwrap());
    source
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    source
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:BTCUSDT".into()),
            tests_per_trial: 3,
            events: ctx.registered_sources[0]
                .candidates
                .iter()
                .map(|c| event("original", c.candidate_index, &c.strategy_hash))
                .collect(),
        })
        .unwrap();
    let current = witnessed::original(&source, &ctx);
    source
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            blobs: vec![HistoryBlob::new(BlobKind::SourceWitness, current.text()).unwrap()],
            ..Default::default()
        })
        .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    TrialLedger::validate_witnessed_archive(&bytes).unwrap();
    bad(&omit_trial_record(&bytes, "protocolUpgrade"));
}
fn omit_trial_record(bytes: &[u8], kind: &str) -> Vec<u8> {
    let (h, mut outer) = parts(bytes);
    let (ah, mut alpha) = parts(outer[0]["export"].as_str().unwrap().as_bytes());
    let (th, trial) = parts(alpha[0]["export"].as_str().unwrap().as_bytes());
    let mut t = trial.clone();
    t.retain(|r| r["type"] != kind);
    // These are event-less changes: chain/head/counts still agree.
    let body: String = t
        .iter()
        .map(|r| format!("{}\n", ledger::canonical_text(r).unwrap()))
        .collect();
    let mut th = th.clone();
    th["bodySha256"] = json!(ledger::sha256_hex(body.as_bytes()));
    alpha[0]["export"] = json!(format!("{}\n{body}", ledger::canonical_text(&th).unwrap()));
    let body: String = alpha
        .iter()
        .map(|r| format!("{}\n", ledger::canonical_text(r).unwrap()))
        .collect();
    let mut ah = ah.clone();
    ah["bodySha256"] = json!(ledger::sha256_hex(body.as_bytes()));
    outer[0]["export"] = json!(format!("{}\n{body}", ledger::canonical_text(&ah).unwrap()));
    wire(h, &outer)
}
#[test]
fn original_restrictive_source_evidence_is_retained_and_cannot_be_replaced_by_unknown() {
    let (_dirs, source, mut input, ctx) = fixture();
    let old = witnessed::original(&source, &ctx);
    source.lock().unwrap().execute("INSERT INTO family_conflicts (family_id,kind,detail_json,recorded_at) VALUES (?1,'import_conflict','[]','source-audit')",[&ctx.registered_sources[0].source.family_id]).unwrap();
    source.lock().unwrap().execute("INSERT INTO registry_conflicts (origin_registry_id,detail_json,recorded_at) VALUES (?1,'[]','source-audit')",["f".repeat(32)]).unwrap();
    let current = witnessed::original(&source, &ctx);
    input.blobs.extend([
        HistoryBlob::new(BlobKind::SourceWitness, old.text()).unwrap(),
        HistoryBlob::new(BlobKind::SourceWitness, current.text()).unwrap(),
    ]);
    input.unknown_origins.push("f".repeat(32));
    source
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    TrialLedger::validate_witnessed_archive(&bytes).unwrap();
    for kind in ["familyConflict", "registryConflict"] {
        bad(&omit_trial_record(&bytes, kind));
    }
}

#[test]
fn unincorporated_original_source_requires_its_explicit_permanent_stop() {
    let (_dirs, original, _input, ctx) = fixture();
    let proof = witnessed::original(&original, &ctx);
    let dirs = Dirs::new();
    let source = dirs.open();
    source
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            blobs: vec![HistoryBlob::new(BlobKind::SourceWitness, proof.text()).unwrap()],
            ..Default::default()
        })
        .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    TrialLedger::validate_witnessed_archive(&bytes).unwrap();
    let (h, mut rows) = parts(&bytes);
    assert!(rows
        .iter()
        .any(|r| r["type"] == "confirmationUnknownOrigin"));
    rows.retain(|r| r["type"] != "confirmationUnknownOrigin");
    bad(&wire(h, &rows));
}

#[test]
fn unknown_origin_limit_is_exact_in_real_complete_input_without_any_destination() {
    let dirs = Dirs::new();
    let source = dirs.open();
    let (h, mut rows) = parts(&source.export_witnessed_json_lines().unwrap());
    rows.extend((0..MAX_UNKNOWN_ORIGINS).map(
        |n| json!({"type":"confirmationUnknownOrigin","originRegistryId":format!("{n:032x}")}),
    ));
    TrialLedger::validate_witnessed_archive(&wire(h.clone(), &rows)).unwrap();
    rows.push(json!({"type":"confirmationUnknownOrigin","originRegistryId":format!("{:032x}",MAX_UNKNOWN_ORIGINS)}));
    bad(&wire(h, &rows));
    assert!(source
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .unknown_origins
        .is_empty());
}

#[test]
fn complete_source_includes_foreign_events_checkpoints_receipts_genesis_and_legacy_stops() {
    let (_dirs, source, mut input, ctx) = fixture();
    let old = witnessed::original(&source, &ctx);
    let foreign_dirs = Dirs::new();
    let foreign = foreign_dirs.open();
    foreign
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:ETHUSDT".into()),
            tests_per_trial: 2,
            events: vec![event("foreign", 0, "foreign")],
        })
        .unwrap();
    source
        .import_json_lines(&foreign.export_json_lines().unwrap())
        .unwrap();
    let combined = witnessed::original(&source, &ctx);
    input.blobs.extend([
        HistoryBlob::new(BlobKind::SourceWitness, old.text()).unwrap(),
        HistoryBlob::new(BlobKind::SourceWitness, combined.text()).unwrap(),
    ]);
    source
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let before = snapshot(&source);
    TrialLedger::validate_witnessed_archive(&bytes).unwrap();
    assert_eq!(snapshot(&source), before);
    let conn = source.lock().unwrap();
    for table in [
        "origin_genesis",
        "origin_checkpoints",
        "batch_receipts",
        "alpha_unknown_origins",
        "confirmation_unknown_origins",
    ] {
        assert!(
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap()
                > 0
        );
    }
}

#[test]
fn source_failure_diagnostics_never_echo_supplied_original_text() {
    let (_dirs, source) = installed();
    let (h, mut rows) = parts(&source.export_witnessed_json_lines().unwrap());
    let record = rows
        .iter_mut()
        .find(|r| r["type"] == "confirmationOperation")
        .unwrap();
    record["record"]["body"]["PROTECTED_SOURCE_SENTINEL"] = json!("PROTECTED_SOURCE_SENTINEL");
    let err = TrialLedger::validate_witnessed_archive(&wire(h, &rows)).unwrap_err();
    assert_eq!(err.code(), "import_integrity_failed");
    assert!(!err.to_string().contains("PROTECTED_SOURCE_SENTINEL"));
}

#[test]
fn internal_exact_source_stager_refuses_an_existing_registry_before_writing() {
    let (_dirs, source) = installed();
    let (_, rows) = parts(&source.export_witnessed_json_lines().unwrap());
    let checked =
        ledger::alpha::ValidatedComplete::parse(rows[0]["export"].as_str().unwrap().as_bytes())
            .unwrap();
    let before = snapshot(&source);
    {
        let mut conn = source.lock().unwrap();
        let tx = conn.transaction().unwrap();
        assert!(checked.stage_source_in(&tx).is_err());
    }
    assert_eq!(snapshot(&source), before);
}
