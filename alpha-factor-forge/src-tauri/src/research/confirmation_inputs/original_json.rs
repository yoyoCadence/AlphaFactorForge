//! Bounded syntax observations of exact original bytes, not source authority.
//! Numbers remain opaque tokens until a separate declared-policy adapter.

use std::collections::BTreeSet;

use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};
use serde::Deserializer as _;
use serde_json::value::RawValue;

use crate::error::{AppError, AppResult};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_NODES: usize = 65_536;
const MAX_DEPTH: usize = 64;
const MAX_KEY_BYTES: usize = 1024;

fn invalid() -> AppError {
    AppError::Other("source_json_invalid".into())
}

/// Owns checked original bytes. Syntax validity grants no domain permission.
#[derive(Clone, Eq, PartialEq)]
pub(super) struct OriginalJson {
    bytes: Vec<u8>,
    nodes: usize,
}

impl OriginalJson {
    pub(super) fn parse(raw: &[u8]) -> AppResult<Self> {
        if raw.len() > MAX_BYTES {
            return Err(invalid());
        }
        let text = std::str::from_utf8(raw).map_err(|_| invalid())?;
        bound_container_depth(text)?;
        let value: &RawValue = serde_json::from_str(text).map_err(|_| invalid())?;
        let mut nodes = 0;
        walk(value, 1, &mut nodes)?;
        Ok(Self {
            bytes: raw.to_vec(),
            nodes,
        })
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) fn node_count(&self) -> usize {
        self.nodes
    }
}

/// RawValue skips containers iteratively with an internal scratch stack. Fence
/// that stack first; the full parser validates syntax and walk counts scalars.
fn bound_container_depth(text: &str) -> AppResult<()> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'[' | b'{' => {
                    if depth >= MAX_DEPTH {
                        return Err(invalid());
                    }
                    depth += 1;
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

fn walk(value: &RawValue, depth: usize, nodes: &mut usize) -> AppResult<()> {
    if depth > MAX_DEPTH || *nodes >= MAX_NODES {
        return Err(invalid());
    }
    // Both increments are bounded by the fixed limits before descending.
    *nodes += 1;
    let text = value.get();
    match text.as_bytes().first() {
        Some(b'{') => {
            let mut parser = serde_json::Deserializer::from_str(text);
            parser
                .deserialize_map(Children { depth, nodes })
                .map_err(|_| invalid())?;
            parser.end().map_err(|_| invalid())?;
        }
        Some(b'[') => {
            let mut parser = serde_json::Deserializer::from_str(text);
            parser
                .deserialize_seq(Children { depth, nodes })
                .map_err(|_| invalid())?;
            parser.end().map_err(|_| invalid())?;
        }
        Some(b'"') => {
            // Raw syntax skipping does not decode Unicode scalar escapes.
            let _: String = serde_json::from_str(text).map_err(|_| invalid())?;
        }
        // RawValue has checked bool/null/number grammar without interpreting
        // any numeric token, including values outside all machine ranges.
        _ => {}
    }
    Ok(())
}

struct Children<'a> {
    depth: usize,
    nodes: &'a mut usize,
}

impl<'de> Visitor<'de> for Children<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("bounded original JSON")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if key.len() > MAX_KEY_BYTES || !keys.insert(key) {
                return Err(A::Error::custom("source_json_invalid"));
            }
            let value = map.next_value::<&RawValue>()?;
            walk(value, self.depth + 1, self.nodes)
                .map_err(|_| A::Error::custom("source_json_invalid"))?;
        }
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(value) = seq.next_element::<&RawValue>()? {
            walk(value, self.depth + 1, self.nodes)
                .map_err(|_| A::Error::custom("source_json_invalid"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
