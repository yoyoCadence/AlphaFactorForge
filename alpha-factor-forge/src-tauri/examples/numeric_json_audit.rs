//! NUMERIC-JSON-001 audit only. Compare default parsing with a command-only
//! `--features serde_json/float_roundtrip` build; production Cargo.toml is unchanged.
//! Output is diagnostic evidence, never a replacement for old identities.
use alpha_factor_forge::discovery_core::identity::{canonical_bytes, strategy_hash};
use serde_json::{json, Value};
use std::{hint::black_box, time::Instant};

fn main() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../fixtures/research/numeric-json-audit-v1.json"
    ))
    .unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let reports: Vec<Value> = cases
        .iter()
        .map(|case| {
            let literal = case["literal"].as_str().unwrap();
            let standard: f64 = literal.parse().unwrap();
            let parsed: f64 = serde_json::from_str(literal).unwrap();
            let serialized = serde_json::to_string(&standard).unwrap();
            let roundtrip: f64 = serde_json::from_str(&serialized).unwrap();
            let mut original: Value =
                serde_json::from_str(case["definitionJson"].as_str().unwrap()).unwrap();
            let hash = strategy_hash(&original, original["feePct"].as_f64().unwrap(), 0.0)
                .unwrap();
            let canonical = hex::encode(canonical_bytes(&original).unwrap());
            original["feePct"] = json!(standard);
            let correct_hash = strategy_hash(&original, standard, 0.0).unwrap();
            assert_eq!(correct_hash, case["expectedJsStrategyHash"].as_str().unwrap());
            assert_eq!(
                hex::encode(canonical_bytes(&original).unwrap()),
                case["expectedJsCanonical"].as_str().unwrap()
            );
            let encoded = serde_json::to_string(&original).unwrap();
            let reread: Value = serde_json::from_str(&encoded).unwrap();
            json!({
                "id": case["id"], "standardBits": format!("{:016x}", standard.to_bits()),
                "parsedBits": format!("{:016x}", parsed.to_bits()),
                "serializedStandard": serialized,
                "roundtripBits": format!("{:016x}", roundtrip.to_bits()),
                "canonical": canonical, "strategyHash": hash,
                "rereadCorrectHash": strategy_hash(&reread, reread["feePct"].as_f64().unwrap(), 0.0).unwrap(),
            })
        })
        .collect();
    // Same 12,288-number document and 200 parses in each of three samples.
    // Time excludes construction/serialization and includes Value allocation.
    let document = format!(
        "[{}]",
        (0..2048)
            .flat_map(|_| cases.iter().map(|case| case["literal"].as_str().unwrap()))
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut timings = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        for _ in 0..200 {
            black_box(serde_json::from_str::<Value>(black_box(&document)).unwrap());
        }
        timings.push(start.elapsed().as_micros() as u64);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "version": "numeric-json-audit-v1", "cases": reports,
            "benchmark": {"numbersPerParse": 12288, "parsesPerSample": 200, "elapsedMicros": timings},
        }))
        .unwrap()
    );
}
