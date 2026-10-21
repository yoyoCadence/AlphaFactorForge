use super::*;

fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-confirmation-export-v5.json"
    ))
    .unwrap()
}
fn valid() -> Vec<u8> {
    oracle()["cases"][1]["text"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec()
}
fn parts(bytes: &[u8]) -> (Value, Vec<Value>) {
    let text = std::str::from_utf8(bytes).unwrap();
    let (header, body) = text.split_once('\n').unwrap();
    (
        serde_json::from_str(header).unwrap(),
        body.lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect(),
    )
}
fn join(mut header: Value, rows: &[Value]) -> Vec<u8> {
    let body: String = rows
        .iter()
        .map(|v| format!("{}\n", canonical_text(v).unwrap()))
        .collect();
    header["bodySha256"] = sha256_hex(body.as_bytes()).into();
    format!("{}\n{body}", canonical_text(&header).unwrap()).into_bytes()
}
fn refused(bytes: &[u8]) {
    assert_eq!(
        TrialLedger::validate_witnessed_archive(bytes)
            .err()
            .map(|e| e.code()),
        Some("import_integrity_failed")
    );
}

#[test]
fn independent_original_source_oracles_pass_with_exact_blobs_and_file_digest() {
    for case in oracle()["cases"].as_array().unwrap() {
        let bytes = case["text"].as_str().unwrap().as_bytes();
        let parsed = ValidatedWitnessed::parse(bytes).unwrap();
        assert_eq!(parsed.file_sha256, case["sha256"]);
        assert_eq!(
            parsed.alpha.registry_id(),
            case["registryId"].as_str().unwrap()
        );
        let inventory = parsed.graph.inventory();
        assert_eq!(
            inventory.blobs.len(),
            case["inventory"]["blobs"].as_array().unwrap().len()
        );
        for (actual, expected) in inventory
            .blobs
            .iter()
            .zip(case["inventory"]["blobs"].as_array().unwrap())
        {
            assert_eq!(actual.text(), expected["text"].as_str().unwrap());
            assert_eq!(actual.sha256(), sha256_hex(actual.text().as_bytes()));
        }
        assert_eq!(
            serde_json::to_value(&inventory.unknown_origins).unwrap(),
            case["inventory"]["unknownOrigins"]
        );
        TrialLedger::validate_witnessed_archive(bytes).unwrap();
    }
}
#[test]
fn input_bounds_are_inclusive_and_first_projection_has_its_own_decoded_bound() {
    let bytes = valid();
    let text = std::str::from_utf8(&bytes).unwrap();
    let (_, rows) = parts(&bytes);
    let limits = Limits {
        file: bytes.len(),
        header: text.lines().next().unwrap().len(),
        alpha: rows[0]["export"].as_str().unwrap().len(),
        line: text.lines().skip(2).map(str::len).max().unwrap(),
    };
    ValidatedWitnessed::parse_with_limits(&bytes, limits).unwrap();
    let first = oracle()["cases"][0]["text"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec();
    // No subsequent line: a tiny other-line ceiling must not constrain the
    // much larger first nested projection's encoded escaping.
    ValidatedWitnessed::parse_with_limits(&first, Limits { line: 1, ..LIMITS }).unwrap();
    for bad in [
        Limits {
            file: limits.file - 1,
            ..limits
        },
        Limits {
            header: limits.header - 1,
            ..limits
        },
        Limits {
            alpha: limits.alpha - 1,
            ..limits
        },
        Limits {
            line: limits.line - 1,
            ..limits
        },
    ] {
        assert!(ValidatedWitnessed::parse_with_limits(&bytes, bad).is_err());
    }
}
#[test]
fn original_closed_canonical_frame_counts_order_and_blobs_are_not_normalized() {
    let bytes = valid();
    let (header, rows) = parts(&bytes);
    let mut inputs = vec![
        bytes[..bytes.len() - 1].to_vec(),
        vec![0xff],
        b"\n".to_vec(),
    ];
    for (field, value) in [
        ("version", json!("trial-ledger-export-v4")),
        ("schemaVersion", json!("0006_confirmation_history")),
        ("registryId", json!("f".repeat(32))),
        ("extra", json!(true)),
    ] {
        let mut h = header.clone();
        h[field] = value;
        inputs.push(join(h, &rows));
    }
    for name in [
        "batches",
        "selections",
        "operations",
        "exposures",
        "admissions",
        "events",
        "blobs",
        "conflicts",
        "unknownOrigins",
    ] {
        let mut h = header.clone();
        h["counts"][name] = json!(h["counts"][name].as_u64().unwrap() + 1);
        inputs.push(join(h, &rows));
    }
    let mut h = header.clone();
    h["counts"]["hidden"] = json!(0);
    inputs.push(join(h, &rows));
    let mut h = header.clone();
    h["bodySha256"] = json!("0".repeat(64));
    let (_, raw_body) = std::str::from_utf8(&bytes)
        .unwrap()
        .split_once('\n')
        .unwrap();
    inputs.push(format!("{}\n{raw_body}", canonical_text(&h).unwrap()).into_bytes());
    let mut r = rows.clone();
    r.swap(0, 1);
    inputs.push(join(header.clone(), &r));
    let mut r = rows.clone();
    r.insert(1, r[1].clone());
    inputs.push(join(header.clone(), &r));
    let mut r = rows.clone();
    r[1]["hidden"] = json!(true);
    inputs.push(join(header.clone(), &r));
    let mut r = rows.clone();
    r[1]["kind"] = json!("unknown");
    inputs.push(join(header.clone(), &r));
    let mut r = rows.clone();
    r[1]["sha256"] = json!("0".repeat(64));
    inputs.push(join(header.clone(), &r));
    let mut r = rows.clone();
    r[1]["text"] = json!(format!("{} ", r[1]["text"].as_str().unwrap()));
    inputs.push(join(header.clone(), &r));
    let mut noncanonical = String::from_utf8(bytes.clone()).unwrap();
    noncanonical.insert(1, ' ');
    inputs.push(noncanonical.into_bytes());
    let mut duplicate = String::from_utf8(bytes.clone()).unwrap();
    duplicate.insert_str(1, "\"version\":\"trial-ledger-export-v5\",");
    inputs.push(duplicate.into_bytes());
    let mut corrupt = bytes.clone();
    let end = corrupt.len() - 3;
    corrupt[end] ^= 1;
    inputs.push(corrupt);
    let mut extra = bytes.clone();
    extra.push(b'\n');
    inputs.push(extra);
    for (index, input) in inputs.iter().enumerate() {
        assert!(
            ValidatedWitnessed::parse(input).is_err(),
            "accepted mutation {index}"
        );
        refused(input);
    }
}

#[test]
fn rehashed_nested_trial_payload_still_requires_independent_complete_kind_validation() {
    use crate::research::trial_ledger as ledger;
    use std::collections::BTreeMap;
    let bytes = oracle()["cases"][0]["text"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec();
    let (h, mut outer) = parts(&bytes);
    let (ah, mut alpha) = parts(outer[0]["export"].as_str().unwrap().as_bytes());
    let (mut th, mut trial) = parts(alpha[0]["export"].as_str().unwrap().as_bytes());
    let mut event_ids = BTreeMap::new();
    for row in trial.iter_mut().filter(|r| r["type"] == "event") {
        let old = row["eventId"].as_str().unwrap().to_owned();
        row["payload"]["seedsHash"] = Value::Null;
        let id = ledger::event_id_for(&canonical_text(&row["payload"]).unwrap());
        row["eventId"] = json!(id);
        event_ids.insert(old, id);
    }
    let mut batch_ids = BTreeMap::new();
    for row in trial.iter_mut().filter(|r| r["type"] == "batch") {
        let old = row["batchId"].as_str().unwrap().to_owned();
        for member in row["members"].as_array_mut().unwrap() {
            member[1] = json!(event_ids[member[1].as_str().unwrap()]);
        }
        let id = ledger::batch_id_for(row["familyId"].as_str(), &row["members"]).unwrap();
        row["batchId"] = json!(id);
        batch_ids.insert(old, id);
    }
    let mut chain = ledger::genesis(th["registryId"].as_str().unwrap());
    for row in &mut trial {
        match row["type"].as_str().unwrap() {
            "event" => {
                row["batchId"] = json!(batch_ids[row["batchId"].as_str().unwrap()]);
                chain = ledger::next_chain(&chain, row["eventId"].as_str().unwrap());
                row["chain"] = json!(chain);
            }
            "receipt" => row["batchId"] = json!(batch_ids[row["batchId"].as_str().unwrap()]),
            _ => {}
        }
    }
    th["headChain"] = json!(chain);
    let nested = join(th, &trial);
    // Structural parsing, all identities and all three framing hashes agree.
    assert!(ledger::transfer::ValidatedExport::parse(&nested).is_ok());
    alpha[0]["export"] = json!(String::from_utf8(nested).unwrap());
    outer[0]["export"] = json!(String::from_utf8(join(ah, &alpha)).unwrap());
    refused(&join(h, &outer));
}
