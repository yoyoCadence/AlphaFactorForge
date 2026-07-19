//! Versioned durable identity contracts shared byte-for-byte with TypeScript.
//! This module is pure: it owns validation/encoding/hashing, not persistence.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    db::repositories::{Candle, Dataset, StrategyDef},
    error::{AppError, AppResult},
};

pub const STRATEGY_HASH_VERSION: &str = "strategy-v2";
pub const DATASET_HASH_VERSION: &str = "dataset-content-v2";
pub const DATASET_FIELD_MAPPING_VERSION: &str =
    "ohlcv(timestamp:i64-ms,open:f64,high:f64,low:f64,close:f64,volume:f64)-v1";
const JS_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

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

    fn u32(&mut self, value: usize) -> AppResult<()> {
        let value = u32::try_from(value)
            .map_err(|_| AppError::Other(format!("identity length exceeds u32: {value}")))?;
        self.raw(&value.to_be_bytes());
        Ok(())
    }

    fn i64(&mut self, value: i64) {
        self.raw(&value.to_be_bytes());
    }

    fn f64(&mut self, value: f64) -> AppResult<()> {
        if !value.is_finite() {
            return Err(AppError::Other("identity numbers must be finite".into()));
        }
        let normalized = if value == 0.0 { 0.0 } else { value };
        self.raw(&normalized.to_bits().to_be_bytes());
        Ok(())
    }

    fn string(&mut self, value: &str) -> AppResult<()> {
        self.u32(value.len())?;
        self.raw(value.as_bytes());
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.0
    }
}

fn sha256_versioned(version: &str, encoded: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(version.as_bytes());
    hasher.update([0]);
    hasher.update(encoded);
    format!("{version}:{}", hex::encode(hasher.finalize()))
}

fn write_canonical_value(writer: &mut ByteWriter, value: &Value) -> AppResult<()> {
    match value {
        Value::Null => writer.byte(0x00),
        Value::Bool(false) => writer.byte(0x01),
        Value::Bool(true) => writer.byte(0x02),
        Value::Number(number) => {
            writer.byte(0x03);
            let value = number.as_f64().ok_or_else(|| {
                AppError::Other("strategy number cannot be represented as f64".into())
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

pub fn canonical_bytes(value: &Value) -> AppResult<Vec<u8>> {
    let mut writer = ByteWriter::new();
    write_canonical_value(&mut writer, value)?;
    Ok(writer.finish())
}

pub fn strategy_hash_from_definition_json(definition_json: &str) -> AppResult<String> {
    let definition: Value = serde_json::from_str(definition_json)?;
    let object = definition
        .as_object()
        .ok_or_else(|| AppError::Other("strategy definition must be a JSON object".into()))?;
    let fee_pct = numeric_field(object, "feePct")?;
    let slippage_pct = numeric_field(object, "slipPct")?;

    let mut exec_model = Map::new();
    exec_model.insert("feePct".into(), fee_pct.clone());
    exec_model.insert("slippagePct".into(), slippage_pct.clone());
    let mut payload = Map::new();
    payload.insert("definition".into(), definition);
    payload.insert("execModel".into(), Value::Object(exec_model));
    let encoded = canonical_bytes(&Value::Object(payload))?;
    Ok(sha256_versioned(STRATEGY_HASH_VERSION, &encoded))
}

fn numeric_field<'a>(object: &'a Map<String, Value>, key: &str) -> AppResult<&'a Value> {
    let value = object
        .get(key)
        .ok_or_else(|| AppError::Other(format!("strategy definition is missing {key}")))?;
    let number = value
        .as_f64()
        .ok_or_else(|| AppError::Other(format!("strategy {key} must be numeric")))?;
    if !number.is_finite() {
        return Err(AppError::Other(format!("strategy {key} must be finite")));
    }
    Ok(value)
}

pub fn verify_strategy_identity(strategy: &StrategyDef) -> AppResult<()> {
    let definition: Value = serde_json::from_str(&strategy.original_definition_json)?;
    let mode = definition
        .as_object()
        .and_then(|object| object.get("mode"))
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Other("strategy definition must contain string mode".into()))?;
    if mode != strategy.kind {
        return Err(AppError::Other(format!(
            "strategy type {} does not match definition mode {mode}",
            strategy.kind
        )));
    }
    let expected = strategy_hash_from_definition_json(&strategy.original_definition_json)?;
    if strategy.strategy_hash != expected {
        return Err(AppError::Other(format!(
            "strategy identity mismatch: expected {expected}"
        )));
    }
    Ok(())
}

fn require_metadata<'a>(value: &'a str, field: &str) -> AppResult<&'a str> {
    if value.is_empty() || value.trim() != value {
        return Err(AppError::Other(format!(
            "dataset {field} must be non-empty and already trimmed"
        )));
    }
    Ok(value)
}

fn normalized_number(value: f64) -> AppResult<f64> {
    if !value.is_finite() {
        return Err(AppError::Other(
            "dataset OHLCV values must be finite".into(),
        ));
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

pub fn normalize_dataset_candles(candles: &[Candle]) -> AppResult<Vec<Candle>> {
    if candles.is_empty() {
        return Err(AppError::Other("no candles to import".into()));
    }
    u32::try_from(candles.len())
        .map_err(|_| AppError::Other("dataset candle count exceeds u32".into()))?;
    let mut normalized = Vec::with_capacity(candles.len());
    for candle in candles {
        if candle.timestamp.unsigned_abs() > JS_MAX_SAFE_INTEGER {
            return Err(AppError::Other(format!(
                "dataset timestamp must be a JavaScript safe integer: {}",
                candle.timestamp
            )));
        }
        normalized.push(Candle {
            timestamp: candle.timestamp,
            open: normalized_number(candle.open)?,
            high: normalized_number(candle.high)?,
            low: normalized_number(candle.low)?,
            close: normalized_number(candle.close)?,
            volume: normalized_number(candle.volume)?,
        });
    }
    normalized.sort_by_key(|candle| candle.timestamp);
    for pair in normalized.windows(2) {
        if pair[0].timestamp == pair[1].timestamp {
            return Err(AppError::Other(format!(
                "duplicate candle timestamp: {}",
                pair[0].timestamp
            )));
        }
    }
    Ok(normalized)
}

#[cfg(test)]
pub fn dataset_content_hash(dataset: &Dataset, candles: &[Candle]) -> AppResult<String> {
    let normalized = normalize_dataset_candles(candles)?;
    dataset_content_hash_normalized(dataset, &normalized)
}

fn dataset_content_hash_normalized(dataset: &Dataset, normalized: &[Candle]) -> AppResult<String> {
    require_metadata(&dataset.exchange, "exchange")?;
    require_metadata(&dataset.symbol, "symbol")?;
    require_metadata(&dataset.interval, "interval")?;

    if dataset.candle_count != normalized.len() as i64
        || dataset.start_time != normalized[0].timestamp
        || dataset.end_time != normalized[normalized.len() - 1].timestamp
    {
        return Err(AppError::Other(
            "dataset count or time bounds do not match normalized candles".into(),
        ));
    }

    let mut writer = ByteWriter::new();
    writer.string(DATASET_FIELD_MAPPING_VERSION)?;
    writer.string(&dataset.exchange)?;
    writer.string(&dataset.symbol)?;
    writer.string(&dataset.interval)?;
    writer.i64(dataset.start_time);
    writer.i64(dataset.end_time);
    writer.u32(normalized.len())?;
    for candle in normalized {
        writer.i64(candle.timestamp);
        writer.f64(candle.open)?;
        writer.f64(candle.high)?;
        writer.f64(candle.low)?;
        writer.f64(candle.close)?;
        writer.f64(candle.volume)?;
    }
    Ok(sha256_versioned(DATASET_HASH_VERSION, &writer.finish()))
}

pub fn verify_dataset_identity(dataset: &Dataset, candles: &[Candle]) -> AppResult<Vec<Candle>> {
    let normalized = normalize_dataset_candles(candles)?;
    let expected = dataset_content_hash_normalized(dataset, &normalized)?;
    if dataset.dataset_hash != expected {
        return Err(AppError::Other(format!(
            "dataset identity mismatch: expected {expected}"
        )));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Fixture {
        fixture_version: String,
        strategy: StrategyFixture,
        dataset: DatasetFixture,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct StrategyFixture {
        definition: Value,
        expected_hash: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DatasetFixture {
        meta: DatasetMetaFixture,
        candles: Vec<Candle>,
        expected_hash: String,
    }

    #[derive(Deserialize)]
    struct DatasetMetaFixture {
        exchange: String,
        symbol: String,
        interval: String,
    }

    fn dataset(candles: &[Candle]) -> Dataset {
        Dataset {
            id: None,
            exchange: "binance".into(),
            symbol: "BTCUSDT".into(),
            interval: "1h".into(),
            start_time: candles.iter().map(|candle| candle.timestamp).min().unwrap(),
            end_time: candles.iter().map(|candle| candle.timestamp).max().unwrap(),
            candle_count: candles.len() as i64,
            source: "fixture".into(),
            dataset_hash: String::new(),
        }
    }

    #[test]
    fn dataset_hash_is_order_independent_and_content_sensitive() {
        let first = Candle {
            timestamp: 1,
            open: 10.0,
            high: 11.0,
            low: 9.0,
            close: 10.5,
            volume: 100.0,
        };
        let second = Candle {
            timestamp: 2,
            open: 10.5,
            high: 12.0,
            low: 10.0,
            close: 11.5,
            volume: 120.0,
        };
        let ordered = vec![first.clone(), second.clone()];
        let reversed = vec![second, first];
        let meta = dataset(&ordered);
        assert_eq!(
            dataset_content_hash(&meta, &ordered).unwrap(),
            dataset_content_hash(&meta, &reversed).unwrap()
        );

        let mut changed = ordered.clone();
        changed[1].close = 11.6;
        assert_ne!(
            dataset_content_hash(&meta, &ordered).unwrap(),
            dataset_content_hash(&meta, &changed).unwrap()
        );
    }

    #[test]
    fn dataset_identity_rejects_duplicates_and_non_finite_values() {
        let candle = Candle {
            timestamp: 1,
            open: 10.0,
            high: 11.0,
            low: 9.0,
            close: 10.5,
            volume: 100.0,
        };
        assert!(normalize_dataset_candles(&[candle.clone(), candle.clone()]).is_err());
        let mut invalid = candle;
        invalid.close = f64::INFINITY;
        assert!(normalize_dataset_candles(&[invalid]).is_err());
    }

    #[test]
    fn matches_the_committed_typescript_reference_fixture_exactly() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../src/core/hashing/identity-v2.fixture.json"
        ))
        .unwrap();
        assert_eq!(fixture.fixture_version, "identity-v2-fixture-1");

        let definition_json = serde_json::to_string(&fixture.strategy.definition).unwrap();
        assert_eq!(
            strategy_hash_from_definition_json(&definition_json).unwrap(),
            fixture.strategy.expected_hash
        );

        let mut dataset = Dataset {
            id: None,
            exchange: fixture.dataset.meta.exchange,
            symbol: fixture.dataset.meta.symbol,
            interval: fixture.dataset.meta.interval,
            start_time: fixture
                .dataset
                .candles
                .iter()
                .map(|candle| candle.timestamp)
                .min()
                .unwrap(),
            end_time: fixture
                .dataset
                .candles
                .iter()
                .map(|candle| candle.timestamp)
                .max()
                .unwrap(),
            candle_count: fixture.dataset.candles.len() as i64,
            source: "fixture".into(),
            dataset_hash: fixture.dataset.expected_hash,
        };
        let expected = dataset.dataset_hash.clone();
        assert_eq!(
            dataset_content_hash(&dataset, &fixture.dataset.candles).unwrap(),
            expected
        );
        verify_dataset_identity(&dataset, &fixture.dataset.candles).unwrap();
        dataset.dataset_hash = "legacy-unversioned".into();
        assert!(verify_dataset_identity(&dataset, &fixture.dataset.candles).is_err());
    }
}
