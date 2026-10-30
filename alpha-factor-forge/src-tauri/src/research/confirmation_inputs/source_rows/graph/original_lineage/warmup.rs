//! Whole-operation bounded warmup from one sealed, completely agreeing graph.
//! Reservation units are frozen in research-original-warmup-consumer-v1.md.
use super::{
    prepare_original_lineage, semantics, OriginalWarmupObservation, PreparedOriginalLineage,
};
use crate::{
    discovery_runner::execution::original_candidate_embargo,
    error::{AppError, AppResult},
    research::confirmation_inputs::source_rows::graph::SourceRowsObservation,
};
use serde_json::Value;

const MAX_HASH: usize = 33_554_432;
const MAX_COPY: usize = 67_108_864;
const MAX_ANALYSIS: usize = 268_435_456;
const MAX_OUTPUT: usize = 4_194_304;

fn invalid() -> AppError {
    AppError::Other("original_warmup_invalid".into())
}
fn add(a: usize, b: usize) -> AppResult<usize> {
    a.checked_add(b).ok_or_else(invalid)
}
fn mul(a: usize, b: usize) -> AppResult<usize> {
    a.checked_mul(b).ok_or_else(invalid)
}
fn text_len(v: &Value, keys: &[&str]) -> AppResult<usize> {
    keys.iter().try_fold(0, |n, key| {
        add(
            n,
            v.get(*key)
                .and_then(Value::as_str)
                .ok_or_else(invalid)?
                .len(),
        )
    })
}
fn nodes(v: &Value) -> AppResult<usize> {
    let n = match v {
        Value::Object(o) => o.values().try_fold(1, |n, v| add(n, nodes(v)?))?,
        Value::Array(a) => a.iter().try_fold(1, |n, v| add(n, nodes(v)?))?,
        _ => 1,
    };
    if n > 262_144 {
        return Err(invalid());
    }
    Ok(n)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateWork {
    json: usize,
    value_nodes: usize,
    text: usize,
    copy: usize,
    analysis: usize,
    clones: usize,
}
fn candidate_work(d: &Value) -> AppResult<CandidateWork> {
    match d.get("mode").and_then(Value::as_str) {
        Some("params") => {
            let s = text_len(d, &["entrySig", "exitSig", "fillMode", "direction"])?;
            Ok(CandidateWork {
                json: 0,
                value_nodes: 0,
                text: s,
                copy: add(mul(4, s)?, 1024)?,
                analysis: add(s, 256)?,
                clones: 0,
            })
        }
        Some("dsl") => {
            let v = d.get("dsl").filter(|v| v.is_object()).ok_or_else(invalid)?;
            if v.get("params")
                .and_then(Value::as_object)
                .is_some_and(|p| p.keys().any(|k| k.len() > 64))
            {
                return Err(invalid());
            }
            let j = semantics::json_len(v).map_err(|_| invalid())?;
            let n = nodes(v)?;
            let s = text_len(d, &["fillMode", "direction"])?;
            let metadata = add(mul(4096, n)?, 8192)?;
            Ok(CandidateWork {
                json: j,
                value_nodes: n,
                text: s,
                copy: add(add(mul(16, j)?, metadata)?, mul(4, s)?)?,
                analysis: add(mul(128, j)?, metadata)?,
                clones: n,
            })
        }
        _ => Err(invalid()),
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Work {
    hash: usize,
    copy: usize,
    analysis: usize,
    clones: usize,
    output: usize,
}
fn reserve(
    p: &PreparedOriginalLineage<'_>,
    lineage: &semantics::ReservedLineage<'_, '_>,
) -> AppResult<Work> {
    let (hash, copy, clones, output) = lineage.work();
    let scalars = mul(24, lineage.definitions().len())?;
    let mut w = Work {
        hash,
        copy: add(copy, scalars)?,
        analysis: 0,
        clones,
        output: add(output, scalars)?,
    };
    for d in lineage.definitions() {
        let c = candidate_work(d)?;
        w.copy = add(w.copy, c.copy)?;
        w.analysis = add(w.analysis, c.analysis)?;
        w.clones = add(w.clones, c.clones)?;
    }
    if w.hash > MAX_HASH
        || w.copy > MAX_COPY
        || w.analysis > MAX_ANALYSIS
        || w.output > MAX_OUTPUT
        || w.clones > p.combined_work().1
    {
        return Err(invalid());
    }
    Ok(w)
}

pub(super) fn observe(rows: &SourceRowsObservation) -> AppResult<OriginalWarmupObservation> {
    observe_inner(rows).map_err(|_| invalid())
}
fn observe_inner(rows: &SourceRowsObservation) -> AppResult<OriginalWarmupObservation> {
    let p = prepare_original_lineage(rows)?;
    let lineage = semantics::reserve(&p)?;
    reserve(&p, &lineage)?;
    let verified = lineage.verify()?;
    finish(&p, verified)
}

pub(super) fn reserved_work(
    p: &PreparedOriginalLineage<'_>,
    lineage: &semantics::ReservedLineage<'_, '_>,
) -> AppResult<(usize, usize, usize, usize, usize)> {
    let w = reserve(p, lineage)?;
    Ok((w.hash, w.copy, w.analysis, w.clones, w.output))
}

pub(super) fn finish(
    p: &PreparedOriginalLineage<'_>,
    verified: semantics::VerifiedLineage<'_, '_>,
) -> AppResult<OriginalWarmupObservation> {
    let mut derivations = Vec::with_capacity(verified.definitions().len());
    for definition in verified.definitions() {
        derivations.push(
            original_candidate_embargo(definition, p.config().embargo.holding_allowance_bars)
                .map_err(|_| invalid())?,
        );
    }
    Ok(OriginalWarmupObservation {
        lineage: verified.finish(),
        derivations,
    })
}

#[cfg(test)]
pub(super) fn planned_work(
    p: &PreparedOriginalLineage<'_>,
) -> AppResult<(usize, usize, usize, usize, usize)> {
    let lineage = semantics::reserve(p).map_err(|_| invalid())?;
    let w = reserve(p, &lineage)?;
    Ok((w.hash, w.copy, w.analysis, w.clones, w.output))
}
#[cfg(test)]
pub(super) fn planned_candidate(
    d: &Value,
) -> AppResult<(usize, usize, usize, usize, usize, usize)> {
    let w = candidate_work(d)?;
    Ok((w.json, w.value_nodes, w.text, w.copy, w.analysis, w.clones))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn warmup_checked_size_arithmetic_refuses_overflow() {
        assert_eq!(add(usize::MAX - 1, 1).unwrap(), usize::MAX);
        assert_eq!(mul(usize::MAX, 1).unwrap(), usize::MAX);
        for result in [add(usize::MAX, 1), mul(usize::MAX, 2)] {
            assert!(
                matches!(result, Err(AppError::Other(code)) if code == "original_warmup_invalid")
            );
        }
    }
}
