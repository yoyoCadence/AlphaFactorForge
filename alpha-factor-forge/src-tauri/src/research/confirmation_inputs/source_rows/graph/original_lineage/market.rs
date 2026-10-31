//! One bounded historical market observation. No IO or source authority.
use super::{
    materialize, parse_work, prepare_with_joint_parse, semantics, warmup,
    OriginalMarketProjectionObservation, OriginalMarketSourceBinding, PreparedOriginalLineage,
    SourceRowsObservation, Work,
};
use crate::error::{AppError, AppResult};
use serde_json::Value;

mod measure;
mod verify;

fn invalid() -> AppError {
    AppError::Other("original_market_invalid".into())
}
fn require(ok: bool) -> AppResult<()> {
    if ok {
        Ok(())
    } else {
        Err(invalid())
    }
}
fn add(a: usize, b: usize) -> AppResult<usize> {
    a.checked_add(b).ok_or_else(invalid)
}
fn mul(a: usize, b: usize) -> AppResult<usize> {
    a.checked_mul(b).ok_or_else(invalid)
}

#[cfg(test)]
thread_local! {
    pub(super) static PAYLOAD_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static OUTPUT_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static CLONE_NODES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static DENY_PAYLOAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static DENY_OUTPUT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn enter_payload() {
    #[cfg(test)]
    {
        PAYLOAD_ENTRIES.with(|n| n.set(n.get() + 1));
        assert!(
            !DENY_PAYLOAD.with(std::cell::Cell::get),
            "market payload preceded joint reserve"
        );
    }
}
fn enter_output() {
    #[cfg(test)]
    {
        OUTPUT_ENTRIES.with(|n| n.set(n.get() + 1));
        assert!(
            !DENY_OUTPUT.with(std::cell::Cell::get),
            "market output preceded joint reserve"
        );
    }
}
fn constructed(value: &Value) -> AppResult<()> {
    #[cfg(test)]
    {
        let n = measure::value(value)?.1;
        CLONE_NODES.with(|v| v.set(v.get() + n));
    }
    let _ = value;
    Ok(())
}
fn copy(value: &Value) -> AppResult<Value> {
    constructed(value)?;
    Ok(value.clone())
}

#[cfg(test)]
struct CloneAudit {
    before: usize,
    limit: usize,
}
#[cfg(test)]
impl Drop for CloneAudit {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(
                CLONE_NODES.with(std::cell::Cell::get) - self.before <= self.limit,
                "market clone/construction work exceeded the whole reservation"
            );
        }
    }
}

struct Scope<'a> {
    row: &'a super::super::ProvenanceRow,
    value: Value,
}
struct Documents<'a> {
    rows: &'a SourceRowsObservation,
    coverage: Value,
    snapshot: Value,
    suspensions: Value,
    capabilities: Value,
    instrument: Value,
    calendar: Value,
    scopes: Vec<Scope<'a>>,
}
struct Prepared<'a> {
    lineage: PreparedOriginalLineage<'a>,
    documents: Documents<'a>,
    joint_parse: Work,
    market_parse: Work,
}

fn market_parse(rows: &SourceRowsObservation) -> AppResult<Work> {
    require(!rows.provenance.is_empty() && rows.provenance.len() <= 1024)?;
    let mut w = Work { bytes: 0, nodes: 0 };
    for d in [
        &rows.snapshot.coverage_json,
        &rows.snapshot.content_json,
        &rows.instrument.suspensions_json,
        &rows.instrument.source_capabilities_json,
        &rows.instrument.content_json,
        &rows.calendar.definition_json,
    ] {
        w.document(d, 1)?;
    }
    // Capture already retained provenance in ascending original ID order.
    for p in &rows.provenance {
        w.document(&p.request_scope_json, 1)?;
    }
    Ok(w)
}
fn prepare(rows: &SourceRowsObservation) -> AppResult<Prepared<'_>> {
    let old = parse_work(rows)?;
    let market_parse = market_parse(rows)?;
    let mut joint_parse = old;
    joint_parse.add(1, market_parse.bytes, market_parse.nodes)?;
    let lineage = prepare_with_joint_parse(rows, old, joint_parse)?;
    let documents = Documents {
        rows,
        coverage: materialize(&rows.snapshot.coverage_json)?,
        snapshot: materialize(&rows.snapshot.content_json)?,
        suspensions: materialize(&rows.instrument.suspensions_json)?,
        capabilities: materialize(&rows.instrument.source_capabilities_json)?,
        instrument: materialize(&rows.instrument.content_json)?,
        calendar: materialize(&rows.calendar.definition_json)?,
        scopes: rows
            .provenance
            .iter()
            .map(|row| {
                Ok(Scope {
                    row,
                    value: materialize(&row.request_scope_json)?,
                })
            })
            .collect::<AppResult<_>>()?,
    };
    Ok(Prepared {
        lineage,
        documents,
        joint_parse,
        market_parse,
    })
}

fn reserve(
    warm: (usize, usize, usize, usize, usize),
    market: &measure::MarketWork,
) -> AppResult<()> {
    require(
        add(warm.0, market.hash)? <= 33_554_432
            && add(warm.1, market.copy)? <= 67_108_864
            && add(warm.2, market.analysis)? <= 268_435_456
            && add(warm.3, market.clones)? <= 262_144
            && add(warm.4, market.output)? <= 4_194_304,
    )
}
fn bindings(rows: &SourceRowsObservation) -> Vec<OriginalMarketSourceBinding> {
    enter_output();
    let mut sources: Vec<_> = rows
        .provenance
        .iter()
        .map(|p| OriginalMarketSourceBinding {
            provenance_id: p.id,
            record_hash: p.record_hash.clone(),
        })
        .collect();
    sources.sort_unstable_by_key(|p| p.provenance_id);
    sources
}
pub(super) fn observe(
    rows: &SourceRowsObservation,
) -> AppResult<OriginalMarketProjectionObservation> {
    observe_inner(rows).map_err(|_| invalid())
}
fn observe_inner(rows: &SourceRowsObservation) -> AppResult<OriginalMarketProjectionObservation> {
    let prepared = prepare(rows)?;
    let lineage = semantics::reserve(&prepared.lineage)?;
    let warm = warmup::reserved_work(&prepared.lineage, &lineage)?;
    let work = measure::work(&prepared.documents, prepared.market_parse)?;
    reserve(warm, &work)?;
    #[cfg(test)]
    let _clone_audit = CloneAudit {
        before: CLONE_NODES.with(std::cell::Cell::get),
        limit: work.clones,
    };
    let verified = lineage.verify()?;
    verify::all(&prepared.documents)?;
    let warmup = warmup::finish(&prepared.lineage, verified)?;
    let result = OriginalMarketProjectionObservation {
        warmup,
        sources: bindings(rows),
        snapshot_id: rows.snapshot.snapshot_id.clone(),
        instrument_revision_hash: rows.instrument.content_hash.clone(),
        calendar_hash: rows.calendar.content_hash.clone(),
    };
    Ok(result)
}

#[cfg(test)]
pub(super) fn preparation_work(
    rows: &SourceRowsObservation,
) -> AppResult<((usize, usize), (usize, usize))> {
    let p = prepare(rows)?;
    Ok((
        (p.joint_parse.bytes, p.joint_parse.nodes),
        p.lineage.expansion_work(),
    ))
}

#[cfg(test)]
pub(super) fn planned(rows: &SourceRowsObservation) -> AppResult<Value> {
    let p = prepare(rows)?;
    let lineage = semantics::reserve(&p.lineage)?;
    let warm = warmup::reserved_work(&p.lineage, &lineage)?;
    let w = measure::work(&p.documents, p.market_parse)?;
    let (eb, en) = p.lineage.expansion_work();
    Ok(serde_json::json!({
        "marketWork": {"originalBytes":p.market_parse.bytes,"originalNodes":p.market_parse.nodes,
            "retainedTextBytes":w.text,"documentCompactBytes":w.document_bytes,
            "identityCompactBytes":w.identity_bytes,"identityNodes":w.identity_nodes,
            "sourceCount":p.documents.scopes.len(),"hashBytes":w.hash,"copyBytes":w.copy,
            "analysisUnits":w.analysis,"cloneNodes":w.clones,"outputBytes":w.output},
        "jointPreparation":{"parseBytes":p.joint_parse.bytes,"parseNodes":p.joint_parse.nodes,
            "expansionBytes":eb,"expansionNodes":en,"scratchBytes":add(p.joint_parse.bytes,eb)?,
            "scratchNodes":add(p.joint_parse.nodes,en)?},
        "oldPreparation":{"parseBytes":p.lineage.parse_work().0,"parseNodes":p.lineage.parse_work().1,
            "expansionBytes":eb,"expansionNodes":en,"scratchBytes":p.lineage.combined_work().0,
            "scratchNodes":p.lineage.combined_work().1},
        "jointWork":{"hashBytes":add(warm.0,w.hash)?,"copyBytes":add(warm.1,w.copy)?,
            "analysisUnits":add(warm.2,w.analysis)?,"cloneNodes":add(warm.3,w.clones)?,
            "outputBytes":add(warm.4,w.output)?}
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn market_checked_resource_arithmetic_refuses_overflow() {
        assert_eq!(add(usize::MAX - 1, 1).unwrap(), usize::MAX);
        assert_eq!(mul(usize::MAX, 1).unwrap(), usize::MAX);
        for result in [add(usize::MAX, 1), mul(usize::MAX, 2)] {
            assert!(matches!(result,Err(AppError::Other(code)) if code=="original_market_invalid"));
        }
    }
    #[test]
    fn market_structural_json_counter_handles_original_numeric_and_string_encodings() {
        for raw in [
            r#"null"#,
            r#"[]"#,
            r#"{}"#,
            r#"{"s":"\"\\\u0000\b\t\n\f\r\u00e9","a":[true,false,null]}"#,
            r#"[0,-0.0,1.0,1e-7,1e20,-9223372036854775808,18446744073709551615]"#,
        ] {
            let value: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(
                measure::value(&value).unwrap().0,
                serde_json::to_vec(&value).unwrap().len()
            );
        }
        let value = serde_json::json!({"keys do not count":[null,true,{"x":0}]});
        assert_eq!(measure::value(&value).unwrap().1, 6);
    }
}
