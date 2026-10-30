//! Structural byte/node counts only. No serialization, sorting or hashing.
use super::{add, invalid, mul, AppResult, Documents, Work};
use serde_json::Value;
use std::fmt::{self, Write};

struct Count(usize);
impl Write for Count {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0 = self.0.checked_add(s.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}
fn number(n: impl fmt::Display) -> AppResult<usize> {
    let mut out = Count(0);
    write!(&mut out, "{n}").map_err(|_| invalid())?;
    Ok(out.0)
}
pub(super) fn text(s: &str) -> AppResult<usize> {
    s.chars().try_fold(2, |n, c| {
        add(
            n,
            match c {
                '"' | '\\' | '\u{8}' | '\t' | '\n' | '\u{c}' | '\r' => 2,
                '\0'..='\u{1f}' => 6,
                _ => c.len_utf8(),
            },
        )
    })
}
pub(super) fn value(v: &Value) -> AppResult<(usize, usize)> {
    match v {
        Value::Null => Ok((4, 1)),
        Value::Bool(v) => Ok((if *v { 4 } else { 5 }, 1)),
        Value::Number(v) => Ok((number(v)?, 1)),
        Value::String(v) => Ok((text(v)?, 1)),
        Value::Array(a) => {
            a.iter()
                .try_fold((add(2, a.len().saturating_sub(1))?, 1), |(b, n), v| {
                    let (vb, vn) = value(v)?;
                    Ok((add(b, vb)?, add(n, vn)?))
                })
        }
        Value::Object(o) => {
            o.iter()
                .try_fold((add(2, o.len().saturating_sub(1))?, 1), |(b, n), (k, v)| {
                    let (vb, vn) = value(v)?;
                    Ok((add(add(add(b, text(k)?)?, 1)?, vb)?, add(n, vn)?))
                })
        }
    }
}
fn optional_text(s: &Option<String>) -> AppResult<usize> {
    s.as_deref().map(text).unwrap_or(Ok(4))
}
fn optional_integer(v: Option<i64>) -> AppResult<usize> {
    v.map(number).unwrap_or(Ok(4))
}
fn object(fields: &[(&str, usize)]) -> AppResult<usize> {
    fields
        .iter()
        .try_fold(add(2, fields.len().saturating_sub(1))?, |n, (k, v)| {
            add(add(add(n, text(k)?)?, 1)?, *v)
        })
}
fn provenance(d: &super::Scope<'_>) -> AppResult<(usize, usize)> {
    let p = d.row;
    let (scope, nodes) = value(&d.value)?;
    Ok((
        object(&[
            ("version", text(&p.version)?),
            ("instrumentId", text(&p.instrument_id)?),
            ("interval", text(&p.interval)?),
            ("source", text(&p.source)?),
            ("role", text(&p.role)?),
            ("requestScope", scope),
            ("rawSha256", text(&p.raw_sha256)?),
            ("rawByteLen", number(p.raw_byte_len)?),
            ("mediaType", text(&p.raw_media_type)?),
            ("retrievedAt", text(&p.retrieved_at)?),
            ("availableAt", optional_text(&p.available_at)?),
            ("accepted", if p.accepted == 1 { 4 } else { 5 }),
            ("rejectionReason", optional_text(&p.rejection_reason)?),
            ("revisionOf", optional_integer(p.revision_of)?),
        ])?,
        add(14, nodes)?,
    ))
}
fn retained(d: &Documents<'_>) -> AppResult<usize> {
    let r = d.rows;
    let s = &r.snapshot;
    let i = &r.instrument;
    let c = &r.calendar;
    let ds = &r.dataset;
    let mut n = 0;
    for text in [
        &s.snapshot_id,
        &s.version,
        &s.instrument_id,
        &s.interval,
        &s.dataset_hash,
        &s.price_basis,
        &s.calendar_id,
        &s.kind,
        &s.status,
        &i.instrument_id,
        &i.content_hash,
        &i.version,
        &i.market,
        &i.venue,
        &i.symbol,
        &i.base,
        &i.quote,
        &i.asset_type,
        &i.session_calendar_id,
        &i.timezone,
        &c.calendar_id,
        &c.version,
        &c.kind,
        &c.timezone,
        &c.content_hash,
        &ds.exchange,
        &ds.symbol,
        &ds.interval,
        &ds.source,
        &ds.dataset_hash,
    ] {
        n = add(n, text.len())?;
    }
    for text in [&s.corporate_action_version, &s.cost_profile_version]
        .into_iter()
        .flatten()
    {
        n = add(n, text.len())?;
    }
    for original in [
        &s.coverage_json,
        &s.content_json,
        &i.suspensions_json,
        &i.source_capabilities_json,
        &i.content_json,
        &c.definition_json,
    ] {
        n = add(n, original.as_bytes().len())?;
    }
    for p in &r.provenance {
        for text in [
            &p.record_hash,
            &p.version,
            &p.instrument_id,
            &p.interval,
            &p.source,
            &p.role,
            &p.raw_sha256,
            &p.raw_artifact_path,
            &p.raw_media_type,
            &p.retrieved_at,
        ] {
            n = add(n, text.len())?;
        }
        for text in [&p.available_at, &p.rejection_reason].into_iter().flatten() {
            n = add(n, text.len())?;
        }
        n = add(n, p.request_scope_json.as_bytes().len())?;
    }
    Ok(n)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MarketWork {
    pub(super) text: usize,
    pub(super) document_bytes: usize,
    pub(super) identity_bytes: usize,
    pub(super) identity_nodes: usize,
    pub(super) hash: usize,
    pub(super) copy: usize,
    pub(super) analysis: usize,
    pub(super) clones: usize,
    pub(super) output: usize,
}
pub(super) fn work(d: &Documents<'_>, parse: Work) -> AppResult<MarketWork> {
    let text = retained(d)?;
    let mut document_bytes = 0;
    for v in [
        &d.coverage,
        &d.snapshot,
        &d.suspensions,
        &d.capabilities,
        &d.instrument,
        &d.calendar,
    ] {
        document_bytes = add(document_bytes, value(v)?.0)?;
    }
    for s in &d.scopes {
        document_bytes = add(document_bytes, value(&s.value)?.0)?;
    }
    let mut identity_bytes = 0;
    let mut identity_nodes = 0;
    for v in [&d.calendar, &d.instrument, &d.snapshot] {
        let (b, n) = value(v)?;
        identity_bytes = add(identity_bytes, b)?;
        identity_nodes = add(identity_nodes, n)?;
    }
    for s in &d.scopes {
        let (b, n) = provenance(s)?;
        identity_bytes = add(identity_bytes, b)?;
        identity_nodes = add(identity_nodes, n)?;
    }
    let all_text = add(add(text, document_bytes)?, identity_bytes)?;
    let metadata = add(
        mul(4096, add(parse.nodes, identity_nodes)?)?,
        mul(8192, add(d.scopes.len(), 4)?)?,
    )?;
    let output = add(192, mul(72, d.scopes.len())?)?;
    Ok(MarketWork {
        text,
        document_bytes,
        identity_bytes,
        identity_nodes,
        hash: identity_bytes,
        copy: add(add(mul(32, all_text)?, metadata)?, output)?,
        analysis: add(mul(128, all_text)?, metadata)?,
        clones: add(mul(4, parse.nodes)?, mul(2, identity_nodes)?)?,
        output,
    })
}
