use super::*;
use crate::discovery_core::confirmation_contracts::read_operation_provenance;
use serde_json::{json, Value};

fn fixtures() -> (Value, Value) {
    (
        serde_json::from_str(include_str!(
            "../../../../fixtures/rs-core/research-confirmation-graph-v1.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../../fixtures/rs-core/research-confirmation-history-records-v1.json"
        ))
        .unwrap(),
    )
}
#[test]
fn provenance_getters_reproduce_original_ids_and_origins_without_execution_records() {
    let (g, h) = fixtures();
    for name in ["validationOperation", "testOperation"] {
        let r = &g["records"][name];
        assert_eq!(
            read_operation_provenance(&r.to_string()).unwrap(),
            (
                r["id"].as_str().unwrap().into(),
                r["body"]["originRegistryId"].as_str().unwrap().into()
            )
        );
    }
    let mut exposures = 0;
    for record in h["records"]
        .as_object()
        .unwrap()
        .values()
        .filter(|v| v["body"]["contractVersion"] == EXPOSURE_VERSION)
    {
        exposures += 1;
        assert_eq!(
            read_exposure_provenance(&record.to_string()).unwrap(),
            (
                record["id"].as_str().unwrap().into(),
                record["body"]["originRegistryId"].as_str().unwrap().into()
            )
        );
    }
    assert!(exposures >= 3);
}
#[test]
fn provenance_never_normalizes_duplicate_unknown_or_mismatched_original_data() {
    let (g, _) = fixtures();
    let originals = [
        g["records"]["validationOperation"].clone(),
        g["records"]["validationExposure"].clone(),
    ];
    for (kind, original) in originals.into_iter().enumerate() {
        let check = |v: &str| {
            let error = if kind == 0 {
                read_operation_provenance(v).unwrap_err().to_string()
            } else {
                read_exposure_provenance(v).unwrap_err().to_string()
            };
            assert!(!error.contains("PROTECTED_SENTINEL"));
        };
        let mut v = original.clone();
        v["body"]["PROTECTED_SENTINEL"] = json!(true);
        check(&v.to_string());
        let mut v = original.clone();
        v["id"] = json!("f".repeat(64));
        check(&v.to_string());
        let mut v = original.clone();
        v["body"]["originRegistryId"] = json!("PROTECTED_SENTINEL");
        check(&v.to_string());
        let raw = original.to_string();
        let needle = format!(
            "\"originRegistryId\":{}",
            original["body"]["originRegistryId"]
        );
        let duplicate = raw.replacen(&needle, &format!("{needle},{needle}"), 1);
        assert_ne!(raw, duplicate);
        check(&duplicate);
        let mut v = original;
        v["body"]
            .as_object_mut()
            .unwrap()
            .remove("originRegistryId");
        check(&v.to_string());
    }
}
#[test]
fn rehashed_wrong_operation_domains_do_not_become_migration_provenance() {
    let (g, _) = fixtures();
    for name in [
        "confirmationBatchId",
        "allocationId",
        "familyId",
        "workspaceId",
        "stage",
        "selectionId",
    ] {
        let mut v = g["records"]["validationOperation"].clone();
        v["body"][name] = json!("PROTECTED_SENTINEL");
        v["id"] =
            json!(identity(v["body"]["contractVersion"].as_str().unwrap(), &v["body"]).unwrap());
        let error = read_operation_provenance(&v.to_string())
            .unwrap_err()
            .to_string();
        assert!(!error.contains("PROTECTED_SENTINEL"));
    }
}
