//! Frozen whole-operation recipes, always installing original writers first.
use super::*;

fn n(value: &Value, key: &str) -> usize {
    value[key].as_u64().unwrap() as usize
}
fn recipe<'a>(value: &'a Value, collection: &str, kind: &str) -> &'a Value {
    value[collection]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == kind)
        .unwrap()
}
pub(super) struct DenyMarket {
    prior: (bool, bool),
    before: (usize, usize),
    payload: bool,
    output: bool,
}
impl DenyMarket {
    pub(super) fn new(payload: bool, output: bool) -> Self {
        Self {
            prior: (
                market::DENY_PAYLOAD.with(|v| v.replace(payload)),
                market::DENY_OUTPUT.with(|v| v.replace(output)),
            ),
            before: (
                market::PAYLOAD_ENTRIES.with(std::cell::Cell::get),
                market::OUTPUT_ENTRIES.with(std::cell::Cell::get),
            ),
            payload,
            output,
        }
    }
}
impl Drop for DenyMarket {
    fn drop(&mut self) {
        market::DENY_PAYLOAD.with(|v| v.set(self.prior.0));
        market::DENY_OUTPUT.with(|v| v.set(self.prior.1));
        if !std::thread::panicking() {
            if self.payload {
                assert_eq!(
                    market::PAYLOAD_ENTRIES.with(std::cell::Cell::get),
                    self.before.0
                );
            }
            if self.output {
                assert_eq!(
                    market::OUTPUT_ENTRIES.with(std::cell::Cell::get),
                    self.before.1
                );
            }
        }
    }
}
fn resource(kind: &str, over: bool) -> (Actual, Value) {
    let v = oracle();
    let r = recipe(&v, "resourceBoundaryRecipes", kind).clone();
    // Resource manifests freeze originals BEFORE the stored-row poison below.
    let manifest = recipe(&v["seedCorrection"], "resourceManifests", kind);
    let mut raw: Value =
        serde_json::from_str(manifest["originalConfigJson"].as_str().unwrap()).unwrap();
    if kind == "output" && over {
        raw["bases"][1]["id"] = json!(raw["bases"][1]["id"].as_str().unwrap().to_owned() + "o");
    }
    let c = case("captured-old-revision");
    let mut f = Actual::market();
    f.install(
        &serde_json::to_string(&raw).unwrap(),
        c["family"]["campaignDocumentJson"].as_str().unwrap(),
        c["family"]["campaignId"].as_str().unwrap(),
        "crypto:binance:BTCUSDT",
        "1h",
    );
    let definitions: Vec<(i64, String)> = f
        .conn()
        .prepare("SELECT id,original_definition_json FROM strategy_def ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(definitions.len(), n(manifest, "candidateCount"));
    for (i, (id, text)) in definitions.into_iter().enumerate() {
        let (hash,input):(String,String)=f.conn().query_row("SELECT s.strategy_hash,a.input_fingerprint_json FROM strategy_def s JOIN research_attempts a ON a.strategy_id=s.id WHERE s.id=?",[id],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(hash, manifest["candidates"][i]["strategyHash"]);
        let input: Value = serde_json::from_str(&input).unwrap();
        assert_eq!(
            input["seeds"]["randomEntry"],
            manifest["candidates"][i]["randomEntry"]
        );
        let mut definition: Value = serde_json::from_str(&text).unwrap();
        if kind == "output" {
            continue;
        }
        if kind == "hash" {
            let old: Value = serde_json::from_str(include_str!(
                "../../../../../../../fixtures/research/original-lineage-semantics-v1.json"
            ))
            .unwrap();
            let reference = recipe(&old, "resourceBoundaryRecipes", "hash");
            let padding = reference["asciiPaddingBytesByCandidateIndex"][i]
                .as_u64()
                .unwrap() as usize;
            definition["entryRules"] = json!(vec![0; 32000]);
            definition["entryCode"] = json!("x".repeat(
                padding
                    - if i == 0 {
                        n(&r, "storedFirstEntryCodeShortenBytes")
                    } else {
                        0
                    }
                    + usize::from(over && i == 0)
            ));
        } else if definition["mode"] == "dsl" {
            definition["dsl"]["name"] = json!("x".repeat(n(&r, "storedDslNameBytes")));
            if kind == "copy" {
                for j in 0..12000 {
                    definition["dsl"]["params"][format!("p{j:05}")] =
                        json!(if j < n(&r, "tenReplacements") { 10 } else { 0 });
                }
                definition["resourceCounter"] = r[if over {
                    "overWrapperResourceCounter"
                } else {
                    "wrapperResourceCounter"
                }]
                .clone();
            }
            f.conn()
                .execute(
                    "UPDATE strategy_def SET dsl_json=? WHERE id=?",
                    rusqlite::params![serde_json::to_string(&definition["dsl"]).unwrap(), id],
                )
                .unwrap();
        } else {
            definition["entrySig"] = json!(
                definition["entrySig"].as_str().unwrap().to_owned()
                    + &"x".repeat(n(&r, "paramsEntrySigSuffixBytes") + usize::from(over))
            );
        }
        f.conn()
            .execute(
                "UPDATE strategy_def SET original_definition_json=? WHERE id=?",
                rusqlite::params![serde_json::to_string(&definition).unwrap(), id],
            )
            .unwrap();
    }
    (f, r)
}
fn boundary(kind: &str) {
    for over in [false, true] {
        let (mut f, r) = resource(kind, over);
        let rows = f.capture();
        let before = entries();
        let planned = {
            let _old = warmup_controls::DenyWork::new(true, true);
            let _market = DenyMarket::new(true, true);
            let planned = market::planned(&rows).unwrap();
            assert_eq!(
                planned["jointPreparation"],
                r[if over {
                    "overJointPreparation"
                } else {
                    "exactJointPreparation"
                }],
                "{kind}/{over}"
            );
            assert_eq!(
                planned["jointWork"],
                r[if over { "overJoint" } else { "exactJoint" }],
                "{kind}/{over}"
            );
            if over {
                preserves(&mut f, &rows, false);
            }
            planned
        };
        assert_eq!(entries().1, before.1 + 1 + usize::from(over));
        if !over {
            if kind == "output" {
                let before = market::OUTPUT_ENTRIES.with(std::cell::Cell::get);
                let warm = warmup_controls::warm_entries();
                let output = preserves(&mut f, &rows, true).unwrap();
                assert_eq!(output.warmup().derivations().len(), 1101);
                assert_eq!(output.warmup().lineage().candidates().len(), 1101);
                for derivation in output.warmup().derivations() {
                    assert_eq!(
                        serde_json::to_value(derivation).unwrap(),
                        r["expectedEveryDerivation"]
                    );
                }
                assert_eq!(warmup_controls::warm_entries().0, warm.0 + 1101);
                assert_eq!(
                    market::OUTPUT_ENTRIES.with(std::cell::Cell::get),
                    before + 1
                );
            } else {
                let _old = warmup_controls::DenyWork::new(false, true);
                let _market = DenyMarket::new(true, true);
                // Exact resource acceptance then intentional old identity poison.
                preserves(&mut f, &rows, false);
            }
        }
        assert!(planned["jointWork"]["cloneNodes"].as_u64().unwrap() <= 262144);
    }
}
#[test]
fn market_whole_hash_exact_and_one_over_precede_all_payload() {
    boundary("hash");
}
#[test]
fn market_whole_copy_exact_and_one_over_precede_all_payload() {
    boundary("copy");
}
#[test]
fn market_whole_analysis_exact_and_one_over_precede_all_payload() {
    boundary("analysis");
}
#[test]
fn market_whole_output_exact1101_complete_and_one_over_before_payload() {
    boundary("output");
}

#[test]
fn market_joint_clone_limit_refuses_without_any_other_limit_exceeded() {
    let (mut f, _) = resource("hash", false);
    let originals: Vec<(i64, String)> = f
        .conn()
        .prepare("SELECT id,original_definition_json FROM strategy_def ORDER BY id LIMIT 2")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for (id, text) in originals {
        let mut definition: Value = serde_json::from_str(&text).unwrap();
        let old = definition["entryCode"].as_str().unwrap();
        definition["entryCode"] = json!(&old[..old.len() - 2_500_000]);
        f.conn()
            .execute(
                "UPDATE strategy_def SET original_definition_json=? WHERE id=?",
                rusqlite::params![serde_json::to_string(&definition).unwrap(), id],
            )
            .unwrap();
    }
    let source = data(SOURCE);
    let template = source["provenance"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["expectedRow"] == 7)
        .unwrap();
    let mut input: provenance::RawObservation =
        serde_json::from_str(template["inputJson"].as_str().unwrap()).unwrap();
    input.request_scope = json!(vec![Value::Null; 1000]);
    input.revision_of = None;
    let raw = hex::decode(template["rawHex"].as_str().unwrap()).unwrap();
    let (id, created) = provenance::record_raw(f.conn(), &f.store, &input, &raw).unwrap();
    assert!(created);
    // Resource-only extra relation claim: the frozen snapshot identity is NOT
    // updated, so this never asserts semantic success. Recapture measures the
    // real scope/third relation under the actual SQL capture budget.
    f.conn()
        .execute(
            "INSERT INTO market_snapshot_sources(snapshot_row_id,provenance_id) VALUES(1,?)",
            [id],
        )
        .unwrap();
    let rows = f.capture();
    assert_eq!(rows.provenance.len(), 3);
    assert!(rows.usage.text_bytes <= 33554432 && rows.usage.json_nodes <= 262144);
    let planned = market::planned(&rows).unwrap();
    let work = &planned["jointWork"];
    assert!(n(work, "cloneNodes") > 262144);
    for (key, limit) in [
        ("hashBytes", 33554432),
        ("copyBytes", 67108864),
        ("analysisUnits", 268435456),
        ("outputBytes", 4194304),
    ] {
        assert!(n(work, key) <= limit, "{key}");
    }
    assert!(n(&planned["jointPreparation"], "scratchBytes") <= 33554432);
    assert!(n(&planned["jointPreparation"], "scratchNodes") <= 262144);
    let _old = warmup_controls::DenyWork::new(true, true);
    let _market = DenyMarket::new(true, true);
    preserves(&mut f, &rows, false);
}
