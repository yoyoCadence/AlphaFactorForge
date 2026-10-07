//! Manual-definition parser policies. Legacy JSON keeps serde_json's original
//! interpretation; only a hash-covered, explicitly marked document opts in to
//! correctly rounded binary64 numbers. This does not change runner/artifact IO.

use std::collections::BTreeMap;

use serde::{de::MapAccess, de::Visitor, Deserialize, Deserializer};
use serde_json::{value::RawValue, Map, Number, Value};

use crate::error::{AppError, AppResult};

pub const MANUAL_DEFINITION_VERSION: &str = "manual-strategy-definition-v1";
pub const ROUNDED_NUMERIC_POLICY: &str = "json-f64-roundtrip-v1";
pub const LEGACY_NUMERIC_POLICY: &str = "serde-json-default-v1";
const MAX_DEPTH: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionPolicy {
    Legacy,
    Rounded,
}

impl DefinitionPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => LEGACY_NUMERIC_POLICY,
            Self::Rounded => ROUNDED_NUMERIC_POLICY,
        }
    }
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Other(message.into())
}

/// Both markers must be absent for legacy interpretation. String-only marker
/// probing leaves every numeric token untouched until its policy is selected.
pub fn parse_definition_json(text: &str) -> AppResult<(DefinitionPolicy, Value)> {
    let header: BTreeMap<String, Box<RawValue>> = serde_json::from_str(text)?;
    let policy = match (header.get("definitionVersion"), header.get("numericPolicy")) {
        (None, None) => DefinitionPolicy::Legacy,
        (Some(version), Some(policy)) => {
            let version: String = serde_json::from_str(version.get())
                .map_err(|_| invalid("strategy definitionVersion must be a supported string"))?;
            let policy: String = serde_json::from_str(policy.get())
                .map_err(|_| invalid("strategy numericPolicy must be a supported string"))?;
            if version != MANUAL_DEFINITION_VERSION || policy != ROUNDED_NUMERIC_POLICY {
                return Err(invalid(
                    "unsupported manual strategy definitionVersion or numericPolicy",
                ));
            }
            DefinitionPolicy::Rounded
        }
        _ => {
            return Err(invalid(
                "strategy definitionVersion and numericPolicy must appear together",
            ))
        }
    };
    let value = match policy {
        // Preserve the existing parser, including legacy duplicate-key handling.
        DefinitionPolicy::Legacy => serde_json::from_str(text)?,
        DefinitionPolicy::Rounded => parse_rounded(text, 0)?,
    };
    Ok((policy, value))
}

struct RawObject(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for RawObject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = RawObject;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(RawObject(entries))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

fn parse_rounded(text: &str, depth: usize) -> AppResult<Value> {
    if depth > MAX_DEPTH {
        return Err(invalid(
            "manual strategy JSON exceeds the numeric policy depth limit",
        ));
    }
    // RawValue validates JSON syntax without converting the number token.
    let raw: Box<RawValue> = serde_json::from_str(text)?;
    let token = raw.get().trim();
    match token.as_bytes().first().copied() {
        Some(b'{') => {
            let RawObject(entries) = serde_json::from_str(token)?;
            let mut object = Map::new();
            for (key, value) in entries {
                if object.contains_key(&key) {
                    return Err(invalid(format!(
                        "manual strategy JSON has duplicate key {key:?}"
                    )));
                }
                object.insert(key, parse_rounded(value.get(), depth + 1)?);
            }
            Ok(Value::Object(object))
        }
        Some(b'[') => {
            let items: Vec<Box<RawValue>> = serde_json::from_str(token)?;
            items
                .into_iter()
                .map(|item| parse_rounded(item.get(), depth + 1))
                .collect::<AppResult<Vec<_>>>()
                .map(Value::Array)
        }
        Some(b'-' | b'0'..=b'9') => {
            // Keep integral tokens integral for typed integer fields. -0 is an
            // IEEE negative zero; canonical v2 hashing still normalizes it.
            if token != "-0" && !token.contains(['.', 'e', 'E']) {
                if let Ok(value) = token.parse::<i64>() {
                    return Ok(Value::Number(Number::from(value)));
                }
                if let Ok(value) = token.parse::<u64>() {
                    return Ok(Value::Number(Number::from(value)));
                }
            }
            let value = token.parse::<f64>().map_err(|error| {
                invalid(format!("manual strategy number is not binary64: {error}"))
            })?;
            Number::from_f64(value)
                .map(Value::Number)
                .ok_or_else(|| invalid("manual strategy numeric policy requires finite numbers"))
        }
        // String escaping, booleans and null never need a different parser.
        _ => Ok(serde_json::from_str(token)?),
    }
}

#[cfg(test)]
mod tests;
