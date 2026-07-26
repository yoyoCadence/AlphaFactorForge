//! `strategy-v2`: the pure, dependency-free half of the durable identity
//! contract, ported from `src/core/hashing/index.ts`.
//!
//! The binary crate's `identity` module owns the same encoding at the SQLite
//! write boundary, but it lives outside this library (it depends on
//! `AppError` and the repository row types, which discovery core must not
//! reach). Both implementations are locked to the SAME committed
//! `src/core/hashing/identity-v2.fixture.json` — in this crate's test below
//! and in the binary crate's own test — so a divergence fails a test rather
//! than silently splitting one durable identity in two.
//!
//! Consolidating the two copies behind this module is a proposed follow-up; it
//! touches the product write boundary and is deliberately out of this slice's
//! scope.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const STRATEGY_HASH_VERSION: &str = "strategy-v2";
pub const DATASET_HASH_VERSION: &str = "dataset-content-v2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityError(pub String);

impl std::fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for IdentityError {}

struct ByteWriter(Vec<u8>);

impl ByteWriter {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn byte(&mut self, value: u8) {
        self.0.push(value);
    }

    fn raw(&mut self, value: &[u8]) {
        self.0.extend_from_slice(value);
    }

    fn u32(&mut self, value: usize) -> Result<(), IdentityError> {
        let value = u32::try_from(value)
            .map_err(|_| IdentityError(format!("identity length exceeds u32: {value}")))?;
        self.raw(&value.to_be_bytes());
        Ok(())
    }

    fn f64(&mut self, value: f64) -> Result<(), IdentityError> {
        if !value.is_finite() {
            return Err(IdentityError("identity numbers must be finite".into()));
        }
        // Normalize -0.0 to +0.0 exactly as the TypeScript encoder does.
        let normalized = if value == 0.0 { 0.0 } else { value };
        self.raw(&normalized.to_bits().to_be_bytes());
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), IdentityError> {
        self.u32(value.len())?;
        self.raw(value.as_bytes());
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.0
    }
}

fn write_canonical_value(writer: &mut ByteWriter, value: &Value) -> Result<(), IdentityError> {
    match value {
        Value::Null => writer.byte(0x00),
        Value::Bool(false) => writer.byte(0x01),
        Value::Bool(true) => writer.byte(0x02),
        Value::Number(number) => {
            writer.byte(0x03);
            let value = number.as_f64().ok_or_else(|| {
                IdentityError("strategy number cannot be represented as f64".into())
            })?;
            writer.f64(value)?;
        }
        Value::String(value) => {
            writer.byte(0x04);
            writer.string(value)?;
        }
        Value::Array(values) => {
            writer.byte(0x05);
            writer.u32(values.len())?;
            for value in values {
                write_canonical_value(writer, value)?;
            }
        }
        Value::Object(values) => {
            writer.byte(0x06);
            writer.u32(values.len())?;
            // serde_json's default map is ordered; TypeScript sorts by UTF-8
            // bytes. Sort explicitly so neither side depends on the other's
            // iteration guarantees.
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            for key in keys {
                writer.string(key)?;
                write_canonical_value(writer, &values[key])?;
            }
        }
    }
    Ok(())
}

/// Type-tagged canonical encoding shared byte-for-byte with TypeScript.
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, IdentityError> {
    let mut writer = ByteWriter::new();
    write_canonical_value(&mut writer, value)?;
    Ok(writer.finish())
}

fn sha256_versioned(version: &str, encoded: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(version.as_bytes());
    hasher.update([0]);
    hasher.update(encoded);
    format!("{version}:{}", hex::encode(hasher.finalize()))
}

/// Durable `strategy-v2` identity over `{ definition, execModel }`.
pub fn strategy_hash(
    definition: &Value,
    fee_pct: f64,
    slippage_pct: f64,
) -> Result<String, IdentityError> {
    let fee = serde_json::Number::from_f64(fee_pct)
        .ok_or_else(|| IdentityError("strategy feePct must be finite".into()))?;
    let slippage = serde_json::Number::from_f64(slippage_pct)
        .ok_or_else(|| IdentityError("strategy slipPct must be finite".into()))?;

    let mut exec_model = Map::new();
    exec_model.insert("feePct".into(), Value::Number(fee));
    exec_model.insert("slippagePct".into(), Value::Number(slippage));
    let mut payload = Map::new();
    payload.insert("definition".into(), definition.clone());
    payload.insert("execModel".into(), Value::Object(exec_model));

    let encoded = canonical_bytes(&Value::Object(payload))?;
    Ok(sha256_versioned(STRATEGY_HASH_VERSION, &encoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same committed fixture `crate::identity` is locked to, so the two
    /// copies of the encoder cannot drift apart unnoticed.
    #[test]
    fn matches_the_committed_typescript_identity_fixture() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../src/core/hashing/identity-v2.fixture.json"
        ))
        .unwrap();
        let strategy = &fixture["strategy"];
        let definition = &strategy["definition"];
        let fee_pct = strategy["execModel"]["feePct"].as_f64().unwrap();
        let slippage_pct = strategy["execModel"]["slippagePct"].as_f64().unwrap();

        assert_eq!(
            strategy_hash(definition, fee_pct, slippage_pct).unwrap(),
            strategy["expectedHash"].as_str().unwrap()
        );
    }

    #[test]
    fn sorts_object_keys_by_utf8_bytes_regardless_of_input_order() {
        // The write-boundary implementation makes the same guarantee; both are
        // pinned by the shared fixture above.
        let ordered = serde_json::json!({ "a": 1, "b": 2 });
        let mut reversed = serde_json::Map::new();
        reversed.insert("b".into(), serde_json::json!(2));
        reversed.insert("a".into(), serde_json::json!(1));
        assert_eq!(
            canonical_bytes(&ordered).unwrap(),
            canonical_bytes(&Value::Object(reversed)).unwrap()
        );
    }

    #[test]
    fn normalizes_negative_zero_and_rejects_non_finite_numbers() {
        let positive = serde_json::json!({ "value": 0.0 });
        let negative = serde_json::json!({ "value": -0.0 });
        assert_eq!(
            canonical_bytes(&positive).unwrap(),
            canonical_bytes(&negative).unwrap()
        );
        assert!(strategy_hash(&positive, f64::INFINITY, 0.0).is_err());
    }
}
