use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::{
    freeze_manual_exposure, ConfirmationBindingSet, ConfirmationInventory,
};

fn manual(origin: &str) -> String {
    freeze_manual_exposure(
        &serde_json::json!({
            "contractVersion":"research-confirmation-exposure-v1","originRegistryId":origin,
            "reservationKey":"a".repeat(32),"instrumentId":"crypto:binance:BTCUSDT",
            "intervals":[{"fromMs":946684800001u64,"toExclusiveMs":946684800002u64}],
            "claim":{"kind":"manual","accessKind":"chart","afterSealId":null}
        })
        .to_string(),
    )
    .unwrap()
    .document()
    .to_string()
}

#[test]
fn all_legacy_formats_keep_confirmation_unknown_until_actual_complete_adoption() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    ledger
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            exposures: vec![manual(ledger.registry_id())],
            unknown_origins: vec!["e".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    let saved = current(&ledger, None);
    let c = settings();
    write_workspace_binding(&c, &saved).unwrap();
    let raw: String = c
        .query_row("SELECT value_json FROM app_settings", [], |r| r.get(0))
        .unwrap();
    let stored: Value = serde_json::from_str(&raw).unwrap();
    for version in [
        None,
        Some(LEGACY_EVIDENCE_VERSION),
        Some(LEGACY_ALPHA_VERSION),
        Some(LEGACY_PROVENANCE_VERSION),
    ] {
        let mut value = stored.clone();
        let object = value.as_object_mut().unwrap();
        object.remove("confirmation");
        if let Some(version) = version {
            object.insert("version".into(), Value::String(version.into()));
        } else {
            object.remove("version");
            object.remove("evidence");
        }
        if version.is_none() || version == Some(LEGACY_EVIDENCE_VERSION) {
            object.remove("alpha");
        }
        if version != Some(LEGACY_PROVENANCE_VERSION) {
            object.remove("alphaUnknownOrigins");
        }
        store_raw(&c, &value.to_string());
        let old = read_workspace_binding(&c).unwrap().unwrap();
        assert!(old.confirmation.is_none(), "{version:?}");
        assert!(write_workspace_binding(&c, &old).is_err());
        let adopted = current(&ledger, Some(&old));
        assert_eq!(adopted, saved);
        write_workspace_binding(&c, &adopted).unwrap();
        assert_eq!(read_workspace_binding(&c).unwrap(), Some(saved.clone()));
    }
}

#[test]
fn original_v5_text_rejects_missing_duplicate_dynamic_keys_and_redacts_errors() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let mut input = batch("real");
    input.instrument_id = Some("crypto:binance:BTCUSDT".into());
    input.events[0].snapshot_id = Some("snapshot".into());
    ledger.register_batch(&input).unwrap();
    let c = settings();
    write_workspace_binding(&c, &current(&ledger, None)).unwrap();
    let raw: String = c
        .query_row("SELECT value_json FROM app_settings", [], |r| r.get(0))
        .unwrap();
    let stored: Value = serde_json::from_str(&raw).unwrap();
    let family = stored["evidence"]["familyTests"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    let pair = format!("\"{family}\":1");
    for bad in [
        raw.replacen(&pair, &format!("{pair},{pair}"), 1),
        raw.replacen(
            "\"confirmation\":",
            "\"confirmation\":{},\"confirmation\":",
            1,
        ),
        raw.replacen("\"blobs\":", "\"blobs\":[],\"blobs\":", 1),
        raw.replacen(
            "\"version\":",
            "\"version\":\"private-sentinel\",\"version\":",
            1,
        ),
        raw.replacen("\"version\":", "\"private-sentinel\":true,\"version\":", 1),
    ] {
        assert_ne!(bad, raw);
        let error = parse_binding(&bad).unwrap_err();
        assert!(!error.to_string().contains("private-sentinel"));
    }
    for section in [
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
        let mut bad = stored.clone();
        bad["confirmation"].as_object_mut().unwrap().remove(section);
        assert!(parse_binding(&bad.to_string()).is_err(), "{section}");
    }
    let mut missing = stored;
    missing.as_object_mut().unwrap().remove("confirmation");
    assert!(parse_binding(&missing.to_string()).is_err());
}

#[test]
fn v5_and_legacy_exact_raw_limits_and_sql_type_preflight_are_distinct() {
    let dirs = Dirs::new();
    let c = settings();
    write_workspace_binding(&c, &current(&dirs.open(), None)).unwrap();
    let raw: String = c
        .query_row("SELECT value_json FROM app_settings", [], |r| r.get(0))
        .unwrap();
    let exact = format!("{raw}{}", " ".repeat(v5::MAX_BYTES - raw.len()));
    store_raw(&c, &exact);
    assert!(read_workspace_binding(&c).unwrap().is_some());
    store_raw(&c, &format!("{exact} "));
    assert!(read_workspace_binding(&c).is_err());
    let mut legacy: Value = serde_json::from_str(&raw).unwrap();
    legacy["version"] = Value::String(LEGACY_PROVENANCE_VERSION.into());
    legacy.as_object_mut().unwrap().remove("confirmation");
    let legacy = legacy.to_string();
    let exact = format!("{legacy}{}", " ".repeat(MAX_BINDING_BYTES - legacy.len()));
    assert!(parse_binding(&exact).unwrap().confirmation.is_none());
    assert!(parse_binding(&format!("{exact} "))
        .unwrap_err()
        .to_string()
        .contains("4 MiB"));
    c.execute("UPDATE app_settings SET value_json=X'FF'", [])
        .unwrap();
    assert!(read_workspace_binding(&c).is_err());
}

#[test]
fn independent_writer_cannot_split_trial_alpha_and_confirmation_read_snapshot() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let mut conn = ledger.lock().unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .unwrap();
    let PrefixOutcome::Proven { seq, chain_head } = ledger.event_prefix(&tx, None).unwrap() else {
        panic!("intact");
    };
    let writer = dirs.open();
    writer.register_batch(&batch("later")).unwrap();
    writer
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            exposures: vec![manual(writer.registry_id())],
            unknown_origins: vec!["e".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(seq, 0);
    assert_eq!(chain_head, genesis(ledger.registry_id()));
    assert!(observed_evidence(&tx).unwrap().family_tests.is_empty());
    assert!(super::super::super::alpha::observed_identities(&tx)
        .unwrap()
        .budgets
        .is_empty());
    let old = v5::observe(&tx, ledger.registry_id()).unwrap();
    assert!(old.ids(ConfirmationBindingSet::Exposures).is_empty());
    assert!(old.ids(ConfirmationBindingSet::UnknownOrigins).is_empty());
    tx.commit().unwrap();
    drop(conn);
    let observed = current(&ledger, None);
    assert_eq!(observed.seq, 1);
    assert_eq!(
        observed
            .confirmation
            .as_ref()
            .unwrap()
            .ids(ConfirmationBindingSet::Exposures)
            .len(),
        1
    );
    assert_eq!(
        observed
            .confirmation
            .as_ref()
            .unwrap()
            .ids(ConfirmationBindingSet::UnknownOrigins),
        ["e".repeat(32)]
    );
}
