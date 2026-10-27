//! Whole-family original lineage agreement after bounded preparation.
//! Only borrowed indexes precede the complete payload-work reservation.
use super::{
    invalid, CandidateLineageObservation, OriginalLineageObservation, PreparedOriginalLineage,
};
use crate::{
    error::AppResult,
    research::{canonical_json, sha256_hex},
};
use alpha_factor_forge::discovery_core::{config::DiscoveryBase, identity};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
mod measure;
pub(super) fn json_len(v: &Value) -> AppResult<usize> {
    measure::json(v)
}
const MAX_HASH_BYTES: usize = 33_554_432;
const MAX_COPY_BYTES: usize = 67_108_864;
const MAX_OUTPUT_BYTES: usize = 4_194_304;
const PARAM_FAILURE: &str="Not stated: a mechanical parameter sweep of a preset strategy. Treated as unexplained until a hypothesis with stated failure modes replaces it (P15).";
const DSL_FAILURE: &str="Authored DSL candidate: fails if the frozen expression has no trades, loses after costs, or does not pass the existing validation gates. AI rationale/failure-mode generation remains P15.";

#[cfg(test)]
thread_local! {
    pub(super) static PAYLOAD_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static DENY_PAYLOAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn enter_payload() {
    #[cfg(test)]
    {
        PAYLOAD_ENTRIES.with(|n| n.set(n.get() + 1));
        assert!(
            !DENY_PAYLOAD.with(std::cell::Cell::get),
            "semantic payload work must not precede whole preflight"
        );
    }
}
fn require(ok: bool) -> AppResult<()> {
    if ok {
        Ok(())
    } else {
        Err(invalid())
    }
}
fn text<'a>(v: &'a Value, k: &str) -> AppResult<&'a str> {
    v.get(k).and_then(Value::as_str).ok_or_else(invalid)
}
fn keys(v: &Value, expected: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn cost(v: &Value, k: &str) -> AppResult<f64> {
    v.get(k)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite() && *n >= 0.0)
        .ok_or_else(invalid)
}
fn contracts(v: &Value, version: &str) -> bool {
    let v1 = version == "discovery-config-v1";
    let v3 = version == "discovery-config-v3";
    let expected = [
        ("strategyHash", "strategy-v2"),
        ("datasetHash", "dataset-content-v2"),
        ("split", "validation-split-v1"),
        ("embargo", "embargo-derivation-v1"),
        ("backtest", "backtest-execution-v1"),
        ("metrics", "metrics-v2"),
        ("benchmarks", "benchmark-suite-v1"),
        ("randomEntry", "random-entry-v1"),
        ("gate", "gate-v1"),
        ("score", "score-v1"),
        ("seed", "seed-v1"),
        (
            "enumeration",
            if v1 {
                "discovery-enumeration-v1"
            } else {
                "discovery-enumeration-v2"
            },
        ),
    ];
    v.as_object()
        .is_some_and(|o| o.len() == 12 + usize::from(!v1) + 2 * usize::from(v3))
        && expected
            .iter()
            .all(|(k, s)| v.get(*k).and_then(Value::as_str) == Some(*s))
        && (v1 || v.get("strategyDsl").and_then(Value::as_str) == Some("strategy-dsl-v1"))
        && (!v3
            || (v.get("walkForward").and_then(Value::as_str) == Some("research-walk-forward-v1")
                && v.get("walkForwardEvidence").and_then(Value::as_str)
                    == Some("walk-forward-evidence-v1")))
}
fn check_engine(engine: &Value, raw: &Value, version: &str) -> AppResult<()> {
    require(keys(
        engine,
        &[
            "package",
            "configContracts",
            "execution",
            "metrics",
            "benchmarks",
            "gate",
            "score",
            "validationRecord",
            "benchmarkRecord",
        ],
    ))?;
    for (k, v) in [
        ("package", "0.1.0"),
        ("execution", "backtest-execution-v1"),
        ("metrics", "metrics-v2"),
        ("benchmarks", "benchmark-suite-v1"),
        ("gate", "gate-v1"),
        ("score", "score-v1"),
        ("validationRecord", "validation-record-v2"),
        ("benchmarkRecord", "bench-record-v1"),
    ] {
        require(text(engine, k)? == v)?;
    }
    require(
        contracts(&engine["configContracts"], version)
            && contracts(&raw["contracts"], version)
            && engine["configContracts"] == raw["contracts"],
    )
}

// Emit the original producer's pieces directly into borrowed saved text.
struct Compare<'a>(&'a str);
impl fmt::Write for Compare<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0 = self.0.strip_prefix(s).ok_or(fmt::Error)?;
        Ok(())
    }
}
fn mechanism(actual: &str, base: &DiscoveryBase) -> bool {
    let mut out = Compare(actual);
    let result = (|| -> fmt::Result {
        if let Some(dsl) = base
            .strategy
            .get("dsl")
            .filter(|_| base.strategy.get("mode").and_then(Value::as_str) == Some("dsl"))
        {
            write!(&mut out,"{} base {}: fixed executable DSL \"{}\"; entry/exit expression trees are frozen before queue admission",base.preset_version,base.id,dsl.get("name").and_then(Value::as_str).unwrap_or("unnamed"))?;
        } else {
            let signal = |k| base.strategy.get(k).and_then(Value::as_str).unwrap_or("?");
            write!(&mut out,"preset {} base {}: entry on {}, exit on {}; the parameter sweep asks whether the mechanism survives neighbouring ",base.preset_version,base.id,signal("entrySig"),signal("exitSig"))?;
            if base.axes.is_empty() {
                out.write_str("parameters (none varied)")?;
            }
            for (i, a) in base.axes.iter().enumerate() {
                if i > 0 {
                    out.write_str(", ")?;
                }
                out.write_str(a.key.as_str())?;
            }
        }
        Ok(())
    })();
    result.is_ok() && out.0.is_empty()
}
fn variation(actual: &str, base: &DiscoveryBase) -> bool {
    if base.strategy.get("mode").and_then(Value::as_str) == Some("dsl") {
        return actual == "dsl-fixed";
    }
    if base.axes.is_empty() {
        return actual == "param-sweep:none";
    }
    let mut out = Compare(actual);
    let result = (|| -> fmt::Result {
        out.write_str("param-sweep:")?;
        for (i, a) in base.axes.iter().enumerate() {
            if i > 0 {
                out.write_str(",")?;
            }
            out.write_str(a.key.as_str())?;
        }
        Ok(())
    })();
    result.is_ok() && out.0.is_empty()
}
struct Reference {
    strategy: usize,
    hypothesis: usize,
    base: usize,
}
struct Context<'p, 'g> {
    p: &'p PreparedOriginalLineage<'g>,
    references: Vec<Reference>,
    winners: Vec<(usize, usize)>,
}
impl<'p, 'g> Context<'p, 'g> {
    fn new(p: &'p PreparedOriginalLineage<'g>) -> AppResult<Self> {
        let rows = p.rows();
        let cfg = p.config();
        let plan = p.plan();
        let params_only = match cfg.envelope_version.as_str() {
            "discovery-config-v1" => true,
            "discovery-config-v2" | "discovery-config-v3" => false,
            _ => return Err(invalid()),
        };
        require(cfg.bases.iter().all(|b| {
            b.strategy.get("mode").and_then(Value::as_str) == Some("params")
                || (!params_only && b.strategy.get("mode").and_then(Value::as_str) == Some("dsl"))
        }))?;
        let n = plan.candidates.len();
        require(
            n > 0
                && n <= 4096
                && rows.attempts.len() == n
                && rows.strategies.len() == n
                && p.strategies().len() == n
                && p.attempts().len() == n
                && p.hypotheses().len() == rows.hypotheses.len(),
        )?;
        let fee = f64::from_bits(rows.roots.admission.fee_pct_bits);
        let slip = f64::from_bits(rows.roots.admission.slip_pct_bits);
        require(
            fee.is_finite()
                && fee >= 0.0
                && slip.is_finite()
                && slip >= 0.0
                && fee.to_bits() == cfg.benchmark_costs.fee_pct.to_bits()
                && slip.to_bits() == cfg.benchmark_costs.slip_pct.to_bits(),
        )?;
        let strategies: BTreeMap<_, _> = rows
            .strategies
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id, i))
            .collect();
        let hypotheses: BTreeMap<_, _> = rows
            .hypotheses
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id, i))
            .collect();
        let bases: BTreeMap<_, _> = cfg
            .bases
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id.as_str(), i))
            .collect();
        require(
            strategies.len() == n
                && hypotheses.len() == rows.hypotheses.len()
                && bases.len() == cfg.bases.len(),
        )?;
        let mut references = Vec::with_capacity(n);
        let mut winners = BTreeMap::new();
        let mut used_strategies = BTreeSet::new();
        for (i, candidate) in plan.candidates.iter().enumerate() {
            let a = &rows.attempts[i];
            let si = *strategies.get(&a.strategy_id).ok_or_else(invalid)?;
            let hi = *hypotheses.get(&a.hypothesis_id).ok_or_else(invalid)?;
            let bi = *bases.get(candidate.base_id.as_str()).ok_or_else(invalid)?;
            let mode = text(&cfg.bases[bi].strategy, "mode")?;
            let s = &rows.strategies[si];
            let docs = &p.strategies()[si];
            let d = docs.definition();
            require(
                used_strategies.insert(si)
                    && candidate.index == i64::try_from(i).map_err(|_| invalid())?
                    && a.candidate_index == Some(candidate.index)
                    && s.strategy_hash == candidate.strategy_hash
                    && s.strategy_hash.len() == 76
                    && s.strategy_hash.starts_with("strategy-v2:")
                    && s.parent_strategy_id.is_none()
                    && s.r#type == mode
                    && text(d, "mode")? == mode
                    && d.get("definitionVersion").is_none()
                    && d.get("numericPolicy").is_none(),
            )?;
            require(
                cost(d, "feePct")?.to_bits() == fee.to_bits()
                    && cost(d, "slipPct")?.to_bits() == slip.to_bits(),
            )?;
            require(if mode == "params" {
                docs.dsl().is_none()
            } else {
                d.get("dsl").and_then(Value::as_object).is_some()
                    && docs.dsl().and_then(Value::as_object).is_some()
            })?;
            check_engine(
                p.attempts()[i].engine(),
                p.raw_config(),
                &cfg.envelope_version,
            )?;
            check_input(p, i)?;
            if let Some(previous) = winners.insert(bi, hi) {
                require(previous == hi)?;
            }
            references.push(Reference {
                strategy: si,
                hypothesis: hi,
                base: bi,
            });
        }
        let used: BTreeSet<_> = winners.values().copied().collect();
        require(used.len() == winners.len() && used.len() == rows.hypotheses.len())?;
        let mut hashes = BTreeSet::new();
        for (bi, hi) in &winners {
            let row = &rows.hypotheses[*hi];
            require(hashes.insert(row.hypothesis_hash.as_str()) && hash(&row.hypothesis_hash))?;
            let base = &cfg.bases[*bi];
            check_hypothesis(p, *hi, base, text(&base.strategy, "mode")?)?;
        }
        Ok(Self {
            p,
            references,
            winners: winners.into_iter().collect(),
        })
    }
}
fn check_input(p: &PreparedOriginalLineage<'_>, i: usize) -> AppResult<()> {
    let c = &p.plan().candidates[i];
    let a = &p.rows().attempts[i];
    let v = p.attempts()[i].input();
    let cfg = p.config();
    require(keys(
        v,
        &[
            "envelopeVersion",
            "configHash",
            "rootSeed",
            "datasetId",
            "datasetHash",
            "interval",
            "strategyId",
            "strategyHash",
            "baseId",
            "candidateIndex",
            "appliedAxes",
            "seeds",
        ],
    ))?;
    require(
        text(v, "envelopeVersion")? == cfg.envelope_version
            && hash(text(v, "configHash")?)
            && v["rootSeed"].as_u64() == Some(u64::from(cfg.root_seed))
            && v["datasetId"].as_i64() == Some(cfg.dataset.id)
            && text(v, "datasetHash")? == cfg.dataset.content_hash
            && text(v, "interval")? == p.rows().dataset.interval
            && v["strategyId"].as_i64() == Some(a.strategy_id)
            && text(v, "strategyHash")? == c.strategy_hash
            && text(v, "baseId")? == c.base_id
            && v["candidateIndex"].as_i64() == Some(c.index),
    )?;
    require(v["appliedAxes"].as_object().is_some_and(|o| {
        o.len() == c.applied_axes.len()
            && c.applied_axes.iter().all(|(k, n)| {
                o.get(k)
                    .and_then(Value::as_f64)
                    .is_some_and(|v| v.to_bits() == n.to_bits())
            })
    }))?;
    require(
        keys(&v["seeds"], &["randomEntry"])
            && v["seeds"]["randomEntry"].as_u64() == Some(u64::from(c.seeds.random_entry)),
    )
}
fn check_hypothesis(
    p: &PreparedOriginalLineage<'_>,
    i: usize,
    base: &DiscoveryBase,
    mode: &str,
) -> AppResult<()> {
    let row = &p.rows().hypotheses[i];
    let docs = &p.hypotheses()[i];
    let v = docs.content();
    let a = docs.applicability();
    let cfg = p.config();
    require(
        keys(
            v,
            &[
                "version",
                "source",
                "mechanism",
                "applicability",
                "failureModes",
                "strategyHash",
                "strategyId",
                "parentStrategyId",
                "variationKind",
            ],
        ) && keys(
            a,
            &[
                "datasetId",
                "datasetHash",
                "interval",
                "split",
                "embargo",
                "axes",
            ],
        ),
    )?;
    require(
        row.version == "hypothesis-v1"
            && text(v, "version")? == row.version
            && row.source == "discovery"
            && text(v, "source")? == row.source
            && row.strategy_id.is_none()
            && row.parent_strategy_id.is_none()
            && v["strategyId"].is_null()
            && v["parentStrategyId"].is_null()
            && v["applicability"] == *a,
    )?;
    require(
        text(v, "mechanism")? == row.mechanism
            && mechanism(&row.mechanism, base)
            && text(v, "failureModes")? == row.failure_modes
            && row.failure_modes
                == if mode == "dsl" {
                    DSL_FAILURE
                } else {
                    PARAM_FAILURE
                }
            && text(v, "strategyHash")? == row.strategy_hash
            && row
                .strategy_hash
                .strip_prefix("strategy-doc-v1:")
                .is_some_and(hash),
    )?;
    let kind = row.variation_kind.as_deref().ok_or_else(invalid)?;
    require(text(v, "variationKind")? == kind && variation(kind, base))?;
    require(
        a["datasetId"].as_i64() == Some(cfg.dataset.id)
            && text(a, "datasetHash")? == cfg.dataset.content_hash
            && text(a, "interval")? == p.rows().dataset.interval
            && text(a, "split")? == cfg.contracts.split
            && text(a, "embargo")? == cfg.contracts.embargo
            && a["axes"].as_array().is_some_and(|axes| {
                axes.len() == base.axes.len()
                    && axes
                        .iter()
                        .zip(&base.axes)
                        .all(|(v, axis)| v.as_str() == Some(axis.key.as_str()))
            }),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Work {
    hash: usize,
    copy: usize,
    nodes: usize,
    output: usize,
}
fn add(a: usize, b: usize) -> AppResult<usize> {
    a.checked_add(b).ok_or_else(invalid)
}
fn twice(n: usize) -> AppResult<usize> {
    n.checked_mul(2).ok_or_else(invalid)
}
fn base_document(base: &DiscoveryBase) -> &Value {
    if base.strategy.get("mode").and_then(Value::as_str) == Some("dsl") {
        &base.strategy["dsl"]
    } else {
        &base.strategy
    }
}
impl Context<'_, '_> {
    fn reserve(&self) -> AppResult<Work> {
        let p = self.p;
        let mut w = Work {
            hash: 0,
            copy: 0,
            nodes: 0,
            output: 64,
        };
        for c in &p.plan().candidates {
            w.output = add(w.output, add(140, c.base_id.len())?)?;
        }
        require(w.output <= MAX_OUTPUT_BYTES)?;
        let j = measure::json(p.raw_config())?;
        w.hash = j;
        w.copy = twice(j)?;
        for (row, docs) in p.rows().strategies.iter().zip(p.strategies()) {
            // Existing two-key identity payload adds 80 encoded bytes to D.
            let b = add(80, measure::binary(docs.definition())?)?;
            w.hash = add(w.hash, add(12, b)?)?;
            w.copy = add(
                w.copy,
                add(add(measure::json(docs.definition())?, b)?, 176)?,
            )?;
            w.nodes = add(w.nodes, add(row.original_definition_json.node_count(), 4)?)?;
            if let Some(saved) = docs.dsl() {
                w.copy = add(
                    w.copy,
                    add(
                        measure::binary(&docs.definition()["dsl"])?,
                        measure::binary(saved)?,
                    )?,
                )?;
            }
        }
        for (base, _) in &self.winners {
            let j = measure::json(base_document(&p.config().bases[*base]))?;
            w.hash = add(w.hash, j)?;
            w.copy = add(w.copy, add(twice(j)?, 64)?)?;
        }
        for docs in p.hypotheses() {
            let j = measure::json(docs.content())?;
            w.hash = add(w.hash, j)?;
            w.copy = add(w.copy, add(twice(j)?, 64)?)?;
        }
        w.copy = add(w.copy, w.output)?;
        require(
            w.hash <= MAX_HASH_BYTES && w.copy <= MAX_COPY_BYTES && w.nodes <= p.combined_work().1,
        )?;
        Ok(w)
    }
    fn output(&self, config_hash: String) -> OriginalLineageObservation {
        enter_payload();
        let candidates = self
            .references
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let s = &self.p.rows().strategies[r.strategy];
                let h = &self.p.rows().hypotheses[r.hypothesis];
                CandidateLineageObservation {
                    index: self.p.plan().candidates[i].index,
                    strategy_id: s.id,
                    strategy_hash: s.strategy_hash.clone(),
                    hypothesis_id: h.id,
                    hypothesis_hash: h.hypothesis_hash.clone(),
                    base_id: self.p.config().bases[r.base].id.clone(),
                }
            })
            .collect();
        OriginalLineageObservation {
            config_hash,
            candidates,
        }
    }
}
fn document_hash(v: &Value) -> AppResult<String> {
    enter_payload();
    Ok(sha256_hex(&canonical_json(v).map_err(|_| invalid())?))
}
pub(super) fn observe(p: &PreparedOriginalLineage<'_>) -> AppResult<OriginalLineageObservation> {
    reserve(p)?.verify().map(VerifiedLineage::finish)
}

/// Fixed stages share one borrowed context; no payload work happens at reserve.
pub(super) struct ReservedLineage<'p, 'g> {
    ctx: Context<'p, 'g>,
    work: Work,
}
pub(super) struct VerifiedLineage<'p, 'g> {
    ctx: Context<'p, 'g>,
    config_hash: String,
}
pub(super) fn reserve<'p, 'g>(
    p: &'p PreparedOriginalLineage<'g>,
) -> AppResult<ReservedLineage<'p, 'g>> {
    let ctx = Context::new(p)?;
    let work = ctx.reserve()?;
    Ok(ReservedLineage { ctx, work })
}
impl<'p, 'g> ReservedLineage<'p, 'g> {
    pub(super) fn work(&self) -> (usize, usize, usize, usize) {
        let w = self.work;
        (w.hash, w.copy, w.nodes, w.output)
    }
    pub(super) fn definitions(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.ctx
            .references
            .iter()
            .map(|r| self.ctx.p.strategies()[r.strategy].definition())
    }
    pub(super) fn verify(self) -> AppResult<VerifiedLineage<'p, 'g>> {
        let config_hash = verify(&self.ctx)?;
        Ok(VerifiedLineage {
            ctx: self.ctx,
            config_hash,
        })
    }
}
impl VerifiedLineage<'_, '_> {
    pub(super) fn definitions(&self) -> impl ExactSizeIterator<Item = &Value> {
        self.ctx
            .references
            .iter()
            .map(|r| self.ctx.p.strategies()[r.strategy].definition())
    }
    pub(super) fn finish(self) -> OriginalLineageObservation {
        self.ctx.output(self.config_hash)
    }
}
fn verify(ctx: &Context<'_, '_>) -> AppResult<String> {
    let p = ctx.p;
    let config_hash = document_hash(p.raw_config())?;
    for (i, r) in ctx.references.iter().enumerate() {
        let docs = &p.strategies()[r.strategy];
        let d = docs.definition();
        enter_payload();
        let actual = identity::strategy_hash(d, cost(d, "feePct")?, cost(d, "slipPct")?)
            .map_err(|_| invalid())?;
        require(
            actual == p.rows().strategies[r.strategy].strategy_hash
                && actual == p.plan().candidates[i].strategy_hash,
        )?;
        if let Some(saved) = docs.dsl() {
            enter_payload();
            let nested = identity::canonical_bytes(&d["dsl"]).map_err(|_| invalid())?;
            enter_payload();
            let saved = identity::canonical_bytes(saved).map_err(|_| invalid())?;
            require(nested == saved)?;
        }
        require(text(p.attempts()[i].input(), "configHash")? == config_hash)?;
    }
    for (bi, hi) in &ctx.winners {
        let actual = document_hash(base_document(&p.config().bases[*bi]))?;
        require(
            p.rows().hypotheses[*hi]
                .strategy_hash
                .strip_prefix("strategy-doc-v1:")
                == Some(actual.as_str()),
        )?;
    }
    for (row, docs) in p.rows().hypotheses.iter().zip(p.hypotheses()) {
        require(document_hash(docs.content())? == row.hypothesis_hash)?;
    }
    Ok(config_hash)
}
#[cfg(test)]
pub(super) fn planned_work(
    p: &PreparedOriginalLineage<'_>,
) -> AppResult<(usize, usize, usize, usize)> {
    let w = Context::new(p)?.reserve()?;
    Ok((w.hash, w.copy, w.nodes, w.output))
}
