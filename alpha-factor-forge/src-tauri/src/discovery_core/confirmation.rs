//! `research-confirmation-statistics-v1`: the P12e-1 pure confirmation
//! calculation (docs/research-confirmation-statistics-v1.md).
//!
//! For each candidate it tests two one-sided hypotheses on per-bar returns —
//! mean net return above zero, and mean excess over buy-and-hold above zero —
//! with a circular block bootstrap, then Holm-adjusts every p-value against
//! the WHOLE trial family's test count. The block length, sample count and
//! seed are declared and frozen before any result is read; nothing here
//! estimates them from the data.
//!
//! P-values and Holm adjustments are exact rationals over `B + 1`, the
//! estimator P12a's precision precheck assumes. The only floating-point work
//! is summing returns in a specified order.
//!
//! Pure: no Tauri, rusqlite, threads, events, UI, clock or IO. Not called by
//! any runtime path. A rejected null hypothesis is a statistical statement
//! about one test; it is never a confirmation `PASS` (P13 owns that).

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::{Map, Value};

use super::precision::{Ratio, PRECISION_MAX_COUNT};

pub const CONFIRMATION_STATISTICS_VERSION: &str = "research-confirmation-statistics-v1";
pub const CONFIRMATION_CORRECTION_HOLM: &str = "holm";
pub const CONFIRMATION_SCHEME: &str = "circular-block";
pub const CONFIRMATION_PRNG: &str = "splitmix64";
/// Net return and benchmark excess: the trial ledger's `testsPerTrial`.
pub const CONFIRMATION_TESTS_PER_TRIAL: u64 = 2;
pub const CONFIRMATION_MAX_RATIONALE_BYTES: usize = 1024;

const PPM: u128 = 1_000_000;
const FIELDS: [&str; 9] = [
    "contractVersion",
    "correction",
    "scheme",
    "prng",
    "alphaPpm",
    "blockLength",
    "bootstrapSamples",
    "seed",
    "blockLengthRationale",
];

/// Everything the bootstrap depends on besides the data, frozen beforehand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmationDeclaration {
    /// Family-wise alpha allocated to this confirmation, ppm, `[1, 999_999]`.
    pub alpha_ppm: u64,
    /// Bars per resampled block; must not exceed any tested series.
    pub block_length: u64,
    /// Bootstrap sample count `B`.
    pub bootstrap_samples: u64,
    pub seed: u64,
    /// Why this block length was chosen, recorded before results exist.
    pub block_length_rationale: String,
}

/// One candidate's per-bar series over the confirmation segment.
#[derive(Clone, Copy, Debug)]
pub struct CandidateSeries<'a> {
    pub candidate_index: u64,
    /// Net strategy return of each bar, after costs.
    pub returns: &'a [f64],
    /// Buy-and-hold return of the same bars.
    pub benchmark_returns: &'a [f64],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationTest {
    /// H0: mean net return <= 0.
    NetReturn,
    /// H0: mean (net return - benchmark return) <= 0.
    BenchmarkExcess,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationTestResult {
    pub candidate_index: u64,
    pub test: ConfirmationTest,
    pub observations: u64,
    pub observed_mean: f64,
    /// Bootstrap samples whose centered statistic reached the observed one.
    pub extreme_count: u64,
    /// `(1 + extremeCount) / (B + 1)`.
    pub raw_p: Ratio,
    pub raw_p_value: f64,
    /// 1-based position in ascending raw-p order across this batch.
    pub holm_rank: u64,
    /// Holm step-down against the whole family, capped at 1.
    pub adjusted_p: Ratio,
    pub adjusted_p_value: f64,
    /// `adjustedP <= alpha`. A statistical statement for this one test.
    pub rejects_null: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationReport {
    pub contract_version: &'static str,
    pub correction: &'static str,
    pub scheme: &'static str,
    pub prng: &'static str,
    pub alpha_ppm: u64,
    pub block_length: u64,
    pub bootstrap_samples: u64,
    pub seed: u64,
    /// The family size `m` every adjustment used.
    pub family_tests: u64,
    /// Ordered by candidate index, then net return before benchmark excess.
    pub tests: Vec<ConfirmationTestResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmationError(pub String);

impl std::fmt::Display for ConfirmationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfirmationError {}

fn fail<T>(message: impl Into<String>) -> Result<T, ConfirmationError> {
    Err(ConfirmationError(message.into()))
}

fn read_count(
    object: &Map<String, Value>,
    field: &str,
    min: u64,
    max: u64,
) -> Result<u64, ConfirmationError> {
    let Some(value) = object.get(field) else {
        return fail(format!("confirmation.{field}: required"));
    };
    // `as_u64` is `None` for negatives and for any float literal (even `1.0`).
    match value.as_u64() {
        Some(count) if (min..=max).contains(&count) => Ok(count),
        _ => fail(format!(
            "confirmation.{field}: must be an integer in [{min}, {max}]"
        )),
    }
}

fn read_literal(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), ConfirmationError> {
    match object.get(field) {
        None => fail(format!("confirmation.{field}: required")),
        Some(Value::String(text)) if text == expected => Ok(()),
        Some(_) => fail(format!("confirmation.{field}: must be \"{expected}\"")),
    }
}

fn rationale_is_valid(text: &str) -> bool {
    !text.is_empty() && text.trim() == text && text.len() <= CONFIRMATION_MAX_RATIONALE_BYTES
}

/// Strictly parses a declaration. Rejection order: not an object, unknown
/// fields (sorted), then [`FIELDS`] in order.
pub fn parse_confirmation_declaration(
    raw: &Value,
) -> Result<ConfirmationDeclaration, ConfirmationError> {
    let Some(object) = raw.as_object() else {
        return fail("confirmation: must be an object");
    };
    let mut unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !FIELDS.contains(key))
        .collect();
    unknown.sort_unstable();
    if let Some(first) = unknown.first() {
        return fail(format!("confirmation.{first}: unknown field"));
    }
    read_literal(object, "contractVersion", CONFIRMATION_STATISTICS_VERSION)?;
    read_literal(object, "correction", CONFIRMATION_CORRECTION_HOLM)?;
    read_literal(object, "scheme", CONFIRMATION_SCHEME)?;
    read_literal(object, "prng", CONFIRMATION_PRNG)?;
    let alpha_ppm = read_count(object, "alphaPpm", 1, 999_999)?;
    let block_length = read_count(object, "blockLength", 1, PRECISION_MAX_COUNT)?;
    let bootstrap_samples = read_count(object, "bootstrapSamples", 1, PRECISION_MAX_COUNT)?;
    let seed = read_count(object, "seed", 0, PRECISION_MAX_COUNT)?;
    let block_length_rationale = match object.get("blockLengthRationale") {
        None => return fail("confirmation.blockLengthRationale: required"),
        Some(Value::String(text)) if rationale_is_valid(text) => text.clone(),
        Some(_) => {
            return fail(format!(
                "confirmation.blockLengthRationale: must be non-empty trimmed text of at most {CONFIRMATION_MAX_RATIONALE_BYTES} bytes"
            ))
        }
    };
    Ok(ConfirmationDeclaration {
        alpha_ppm,
        block_length,
        bootstrap_samples,
        seed,
        block_length_rationale,
    })
}

const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// SplitMix64's output function (Steele, Lea & Flood; Vigna's reference).
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(state: u64) -> Self {
        Self { state }
    }

    /// One independent stream per candidate, so a result does not depend on
    /// which other candidates share its batch or on their order.
    fn for_candidate(seed: u64, candidate_index: u64) -> Self {
        Self::new(mix64(seed ^ mix64(candidate_index.wrapping_add(1))))
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        mix64(self.state)
    }

    /// Uniform in `[0, n)` without modulo bias: draws below `2^64 mod n` are
    /// discarded, leaving a range whose size is a multiple of `n`.
    fn below(&mut self, n: u64) -> u64 {
        let threshold = n.wrapping_neg() % n;
        loop {
            let draw = self.next_u64();
            if draw >= threshold {
                return draw % n;
            }
        }
    }
}

/// Left-to-right `f64` sum: part of the contract, so any reader reproduces
/// the same bits.
fn ordered_sum(values: &[f64]) -> f64 {
    values.iter().fold(0.0, |total, value| total + value)
}

struct Observed {
    candidate_index: u64,
    test: ConfirmationTest,
    observations: u64,
    observed_mean: f64,
    extreme_count: u64,
}

fn declaration_in_domain(declaration: &ConfirmationDeclaration) -> bool {
    (1..=999_999).contains(&declaration.alpha_ppm)
        && (1..=PRECISION_MAX_COUNT).contains(&declaration.block_length)
        && (1..=PRECISION_MAX_COUNT).contains(&declaration.bootstrap_samples)
        && declaration.seed <= PRECISION_MAX_COUNT
        && rationale_is_valid(&declaration.block_length_rationale)
}

/// Bootstrap both tests of one candidate on the same resampled bars.
fn bootstrap_candidate(
    declaration: &ConfirmationDeclaration,
    candidate: &CandidateSeries<'_>,
) -> Result<[Observed; 2], ConfirmationError> {
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
    // A block as long as the series only rotates it: every resample has the
    // observed sum, so any positive mean would get the smallest possible p.
    // Requiring L^2 <= n keeps at least sqrt(n) blocks per resample.
    if u128::from(declaration.block_length) * u128::from(declaration.block_length) > n as u128 {
        return fail(format!(
            "confirmation: blockLength {} squared exceeds candidate {index}'s {n} bars",
            declaration.block_length
        ));
    }
    let excess: Vec<f64> = returns
        .iter()
        .zip(candidate.benchmark_returns)
        .map(|(net, benchmark)| net - benchmark)
        .collect();
    let observed_net = ordered_sum(returns);
    let observed_excess = ordered_sum(&excess);
    if returns.iter().any(|value| !value.is_finite())
        || candidate
            .benchmark_returns
            .iter()
            .any(|value| !value.is_finite())
        || excess.iter().any(|value| !value.is_finite())
        || !observed_net.is_finite()
        || !observed_excess.is_finite()
    {
        return fail(format!(
            "confirmation: candidate {index} has a non-finite return or sum"
        ));
    }

    let block = declaration.block_length as usize;
    let blocks = n.div_ceil(block);
    let mut rng = SplitMix64::for_candidate(declaration.seed, index);
    let (mut extreme_net, mut extreme_excess) = (0u64, 0u64);
    for _ in 0..declaration.bootstrap_samples {
        let (mut net, mut ex) = (0.0f64, 0.0f64);
        let mut filled = 0usize;
        for _ in 0..blocks {
            let start = rng.below(n as u64) as usize;
            let take = block.min(n - filled);
            for offset in 0..take {
                let bar = (start + offset) % n;
                net += returns[bar];
                ex += excess[bar];
            }
            filled += take;
        }
        // Centered null: (mean* - mean) >= mean  <=>  sum* >= 2 * sum.
        if net >= 2.0 * observed_net {
            extreme_net += 1;
        }
        if ex >= 2.0 * observed_excess {
            extreme_excess += 1;
        }
    }
    let observations = n as u64;
    Ok([
        Observed {
            candidate_index: index,
            test: ConfirmationTest::NetReturn,
            observations,
            observed_mean: observed_net / n as f64,
            extreme_count: extreme_net,
        },
        Observed {
            candidate_index: index,
            test: ConfirmationTest::BenchmarkExcess,
            observations,
            observed_mean: observed_excess / n as f64,
            extreme_count: extreme_excess,
        },
    ])
}

/// Run the declared bootstrap for every candidate and Holm-adjust against
/// `family_tests` — the ledger's `familyEffectiveTrials * testsPerTrial`, not
/// just this batch — so trials tested earlier still tighten the threshold.
pub fn evaluate_confirmation(
    declaration: &ConfirmationDeclaration,
    family_tests: u64,
    candidates: &[CandidateSeries<'_>],
) -> Result<ConfirmationReport, ConfirmationError> {
    if !declaration_in_domain(declaration) {
        return fail(
            "confirmation: declaration is outside the research-confirmation-statistics-v1 domain",
        );
    }
    if candidates.is_empty() {
        return fail("confirmation: no candidates");
    }
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        if candidate.candidate_index > PRECISION_MAX_COUNT
            || !seen.insert(candidate.candidate_index)
        {
            return fail(format!(
                "confirmation: candidate index {} is repeated or out of range",
                candidate.candidate_index
            ));
        }
    }
    let batch_tests = candidates.len() as u64 * CONFIRMATION_TESTS_PER_TRIAL;
    if family_tests > PRECISION_MAX_COUNT
        || !family_tests.is_multiple_of(CONFIRMATION_TESTS_PER_TRIAL)
        || family_tests < batch_tests
    {
        return fail(format!(
            "confirmation: familyTests must be a multiple of {CONFIRMATION_TESTS_PER_TRIAL} in [{batch_tests}, {PRECISION_MAX_COUNT}]"
        ));
    }

    let mut observed = Vec::with_capacity(candidates.len() * 2);
    for candidate in candidates {
        observed.extend(bootstrap_candidate(declaration, candidate)?);
    }

    // Ascending raw p == ascending extreme count (one shared denominator).
    let mut order: Vec<usize> = (0..observed.len()).collect();
    order.sort_by_key(|&i| {
        (
            observed[i].extreme_count,
            observed[i].candidate_index,
            observed[i].test,
        )
    });
    let denominator = declaration.bootstrap_samples + 1;
    let alpha = u128::from(declaration.alpha_ppm);
    let mut adjusted = vec![(0u64, 0u64); observed.len()];
    let mut running = 0u128;
    for (position, &i) in order.iter().enumerate() {
        let rank = position as u64 + 1;
        let multiplier = u128::from(family_tests - rank + 1);
        let numerator =
            (multiplier * u128::from(observed[i].extreme_count + 1)).min(u128::from(denominator));
        running = running.max(numerator);
        adjusted[i] = (rank, running as u64);
    }

    let mut tests: Vec<ConfirmationTestResult> = observed
        .iter()
        .zip(&adjusted)
        .map(|(row, &(holm_rank, adjusted_numerator))| {
            let raw_numerator = row.extreme_count + 1;
            ConfirmationTestResult {
                candidate_index: row.candidate_index,
                test: row.test,
                observations: row.observations,
                observed_mean: row.observed_mean,
                extreme_count: row.extreme_count,
                raw_p: Ratio {
                    numerator: raw_numerator,
                    denominator,
                },
                raw_p_value: raw_numerator as f64 / denominator as f64,
                holm_rank,
                adjusted_p: Ratio {
                    numerator: adjusted_numerator,
                    denominator,
                },
                adjusted_p_value: adjusted_numerator as f64 / denominator as f64,
                // adjusted / (B + 1) <= alphaPpm / 1e6, in exact integers.
                rejects_null: u128::from(adjusted_numerator) * PPM
                    <= alpha * u128::from(denominator),
            }
        })
        .collect();
    tests.sort_by_key(|row| (row.candidate_index, row.test));
    Ok(ConfirmationReport {
        contract_version: CONFIRMATION_STATISTICS_VERSION,
        correction: CONFIRMATION_CORRECTION_HOLM,
        scheme: CONFIRMATION_SCHEME,
        prng: CONFIRMATION_PRNG,
        alpha_ppm: declaration.alpha_ppm,
        block_length: declaration.block_length,
        bootstrap_samples: declaration.bootstrap_samples,
        seed: declaration.seed,
        family_tests,
        tests,
    })
}

#[cfg(test)]
mod tests;
