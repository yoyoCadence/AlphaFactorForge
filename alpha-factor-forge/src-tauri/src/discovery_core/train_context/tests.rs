use super::*;
use serde_json::json;

const T: u64 = 946684800000;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-train-context-v1.json"
    ))
    .unwrap()
}
fn scope(f: &Value) -> TrainScope {
    freeze_train_scope(&f["scope"]["body"].to_string()).unwrap()
}
fn origins(f: &Value, s: &TrainScope) -> Vec<TrainOrigin> {
    f["origins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| freeze_train_origin(&v["body"].to_string(), s).unwrap())
        .collect()
}
fn known() -> ObservedProtection {
    ObservedProtection {
        knowledge: HistoryKnowledge::Complete,
        exposures: vec![],
        protected_references: vec![],
    }
}
fn footprint(instrument: &str, from: u64, to: u64) -> ExposureFootprint {
    ExposureFootprint::new(instrument, vec![TimeInterval::new(from, to).unwrap()]).unwrap()
}
fn request(s: &TrainScope, o: &[TrainOrigin]) -> String {
    json!({"contractVersion":REQUEST_VERSION,"scopeId":s.id(),"originIds":o.iter().map(TrainOrigin::id).collect::<Vec<_>>()} ).to_string()
}
fn summary_body(f: &Value) -> Value {
    f["origins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["body"]["fact"]["kind"] == "summary")
        .unwrap()["body"]
        .clone()
}

#[test]
fn independent_fixture_roundtrips_and_explicitly_projects_only_train_fields() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    assert_eq!(s.id(), f["scope"]["id"]);
    for (actual, expected) in o.iter().zip(f["origins"].as_array().unwrap()) {
        assert_eq!(actual.id(), expected["id"]);
    }
    let c = build_train_context(&f["request"].to_string(), &s, &o, &known()).unwrap();
    assert_eq!(c.id(), f["context"]["id"]);
    assert_eq!(c.document(), f["context"]);
    assert_eq!(
        parse_train_context(&c.document().to_string(), &s, &o, &known()).unwrap(),
        c
    );
    let text = c.document().to_string();
    for forbidden in [
        "inputs",
        "provenance",
        "support",
        "observed",
        "selection",
        "validation",
        "test",
        "notes",
    ] {
        assert!(!text.contains(&format!("\"{forbidden}\"")), "{forbidden}");
    }
}

#[test]
fn each_scope_binding_changes_identity_and_old_requests_or_origins_are_refused() {
    let f = fixture();
    let original = scope(&f);
    let o = origins(&f, &original);
    for (pointer, value) in [
        ("/workspaceId", json!("a".repeat(32))),
        ("/runId", json!(43)),
        ("/campaignId", json!("a".repeat(64))),
        ("/engineManifestHash", json!("a".repeat(64))),
        ("/instrumentId", json!("crypto:binance:ETHUSDT")),
        (
            "/trainDataHash",
            json!(format!("dataset-content-v2:{}", "a".repeat(64))),
        ),
        ("/trainWindow/toExclusiveMs", json!(T + 10001)),
    ] {
        let mut body = f["scope"]["body"].clone();
        *body.pointer_mut(pointer).unwrap() = value;
        let newer = freeze_train_scope(&body.to_string()).unwrap();
        assert_ne!(newer.id(), original.id());
        assert_eq!(
            build_train_context(&f["request"].to_string(), &newer, &o, &known()).unwrap_err(),
            ContextError::StaleScope
        );
        assert_eq!(
            build_train_context(&request(&newer, &o), &newer, &o, &known()).unwrap_err(),
            ContextError::StaleScope
        );
        assert_eq!(
            freeze_train_origin(&f["origins"][0]["body"].to_string(), &newer).unwrap_err(),
            ContextError::StaleScope
        );
    }
}

#[test]
fn complete_fixed_origin_set_refuses_protected_selection_and_reordering() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    for ids in [
        json!([]),
        json!([o[0].id()]),
        json!([o[0].id(), o[1].id(), o[2].id(), "a".repeat(64)]),
        json!([o[1].id(), o[0].id(), o[2].id()]),
        json!([o[0].id(), o[1].id(), "a".repeat(64)]),
        json!([o[0].id(), o[0].id(), o[2].id()]),
    ] {
        let mut r = f["request"].clone();
        r["originIds"] = ids;
        assert_eq!(
            build_train_context(&r.to_string(), &s, &o, &known()).unwrap_err(),
            ContextError::OriginSetMismatch
        );
    }
    let unsorted = vec![o[1].clone(), o[0].clone(), o[2].clone()];
    assert_eq!(
        build_train_context(&request(&s, &unsorted), &s, &unsorted, &known()).unwrap_err(),
        ContextError::OriginSetMismatch
    );
    let repeated = vec![o[0].clone(), o[0].clone()];
    assert_eq!(
        build_train_context(&request(&s, &repeated), &s, &repeated, &known()).unwrap_err(),
        ContextError::OriginSetMismatch
    );
}

#[test]
fn aggregate_and_every_nested_input_refuse_mixed_or_unknown_provenance() {
    let f = fixture();
    let s = scope(&f);
    for provenance in ["validation", "test", "mixed", "unknown"] {
        for pointer in [
            "/provenance",
            "/inputs/0/provenance",
            "/inputs/1/provenance",
        ] {
            let mut b = summary_body(&f);
            let input = b["inputs"][0].clone();
            b["inputs"].as_array_mut().unwrap().push(input);
            *b.pointer_mut(pointer).unwrap() = json!(provenance);
            assert_eq!(
                freeze_train_origin(&b.to_string(), &s).unwrap_err(),
                ContextError::ProtectedProvenance
            );
        }
    }
}

#[test]
fn input_identity_support_and_calendar_boundaries_cannot_be_relabelled_train() {
    let f = fixture();
    let s = scope(&f);
    for (pointer, value) in [
        ("/inputs/0/instrumentId", json!("crypto:binance:ETHUSDT")),
        (
            "/inputs/0/trainDataHash",
            json!(format!("dataset-content-v2:{}", "a".repeat(64))),
        ),
        ("/inputs/0/support/0/fromMs", json!(T - 1)),
        ("/inputs/0/support/0/toExclusiveMs", json!(T + 10001)),
        ("/inputs/0/support/0/toExclusiveMs", json!(T)),
    ] {
        let mut b = summary_body(&f);
        *b.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            freeze_train_origin(&b.to_string(), &s).unwrap_err(),
            ContextError::InvalidOrigin
        );
    }
    for intervals in [
        json!([]),
        json!([{"fromMs":T+2,"toExclusiveMs":T+3},{"fromMs":T,"toExclusiveMs":T+1}]),
        json!([{"fromMs":T,"toExclusiveMs":T+2},{"fromMs":T+2,"toExclusiveMs":T+3}]),
        json!([{"fromMs":T,"toExclusiveMs":T+3},{"fromMs":T+1,"toExclusiveMs":T+2}]),
    ] {
        let mut b = summary_body(&f);
        b["inputs"][0]["support"] = intervals;
        assert!(freeze_train_origin(&b.to_string(), &s).is_err());
    }
}

#[test]
fn protected_fields_notes_hashes_and_payloads_are_refused_at_every_nested_object() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    for key in [
        "validation",
        "test",
        "selection",
        "selectedCandidates",
        "sealedReceiptId",
        "reportSha256",
        "notes",
        "rationale",
        "protectedHash",
    ] {
        for pointer in ["", "/inputs/0", "/inputs/0/support/0", "/fact"] {
            let mut b = summary_body(&f);
            b.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(
                    key.into(),
                    json!({"nested":[{"testReturn":0.314,"notes":"PROTECTED_SENTINEL"}]}),
                );
            assert_eq!(
                freeze_train_origin(&b.to_string(), &s).unwrap_err(),
                ContextError::MalformedInput
            );
        }
        let mut r = f["request"].clone();
        r[key] = json!("PROTECTED_SENTINEL");
        assert_eq!(
            build_train_context(&r.to_string(), &s, &o, &known()).unwrap_err(),
            ContextError::MalformedInput
        );
        for pointer in ["", "/trainWindow"] {
            let mut b = f["scope"]["body"].clone();
            b.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), json!("PROTECTED_SENTINEL"));
            assert_eq!(
                freeze_train_scope(&b.to_string()).unwrap_err(),
                ContextError::MalformedInput
            );
        }
    }
    for field in ["observations", "closedTrades", "netReturnPpm"] {
        for payload in [
            json!("PROTECTED_SENTINEL"),
            json!({"test":0.314}),
            json!(["protected-hash"]),
        ] {
            let mut b = summary_body(&f);
            b["fact"][field] = payload;
            assert_eq!(
                freeze_train_origin(&b.to_string(), &s).unwrap_err(),
                ContextError::MalformedInput
            );
        }
    }
}

#[test]
fn finite_hypothesis_and_failure_codes_cannot_carry_freeform_protected_notes() {
    let f = fixture();
    let s = scope(&f);
    for origin in f["origins"].as_array().unwrap() {
        let b = &origin["body"];
        for field in ["mechanism", "invalidation", "class"] {
            if b["fact"].get(field).is_none() {
                continue;
            }
            let mut changed = b.clone();
            changed["fact"][field] = json!("PROTECTED_SENTINEL: chosen after Test");
            let error = freeze_train_origin(&changed.to_string(), &s).unwrap_err();
            assert_eq!(error, ContextError::MalformedInput);
            assert!(!format!("{error:?} {error}").contains("PROTECTED_SENTINEL"));
        }
    }
}

#[test]
fn every_protected_digest_reference_and_recognized_prefix_is_refused() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    let references = [
        s.id(),
        o[0].id(),
        o[1].id(),
        o[2].id(),
        s.body.campaign_id.as_str(),
        s.body.engine_manifest_hash.as_str(),
        digest(&s.body.train_data_hash).unwrap(),
    ];
    for reference in references {
        for prefix in ["", "dataset-content-v2:", "strategy-v2:"] {
            let mut observed = known();
            observed
                .protected_references
                .push(format!("{prefix}{reference}"));
            assert_eq!(
                build_train_context(&f["request"].to_string(), &s, &o, &observed).unwrap_err(),
                ContextError::ProtectedReference
            );
            assert_eq!(
                parse_train_context(&f["context"].to_string(), &s, &o, &observed).unwrap_err(),
                ContextError::ProtectedReference
            );
        }
    }
    for invalid in ["", "A", "notes:PROTECTED_SENTINEL", "a".repeat(63).as_str()] {
        let mut observed = known();
        observed.protected_references.push(invalid.into());
        assert_eq!(
            build_train_context(&f["request"].to_string(), &s, &o, &observed).unwrap_err(),
            ContextError::ProtectedReference
        );
    }
}

#[test]
fn whole_train_window_not_only_fact_support_is_checked_against_current_exposure() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    for (from, to) in [
        (T, T + 1),
        (T + 9000, T + 9001),
        (T + 9999, T + 10000),
        (T, T + 10000),
        (T, T + 10001),
    ] {
        let mut observed = known();
        observed
            .exposures
            .push(footprint(&s.body.instrument_id, from, to));
        assert_eq!(
            build_train_context(&f["request"].to_string(), &s, &o, &observed).unwrap_err(),
            ContextError::ProtectedExposure
        );
        assert_eq!(
            parse_train_context(&f["context"].to_string(), &s, &o, &observed).unwrap_err(),
            ContextError::ProtectedExposure
        );
    }
    for (instrument, from, to) in [
        ("crypto:binance:BTCUSDT", T + 10000, T + 10001),
        ("crypto:binance:ETHUSDT", T, T + 10000),
    ] {
        let mut observed = known();
        observed.exposures.push(footprint(instrument, from, to));
        let c = build_train_context(&f["request"].to_string(), &s, &o, &observed).unwrap();
        assert_eq!(c.document(), f["context"]); // history never becomes context/hash data
    }
}

#[test]
fn unknown_or_newly_unknown_history_refuses_both_assembly_and_cached_context() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    let c = build_train_context(&f["request"].to_string(), &s, &o, &known()).unwrap();
    let mut observed = known();
    observed.knowledge = HistoryKnowledge::Unknown;
    assert_eq!(
        build_train_context(&f["request"].to_string(), &s, &o, &observed).unwrap_err(),
        ContextError::ProtectionUnknown
    );
    assert_eq!(
        parse_train_context(&c.document().to_string(), &s, &o, &observed).unwrap_err(),
        ContextError::ProtectionUnknown
    );
}

#[test]
fn stored_context_cannot_be_rehashed_to_add_or_substitute_provider_fields() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    for (pointer, value) in [
        ("/body/facts/0/originId", json!("a".repeat(64))),
        ("/body/scope/runId", json!(43)),
        ("/body/scopeId", json!("a".repeat(64))),
        ("/body/contractVersion", json!("research-train-context-v2")),
        ("/body/facts", json!([])),
    ] {
        let mut stored = f["context"].clone();
        *stored.pointer_mut(pointer).unwrap() = value;
        stored["id"] = json!(identity(CONTEXT_VERSION, &stored["body"]).unwrap());
        assert_eq!(
            parse_train_context(&stored.to_string(), &s, &o, &known()).unwrap_err(),
            ContextError::ContextMismatch
        );
    }
    for pointer in [
        "",
        "/body",
        "/body/scope",
        "/body/scope/trainWindow",
        "/body/facts/0",
        "/body/facts/0/fact",
    ] {
        let mut stored = f["context"].clone();
        stored
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("notes".into(), json!("PROTECTED_SENTINEL"));
        assert_eq!(
            parse_train_context(&stored.to_string(), &s, &o, &known()).unwrap_err(),
            ContextError::MalformedInput
        );
    }
    let mut changed = f["context"].clone();
    changed["id"] = json!("a".repeat(64));
    assert_eq!(
        parse_train_context(&changed.to_string(), &s, &o, &known()).unwrap_err(),
        ContextError::ContextMismatch
    );
}

#[test]
fn original_json_duplicate_float_exponent_missing_and_version_fields_fail_closed() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    let raw = summary_body(&f).to_string();
    for (old, new) in [
        ("\"observations\":256", "\"observations\":256.0"),
        ("\"observations\":256", "\"observations\":2.56e2"),
        (
            "\"observations\":256",
            "\"observations\":256,\"observations\":256",
        ),
        (
            "\"provenance\":\"train_only\"",
            "\"provenance\":\"train_only\",\"provenance\":\"test\"",
        ),
        (
            "\"kind\":\"summary\"",
            "\"kind\":\"summary\",\"kind\":\"summary\"",
        ),
        ("\"netReturnPpm\":-12500", "\"netReturnPpm\":-12500.0"),
        (ORIGIN_VERSION, "research-train-origin-v2"),
    ] {
        assert!(raw.contains(old));
        assert!(freeze_train_origin(&raw.replacen(old, new, 1), &s).is_err());
    }
    for pointer in [
        "/scopeId",
        "/provenance",
        "/inputs/0/provenance",
        "/inputs/0/support",
        "/fact/netReturnPpm",
    ] {
        let mut b = summary_body(&f);
        let (path, key) = pointer.rsplit_once('/').unwrap();
        b.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(freeze_train_origin(&b.to_string(), &s).is_err());
    }
    let scope_raw = f["scope"]["body"].to_string();
    for old in ["\"runId\":42", "\"fromMs\":946684800000"] {
        assert!(freeze_train_scope(&scope_raw.replacen(old, &format!("{old},{old}"), 1)).is_err());
    }
    let r = f["request"].to_string();
    let literal = format!("\"scopeId\":\"{}\"", s.id());
    assert!(build_train_context(
        &r.replacen(&literal, &format!("{literal},{literal}"), 1),
        &s,
        &o,
        &known()
    )
    .is_err());
    assert!(build_train_context(
        &r.replace(REQUEST_VERSION, "research-train-context-request-v2"),
        &s,
        &o,
        &known()
    )
    .is_err());
    let stored = f["context"].to_string();
    let literal = format!("\"id\":\"{}\"", f["context"]["id"].as_str().unwrap());
    assert!(parse_train_context(
        &stored.replacen(&literal, &format!("{literal},{literal}"), 1),
        &s,
        &o,
        &known()
    )
    .is_err());
}

#[test]
fn summary_safe_integer_and_scope_domain_edges_are_enforced() {
    let f = fixture();
    let s = scope(&f);
    for (field, value) in [
        ("observations", json!(0)),
        ("observations", json!(PRECISION_MAX_COUNT + 1)),
        ("closedTrades", json!(-1)),
        ("closedTrades", json!(PRECISION_MAX_COUNT + 1)),
        ("netReturnPpm", json!(i64::MIN)),
        ("netReturnPpm", json!(PRECISION_MAX_COUNT + 1)),
    ] {
        let mut b = summary_body(&f);
        b["fact"][field] = value;
        assert!(freeze_train_origin(&b.to_string(), &s).is_err());
    }
    for signed in [-(PRECISION_MAX_COUNT as i64), PRECISION_MAX_COUNT as i64] {
        let mut b = summary_body(&f);
        b["fact"] = json!({"kind":"summary","observations":PRECISION_MAX_COUNT,"closedTrades":PRECISION_MAX_COUNT,"netReturnPpm":signed});
        assert!(freeze_train_origin(&b.to_string(), &s).is_ok());
    }
    for (pointer, value) in [
        ("/runId", json!(0)),
        ("/runId", json!(PRECISION_MAX_COUNT + 1)),
        ("/workspaceId", json!("A".repeat(32))),
        ("/campaignId", json!("a".repeat(63))),
        ("/instrumentId", json!("crypto:BINANCE:BTCUSDT")),
        ("/trainDataHash", json!("2".repeat(64))),
        ("/contractVersion", json!("research-train-scope-v2")),
        ("/trainWindow/fromMs", json!(T - 1)),
        ("/trainWindow/toExclusiveMs", json!(T)),
    ] {
        let mut b = f["scope"]["body"].clone();
        *b.pointer_mut(pointer).unwrap() = value;
        assert!(freeze_train_scope(&b.to_string()).is_err());
    }
}

#[test]
fn exact_origin_input_interval_and_observation_capacity_are_usable_one_over_refused() {
    let f = fixture();
    let s = scope(&f);
    let mut b = summary_body(&f);
    let intervals: Vec<Value> = (0..128)
        .map(|i| json!({"fromMs":T+i*2,"toExclusiveMs":T+i*2+1}))
        .collect();
    b["inputs"][0]["support"] = json!(intervals);
    b["inputs"] = json!(vec![b["inputs"][0].clone(); MAX_INPUTS]);
    assert!(freeze_train_origin(&b.to_string(), &s).is_ok());
    let mut extra = b.clone();
    extra["inputs"]
        .as_array_mut()
        .unwrap()
        .push(b["inputs"][0].clone());
    assert_eq!(
        freeze_train_origin(&extra.to_string(), &s).unwrap_err(),
        ContextError::CapacityExceeded
    );
    let mut extra = b.clone();
    extra["inputs"][0]["support"]
        .as_array_mut()
        .unwrap()
        .push(json!({"fromMs":T+256,"toExclusiveMs":T+257}));
    assert_eq!(
        freeze_train_origin(&extra.to_string(), &s).unwrap_err(),
        ContextError::InvalidOrigin
    );
    b["inputs"] = json!([]);
    assert_eq!(
        freeze_train_origin(&b.to_string(), &s).unwrap_err(),
        ContextError::CapacityExceeded
    );
    let mut all: Vec<TrainOrigin> = (0..MAX_ORIGINS)
        .map(|i| {
            let mut b = summary_body(&f);
            b["fact"]["closedTrades"] = json!(i);
            freeze_train_origin(&b.to_string(), &s).unwrap()
        })
        .collect();
    all.sort_by(|a, b| a.id.cmp(&b.id));
    let mut observed = known();
    observed.exposures =
        vec![footprint(&s.body.instrument_id, T + 10000, T + 10001); MAX_PROTECTION_ITEMS];
    observed.protected_references = vec!["f".repeat(64); MAX_PROTECTION_ITEMS];
    let c = build_train_context(&request(&s, &all), &s, &all, &observed).unwrap();
    assert_eq!(
        parse_train_context(&c.document().to_string(), &s, &all, &observed).unwrap(),
        c
    );
    observed.protected_references.push("f".repeat(64));
    assert_eq!(
        build_train_context(&request(&s, &all), &s, &all, &observed).unwrap_err(),
        ContextError::CapacityExceeded
    );
    observed.protected_references.pop();
    observed
        .exposures
        .push(footprint(&s.body.instrument_id, T + 10000, T + 10001));
    assert_eq!(
        build_train_context(&request(&s, &all), &s, &all, &observed).unwrap_err(),
        ContextError::CapacityExceeded
    );
    all.push(all[0].clone());
    assert_eq!(
        build_train_context(&request(&s, &all), &s, &all, &known()).unwrap_err(),
        ContextError::CapacityExceeded
    );
    assert_eq!(
        build_train_context(&request(&s, &[]), &s, &[], &known()).unwrap_err(),
        ContextError::CapacityExceeded
    );
}

#[test]
fn raw_byte_limits_apply_before_parsing_and_static_errors_never_echo_secrets() {
    let f = fixture();
    let s = scope(&f);
    let o = origins(&f, &s);
    fn padded(v: &Value, n: usize) -> String {
        let mut raw = v.to_string();
        raw.extend(std::iter::repeat_n(' ', n - raw.len()));
        raw
    }
    assert!(freeze_train_scope(&padded(&f["scope"]["body"], MAX_DOCUMENT_BYTES)).is_ok());
    assert!(freeze_train_origin(&padded(&f["origins"][0]["body"], MAX_DOCUMENT_BYTES), &s).is_ok());
    assert!(
        build_train_context(&padded(&f["request"], MAX_DOCUMENT_BYTES), &s, &o, &known()).is_ok()
    );
    assert!(
        parse_train_context(&padded(&f["context"], MAX_DOCUMENT_BYTES), &s, &o, &known()).is_ok()
    );
    let over = padded(&f["context"], MAX_DOCUMENT_BYTES + 1);
    assert_eq!(
        freeze_train_scope(&over).unwrap_err(),
        ContextError::InputTooLarge
    );
    assert_eq!(
        freeze_train_origin(&over, &s).unwrap_err(),
        ContextError::InputTooLarge
    );
    assert_eq!(
        build_train_context(&over, &s, &o, &known()).unwrap_err(),
        ContextError::InputTooLarge
    );
    assert_eq!(
        parse_train_context(&over, &s, &o, &known()).unwrap_err(),
        ContextError::InputTooLarge
    );
    for raw in [
        "{\"PROTECTED_SENTINEL\":42}",
        "\"PROTECTED_SENTINEL\"",
        "{invalid:PROTECTED_SENTINEL}",
    ] {
        let error = build_train_context(raw, &s, &o, &known()).unwrap_err();
        assert_eq!(error, ContextError::MalformedInput);
        assert!(!format!("{error:?} {error}").contains("PROTECTED_SENTINEL"));
    }
}

#[test]
fn self_consistent_forged_context_is_not_authenticity_or_completeness_proof() {
    let f = fixture();
    let s = scope(&f);
    let complete = origins(&f, &s);
    // The host must prove which complete set was approved from Train alone.
    // A maliciously truncated host observation is outside this pure boundary.
    let invented = vec![complete[0].clone()];
    assert!(build_train_context(&request(&s, &invented), &s, &invented, &known()).is_ok());
    let mut b = summary_body(&f);
    b["fact"]["netReturnPpm"] = json!(314159);
    let relabelled = vec![freeze_train_origin(&b.to_string(), &s).unwrap()];
    assert!(build_train_context(&request(&s, &relabelled), &s, &relabelled, &known()).is_ok());
}
