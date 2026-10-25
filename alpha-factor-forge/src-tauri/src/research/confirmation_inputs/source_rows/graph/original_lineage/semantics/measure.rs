//! No-output byte measurement for the unchanged canonical/identity helpers.
use super::super::invalid;
use crate::error::AppResult;
use serde_json::Value;
use std::io::{self, Write};

fn add(a: usize, b: usize) -> AppResult<usize> {
    a.checked_add(b)
        .filter(|n| *n <= super::MAX_COPY_BYTES)
        .ok_or_else(invalid)
}
struct Count(usize);
impl Write for Count {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|n| *n <= super::MAX_COPY_BYTES)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "original_lineage_invalid")
            })?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn json(value: &Value) -> AppResult<usize> {
    let mut sink = Count(0);
    serde_json::to_writer(&mut sink, value).map_err(|_| invalid())?;
    Ok(sink.0)
}
fn u32_length(n: usize) -> AppResult<()> {
    u32::try_from(n).map(|_| ()).map_err(|_| invalid())
}
pub(super) fn binary(value: &Value) -> AppResult<usize> {
    match value {
        Value::Null | Value::Bool(_) => Ok(1),
        Value::Number(n) if n.as_f64().is_some_and(|n| n.is_finite()) => Ok(9),
        Value::Number(_) => Err(invalid()),
        Value::String(s) => {
            u32_length(s.len())?;
            add(5, s.len())
        }
        Value::Array(a) => {
            u32_length(a.len())?;
            a.iter().try_fold(5, |n, v| add(n, binary(v)?))
        }
        Value::Object(o) => {
            u32_length(o.len())?;
            o.iter().try_fold(5, |n, (k, v)| {
                u32_length(k.len())?;
                add(add(add(n, 4)?, k.len())?, binary(v)?)
            })
        }
    }
}
