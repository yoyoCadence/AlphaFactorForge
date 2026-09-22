//! `strategy-dsl-v1`: strict, data-only validation and deterministic signal
//! evaluation. This is the Rust half of `src/core/strategy-dsl/*`.
//!
//! Only the single-series intersection of both indicator cores is executable.
//! Validation is mandatory before evaluation and the discovery config parser
//! calls it before a candidate can enter the runner.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;
use serde_json::{Map, Value};

use super::backtest::Signals;
use super::indicators::{atr, ema, highest, lowest, roc, rsi, sma, stddev, wma};
use super::types::Candle;

pub const STRATEGY_DSL_VERSION: &str = "strategy-dsl-v1";
pub const DSL_MAX_DEPTH: usize = 8;
pub const DSL_MAX_NODES: usize = 64;
pub const DSL_MIN_LOOKBACK: i64 = 2;
pub const DSL_MAX_LOOKBACK: i64 = 400;
pub const DSL_MAX_CONST_ABS: f64 = 1_000_000_000.0;

const ROOT_KEYS: [&str; 5] = ["version", "name", "params", "entry", "exit"];
const PARAM_KEYS: [&str; 4] = ["type", "min", "max", "default"];
const INDICATORS: [&str; 14] = [
    "EMA", "SMA", "WMA", "RSI", "ROC", "ATR", "STDDEV", "HIGHEST", "LOWEST", "CLOSE", "OPEN",
    "HIGH", "LOW", "HLC3",
];
const OPERATORS: [&str; 21] = [
    "ADD",
    "SUB",
    "MUL",
    "DIV",
    "ABS",
    "MIN",
    "MAX",
    "CLAMP",
    "GT",
    "LT",
    "GTE",
    "LTE",
    "CROSS_UP",
    "CROSS_DOWN",
    "AND",
    "OR",
    "NOT",
    "SHIFT",
    "RISING",
    "FALLING",
    "CONST",
];
const SOURCES: [&str; 6] = ["CLOSE", "OPEN", "HIGH", "LOW", "HLC3", "VOLUME"];
const SUSPICIOUS: [&str; 26] = [
    "eval",
    "function",
    "new function",
    "import",
    "require",
    "fetch",
    "xmlhttp",
    "process",
    "child_process",
    "fs.",
    "readfile",
    "writefile",
    "localstorage",
    "sessionstorage",
    "document",
    "window",
    "globalthis",
    "=>",
    "while",
    "for(",
    "constructor",
    "__proto__",
    "prototype",
    "settimeout",
    "setinterval",
    "`",
];

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DslValidationReport {
    pub ok: bool,
    pub errors: Vec<String>,
    pub node_count: usize,
    pub depth: usize,
    pub max_lookback_bars: i64,
    pub parameter_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DslError(pub String);

impl fmt::Display for DslError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for DslError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpressionType {
    Number,
    Boolean,
}

impl ExpressionType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }
}

#[derive(Clone, Copy)]
struct Analysis {
    kind: ExpressionType,
    lookback: i64,
}

struct Validator {
    errors: Vec<String>,
    params: BTreeMap<String, f64>,
    node_count: usize,
    depth: usize,
    max_lookback: i64,
}

fn object<'a>(
    value: &'a Value,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<&'a Map<String, Value>> {
    match value.as_object() {
        Some(object) => Some(object),
        None => {
            errors.push(format!("{path}: node must be an object"));
            None
        }
    }
}

fn exact_fields(
    object: &Map<String, Value>,
    fields: &[&str],
    path: &str,
    errors: &mut Vec<String>,
) {
    for key in object.keys() {
        if !fields.contains(&key.as_str()) {
            errors.push(format!("{path}: unknown field \"{key}\""));
        }
    }
    for field in fields {
        if !object.contains_key(*field) {
            errors.push(format!("{path}: missing field \"{field}\""));
        }
    }
}

fn valid_param_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or_default();
    (first.is_ascii_alphabetic())
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

impl Validator {
    fn scalar(&mut self, value: Option<&Value>, path: &str, period: bool) -> Option<f64> {
        let resolved = match value {
            Some(Value::Number(value)) => value.as_f64(),
            Some(Value::String(reference)) if reference.starts_with('$') => {
                match self.params.get(&reference[1..]).copied() {
                    Some(value) => Some(value),
                    None => {
                        self.errors
                            .push(format!("{path}: unknown param {reference}"));
                        None
                    }
                }
            }
            Some(Value::String(_)) => {
                self.errors
                    .push(format!("{path}: string must be a $param ref"));
                None
            }
            _ => {
                self.errors
                    .push(format!("{path}: must be a number or $param ref"));
                None
            }
        };
        let value = resolved?;
        if !value.is_finite() {
            self.errors.push(format!("{path}: value must be finite"));
            return None;
        }
        if period {
            if value.fract() != 0.0
                || value < DSL_MIN_LOOKBACK as f64
                || value > DSL_MAX_LOOKBACK as f64
            {
                self.errors.push(format!(
                    "{path}: lookback must be int in [{DSL_MIN_LOOKBACK}, {DSL_MAX_LOOKBACK}]"
                ));
                return None;
            }
        } else if value.abs() > DSL_MAX_CONST_ABS {
            self.errors.push(format!(
                "{path}: value must have |v| <= {}",
                DSL_MAX_CONST_ABS as i64
            ));
            return None;
        }
        Some(value)
    }

    fn require_type(
        &mut self,
        children: &[Option<Analysis>],
        expected: ExpressionType,
        path: &str,
    ) {
        for (index, child) in children.iter().enumerate() {
            if let Some(child) = child {
                if child.kind != expected {
                    self.errors.push(format!(
                        "{path}.args[{index}]: expected {}, received {}",
                        expected.as_str(),
                        child.kind.as_str()
                    ));
                }
            }
        }
    }

    fn walk(&mut self, node: &Value, depth: usize, path: &str) -> Option<Analysis> {
        self.node_count += 1;
        self.depth = self.depth.max(depth);
        if depth > DSL_MAX_DEPTH {
            self.errors
                .push(format!("{path}: exceeds max depth {DSL_MAX_DEPTH}"));
            return None;
        }
        if self.node_count > DSL_MAX_NODES {
            self.errors
                .push(format!("exceeds max node count {DSL_MAX_NODES}"));
            return None;
        }
        let object = object(node, path, &mut self.errors)?;
        let has_ind = object.contains_key("ind");
        let has_op = object.contains_key("op");
        if has_ind == has_op {
            self.errors.push(format!(
                "{path}: node must have exactly one of \"ind\" | \"op\""
            ));
            return None;
        }

        if has_ind {
            let indicator = object.get("ind").and_then(Value::as_str).unwrap_or("");
            if !INDICATORS.contains(&indicator) {
                self.errors.push(format!(
                    "{path}: indicator \"{indicator}\" not executable in {STRATEGY_DSL_VERSION}"
                ));
                return None;
            }
            if ["CLOSE", "OPEN", "HIGH", "LOW", "HLC3"].contains(&indicator) {
                exact_fields(object, &["ind"], path, &mut self.errors);
                self.max_lookback = self.max_lookback.max(1);
                return Some(Analysis {
                    kind: ExpressionType::Number,
                    lookback: 1,
                });
            }
            if indicator == "ATR" {
                exact_fields(object, &["ind", "len"], path, &mut self.errors);
            } else {
                exact_fields(object, &["ind", "src", "len"], path, &mut self.errors);
                let source = object.get("src").and_then(Value::as_str).unwrap_or("");
                if !SOURCES.contains(&source) {
                    self.errors
                        .push(format!("{path}.src: unsupported price source \"{source}\""));
                }
            }
            let period = self
                .scalar(object.get("len"), &format!("{path}.len"), true)
                .map(|value| value as i64)
                .unwrap_or(1);
            let lookback = period + i64::from(indicator == "RSI" || indicator == "ROC");
            self.max_lookback = self.max_lookback.max(lookback);
            return Some(Analysis {
                kind: ExpressionType::Number,
                lookback,
            });
        }

        let operator = object.get("op").and_then(Value::as_str).unwrap_or("");
        if !OPERATORS.contains(&operator) {
            self.errors
                .push(format!("{path}: operator \"{operator}\" not in whitelist"));
            return None;
        }
        if operator == "CONST" {
            exact_fields(object, &["op", "v"], path, &mut self.errors);
            self.scalar(object.get("v"), &format!("{path}.v"), false);
            self.max_lookback = self.max_lookback.max(1);
            return Some(Analysis {
                kind: ExpressionType::Number,
                lookback: 1,
            });
        }

        let temporal = ["SHIFT", "RISING", "FALLING"].contains(&operator);
        exact_fields(
            object,
            if temporal {
                &["op", "args", "n"]
            } else {
                &["op", "args"]
            },
            path,
            &mut self.errors,
        );
        let expected_arity = if ["ABS", "NOT"].contains(&operator) || temporal {
            1
        } else if operator == "CLAMP" {
            3
        } else {
            2
        };
        let args = match object.get("args").and_then(Value::as_array) {
            Some(args) if args.len() == expected_arity => args,
            _ => {
                self.errors.push(format!(
                    "{path}: operator \"{operator}\" requires exactly {expected_arity} args"
                ));
                return None;
            }
        };
        let children: Vec<Option<Analysis>> = args
            .iter()
            .enumerate()
            .map(|(index, child)| self.walk(child, depth + 1, &format!("{path}.args[{index}]")))
            .collect();
        let lookback = children
            .iter()
            .filter_map(|child| child.map(|analysis| analysis.lookback))
            .max()
            .unwrap_or(1);

        let analysis = if ["AND", "OR", "NOT"].contains(&operator) {
            self.require_type(&children, ExpressionType::Boolean, path);
            Analysis {
                kind: ExpressionType::Boolean,
                lookback,
            }
        } else if ["GT", "LT", "GTE", "LTE"].contains(&operator) {
            self.require_type(&children, ExpressionType::Number, path);
            Analysis {
                kind: ExpressionType::Boolean,
                lookback,
            }
        } else if ["CROSS_UP", "CROSS_DOWN"].contains(&operator) {
            self.require_type(&children, ExpressionType::Number, path);
            Analysis {
                kind: ExpressionType::Boolean,
                lookback: lookback + 1,
            }
        } else if operator == "SHIFT" {
            let offset = self
                .scalar(object.get("n"), &format!("{path}.n"), true)
                .unwrap_or(0.0) as i64;
            Analysis {
                kind: children
                    .first()
                    .and_then(|child| *child)
                    .map(|child| child.kind)
                    .unwrap_or(ExpressionType::Number),
                lookback: lookback + offset,
            }
        } else if ["RISING", "FALLING"].contains(&operator) {
            self.require_type(&children, ExpressionType::Number, path);
            let periods = self
                .scalar(object.get("n"), &format!("{path}.n"), true)
                .unwrap_or(0.0) as i64;
            Analysis {
                kind: ExpressionType::Boolean,
                lookback: lookback + periods,
            }
        } else {
            self.require_type(&children, ExpressionType::Number, path);
            Analysis {
                kind: ExpressionType::Number,
                lookback,
            }
        };
        self.max_lookback = self.max_lookback.max(analysis.lookback);
        Some(analysis)
    }
}

pub fn validate_strategy_dsl(input: &Value) -> DslValidationReport {
    let mut validator = Validator {
        errors: Vec::new(),
        params: BTreeMap::new(),
        node_count: 0,
        depth: 0,
        max_lookback: 0,
    };
    let encoded = input.to_string().to_lowercase();
    for token in SUSPICIOUS {
        if encoded.contains(token) {
            validator
                .errors
                .push(format!("suspicious token in payload: \"{token}\""));
        }
    }

    let root = match input.as_object() {
        Some(root) => root,
        None => {
            validator.errors.push("DSL must be an object".into());
            return DslValidationReport {
                ok: false,
                errors: validator.errors,
                node_count: 0,
                depth: 0,
                max_lookback_bars: 0,
                parameter_count: 0,
            };
        }
    };
    exact_fields(root, &ROOT_KEYS, "DSL", &mut validator.errors);
    if root.get("version").and_then(Value::as_str) != Some(STRATEGY_DSL_VERSION) {
        validator
            .errors
            .push(format!("DSL.version must be \"{STRATEGY_DSL_VERSION}\""));
    }
    if root
        .get("name")
        .and_then(Value::as_str)
        .is_none_or(|name| name.trim().is_empty())
    {
        validator.errors.push("missing name".into());
    }
    match root.get("params").and_then(Value::as_object) {
        None => validator.errors.push("missing params object".into()),
        Some(params) => {
            for (name, raw_spec) in params {
                if !valid_param_name(name) {
                    validator.errors.push(format!("param {name}: invalid name"));
                }
                if let Some(value) = raw_spec.as_f64() {
                    validator.params.insert(name.clone(), value);
                    continue;
                }
                let spec = match raw_spec.as_object() {
                    Some(spec) => spec,
                    None => {
                        validator
                            .errors
                            .push(format!("param {name}: type must be int|float"));
                        continue;
                    }
                };
                exact_fields(
                    spec,
                    &PARAM_KEYS,
                    &format!("param {name}"),
                    &mut validator.errors,
                );
                let kind = spec.get("type").and_then(Value::as_str).unwrap_or("");
                if kind != "int" && kind != "float" {
                    validator
                        .errors
                        .push(format!("param {name}: type must be int|float"));
                    continue;
                }
                let min = spec.get("min").and_then(Value::as_f64);
                let max = spec.get("max").and_then(Value::as_f64);
                let default = spec.get("default").and_then(Value::as_f64);
                if min.zip(max).is_none_or(|(min, max)| min > max) {
                    validator
                        .errors
                        .push(format!("param {name}: invalid min/max"));
                }
                if default
                    .zip(min)
                    .zip(max)
                    .is_none_or(|((value, min), max)| value < min || value > max)
                {
                    validator
                        .errors
                        .push(format!("param {name}: default out of range"));
                }
                if kind == "int"
                    && [min, max, default].iter().any(|value| {
                        value.is_none_or(|value| {
                            value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0
                        })
                    })
                {
                    validator.errors.push(format!(
                        "param {name}: int bounds/default must be safe integers"
                    ));
                }
                if let Some(default) = default {
                    validator.params.insert(name.clone(), default);
                }
            }
        }
    }

    let entry = validator.walk(root.get("entry").unwrap_or(&Value::Null), 1, "entry");
    let exit = validator.walk(root.get("exit").unwrap_or(&Value::Null), 1, "exit");
    if entry.is_some_and(|analysis| analysis.kind != ExpressionType::Boolean) {
        validator
            .errors
            .push("entry: root must return boolean".into());
    }
    if exit.is_some_and(|analysis| analysis.kind != ExpressionType::Boolean) {
        validator
            .errors
            .push("exit: root must return boolean".into());
    }
    DslValidationReport {
        ok: validator.errors.is_empty(),
        errors: validator.errors,
        node_count: validator.node_count,
        depth: validator.depth,
        max_lookback_bars: validator.max_lookback,
        parameter_count: validator.params.len(),
    }
}

enum Evaluated {
    Number(Vec<f64>),
    Boolean(Vec<bool>),
}

fn numeric(value: Evaluated) -> Result<Vec<f64>, DslError> {
    match value {
        Evaluated::Number(values) => Ok(values),
        Evaluated::Boolean(_) => Err(DslError("validated DSL type invariant failed".into())),
    }
}

fn boolean(value: Evaluated) -> Result<Vec<bool>, DslError> {
    match value {
        Evaluated::Boolean(values) => Ok(values),
        Evaluated::Number(_) => Err(DslError("validated DSL type invariant failed".into())),
    }
}

fn source(candles: &[Candle], name: &str) -> Vec<f64> {
    candles
        .iter()
        .map(|candle| match name {
            "OPEN" => candle.open,
            "HIGH" => candle.high,
            "LOW" => candle.low,
            "VOLUME" => candle.volume,
            "HLC3" => (candle.high + candle.low + candle.close) / 3.0,
            _ => candle.close,
        })
        .collect()
}

fn resolved_params(dsl: &Value) -> BTreeMap<String, f64> {
    dsl.get("params")
        .and_then(Value::as_object)
        .map(|params| {
            params
                .iter()
                .filter_map(|(name, spec)| {
                    spec.as_f64()
                        .or_else(|| spec.get("default").and_then(Value::as_f64))
                        .map(|value| (name.clone(), value))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn scalar(value: Option<&Value>, params: &BTreeMap<String, f64>) -> Result<f64, DslError> {
    match value {
        Some(Value::Number(number)) => number
            .as_f64()
            .ok_or_else(|| DslError("validated DSL numeric invariant failed".into())),
        Some(Value::String(reference)) => params
            .get(&reference[1..])
            .copied()
            .ok_or_else(|| DslError("validated DSL parameter invariant failed".into())),
        _ => Err(DslError("validated DSL scalar invariant failed".into())),
    }
}

fn evaluate(
    node: &Value,
    candles: &[Candle],
    params: &BTreeMap<String, f64>,
) -> Result<Evaluated, DslError> {
    let object = node
        .as_object()
        .ok_or_else(|| DslError("validated DSL node invariant failed".into()))?;
    if let Some(indicator) = object.get("ind").and_then(Value::as_str) {
        if ["CLOSE", "OPEN", "HIGH", "LOW", "HLC3"].contains(&indicator) {
            return Ok(Evaluated::Number(source(candles, indicator)));
        }
        let period = scalar(object.get("len"), params)? as usize;
        if indicator == "ATR" {
            return atr(
                &source(candles, "HIGH"),
                &source(candles, "LOW"),
                &source(candles, "CLOSE"),
                period,
            )
            .map(Evaluated::Number)
            .map_err(|error| DslError(error.to_string()));
        }
        let input = source(
            candles,
            object.get("src").and_then(Value::as_str).unwrap_or("CLOSE"),
        );
        let output = match indicator {
            "EMA" => ema(&input, period),
            "SMA" => sma(&input, period),
            "WMA" => wma(&input, period),
            "RSI" => rsi(&input, period),
            "ROC" => roc(&input, period),
            "STDDEV" => stddev(&input, period),
            "HIGHEST" => highest(&input, period),
            _ => lowest(&input, period),
        };
        return Ok(Evaluated::Number(output));
    }

    let operator = object.get("op").and_then(Value::as_str).unwrap_or("");
    if operator == "CONST" {
        return Ok(Evaluated::Number(vec![
            scalar(object.get("v"), params)?;
            candles.len()
        ]));
    }
    let args = object
        .get("args")
        .and_then(Value::as_array)
        .ok_or_else(|| DslError("validated DSL args invariant failed".into()))?;
    let mut children = Vec::with_capacity(args.len());
    for child in args {
        children.push(evaluate(child, candles, params)?);
    }
    if operator == "SHIFT" {
        let offset = scalar(object.get("n"), params)? as usize;
        return match children.remove(0) {
            Evaluated::Number(input) => Ok(Evaluated::Number(
                (0..input.len())
                    .map(|index| {
                        index
                            .checked_sub(offset)
                            .map(|source| input[source])
                            .unwrap_or(f64::NAN)
                    })
                    .collect(),
            )),
            Evaluated::Boolean(input) => Ok(Evaluated::Boolean(
                (0..input.len())
                    .map(|index| {
                        index
                            .checked_sub(offset)
                            .is_some_and(|source| input[source])
                    })
                    .collect(),
            )),
        };
    }
    if operator == "RISING" || operator == "FALLING" {
        let input = numeric(children.remove(0))?;
        let periods = scalar(object.get("n"), params)? as usize;
        let output = (0..input.len())
            .map(|index| {
                if index < periods {
                    return false;
                }
                (0..periods).all(|offset| {
                    let current = input[index - offset];
                    let previous = input[index - offset - 1];
                    current.is_finite()
                        && previous.is_finite()
                        && if operator == "RISING" {
                            current > previous
                        } else {
                            current < previous
                        }
                })
            })
            .collect();
        return Ok(Evaluated::Boolean(output));
    }
    if operator == "AND" || operator == "OR" {
        let right = boolean(children.pop().unwrap())?;
        let left = boolean(children.pop().unwrap())?;
        return Ok(Evaluated::Boolean(
            left.into_iter()
                .zip(right)
                .map(|(left, right)| {
                    if operator == "AND" {
                        left && right
                    } else {
                        left || right
                    }
                })
                .collect(),
        ));
    }
    if operator == "NOT" {
        return Ok(Evaluated::Boolean(
            boolean(children.remove(0))?
                .into_iter()
                .map(|value| !value)
                .collect(),
        ));
    }
    let third = if children.len() == 3 {
        Some(numeric(children.pop().unwrap())?)
    } else {
        None
    };
    let right = if children.len() == 2 {
        Some(numeric(children.pop().unwrap())?)
    } else {
        None
    };
    let left = numeric(children.pop().unwrap())?;
    if ["GT", "LT", "GTE", "LTE", "CROSS_UP", "CROSS_DOWN"].contains(&operator) {
        let right = right.unwrap();
        return Ok(Evaluated::Boolean(
            (0..left.len())
                .map(|index| {
                    let a = left[index];
                    let b = right[index];
                    if !a.is_finite() || !b.is_finite() {
                        return false;
                    }
                    match operator {
                        "GT" => a > b,
                        "LT" => a < b,
                        "GTE" => a >= b,
                        "LTE" => a <= b,
                        "CROSS_UP" => {
                            index > 0
                                && left[index - 1].is_finite()
                                && right[index - 1].is_finite()
                                && left[index - 1] <= right[index - 1]
                                && a > b
                        }
                        _ => {
                            index > 0
                                && left[index - 1].is_finite()
                                && right[index - 1].is_finite()
                                && left[index - 1] >= right[index - 1]
                                && a < b
                        }
                    }
                })
                .collect(),
        ));
    }
    let output = (0..left.len())
        .map(|index| {
            let a = left[index];
            if operator == "ABS" {
                return if a.is_finite() { a.abs() } else { f64::NAN };
            }
            let b = right.as_ref().unwrap()[index];
            if !a.is_finite() || !b.is_finite() {
                return f64::NAN;
            }
            match operator {
                "ADD" => a + b,
                "SUB" => a - b,
                "MUL" => a * b,
                "DIV" => {
                    if b == 0.0 {
                        f64::NAN
                    } else {
                        a / b
                    }
                }
                "MIN" => a.min(b),
                "MAX" => a.max(b),
                _ => {
                    let high = third.as_ref().unwrap()[index];
                    if high.is_finite() {
                        a.max(b).min(high)
                    } else {
                        f64::NAN
                    }
                }
            }
        })
        .collect();
    Ok(Evaluated::Number(output))
}

pub fn build_dsl_signals(candles: &[Candle], dsl: &Value) -> Result<Signals, DslError> {
    let report = validate_strategy_dsl(dsl);
    if !report.ok {
        return Err(DslError(format!(
            "invalid {STRATEGY_DSL_VERSION}: {}",
            report.errors.join("; ")
        )));
    }
    let params = resolved_params(dsl);
    Ok(Signals {
        entry: boolean(evaluate(&dsl["entry"], candles, &params)?)?,
        exit: boolean(evaluate(&dsl["exit"], candles, &params)?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cross_dsl() -> Value {
        serde_json::json!({
            "version": STRATEGY_DSL_VERSION,
            "name": "SMA cross",
            "params": {
                "fast": 2,
                "slow": {"type": "int", "min": 3, "max": 10, "default": 3}
            },
            "entry": {"op": "CROSS_UP", "args": [
                {"ind": "SMA", "src": "CLOSE", "len": "$fast"},
                {"ind": "SMA", "src": "CLOSE", "len": "$slow"}
            ]},
            "exit": {"op": "CROSS_DOWN", "args": [
                {"ind": "SMA", "src": "CLOSE", "len": "$fast"},
                {"ind": "SMA", "src": "CLOSE", "len": "$slow"}
            ]}
        })
    }

    #[test]
    fn validation_and_evaluation_match_the_ts_smoke_case() {
        let dsl = cross_dsl();
        let report = validate_strategy_dsl(&dsl);
        assert!(report.ok, "{:?}", report.errors);
        assert_eq!(report.max_lookback_bars, 4);
        let closes = [1.0, 2.0, 3.0, 2.0, 1.0, 2.0, 3.0, 4.0];
        let candles: Vec<Candle> = closes
            .iter()
            .enumerate()
            .map(|(index, close)| Candle {
                timestamp: index as i64 * 60_000,
                open: *close,
                high: close + 0.5,
                low: close - 0.5,
                close: *close,
                volume: 100.0 + index as f64,
            })
            .collect();
        let signals = build_dsl_signals(&candles, &dsl).unwrap();
        assert_eq!(
            signals.entry,
            [false, false, false, false, false, false, true, false]
        );
        assert_eq!(
            signals.exit,
            [false, false, false, false, true, false, false, false]
        );
    }

    #[test]
    fn illegal_code_and_types_fail_closed() {
        let mut dsl = cross_dsl();
        dsl["name"] = Value::String("eval(fetch('x'))".into());
        assert!(validate_strategy_dsl(&dsl)
            .errors
            .iter()
            .any(|error| error.contains("suspicious")));

        let mut dsl = cross_dsl();
        dsl["entry"] = serde_json::json!({"op": "AND", "args": [
            {"ind": "CLOSE"}, {"op": "CONST", "v": 1}
        ]});
        let report = validate_strategy_dsl(&dsl);
        assert!(!report.ok);
        assert!(build_dsl_signals(&[], &dsl).is_err());
    }

    #[test]
    fn authored_fixture_locks_ts_rust_parity_and_rejection() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/rs-core/strategy-dsl-v1.json"
        ))
        .unwrap();
        assert_eq!(fixture["contractVersion"], STRATEGY_DSL_VERSION);
        let candles: Vec<Candle> = fixture["candles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candle| Candle {
                timestamp: candle["t"].as_i64().unwrap(),
                open: candle["o"].as_f64().unwrap(),
                high: candle["h"].as_f64().unwrap(),
                low: candle["l"].as_f64().unwrap(),
                close: candle["c"].as_f64().unwrap(),
                volume: candle["v"].as_f64().unwrap(),
            })
            .collect();
        for case in fixture["validCases"].as_array().unwrap() {
            let report = validate_strategy_dsl(&case["dsl"]);
            assert!(report.ok, "{}: {:?}", case["id"], report.errors);
            assert_eq!(
                report.node_count as u64,
                case["expected"]["nodeCount"].as_u64().unwrap()
            );
            assert_eq!(
                report.depth as u64,
                case["expected"]["depth"].as_u64().unwrap()
            );
            assert_eq!(
                report.max_lookback_bars,
                case["expected"]["maxLookbackBars"].as_i64().unwrap()
            );
            assert_eq!(
                report.parameter_count as u64,
                case["expected"]["parameterCount"].as_u64().unwrap()
            );
            let signals = build_dsl_signals(&candles, &case["dsl"]).unwrap();
            let expected_entry: Vec<bool> = case["expected"]["entry"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_bool().unwrap())
                .collect();
            let expected_exit: Vec<bool> = case["expected"]["exit"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_bool().unwrap())
                .collect();
            assert_eq!(signals.entry, expected_entry, "{}", case["id"]);
            assert_eq!(signals.exit, expected_exit, "{}", case["id"]);
        }
        for case in fixture["invalidCases"].as_array().unwrap() {
            let report = validate_strategy_dsl(&case["dsl"]);
            let fragment = case["errorContains"].as_str().unwrap();
            assert!(!report.ok, "{}", case["id"]);
            assert!(
                report.errors.iter().any(|error| error.contains(fragment)),
                "{}: {:?}",
                case["id"],
                report.errors
            );
            assert!(build_dsl_signals(&candles, &case["dsl"]).is_err());
        }
    }
}
