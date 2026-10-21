use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::HistoryBlob;

fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-confirmation-export-v5.json"
    ))
    .unwrap()
}
fn input(case: &Value) -> ConfirmationInventory {
    let mut inventory = ConfirmationInventory::default();
    for blob in case["inventory"]["blobs"].as_array().unwrap() {
        inventory.blobs.push(
            HistoryBlob::new(BlobKind::SourceWitness, blob["text"].as_str().unwrap()).unwrap(),
        );
    }
    inventory.unknown_origins =
        serde_json::from_value(case["inventory"]["unknownOrigins"].clone()).unwrap();
    inventory
}
fn frame(case: &Value, limits: Limits) -> Result<Vec<u8>, LedgerError> {
    encode(
        case["registryId"].as_str().unwrap(),
        case["alphaText"].as_str().unwrap(),
        &input(case),
        case["exportedAt"].as_str().unwrap(),
        limits,
    )
}

#[test]
fn independent_complete_original_source_wire_bytes_hash_and_nine_counts_match() {
    for case in oracle()["cases"].as_array().unwrap() {
        let actual = frame(case, LIMITS).unwrap();
        assert_eq!(actual, case["text"].as_str().unwrap().as_bytes());
        assert_eq!(sha256_hex(&actual), case["sha256"].as_str().unwrap());
        let text = std::str::from_utf8(&actual).unwrap();
        let (header, body) = text.split_once('\n').unwrap();
        let header: Value = serde_json::from_str(header).unwrap();
        assert_eq!(header["counts"].as_object().unwrap().len(), 9);
        assert_eq!(header["schemaVersion"], SCHEMA);
        assert_eq!(header["bodySha256"], sha256_hex(body.as_bytes()));
        let alpha: Value =
            serde_json::from_str(case["alphaText"].as_str().unwrap().lines().nth(1).unwrap())
                .unwrap();
        let trial = crate::research::trial_ledger::transfer::ValidatedExport::parse(
            alpha["export"].as_str().unwrap().as_bytes(),
        )
        .unwrap();
        assert_eq!(trial.registry_id(), case["registryId"].as_str().unwrap());
        assert!(text.ends_with('\n'));
    }
}

#[test]
fn exact_encoded_file_header_nested_projection_and_other_line_limits_are_inclusive() {
    let fixture = oracle();
    let case = &fixture["cases"][1];
    let bytes = frame(case, LIMITS).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let header = text.lines().next().unwrap().len();
    let other = text.lines().skip(2).map(str::len).max().unwrap();
    let alpha = case["alphaText"].as_str().unwrap().len();
    let exact = Limits {
        file: bytes.len(),
        header,
        line: other,
        alpha,
    };
    assert_eq!(frame(case, exact).unwrap(), bytes);
    for bad in [
        Limits {
            file: exact.file - 1,
            ..exact
        },
        Limits {
            header: header - 1,
            ..exact
        },
        Limits {
            line: other - 1,
            ..exact
        },
        Limits {
            alpha: alpha - 1,
            ..exact
        },
    ] {
        assert!(frame(case, bad).is_err());
    }
    assert_eq!(
        (LIMITS.file, LIMITS.alpha, LIMITS.header, LIMITS.line),
        (
            256 * 1024 * 1024,
            128 * 1024 * 1024,
            4096,
            16 * 1024 * 1024 + 4096
        )
    );
}

#[test]
fn escaping_counts_toward_limits_and_first_projection_has_a_distinct_line_limit() {
    let fixture = oracle();
    let empty = &fixture["cases"][0];
    assert!(frame(empty, Limits { line: 1, ..LIMITS }).is_ok());
    let populated = &fixture["cases"][1];
    let raw_blob = input(populated).blobs[0].text().len();
    assert!(
        frame(
            populated,
            Limits {
                line: raw_blob,
                ..LIMITS
            }
        )
        .is_err(),
        "escaping and wrapper count"
    );
    let raw = populated["alphaText"].as_str().unwrap();
    assert!(frame(
        populated,
        Limits {
            file: raw.len(),
            ..LIMITS
        }
    )
    .is_err());
}

#[test]
fn document_ordering_is_identity_order_and_record_json_is_canonicalized() {
    let fixture = oracle();
    let case = &fixture["cases"][0];
    // Formatter only: actual export first obtains a checked graph. These
    // semantic documents demonstrate ordering/encoding, not graph authority.
    let inventory = ConfirmationInventory {
        exposures: vec![
            format!(
                "{{\"id\":\"{}\",\"body\":{{\"z\":2,\"a\":1}}}}",
                "b".repeat(64)
            ),
            format!(
                "{{ \"body\":{{\"a\":1,\"z\":2}},\"id\":\"{}\" }}",
                "a".repeat(64)
            ),
        ],
        ..Default::default()
    };
    let bytes = encode(
        case["registryId"].as_str().unwrap(),
        case["alphaText"].as_str().unwrap(),
        &inventory,
        case["exportedAt"].as_str().unwrap(),
        LIMITS,
    )
    .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let rows: Vec<Value> = text
        .lines()
        .skip(2)
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows[0]["record"]["id"], "a".repeat(64));
    assert_eq!(rows[1]["record"]["id"], "b".repeat(64));
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        assert_eq!(canonical_text(&value).unwrap(), line);
    }
}
