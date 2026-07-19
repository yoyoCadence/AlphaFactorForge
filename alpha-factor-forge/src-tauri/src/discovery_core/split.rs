//! `validation-split-v1`: pure Rust parity port of `src/core/validation/split`.
//!
//! Deterministic 60/20/20 largest-remainder allocation after two equal
//! embargo gaps, with exact integer arithmetic and the same fail-closed
//! messages as the TypeScript reference (VAL-001 contract).

use serde::{Deserialize, Serialize};

pub const SPLIT_CONTRACT_VERSION: &str = "validation-split-v1";

const MIN_USABLE_BARS: i64 = 5;
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const RATIO_WEIGHTS: [i64; 3] = [3, 1, 1];
const RATIO_DENOMINATOR: i64 = 5;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InclusiveBarRange {
    pub from: i64,
    pub to: i64,
    pub count: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSplitPlan {
    pub total_bars: i64,
    pub usable_bars: i64,
    pub embargo_bars: i64,
    pub train: InclusiveBarRange,
    pub train_validation_embargo: Option<InclusiveBarRange>,
    pub validation: InclusiveBarRange,
    pub validation_test_embargo: Option<InclusiveBarRange>,
    pub test: InclusiveBarRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitError(pub String);

impl std::fmt::Display for SplitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SplitError {}

fn assert_non_negative_safe_integer(value: i64, name: &str) -> Result<(), SplitError> {
    if !(0..=JS_MAX_SAFE_INTEGER).contains(&value) {
        return Err(SplitError(format!(
            "{name} must be a non-negative safe integer"
        )));
    }
    Ok(())
}

struct Allocation {
    train: i64,
    validation: i64,
    test: i64,
}

/// Largest-remainder allocation with Train -> Validation -> Test tie order.
fn allocate_usable_bars(usable_bars: i64) -> Result<Allocation, SplitError> {
    let quotient = usable_bars / RATIO_DENOMINATOR;
    let tail = usable_bars % RATIO_DENOMINATOR;
    let mut counts: Vec<i64> = RATIO_WEIGHTS
        .iter()
        .map(|weight| weight * quotient + (weight * tail) / RATIO_DENOMINATOR)
        .collect();

    let mut remainder_order: Vec<(usize, i64)> = RATIO_WEIGHTS
        .iter()
        .enumerate()
        .map(|(index, weight)| (index, (weight * tail) % RATIO_DENOMINATOR))
        .collect();
    remainder_order.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));

    let mut remaining = usable_bars - counts.iter().sum::<i64>();
    let mut order = remainder_order.iter();
    while remaining > 0 {
        let (index, _) = order.next().expect("at most two remainder bars exist");
        counts[*index] += 1;
        remaining -= 1;
    }

    let (train, validation, test) = (counts[0], counts[1], counts[2]);
    if train < 1 || validation < 1 || test < 1 {
        return Err(SplitError(format!(
            "at least {MIN_USABLE_BARS} usable bars are required for non-empty 60/20/20 segments"
        )));
    }
    Ok(Allocation {
        train,
        validation,
        test,
    })
}

fn range_from(cursor: i64, count: i64) -> InclusiveBarRange {
    InclusiveBarRange {
        from: cursor,
        to: cursor + count - 1,
        count,
    }
}

/// Plan the v1 Train/Validation/Test split over bars sorted oldest to newest.
pub fn plan_validation_split(
    total_bars: i64,
    embargo_bars: i64,
) -> Result<ValidationSplitPlan, SplitError> {
    assert_non_negative_safe_integer(total_bars, "totalBars")?;
    assert_non_negative_safe_integer(embargo_bars, "embargoBars")?;

    let usable_bars = total_bars - embargo_bars * 2;
    if usable_bars < MIN_USABLE_BARS {
        return Err(SplitError(format!(
            "at least {MIN_USABLE_BARS} usable bars are required after two embargo gaps"
        )));
    }

    let allocation = allocate_usable_bars(usable_bars)?;
    let mut cursor = 0i64;

    let train = range_from(cursor, allocation.train);
    cursor = train.to + 1;

    let train_validation_embargo = if embargo_bars > 0 {
        Some(range_from(cursor, embargo_bars))
    } else {
        None
    };
    cursor += embargo_bars;

    let validation = range_from(cursor, allocation.validation);
    cursor = validation.to + 1;

    let validation_test_embargo = if embargo_bars > 0 {
        Some(range_from(cursor, embargo_bars))
    } else {
        None
    };
    cursor += embargo_bars;

    let test = range_from(cursor, allocation.test);

    Ok(ValidationSplitPlan {
        total_bars,
        usable_bars,
        embargo_bars,
        train,
        train_validation_embargo,
        validation,
        validation_test_embargo,
        test,
    })
}
