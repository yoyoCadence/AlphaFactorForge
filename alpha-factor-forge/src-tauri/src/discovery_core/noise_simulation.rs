//! `research-noise-simulation-v1`: the P12e-3 false-positive simulation of the
//! declared confirmation protocol (docs/research-noise-simulation-v1.md).
//!
//! It generates seeded noise with no edge — every tested null hypothesis is
//! true — and runs a trial family's whole confirmation life on it with the
//! production code: each confirmation gets its alpha from
//! `research-alpha-allocation-v1` and is evaluated by
//! `research-confirmation-statistics-v1`. One simulation counts as a false
//! positive when ANY test of ANY candidate in ANY of its confirmations
//! rejects. The observed family rate is compared, in exact integers, with the
//! schedule's alpha plus a tolerance declared beforehand.
//!
//! Everything that decides the outcome — noise model, sizes, block length,
//! sample counts, seed and tolerance — is in the declaration, so a report is
//! reproducible and nothing can be adjusted after reading it.
//!
//! Pure: no Tauri, rusqlite, threads, events, UI, clock or IO, and not called
//! by any runtime path. It is evidence about the test procedure on synthetic
//! data; it is never a confirmation `PASS` and says nothing about a strategy.

use serde::Serialize;
use serde_json::{Map, Value};

use super::alpha_allocation::{
    allocate_confirmation_alpha, alpha_allocation_id, parse_alpha_allocation,
    AlphaAllocationDeclaration,
};
use super::confirmation::{
    evaluate_confirmation, mix64, CandidateSeries, ConfirmationDeclaration, ConfirmationTest,
    SplitMix64, CONFIRMATION_STATISTICS_VERSION, CONFIRMATION_TESTS_PER_TRIAL,
};
use super::precision::{Ratio, PRECISION_MAX_COUNT};

pub const NOISE_SIMULATION_VERSION: &str = "research-noise-simulation-v1";
/// Zero-mean AR(1) noise whose innovations are sums of four uniforms.
pub const NOISE_MODEL_AR1: &str = "ar1-uniform-sum";
/// Bars generated and discarded before each series so it starts in its
/// stationary regime rather than at zero.
pub const NOISE_WARMUP_BARS: usize = 64;
pub const NOISE_MAX_BARS: u64 = 1_000_000;
pub const NOISE_MAX_CANDIDATES: u64 = 1_024;
pub const NOISE_MAX_SIMULATIONS: u64 = 1_000_000;

const PPM: u128 = 1_000_000;
const UNIFORM_SCALE: f64 = 1.0 / 9_007_199_254_740_992.0;
/// Stream domains, so data and bootstrap seeds never share a sequence.
const DOMAIN_DATA: u64 = 0;
const DOMAIN_CONFIRMATION_SEED: u64 = 1;
const ROLE_RETURNS: u64 = 0;
const ROLE_BENCHMARK: u64 = 1;
/// Not read by the calculation; a confirmation declaration requires one.
const SIMULATED_RATIONALE: &str = "declared by research-noise-simulation-v1";

const FIELDS: [&str; 13] = [
    "contractVersion",
    "confirmationContract",
    "noiseModel",
    "autocorrelationPpm",
    "bars",
    "candidatesPerConfirmation",
    "priorTrials",
    "blockLength",
    "bootstrapSamples",
    "simulations",
    "seed",
    "tolerancePpm",
    "allocation",
];

/// One fully declared simulation: the noise, the protocol under test, its
/// size and the tolerance, frozen before it runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoiseSimulationDeclaration {
    /// AR(1) coefficient of every series, ppm, `[0, 999_999]`; `0` is
    /// independent noise.
    pub autocorrelation_ppm: u64,
    /// Bars of each candidate's confirmation segment.
    pub bars: u64,
    pub candidates_per_confirmation: u64,
    /// Trials the family registered before its first confirmation and never
    /// confirms; they only enlarge the family count.
    pub prior_trials: u64,
    pub block_length: u64,
    pub bootstrap_samples: u64,
    /// Independent family lives to simulate.
    pub simulations: u64,
    pub seed: u64,
    /// Allowed excess of the observed family false-positive rate over the
    /// schedule's alpha, ppm.
    pub tolerance_ppm: u64,
    /// The family's alpha budget; every scheduled confirmation is simulated.
    pub allocation: AlphaAllocationDeclaration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoiseSimulationStatus {
    /// The observed family rate is at most the schedule's alpha + tolerance.
    WithinTolerance,
    ExceedsTolerance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseConfirmationResult {
    pub confirmation_number: u64,
    /// The share `research-alpha-allocation-v1` gave this confirmation.
    pub alpha_ppm: u64,
    /// The whole-family test count its Holm adjustment used.
    pub family_tests: u64,
    /// Simulations in which this confirmation rejected at least one null.
    pub false_positives: u64,
    /// `floor(falsePositives * 1e6 / simulations)`.
    pub false_positive_rate_ppm: u64,
    /// Rejected nulls over all simulations, by test.
    pub net_return_rejections: u64,
    pub benchmark_excess_rejections: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseSimulationReport {
    pub contract_version: &'static str,
    pub confirmation_contract: &'static str,
    pub allocation_id: String,
    pub status: NoiseSimulationStatus,
    pub simulations: u64,
    /// Alpha the schedule hands out over the family's life.
    pub nominal_alpha_ppm: u64,
    pub tolerance_ppm: u64,
    /// Largest family false-positive count still within tolerance.
    pub limit_false_positives: u64,
    /// Simulations in which any confirmation rejected any null.
    pub family_false_positives: u64,
    pub family_false_positive_rate: Ratio,
    /// `floor(familyFalsePositives * 1e6 / simulations)`.
    pub family_false_positive_rate_ppm: u64,
    /// `floor(sqrt(a * (1e6 - a) / simulations))` with `a` the nominal alpha:
    /// the sampling error of a rate estimated from this many simulations if
    /// the true rate were exactly nominal.
    pub nominal_standard_error_ppm: u64,
    pub confirmations: Vec<NoiseConfirmationResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoiseSimulationError(pub String);

impl std::fmt::Display for NoiseSimulationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NoiseSimulationError {}

fn fail<T>(message: impl Into<String>) -> Result<T, NoiseSimulationError> {
    Err(NoiseSimulationError(message.into()))
}

struct Domain {
    field: &'static str,
    min: u64,
    max: u64,
}

const AUTOCORRELATION: Domain = Domain {
    field: "autocorrelationPpm",
    min: 0,
    max: 999_999,
};
const BARS: Domain = Domain {
    field: "bars",
    min: 2,
    max: NOISE_MAX_BARS,
};
const CANDIDATES: Domain = Domain {
    field: "candidatesPerConfirmation",
    min: 1,
    max: NOISE_MAX_CANDIDATES,
};
const PRIOR_TRIALS: Domain = Domain {
    field: "priorTrials",
    min: 0,
    max: PRECISION_MAX_COUNT,
};
const BLOCK_LENGTH: Domain = Domain {
    field: "blockLength",
    min: 1,
    max: PRECISION_MAX_COUNT,
};
const BOOTSTRAP_SAMPLES: Domain = Domain {
    field: "bootstrapSamples",
    min: 1,
    max: PRECISION_MAX_COUNT,
};
const SIMULATIONS: Domain = Domain {
    field: "simulations",
    min: 1,
    max: NOISE_MAX_SIMULATIONS,
};
const SEED: Domain = Domain {
    field: "seed",
    min: 0,
    max: PRECISION_MAX_COUNT,
};
const TOLERANCE: Domain = Domain {
    field: "tolerancePpm",
    min: 0,
    max: 999_999,
};

impl Domain {
    fn check(&self, value: u64) -> Result<u64, NoiseSimulationError> {
        if (self.min..=self.max).contains(&value) {
            Ok(value)
        } else {
            fail(format!(
                "simulation.{}: must be an integer in [{}, {}]",
                self.field, self.min, self.max
            ))
        }
    }

    fn read(&self, object: &Map<String, Value>) -> Result<u64, NoiseSimulationError> {
        let Some(value) = object.get(self.field) else {
            return fail(format!("simulation.{}: required", self.field));
        };
        // `as_u64` is `None` for negatives and for any float literal.
        self.check(value.as_u64().unwrap_or(u64::MAX))
    }
}

fn read_literal(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), NoiseSimulationError> {
    match object.get(field) {
        None => fail(format!("simulation.{field}: required")),
        Some(Value::String(text)) if text == expected => Ok(()),
        Some(_) => fail(format!("simulation.{field}: must be \"{expected}\"")),
    }
}

/// The family test count at the family's `confirmation_number`-th
/// confirmation: every prior trial and every candidate confirmed so far.
fn family_tests_at(declaration: &NoiseSimulationDeclaration, confirmation_number: u64) -> u128 {
    (u128::from(declaration.prior_trials)
        + u128::from(confirmation_number) * u128::from(declaration.candidates_per_confirmation))
        * u128::from(CONFIRMATION_TESTS_PER_TRIAL)
}

/// Re-checks a declaration against the contract domain; every public entry
/// point goes through it, so a directly constructed declaration cannot run
/// outside it. Returns the alpha the schedule hands out.
fn validated(declaration: &NoiseSimulationDeclaration) -> Result<u64, NoiseSimulationError> {
    AUTOCORRELATION.check(declaration.autocorrelation_ppm)?;
    BARS.check(declaration.bars)?;
    CANDIDATES.check(declaration.candidates_per_confirmation)?;
    PRIOR_TRIALS.check(declaration.prior_trials)?;
    BLOCK_LENGTH.check(declaration.block_length)?;
    BOOTSTRAP_SAMPLES.check(declaration.bootstrap_samples)?;
    SIMULATIONS.check(declaration.simulations)?;
    SEED.check(declaration.seed)?;
    TOLERANCE.check(declaration.tolerance_ppm)?;
    alpha_allocation_id(&declaration.allocation)
        .map_err(|error| NoiseSimulationError(format!("simulation.{error}")))?;
    // The confirmation contract's own block-length rule, stated here so an
    // impossible plan is refused before any simulation runs.
    let block = u128::from(declaration.block_length);
    if block * block > u128::from(declaration.bars) {
        return fail(format!(
            "simulation.blockLength: {} squared exceeds bars {}",
            declaration.block_length, declaration.bars
        ));
    }
    let confirmations = declaration.allocation.schedule.len() as u64;
    if family_tests_at(declaration, confirmations) > u128::from(PRECISION_MAX_COUNT) {
        return fail(format!(
            "simulation: family tests at confirmation {confirmations} exceed {PRECISION_MAX_COUNT}"
        ));
    }
    Ok(declaration.allocation.schedule.iter().sum())
}

/// Strictly parses a declaration. Rejection order: not an object, unknown
/// fields (sorted), [`FIELDS`] in order (the nested allocation by its own
/// contract's order), then the block-length rule and the family-count bound.
pub fn parse_noise_simulation(
    raw: &Value,
) -> Result<NoiseSimulationDeclaration, NoiseSimulationError> {
    let Some(object) = raw.as_object() else {
        return fail("simulation: must be an object");
    };
    let mut unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !FIELDS.contains(key))
        .collect();
    unknown.sort_unstable();
    if let Some(first) = unknown.first() {
        return fail(format!("simulation.{first}: unknown field"));
    }
    read_literal(object, "contractVersion", NOISE_SIMULATION_VERSION)?;
    read_literal(
        object,
        "confirmationContract",
        CONFIRMATION_STATISTICS_VERSION,
    )?;
    read_literal(object, "noiseModel", NOISE_MODEL_AR1)?;
    let autocorrelation_ppm = AUTOCORRELATION.read(object)?;
    let bars = BARS.read(object)?;
    let candidates_per_confirmation = CANDIDATES.read(object)?;
    let prior_trials = PRIOR_TRIALS.read(object)?;
    let block_length = BLOCK_LENGTH.read(object)?;
    let bootstrap_samples = BOOTSTRAP_SAMPLES.read(object)?;
    let simulations = SIMULATIONS.read(object)?;
    let seed = SEED.read(object)?;
    let tolerance_ppm = TOLERANCE.read(object)?;
    let allocation = match object.get("allocation") {
        None => return fail("simulation.allocation: required"),
        Some(raw) => parse_alpha_allocation(raw)
            .map_err(|error| NoiseSimulationError(format!("simulation.{error}")))?,
    };
    let declaration = NoiseSimulationDeclaration {
        autocorrelation_ppm,
        bars,
        candidates_per_confirmation,
        prior_trials,
        block_length,
        bootstrap_samples,
        simulations,
        seed,
        tolerance_ppm,
        allocation,
    };
    validated(&declaration)?;
    Ok(declaration)
}

/// A stream keyed by a list of small integers: each part is folded in the
/// same way `research-confirmation-statistics-v1` keys a candidate's stream.
fn stream(seed: u64, parts: &[u64]) -> SplitMix64 {
    let state = parts.iter().fold(seed, |state, part| {
        mix64(state ^ mix64(part.wrapping_add(1)))
    });
    SplitMix64::new(state)
}

/// Uniform in `[0, 1)` from the top 53 bits: an exact `f64`.
fn uniform(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 * UNIFORM_SCALE
}

/// One zero-mean AR(1) series: `x = phi * x_prev + e`, with `e` the sum of
/// four uniforms minus two. The warm-up bars are generated and dropped.
fn noise_series(rng: &mut SplitMix64, phi: f64, bars: usize) -> Vec<f64> {
    let mut series = Vec::with_capacity(bars);
    let mut value = 0.0f64;
    for bar in 0..NOISE_WARMUP_BARS + bars {
        let innovation = uniform(rng) + uniform(rng) + uniform(rng) + uniform(rng) - 2.0;
        value = phi * value + innovation;
        if bar >= NOISE_WARMUP_BARS {
            series.push(value);
        }
    }
    series
}

/// `floor(sqrt(a * (1e6 - a) / simulations))` in ppm, in exact integers.
fn nominal_standard_error_ppm(nominal_alpha_ppm: u64, simulations: u64) -> u64 {
    let alpha = u128::from(nominal_alpha_ppm);
    (alpha * (PPM - alpha) / u128::from(simulations)).isqrt() as u64
}

/// Runs the declared simulation. Deterministic: the same declaration always
/// gives the same report.
pub fn simulate_noise(
    declaration: &NoiseSimulationDeclaration,
) -> Result<NoiseSimulationReport, NoiseSimulationError> {
    let nominal_alpha_ppm = validated(declaration)?;
    let allocation_id = alpha_allocation_id(&declaration.allocation)
        .map_err(|error| NoiseSimulationError(format!("simulation.{error}")))?;
    let schedule = &declaration.allocation.schedule;
    let candidates = declaration.candidates_per_confirmation;
    let bars = declaration.bars as usize;
    let phi = declaration.autocorrelation_ppm as f64 / 1_000_000.0;

    let mut confirmations: Vec<NoiseConfirmationResult> = Vec::with_capacity(schedule.len());
    let mut family_false_positives = 0u64;
    for simulation in 0..declaration.simulations {
        let mut reserved: Vec<u64> = Vec::with_capacity(schedule.len());
        let mut family_rejected = false;
        for (position, _) in schedule.iter().enumerate() {
            let confirmation_number = position as u64 + 1;
            // The production allocator, fed the family's own history.
            let alpha_ppm = allocate_confirmation_alpha(&declaration.allocation, &reserved)
                .map_err(|error| NoiseSimulationError(format!("simulation.{error}")))?
                .alpha_ppm
                .ok_or_else(|| {
                    NoiseSimulationError("simulation: the schedule ran out early".into())
                })?;
            reserved.push(alpha_ppm);
            let family_tests = family_tests_at(declaration, confirmation_number) as u64;
            if simulation == 0 {
                confirmations.push(NoiseConfirmationResult {
                    confirmation_number,
                    alpha_ppm,
                    family_tests,
                    false_positives: 0,
                    false_positive_rate_ppm: 0,
                    net_return_rejections: 0,
                    benchmark_excess_rejections: 0,
                });
            }

            let mut series: Vec<(u64, Vec<f64>, Vec<f64>)> =
                Vec::with_capacity(candidates as usize);
            for candidate in 0..candidates {
                let data = |role: u64| {
                    let mut rng = stream(
                        declaration.seed,
                        &[
                            DOMAIN_DATA,
                            simulation,
                            confirmation_number,
                            candidate,
                            role,
                        ],
                    );
                    noise_series(&mut rng, phi, bars)
                };
                series.push((
                    position as u64 * candidates + candidate,
                    data(ROLE_RETURNS),
                    data(ROLE_BENCHMARK),
                ));
            }
            let inputs: Vec<CandidateSeries<'_>> = series
                .iter()
                .map(|(index, returns, benchmark)| CandidateSeries {
                    candidate_index: *index,
                    returns,
                    benchmark_returns: benchmark,
                })
                .collect();
            let confirmation = ConfirmationDeclaration {
                alpha_ppm,
                block_length: declaration.block_length,
                bootstrap_samples: declaration.bootstrap_samples,
                seed: stream(
                    declaration.seed,
                    &[DOMAIN_CONFIRMATION_SEED, simulation, confirmation_number],
                )
                .next_u64()
                    >> 11,
                block_length_rationale: SIMULATED_RATIONALE.into(),
            };
            // The production confirmation statistics, unchanged.
            let report = evaluate_confirmation(&confirmation, family_tests, &inputs)
                .map_err(|error| NoiseSimulationError(format!("simulation.{error}")))?;

            let result = &mut confirmations[position];
            let mut rejected = false;
            for test in report.tests.iter().filter(|test| test.rejects_null) {
                rejected = true;
                match test.test {
                    ConfirmationTest::NetReturn => result.net_return_rejections += 1,
                    ConfirmationTest::BenchmarkExcess => result.benchmark_excess_rejections += 1,
                }
            }
            if rejected {
                result.false_positives += 1;
                family_rejected = true;
            }
        }
        if family_rejected {
            family_false_positives += 1;
        }
    }

    let simulations = declaration.simulations;
    let rate_ppm = |count: u64| (u128::from(count) * PPM / u128::from(simulations)) as u64;
    for result in &mut confirmations {
        result.false_positive_rate_ppm = rate_ppm(result.false_positives);
    }
    let allowed_ppm = u128::from(nominal_alpha_ppm) + u128::from(declaration.tolerance_ppm);
    // observed / simulations <= (nominal + tolerance) / 1e6, in exact integers.
    let within = u128::from(family_false_positives) * PPM <= u128::from(simulations) * allowed_ppm;
    Ok(NoiseSimulationReport {
        contract_version: NOISE_SIMULATION_VERSION,
        confirmation_contract: CONFIRMATION_STATISTICS_VERSION,
        allocation_id,
        status: if within {
            NoiseSimulationStatus::WithinTolerance
        } else {
            NoiseSimulationStatus::ExceedsTolerance
        },
        simulations,
        nominal_alpha_ppm,
        tolerance_ppm: declaration.tolerance_ppm,
        limit_false_positives: (u128::from(simulations) * allowed_ppm / PPM)
            .min(u128::from(simulations)) as u64,
        family_false_positives,
        family_false_positive_rate: Ratio {
            numerator: family_false_positives,
            denominator: simulations,
        },
        family_false_positive_rate_ppm: rate_ppm(family_false_positives),
        nominal_standard_error_ppm: nominal_standard_error_ppm(nominal_alpha_ppm, simulations),
        confirmations,
    })
}

#[cfg(test)]
mod tests;
