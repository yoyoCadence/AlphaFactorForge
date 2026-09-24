//! `research-walk-forward-v1`: Train-only window and sample-length precheck.
//!
//! The outer validation split remains the only source of Train boundaries.
//! This planner never reads candles, runs a backtest, or exposes outer
//! Validation/Test observations to research. P12c-2 owns fold execution;
//! P12d owns protocol freezing and admission.

use serde::Serialize;
use serde_json::{Map, Value};

use super::split::{plan_validation_split, InclusiveBarRange};

pub const WALK_FORWARD_VERSION: &str = "research-walk-forward-v1";
pub const WALK_FORWARD_MAX_COUNT: u64 = 9_007_199_254_740_991;
pub const WALK_FORWARD_MAX_FOLDS: u64 = 128;

const FIELDS: [&str; 6] = [
    "contractVersion",
    "totalBars",
    "embargoBars",
    "minimumTrainBars",
    "foldValidationBars",
    "foldCount",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkForwardPlan {
    pub total_bars: u64,
    /// Reused for the outer split and for every inner fold gap.
    pub embargo_bars: u64,
    /// Caller-declared minimum; P12d must freeze and justify this threshold.
    pub minimum_train_bars: u64,
    pub fold_validation_bars: u64,
    pub fold_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WalkForwardStatus {
    Eligible,
    NotEligible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkForwardReason {
    InsufficientOuterHistory,
    InsufficientTrainHistory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkForwardFold {
    pub index: u64,
    pub train: InclusiveBarRange,
    pub embargo: Option<InclusiveBarRange>,
    pub validation: InclusiveBarRange,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkForwardReport {
    pub contract_version: &'static str,
    pub plan: WalkForwardPlan,
    pub status: WalkForwardStatus,
    pub reasons: Vec<WalkForwardReason>,
    pub outer_train: Option<InclusiveBarRange>,
    pub available_train_bars: Option<u64>,
    /// None when the required count is above the JavaScript safe-integer bound.
    pub required_train_bars: Option<u64>,
    pub folds: Vec<WalkForwardFold>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalkForwardError(pub String);

impl std::fmt::Display for WalkForwardError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for WalkForwardError {}

fn read_count(object: &Map<String, Value>, field: &str) -> Result<u64, WalkForwardError> {
    let value = object
        .get(field)
        .ok_or_else(|| WalkForwardError(format!("missing {field}")))?;
    let count = value
        .as_u64()
        .ok_or_else(|| WalkForwardError(format!("{field} must be a non-negative JSON integer")))?;
    if count > WALK_FORWARD_MAX_COUNT {
        return Err(WalkForwardError(format!(
            "{field} exceeds the safe-integer limit"
        )));
    }
    Ok(count)
}

fn validate_plan(plan: &WalkForwardPlan) -> Result<(), WalkForwardError> {
    for (field, value) in [
        ("totalBars", plan.total_bars),
        ("embargoBars", plan.embargo_bars),
        ("minimumTrainBars", plan.minimum_train_bars),
        ("foldValidationBars", plan.fold_validation_bars),
        ("foldCount", plan.fold_count),
    ] {
        if value > WALK_FORWARD_MAX_COUNT {
            return Err(WalkForwardError(format!(
                "{field} exceeds the safe-integer limit"
            )));
        }
    }
    if plan.minimum_train_bars == 0 {
        return Err(WalkForwardError("minimumTrainBars must be positive".into()));
    }
    if plan.fold_validation_bars == 0 {
        return Err(WalkForwardError(
            "foldValidationBars must be positive".into(),
        ));
    }
    if !(2..=WALK_FORWARD_MAX_FOLDS).contains(&plan.fold_count) {
        return Err(WalkForwardError(format!(
            "foldCount must be between 2 and {WALK_FORWARD_MAX_FOLDS}"
        )));
    }
    Ok(())
}

/// Strict JSON entry point. Malformed declarations are errors; valid plans
/// with too little history return `NOT_ELIGIBLE` instead of fabricated folds.
pub fn precheck_walk_forward(value: &Value) -> Result<WalkForwardReport, WalkForwardError> {
    let object = value
        .as_object()
        .ok_or_else(|| WalkForwardError("plan must be an object".into()))?;
    let mut unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !FIELDS.contains(key))
        .collect();
    unknown.sort_unstable();
    if let Some(first) = unknown.first() {
        return Err(WalkForwardError(format!("unknown field {first}")));
    }
    if object.get("contractVersion").and_then(Value::as_str) != Some(WALK_FORWARD_VERSION) {
        return Err(WalkForwardError(format!(
            "contractVersion must be {WALK_FORWARD_VERSION}"
        )));
    }
    evaluate_walk_forward_plan(&WalkForwardPlan {
        total_bars: read_count(object, "totalBars")?,
        embargo_bars: read_count(object, "embargoBars")?,
        minimum_train_bars: read_count(object, "minimumTrainBars")?,
        fold_validation_bars: read_count(object, "foldValidationBars")?,
        fold_count: read_count(object, "foldCount")?,
    })
}

/// Build expanding Train-only folds. Every fold's validation span is disjoint;
/// later training spans may include earlier inner validation because all of
/// them remain inside the outer Train development period.
pub fn evaluate_walk_forward_plan(
    plan: &WalkForwardPlan,
) -> Result<WalkForwardReport, WalkForwardError> {
    validate_plan(plan)?;
    let required = u128::from(plan.minimum_train_bars)
        + u128::from(plan.fold_count)
            * (u128::from(plan.embargo_bars) + u128::from(plan.fold_validation_bars));
    let required_train_bars =
        (required <= u128::from(WALK_FORWARD_MAX_COUNT)).then_some(required as u64);
    let mut report = WalkForwardReport {
        contract_version: WALK_FORWARD_VERSION,
        plan: *plan,
        status: WalkForwardStatus::NotEligible,
        reasons: Vec::new(),
        outer_train: None,
        available_train_bars: None,
        required_train_bars,
        folds: Vec::new(),
    };

    let outer = match plan_validation_split(plan.total_bars as i64, plan.embargo_bars as i64) {
        Ok(outer) => outer,
        // Typed input validation above leaves insufficient usable bars as the
        // only possible split failure.
        Err(_) => {
            report
                .reasons
                .push(WalkForwardReason::InsufficientOuterHistory);
            return Ok(report);
        }
    };
    let available = outer.train.count as u64;
    report.outer_train = Some(outer.train);
    report.available_train_bars = Some(available);
    if u128::from(available) < required {
        report
            .reasons
            .push(WalkForwardReason::InsufficientTrainHistory);
        return Ok(report);
    }

    let fold_span = plan.embargo_bars + plan.fold_validation_bars;
    let first_train_count = available - plan.fold_count * fold_span;
    let mut cursor = outer.train.from + first_train_count as i64;
    for index in 0..plan.fold_count {
        let train = InclusiveBarRange {
            from: outer.train.from,
            to: cursor - 1,
            count: cursor - outer.train.from,
        };
        let embargo = if plan.embargo_bars == 0 {
            None
        } else {
            Some(InclusiveBarRange {
                from: cursor,
                to: cursor + plan.embargo_bars as i64 - 1,
                count: plan.embargo_bars as i64,
            })
        };
        cursor += plan.embargo_bars as i64;
        let validation = InclusiveBarRange {
            from: cursor,
            to: cursor + plan.fold_validation_bars as i64 - 1,
            count: plan.fold_validation_bars as i64,
        };
        cursor = validation.to + 1;
        report.folds.push(WalkForwardFold {
            index,
            train,
            embargo,
            validation,
        });
    }
    debug_assert_eq!(cursor - 1, outer.train.to);
    report.status = WalkForwardStatus::Eligible;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plan(total_bars: u64) -> WalkForwardPlan {
        WalkForwardPlan {
            total_bars,
            embargo_bars: 2,
            minimum_train_bars: 20,
            fold_validation_bars: 10,
            fold_count: 3,
        }
    }

    #[test]
    fn exact_history_boundary_and_train_only_windows() {
        let short = evaluate_walk_forward_plan(&plan(96)).unwrap();
        assert_eq!(short.status, WalkForwardStatus::NotEligible);
        assert_eq!(
            short.reasons,
            vec![WalkForwardReason::InsufficientTrainHistory]
        );
        assert_eq!(short.available_train_bars, Some(55));
        assert_eq!(short.required_train_bars, Some(56));
        assert!(short.folds.is_empty());

        let mut two_folds = plan(96);
        two_folds.fold_count = 2;
        assert_eq!(
            evaluate_walk_forward_plan(&two_folds).unwrap().status,
            WalkForwardStatus::Eligible
        );

        let exact = evaluate_walk_forward_plan(&plan(97)).unwrap();
        assert_eq!(exact.status, WalkForwardStatus::Eligible);
        assert_eq!(
            exact.outer_train,
            Some(InclusiveBarRange {
                from: 0,
                to: 55,
                count: 56
            })
        );
        assert_eq!(exact.folds.len(), 3);
        assert_eq!(
            exact.folds[0].train,
            InclusiveBarRange {
                from: 0,
                to: 19,
                count: 20
            }
        );
        assert_eq!(
            exact.folds[0].embargo,
            Some(InclusiveBarRange {
                from: 20,
                to: 21,
                count: 2
            })
        );
        assert_eq!(
            exact.folds[0].validation,
            InclusiveBarRange {
                from: 22,
                to: 31,
                count: 10
            }
        );
        assert_eq!(
            exact.folds[1].train,
            InclusiveBarRange {
                from: 0,
                to: 31,
                count: 32
            }
        );
        assert_eq!(
            exact.folds[1].validation,
            InclusiveBarRange {
                from: 34,
                to: 43,
                count: 10
            }
        );
        assert_eq!(
            exact.folds[2].train,
            InclusiveBarRange {
                from: 0,
                to: 43,
                count: 44
            }
        );
        assert_eq!(
            exact.folds[2].validation,
            InclusiveBarRange {
                from: 46,
                to: 55,
                count: 10
            }
        );
        let outer = plan_validation_split(97, 2).unwrap();
        assert!(exact
            .folds
            .iter()
            .all(|fold| fold.validation.to < outer.validation.from));
    }

    #[test]
    fn zero_inner_gap_still_uses_disjoint_validation_windows() {
        let mut input = plan(40);
        input.embargo_bars = 0;
        input.minimum_train_bars = 4;
        input.fold_validation_bars = 4;
        input.fold_count = 2;
        let report = evaluate_walk_forward_plan(&input).unwrap();
        assert_eq!(report.status, WalkForwardStatus::Eligible);
        assert!(report.folds.iter().all(|fold| fold.embargo.is_none()));
        assert_eq!(
            report.folds[0].validation.to + 1,
            report.folds[1].validation.from
        );
    }

    #[test]
    fn insufficient_outer_history_and_oversized_requirement_fail_closed() {
        let outer_short = evaluate_walk_forward_plan(&plan(8)).unwrap();
        assert_eq!(
            outer_short.reasons,
            vec![WalkForwardReason::InsufficientOuterHistory]
        );
        assert!(outer_short.outer_train.is_none());

        let huge = WalkForwardPlan {
            total_bars: WALK_FORWARD_MAX_COUNT,
            embargo_bars: 0,
            minimum_train_bars: WALK_FORWARD_MAX_COUNT,
            fold_validation_bars: WALK_FORWARD_MAX_COUNT,
            fold_count: WALK_FORWARD_MAX_FOLDS,
        };
        let report = evaluate_walk_forward_plan(&huge).unwrap();
        assert_eq!(report.status, WalkForwardStatus::NotEligible);
        assert_eq!(
            report.reasons,
            vec![WalkForwardReason::InsufficientTrainHistory]
        );
        assert_eq!(report.required_train_bars, None);
    }

    #[test]
    fn every_admitted_fold_stays_inside_outer_train_across_small_boundaries() {
        for total_bars in 0..160 {
            for embargo_bars in 0..5 {
                for fold_count in 2..5 {
                    let input = WalkForwardPlan {
                        total_bars,
                        embargo_bars,
                        minimum_train_bars: 4,
                        fold_validation_bars: 3,
                        fold_count,
                    };
                    let report = evaluate_walk_forward_plan(&input).unwrap();
                    if report.status == WalkForwardStatus::NotEligible {
                        assert!(report.folds.is_empty());
                        continue;
                    }
                    let outer =
                        plan_validation_split(total_bars as i64, embargo_bars as i64).unwrap();
                    assert_eq!(report.folds.len(), fold_count as usize);
                    assert_eq!(report.folds.last().unwrap().validation.to, outer.train.to);
                    for (index, fold) in report.folds.iter().enumerate() {
                        assert_eq!(fold.train.from, outer.train.from);
                        assert!(fold.train.count >= 4);
                        assert_eq!(fold.validation.count, 3);
                        assert_eq!(
                            fold.validation.from,
                            fold.train.to + embargo_bars as i64 + 1
                        );
                        assert!(fold.validation.to < outer.validation.from);
                        if index > 0 {
                            assert!(report.folds[index - 1].validation.to < fold.validation.from);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn strict_json_and_typed_inputs_reject_invalid_protocols() {
        let good = json!({
            "contractVersion": WALK_FORWARD_VERSION,
            "totalBars": 97,
            "embargoBars": 2,
            "minimumTrainBars": 20,
            "foldValidationBars": 10,
            "foldCount": 3
        });
        assert_eq!(
            precheck_walk_forward(&good).unwrap().status,
            WalkForwardStatus::Eligible
        );
        for (field, replacement) in [
            ("totalBars", json!(-1)),
            ("embargoBars", json!(1.0)),
            ("minimumTrainBars", json!(0)),
            ("foldValidationBars", json!(0)),
            ("foldCount", json!(1)),
        ] {
            let mut invalid = good.clone();
            invalid[field] = replacement;
            assert!(precheck_walk_forward(&invalid).is_err(), "{field}");
        }
        let mut unknown = good.clone();
        unknown["peekAtTest"] = json!(true);
        assert_eq!(
            precheck_walk_forward(&unknown).unwrap_err().0,
            "unknown field peekAtTest"
        );
        assert!(precheck_walk_forward(&json!({})).is_err());

        let mut typed = plan(97);
        typed.fold_count = WALK_FORWARD_MAX_FOLDS + 1;
        assert!(evaluate_walk_forward_plan(&typed).is_err());
        typed.fold_count = 3;
        typed.total_bars = WALK_FORWARD_MAX_COUNT + 1;
        assert!(evaluate_walk_forward_plan(&typed).is_err());
    }
}
