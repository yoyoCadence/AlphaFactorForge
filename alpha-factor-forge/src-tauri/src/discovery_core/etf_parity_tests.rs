//! Both runtimes consume the same hand-authored examples, not each other's output.
use super::etf::*;
use super::etf_metrics::*;
use super::metrics::{compute_metrics, ClosedTrade, EquityPoint, MetricsInput};
use super::parity_support::{assert_close, NumericTolerance};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/rs-core/etf-semantics-v1.json"
    ))
    .unwrap()
}
fn decode<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap()
}
fn string(v: &Value) -> &str {
    v.as_str().unwrap()
}
fn run(op: &str, i: &Value) -> EtfResult<Value> {
    Ok(match op {
        "sessions" => json!(etf_session_dates(
            &decode(&i["market"]),
            string(&i["from"]),
            string(&i["toExclusive"]),
            &decode::<Vec<_>>(&i["suspensions"])
        )?),
        "costs" => json!(estimate_etf_costs(
            &decode(&i["costs"]),
            decode(&i["currency"]),
            string(&i["side"]),
            i["quantity"].as_f64().unwrap(),
            i["rawPrice"].as_f64().unwrap()
        )?),
        "accrue" => json!(accrue_dividend(
            &decode(&i["state"]),
            &decode(&i["action"])
        )?),
        "pay" => json!(pay_dividend(
            &decode(&i["state"]),
            string(&i["actionId"]),
            string(&i["paymentDate"])
        )?),
        "value" => json!(value_etf_position(
            &decode(&i["state"]),
            i["rawPrice"].as_f64().unwrap()
        )?),
        "split" => json!(apply_split(&decode(&i["state"]), &decode(&i["action"]))?),
        "signals" => json!(split_adjusted_signals(
            &decode::<Vec<_>>(&i["raw"]),
            string(&i["instrumentId"]),
            &decode::<Vec<_>>(&i["splits"]),
            i["asOfMs"].as_i64().unwrap()
        )?),
        "assess" => json!(assess_etf_semantics(
            &decode(&i["market"]),
            &decode(&i["costs"]),
            &decode(&i["evidence"]),
            string(&i["from"]),
            string(&i["toExclusive"])
        )?),
        "metrics" => {
            let equity: Vec<EquityPoint> = decode(&i["equity"]);
            let trades: Vec<ClosedTrade> = decode(&i["trades"]);
            let input = MetricsInput {
                equity: &equity,
                trades: &trades,
                start_equity: i["startEquity"].as_f64(),
                total_bars: i["totalBars"].as_i64().unwrap(),
                bars_per_year: 365.0,
                risk_free_per_bar: i["riskFreePerBar"].as_f64(),
            };
            let r = compute_etf_metrics(
                &input,
                &decode(&i["market"]),
                i["periodStartMs"].as_i64().unwrap(),
                i["periodEndMs"].as_i64().unwrap(),
            )?;
            json!({ "version": r.version, "currency": r.currency, "calendarId": r.calendar_id,
                "sessionsPerYear": r.sessions_per_year, "elapsedYears": r.elapsed_years, "annualizedVolatility": r.annualized_volatility,
                "netReturn": r.metrics.net_return, "cagr": r.metrics.cagr, "maxDrawdown": r.metrics.max_drawdown,
                "calmar": r.metrics.calmar, "sharpe": r.metrics.sharpe })
        }
        _ => panic!("unknown fixture operation {op}"),
    })
}
fn compare(a: &Value, e: &Value, path: &str) {
    match e {
        Value::Number(_) if path.ends_with(".timestamp") => assert_eq!(a, e, "{path}"),
        Value::Number(v) => assert_close(
            path,
            a.as_f64().expect(path),
            v.as_f64().unwrap(),
            NumericTolerance {
                absolute: 1e-12,
                relative: 1e-10,
            },
        ),
        Value::Array(values) => {
            assert_eq!(a.as_array().expect(path).len(), values.len(), "{path}");
            for (n, v) in values.iter().enumerate() {
                compare(&a[n], v, &format!("{path}[{n}]"));
            }
        }
        Value::Object(values) => {
            assert_eq!(
                a.as_object().expect(path).keys().collect::<Vec<_>>(),
                values.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (k, v) in values {
                compare(&a[k], v, &format!("{path}.{k}"));
            }
        }
        _ => assert_eq!(a, e, "{path}"),
    }
}
#[test]
fn all_shared_etf_cases_match() {
    let f = fixture();
    assert_eq!(f["fixtureVersion"], "etf-semantics-parity-v1");
    assert_eq!(f["cases"].as_array().unwrap().len(), 55);
    for case in f["cases"].as_array().unwrap() {
        let id = string(&case["id"]);
        let result = run(string(&case["op"]), &case["input"]);
        if let Some(error) = case.get("error") {
            assert_eq!(result.unwrap_err(), string(error), "{id}");
        } else if case["op"] == "sessions" {
            assert_eq!(result.unwrap(), case["expected"], "{id}");
        } else {
            compare(
                &result.unwrap_or_else(|e| panic!("{id}: {e}")),
                &case["expected"],
                id,
            );
        }
    }
}
#[test]
fn nonfinite_and_legacy_regressions() {
    let f = fixture();
    let cases = f["cases"].as_array().unwrap();
    let i = &cases.iter().find(|c| c["op"] == "split").unwrap()["input"];
    let s: EtfPosition = decode(&i["state"]);
    let mut action: SplitAction = decode(&i["action"]);
    for ratio in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        action.ratio = ratio;
        assert_eq!(apply_split(&s, &action).unwrap_err(), "invalid_split");
    }
    let i = &cases.iter().find(|c| c["op"] == "metrics").unwrap()["input"];
    let equity: Vec<EquityPoint> = decode(&i["equity"]);
    let m = decode(&i["market"]);
    let mut input = MetricsInput {
        equity: &equity,
        trades: &[],
        start_equity: Some(100.0),
        total_bars: 2,
        bars_per_year: 365.0,
        risk_free_per_bar: None,
    };
    let old = compute_metrics(&input);
    assert!((old.cagr - (0.99_f64.powf(365.0 / 2.0) - 1.0)).abs() < 1e-12);
    input.risk_free_per_bar = Some(f64::NAN);
    assert!(matches!(
        compute_etf_metrics(
            &input,
            &m,
            i["periodStartMs"].as_i64().unwrap(),
            i["periodEndMs"].as_i64().unwrap()
        ),
        Err("invalid_metrics_input")
    ));
}
