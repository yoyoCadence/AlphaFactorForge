//! DRAFT confirmation statistics S1 and S2 of the recalibration plan
//! (docs/research-confirmation-candidates-draft-v1.md,
//! docs/plans/confirmation-recalibration-plan-v1.md §4).
//!
//! Both keep everything of `research-confirmation-statistics-v1` — inputs,
//! resampling, exact p-values, Holm — and change only what makes a resample
//! extreme:
//!
//! - S1 studentizes: the resample's deviation over the variance of its DRAWN
//!   blocks against the observed sum over the block variance of the series.
//! - S2 stretches v1's centred deviation by a flat-top over Bartlett
//!   long-run variance ratio.
//!
//! Numerics (draft §2, after the PR #138 review): each test series is first
//! scaled by an exact power of two so its largest magnitude lies in [1, 2),
//! which makes every decision independent of the series' overall scale; any
//! intermediate that would still overflow or underflow is refused rather than
//! rounded to infinity or zero. A series whose bars are all equal is decided
//! by an explicit rule, not by rounding residue.
//!
//! Experimental. Neither is shown to be calibrated and neither may confirm a
//! strategy; only the simulation engine selects them. Pure: no Tauri,
//! rusqlite, threads, events, UI, clock or IO.

use super::confirmation::{
    evaluate_with, fail, ordered_sum, CandidateSeries, ConfirmationDeclaration, ConfirmationError,
    ConfirmationReport, ConfirmationTest, Observed, SplitMix64,
};

pub const CONFIRMATION_CANDIDATE_S1: &str = "research-confirmation-candidate-s1-v1";
pub const CONFIRMATION_CANDIDATE_S2: &str = "research-confirmation-candidate-s2-v1";

/// An intermediate the draft contract refuses instead of rounding it to
/// infinity, zero or a subnormal (draft §2).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutOfRange;

/// A product or quotient whose operands are non-zero must be a finite normal
/// number.
fn checked(result: f64, operands_nonzero: bool) -> Result<f64, OutOfRange> {
    if !result.is_finite() || (operands_nonzero && result.abs() < f64::MIN_POSITIVE) {
        Err(OutOfRange)
    } else {
        Ok(result)
    }
}

fn product(x: f64, y: f64) -> Result<f64, OutOfRange> {
    checked(x * y, x != 0.0 && y != 0.0)
}

fn quotient(x: f64, y: f64) -> Result<f64, OutOfRange> {
    checked(x / y, x != 0.0)
}

/// `floor(log2 m)` for a finite `m > 0`, read from its bits.
fn binary_exponent(m: f64) -> i32 {
    let bits = m.to_bits();
    let field = ((bits >> 52) & 0x7ff) as i32;
    if field == 0 {
        let mantissa = bits & ((1u64 << 52) - 1);
        63 - mantissa.leading_zeros() as i32 - 1074
    } else {
        field - 1023
    }
}

/// `x · 2^k`, with every factor a normal power of two built from its bits.
fn times_power_of_two(x: f64, k: i32) -> f64 {
    let factor = |k: i32| f64::from_bits(((k + 1023) as u64) << 52);
    if (-1022..=1023).contains(&k) {
        x * factor(k)
    } else {
        let half = k / 2;
        x * factor(half) * factor(k - half)
    }
}

/// The series scaled by `2^-e`, `e = floor(log2 max|x|)`, so its largest
/// magnitude lies in `[1, 2)`. Exact unless a value is more than about
/// `2^1022` times smaller than the largest, which is refused. An all-zero
/// series is returned unchanged.
fn normalized(series: &[f64]) -> Result<Vec<f64>, OutOfRange> {
    let largest = series
        .iter()
        .fold(0.0f64, |largest, x| largest.max(x.abs()));
    if largest == 0.0 {
        return Ok(series.to_vec());
    }
    let exponent = binary_exponent(largest);
    series
        .iter()
        .map(|&x| checked(times_power_of_two(x, -exponent), x != 0.0))
        .collect()
}

/// `B(l)`: the block variance over all `n` circular blocks of length
/// `block`, the Bartlett-weighted long-run variance estimate. The summation
/// order is part of the draft contract.
fn block_variance(series: &[f64], sum: f64, block: usize) -> Result<f64, OutOfRange> {
    let n = series.len();
    let centre = product(block as f64, quotient(sum, n as f64)?)?;
    let mut total = 0.0f64;
    for start in 0..n {
        let mut block_sum = 0.0f64;
        for offset in 0..block {
            block_sum += series[(start + offset) % n];
        }
        let deviation = block_sum - centre;
        total += product(deviation, deviation)?;
    }
    quotient(checked(total, false)?, (n * block) as f64)
}

/// `a·√p >= s·√q` for finite `a`, `s` and non-negative `p`, `q`, without a
/// square root (draft contract §2).
fn scaled_at_least(a: f64, p: f64, s: f64, q: f64) -> Result<bool, OutOfRange> {
    let left = product(product(a, a)?, p)?;
    let right = product(product(s, s)?, q)?;
    Ok(match (a >= 0.0, s >= 0.0) {
        (true, true) => left >= right,
        (true, false) => true,
        (false, true) => left == 0.0 && right == 0.0,
        (false, false) => left <= right,
    })
}

/// `v*`: the variance of the blocks DRAWN for one resample, centred on the
/// resample's own mean. Not `B(L)` recomputed on the resampled series.
fn drawn_block_variance(
    drawn: &[([f64; 2], usize)],
    total: f64,
    n: usize,
    test: usize,
) -> Result<f64, OutOfRange> {
    let mean = quotient(total, n as f64)?;
    let mut spread = 0.0f64;
    for (block_sums, take) in drawn {
        let deviation = block_sums[test] - product(*take as f64, mean)?;
        spread += product(deviation, deviation)?;
    }
    quotient(checked(spread, false)?, n as f64)
}

/// The rule for a series whose bars are all equal (draft §3, §4): no
/// variance is read; every resample is extreme when the common value is at
/// most zero, and none is when it is positive.
fn constant_extremes(series: &[f64], samples: u64) -> Option<u64> {
    let first = series[0];
    series
        .iter()
        .all(|&value| value == first)
        .then_some(if first > 0.0 { 0 } else { samples })
}

/// One candidate's two series, checked as v1 checks them, then normalized.
/// `block_multiple` is 1 for S1 and 2 for S2, whose widest block is `2L`.
struct Prepared {
    index: u64,
    n: usize,
    /// The observed sums of the series as given, for the reported means.
    observed_sums: [f64; 2],
    /// Net returns, then excess over the benchmark, normalized.
    series: [Vec<f64>; 2],
    sums: [f64; 2],
    /// `Some(extreme count)` for a test whose series is constant.
    constant: [Option<u64>; 2],
}

fn prepare(
    declaration: &ConfirmationDeclaration,
    candidate: &CandidateSeries<'_>,
    block_multiple: u64,
) -> Result<Prepared, ConfirmationError> {
    let index = candidate.candidate_index;
    let returns = candidate.returns;
    let n = returns.len();
    if n != candidate.benchmark_returns.len() {
        return fail(format!(
            "confirmation: candidate {index} has {n} returns but {} benchmark returns",
            candidate.benchmark_returns.len()
        ));
    }
    if n < 2 {
        return fail(format!(
            "confirmation: candidate {index} needs at least 2 bars"
        ));
    }
    let widest = u128::from(declaration.block_length) * u128::from(block_multiple);
    if widest * widest > n as u128 {
        return fail(if block_multiple == 1 {
            format!(
                "confirmation: blockLength {} squared exceeds candidate {index}'s {n} bars",
                declaration.block_length
            )
        } else {
            format!(
                "confirmation: twice blockLength {} squared exceeds candidate {index}'s {n} bars",
                declaration.block_length
            )
        });
    }
    let excess: Vec<f64> = returns
        .iter()
        .zip(candidate.benchmark_returns)
        .map(|(net, benchmark)| net - benchmark)
        .collect();
    let observed_sums = [ordered_sum(returns), ordered_sum(&excess)];
    if returns.iter().any(|value| !value.is_finite())
        || candidate
            .benchmark_returns
            .iter()
            .any(|value| !value.is_finite())
        || excess.iter().any(|value| !value.is_finite())
        || observed_sums.iter().any(|sum| !sum.is_finite())
    {
        return fail(format!(
            "confirmation: candidate {index} has a non-finite return or sum"
        ));
    }
    let constant = [
        constant_extremes(returns, declaration.bootstrap_samples),
        constant_extremes(&excess, declaration.bootstrap_samples),
    ];
    let series = [
        normalized(returns).map_err(|_| out_of_range(index))?,
        normalized(&excess).map_err(|_| out_of_range(index))?,
    ];
    let sums = [ordered_sum(&series[0]), ordered_sum(&series[1])];
    Ok(Prepared {
        index,
        n,
        observed_sums,
        series,
        sums,
        constant,
    })
}

fn out_of_range(index: u64) -> ConfirmationError {
    ConfirmationError(format!(
        "confirmation: candidate {index} is outside the candidates' numeric range (an intermediate overflowed or underflowed)"
    ))
}

fn observed(prepared: &Prepared, extreme: [u64; 2]) -> [Observed; 2] {
    let n = prepared.n;
    [
        ConfirmationTest::NetReturn,
        ConfirmationTest::BenchmarkExcess,
    ]
    .map(|test| {
        let position = test as usize;
        Observed {
            candidate_index: prepared.index,
            test,
            observations: n as u64,
            observed_mean: prepared.observed_sums[position] / n as f64,
            extreme_count: prepared.constant[position].unwrap_or(extreme[position]),
        }
    })
}

/// S1: both tests of one candidate on the same resampled bars.
fn bootstrap_s1(
    declaration: &ConfirmationDeclaration,
    candidate: &CandidateSeries<'_>,
) -> Result<[Observed; 2], ConfirmationError> {
    let prepared = prepare(declaration, candidate, 1)?;
    let index = prepared.index;
    let n = prepared.n;
    let block = declaration.block_length as usize;
    let blocks = n.div_ceil(block);
    let mut scale = [0.0f64; 2];
    for (test, slot) in scale.iter_mut().enumerate() {
        if prepared.constant[test].is_none() {
            *slot = block_variance(&prepared.series[test], prepared.sums[test], block)
                .map_err(|_| out_of_range(index))?;
        }
    }

    let mut rng = SplitMix64::for_candidate(declaration.seed, index);
    let mut extreme = [0u64; 2];
    // Sum and length of every drawn block of the current resample.
    let mut drawn: Vec<([f64; 2], usize)> = Vec::with_capacity(blocks);
    for _ in 0..declaration.bootstrap_samples {
        drawn.clear();
        let mut totals = [0.0f64; 2];
        let mut filled = 0usize;
        for _ in 0..blocks {
            let start = rng.below(n as u64) as usize;
            let take = block.min(n - filled);
            let mut block_sums = [0.0f64; 2];
            for offset in 0..take {
                let bar = (start + offset) % n;
                for test in 0..2 {
                    let value = prepared.series[test][bar];
                    totals[test] += value;
                    block_sums[test] += value;
                }
            }
            drawn.push((block_sums, take));
            filled += take;
        }
        for test in 0..2 {
            if prepared.constant[test].is_some() {
                continue;
            }
            let decide = || -> Result<bool, OutOfRange> {
                let resampled = drawn_block_variance(&drawn, totals[test], n, test)?;
                // (S* - S) / sqrt(v*) >= S / sqrt(B(L)).
                scaled_at_least(
                    totals[test] - prepared.sums[test],
                    scale[test],
                    prepared.sums[test],
                    resampled,
                )
            };
            if decide().map_err(|_| out_of_range(index))? {
                extreme[test] += 1;
            }
        }
    }
    Ok(observed(&prepared, extreme))
}

/// S2: both tests of one candidate on the same resampled bars.
fn bootstrap_s2(
    declaration: &ConfirmationDeclaration,
    candidate: &CandidateSeries<'_>,
) -> Result<[Observed; 2], ConfirmationError> {
    let prepared = prepare(declaration, candidate, 2)?;
    let index = prepared.index;
    let n = prepared.n;
    let block = declaration.block_length as usize;
    let blocks = n.div_ceil(block);
    // c^2 = p / q >= 1: the flat-top over the Bartlett long-run variance.
    let mut stretch = [(1.0f64, 1.0f64); 2];
    for (test, slot) in stretch.iter_mut().enumerate() {
        if prepared.constant[test].is_some() {
            continue;
        }
        let variances = || -> Result<(f64, f64), OutOfRange> {
            let narrow = block_variance(&prepared.series[test], prepared.sums[test], block)?;
            let wide = block_variance(&prepared.series[test], prepared.sums[test], 2 * block)?;
            Ok((narrow, checked(2.0 * wide - narrow, false)?))
        };
        let (narrow, flat_top) = variances().map_err(|_| out_of_range(index))?;
        if narrow > 0.0 && flat_top > narrow {
            *slot = (flat_top, narrow);
        }
    }

    let mut rng = SplitMix64::for_candidate(declaration.seed, index);
    let mut extreme = [0u64; 2];
    for _ in 0..declaration.bootstrap_samples {
        let mut totals = [0.0f64; 2];
        let mut filled = 0usize;
        for _ in 0..blocks {
            let start = rng.below(n as u64) as usize;
            let take = block.min(n - filled);
            for offset in 0..take {
                let bar = (start + offset) % n;
                totals[0] += prepared.series[0][bar];
                totals[1] += prepared.series[1][bar];
            }
            filled += take;
        }
        for test in 0..2 {
            if prepared.constant[test].is_some() {
                continue;
            }
            // c * (S* - S) >= S.
            let extreme_here = scaled_at_least(
                totals[test] - prepared.sums[test],
                stretch[test].0,
                prepared.sums[test],
                stretch[test].1,
            )
            .map_err(|_| out_of_range(index))?;
            if extreme_here {
                extreme[test] += 1;
            }
        }
    }
    Ok(observed(&prepared, extreme))
}

/// DRAFT candidate S1, the block-variance studentized bootstrap. Same
/// inputs and report as `evaluate_confirmation`.
pub fn evaluate_confirmation_candidate_s1(
    declaration: &ConfirmationDeclaration,
    family_tests: u64,
    candidates: &[CandidateSeries<'_>],
) -> Result<ConfirmationReport, ConfirmationError> {
    evaluate_with(
        CONFIRMATION_CANDIDATE_S1,
        declaration,
        family_tests,
        candidates,
        bootstrap_s1,
    )
}

/// DRAFT candidate S2, the centred bootstrap with a flat-top variance
/// correction. Same inputs and report as `evaluate_confirmation`; requires
/// `(2L)^2 <= n`.
pub fn evaluate_confirmation_candidate_s2(
    declaration: &ConfirmationDeclaration,
    family_tests: u64,
    candidates: &[CandidateSeries<'_>],
) -> Result<ConfirmationReport, ConfirmationError> {
    evaluate_with(
        CONFIRMATION_CANDIDATE_S2,
        declaration,
        family_tests,
        candidates,
        bootstrap_s2,
    )
}

#[cfg(test)]
mod tests;
