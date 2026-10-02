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
//! Experimental. Neither is shown to be calibrated and neither may confirm a
//! strategy; only the simulation engine selects them. Pure: no Tauri,
//! rusqlite, threads, events, UI, clock or IO.

use super::confirmation::{
    evaluate_with, fail, ordered_sum, CandidateSeries, ConfirmationDeclaration, ConfirmationError,
    ConfirmationReport, ConfirmationTest, Observed, SplitMix64,
};

pub const CONFIRMATION_CANDIDATE_S1: &str = "research-confirmation-candidate-s1-v1";
pub const CONFIRMATION_CANDIDATE_S2: &str = "research-confirmation-candidate-s2-v1";

/// `B(l)`: the block variance over all `n` circular blocks of length
/// `block`, the Bartlett-weighted long-run variance estimate. The summation
/// order is part of the draft contract.
fn block_variance(series: &[f64], sum: f64, block: usize) -> f64 {
    let n = series.len();
    let centre = block as f64 * (sum / n as f64);
    let mut total = 0.0f64;
    for start in 0..n {
        let mut block_sum = 0.0f64;
        for offset in 0..block {
            block_sum += series[(start + offset) % n];
        }
        let deviation = block_sum - centre;
        total += deviation * deviation;
    }
    total / (n * block) as f64
}

/// `a·√p >= s·√q` for finite `a`, `s` and non-negative `p`, `q`, without a
/// square root (draft contract §2).
fn scaled_at_least(a: f64, p: f64, s: f64, q: f64) -> bool {
    let left = a * a * p;
    let right = s * s * q;
    match (a >= 0.0, s >= 0.0) {
        (true, true) => left >= right,
        (true, false) => true,
        (false, true) => left == 0.0 && right == 0.0,
        (false, false) => left <= right,
    }
}

/// `v*`: the variance of the blocks DRAWN for one resample, centred on the
/// resample's own mean. Not `B(L)` recomputed on the resampled series.
fn drawn_block_variance(drawn: &[([f64; 2], usize)], total: f64, n: usize, test: usize) -> f64 {
    let mean = total / n as f64;
    let mut spread = 0.0f64;
    for (block_sums, take) in drawn {
        let deviation = block_sums[test] - *take as f64 * mean;
        spread += deviation * deviation;
    }
    spread / n as f64
}

/// One candidate's two series, checked as v1 checks them. `block_multiple`
/// is 1 for S1 and 2 for S2, whose widest block is `2L`.
struct Prepared<'a> {
    index: u64,
    /// Net returns, then excess over the benchmark.
    series: [std::borrow::Cow<'a, [f64]>; 2],
    sums: [f64; 2],
}

fn prepare<'a>(
    declaration: &ConfirmationDeclaration,
    candidate: &CandidateSeries<'a>,
    block_multiple: u64,
) -> Result<Prepared<'a>, ConfirmationError> {
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
    let sums = [ordered_sum(returns), ordered_sum(&excess)];
    if returns.iter().any(|value| !value.is_finite())
        || candidate
            .benchmark_returns
            .iter()
            .any(|value| !value.is_finite())
        || excess.iter().any(|value| !value.is_finite())
        || sums.iter().any(|sum| !sum.is_finite())
    {
        return fail(format!(
            "confirmation: candidate {index} has a non-finite return or sum"
        ));
    }
    Ok(Prepared {
        index,
        series: [returns.into(), excess.into()],
        sums,
    })
}

fn finite_variance(value: f64, index: u64) -> Result<f64, ConfirmationError> {
    if value.is_finite() {
        Ok(value)
    } else {
        fail(format!(
            "confirmation: candidate {index} has a non-finite block variance"
        ))
    }
}

fn observed(prepared: &Prepared<'_>, extreme: [u64; 2]) -> [Observed; 2] {
    let n = prepared.series[0].len();
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
            observed_mean: prepared.sums[position] / n as f64,
            extreme_count: extreme[position],
        }
    })
}

/// S1: both tests of one candidate on the same resampled bars.
fn bootstrap_s1(
    declaration: &ConfirmationDeclaration,
    candidate: &CandidateSeries<'_>,
) -> Result<[Observed; 2], ConfirmationError> {
    let prepared = prepare(declaration, candidate, 1)?;
    let n = prepared.series[0].len();
    let block = declaration.block_length as usize;
    let blocks = n.div_ceil(block);
    let scale = [
        finite_variance(
            block_variance(&prepared.series[0], prepared.sums[0], block),
            prepared.index,
        )?,
        finite_variance(
            block_variance(&prepared.series[1], prepared.sums[1], block),
            prepared.index,
        )?,
    ];

    let mut rng = SplitMix64::for_candidate(declaration.seed, prepared.index);
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
            let resampled = drawn_block_variance(&drawn, totals[test], n, test);
            // (S* - S) / sqrt(v*) >= S / sqrt(B(L)).
            if scaled_at_least(
                totals[test] - prepared.sums[test],
                scale[test],
                prepared.sums[test],
                resampled,
            ) {
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
    let n = prepared.series[0].len();
    let block = declaration.block_length as usize;
    let blocks = n.div_ceil(block);
    // c^2 = p / q >= 1: the flat-top over the Bartlett long-run variance.
    let mut stretch = [(1.0f64, 1.0f64); 2];
    for (test, slot) in stretch.iter_mut().enumerate() {
        let narrow = finite_variance(
            block_variance(&prepared.series[test], prepared.sums[test], block),
            prepared.index,
        )?;
        let wide = finite_variance(
            block_variance(&prepared.series[test], prepared.sums[test], 2 * block),
            prepared.index,
        )?;
        let flat_top = 2.0 * wide - narrow;
        if narrow > 0.0 && flat_top > narrow {
            *slot = (flat_top, narrow);
        }
    }

    let mut rng = SplitMix64::for_candidate(declaration.seed, prepared.index);
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
            // c * (S* - S) >= S.
            if scaled_at_least(
                totals[test] - prepared.sums[test],
                stretch[test].0,
                prepared.sums[test],
                stretch[test].1,
            ) {
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
