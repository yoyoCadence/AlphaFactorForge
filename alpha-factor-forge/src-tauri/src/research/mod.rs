//! P05 — the full research history (plan §3.3, docs/research-history-v1.md).
//!
//! Every candidate the runner executes is a *research attempt* against a
//! *hypothesis* that was frozen before execution, with the exact inputs and
//! engine it ran on, and — when it completes — an immutable *artifact*
//! holding the complete result. The existing `backtest_summary` /
//! `backtest_trades` rows remain the latest-result projection the UI reads
//! today; they are still upserted, so a re-run overwrites the projection but
//! never the attempt that produced the earlier result.
//!
//! `history` is the database half (rows and their state machine, written
//! inside the runner's own transactions); `artifacts` is the file half
//! (content-addressed, staged and renamed into place, never deleted once
//! referenced). Neither names a host or a framework.

pub mod artifacts;
pub mod history;
pub mod trial_ledger;
pub mod trial_ledger_workspace;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::AppResult;

/// The frozen hypothesis document's version.
pub const HYPOTHESIS_VERSION: &str = "hypothesis-v1";
/// The immutable candidate result artifact's version (its `kind`).
pub const CANDIDATE_RESULT_VERSION: &str = "candidate-result-v1";

/// Canonical JSON text (object keys sorted recursively, no whitespace) as
/// bytes: what an artifact file contains and what a hypothesis hashes, so the
/// same document produces the same bytes wherever it is produced and a
/// reader gets plain JSON back. (The identity module's `canonical_bytes` is
/// a hashing encoding, not JSON, and is not used here.)
pub fn canonical_json(value: &Value) -> AppResult<Vec<u8>> {
    let mut out = String::new();
    write_canonical(value, &mut out)?;
    Ok(out.into_bytes())
}

fn write_canonical(value: &Value, out: &mut String) -> AppResult<()> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => out.push_str(&serde_json::to_string(s)?),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key)?);
                out.push(':');
                write_canonical(&map[*key], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_keys_recursively_and_is_plain_json() {
        let value = json!({ "z": [ { "b": 1, "a": "x\"y" } ], "a": null, "m": { "k2": true, "k1": 1.5 } });
        let text = String::from_utf8(canonical_json(&value).unwrap()).unwrap();
        assert_eq!(text, r#"{"a":null,"m":{"k1":1.5,"k2":true},"z":[{"a":"x\"y","b":1}]}"#);
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value, "round-trips");
        let reordered = json!({ "m": { "k1": 1.5, "k2": true }, "z": [ { "a": "x\"y", "b": 1 } ], "a": null });
        assert_eq!(canonical_json(&reordered).unwrap(), canonical_json(&value).unwrap(), "order-independent");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
