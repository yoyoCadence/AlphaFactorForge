//! `research-noise-simulation-v2`: the engine the recalibration plan runs on
//! (docs/research-noise-simulation-v2.md, docs/plans/confirmation-recalibration-plan-v1.md).
//!
//! Same seeded noise as v1 and the same simulated family life, with what the
//! plan needs on top: a selectable confirmation statistic, a rate check for
//! every confirmation AND the family (a point screen, or the 95% Wilson upper
//! bound in exact integers), a shifted scenario for power, and checkpoints
//! that let a short prefix of a long run be re-computed.
//!
//! v1 (`noise_simulation`) and its two declared acceptance runs are
//! unchanged; this module only reads its noise generator.
//!
//! Pure: no Tauri, rusqlite, threads, events, UI, clock or IO, and not called
//! by any runtime path. A report is a measurement of a test procedure on
//! synthetic data; it is never a confirmation `PASS`.

use serde::Serialize;
use serde_json::{Map, Value};

use super::alpha_allocation::{
    allocate_confirmation_alpha, alpha_allocation_id, parse_alpha_allocation,
    AlphaAllocationDeclaration,
};
use super::confirmation::{
    evaluate_confirmation, CandidateSeries, ConfirmationDeclaration, ConfirmationError,
    ConfirmationReport, ConfirmationTest, CONFIRMATION_STATISTICS_VERSION,
    CONFIRMATION_TESTS_PER_TRIAL,
};
use super::confirmation_candidates::{
    evaluate_confirmation_candidate_s1, evaluate_confirmation_candidate_s2,
    CONFIRMATION_CANDIDATE_S1, CONFIRMATION_CANDIDATE_S2,
};
use super::noise_simulation::{
    fail, noise_series, read_literal, stream, Domain, NoiseSimulationError, AUTOCORRELATION, BARS,
    BLOCK_LENGTH, CANDIDATES, DOMAIN_CONFIRMATION_SEED, DOMAIN_DATA, NOISE_MODEL_AR1, PRIOR_TRIALS,
    ROLE_BENCHMARK, ROLE_RETURNS, SEED, SIMULATIONS,
};
use super::precision::PRECISION_MAX_COUNT;

pub const NOISE_SIMULATION_V2_VERSION: &str = "research-noise-simulation-v2";
pub const NOISE_V2_MAX_BOOTSTRAP_SAMPLES: u64 = 1_000_000;
/// Largest shift of the power scenario, in millionths of a return unit.
pub const NOISE_V2_MAX_EFFECT_MILLIONTHS: u64 = 1_000_000_000;
pub const NOISE_V2_MAX_LIMIT_MULTIPLIER_PPM: u64 = 10_000_000;
pub const NOISE_V2_MAX_CHECKPOINTS: usize = 16;

const PPM: u128 = 1_000_000;
/// The Wilson interval's `z = 1.96 = 49/25`, two-sided 95%: `z² = 2401/625`.
/// Written in the decision as the plan writes it, `38416 / 10000`.
const Z_SQUARED_NUMERATOR: u128 = 38_416;
const Z_SQUARED_DENOMINATOR: u128 = 10_000;
/// Not read by the calculation; a confirmation declaration requires one.
const SIMULATED_RATIONALE: &str = "declared by research-noise-simulation-v2";

const FIELDS: [&str; 15] = [
    "contractVersion",
    "statistic",
    "noiseModel",
    "autocorrelationPpm",
    "effectMillionths",
    "bars",
    "candidatesPerConfirmation",
    "priorTrials",
    "blockLength",
    "bootstrapSamples",
    "simulations",
    "seed",
    "check",
    "checkpoints",
    "allocation",
];
const CHECK_FIELDS: [&str; 2] = ["rule", "limitMultiplierPpm"];

const EFFECT: Domain = Domain {
    field: "effectMillionths",
    min: 0,
    max: NOISE_V2_MAX_EFFECT_MILLIONTHS,
};
const BOOTSTRAP_SAMPLES: Domain = Domain {
    field: "bootstrapSamples",
    min: 1,
    max: NOISE_V2_MAX_BOOTSTRAP_SAMPLES,
};
const LIMIT_MULTIPLIER: Domain = Domain {
    field: "check.limitMultiplierPpm",
    min: 1_000_000,
    max: NOISE_V2_MAX_LIMIT_MULTIPLIER_PPM,
};

/// The confirmation statistic a simulation exercises. Candidates of the
/// recalibration plan are added here, each with its own contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SimulatedStatistic {
    /// `research-confirmation-statistics-v1`, the plan's baseline V1.
    #[serde(rename = "research-confirmation-statistics-v1")]
    ConfirmationV1,
    /// DRAFT candidate S1, the block-variance studentized bootstrap.
    #[serde(rename = "research-confirmation-candidate-s1-v1")]
    CandidateS1,
    /// DRAFT candidate S2, the flat-top variance-corrected centred bootstrap.
    #[serde(rename = "research-confirmation-candidate-s2-v1")]
    CandidateS2,
}

impl SimulatedStatistic {
    const ALL: [Self; 3] = [Self::ConfirmationV1, Self::CandidateS1, Self::CandidateS2];

    pub fn contract(self) -> &'static str {
        match self {
            Self::ConfirmationV1 => CONFIRMATION_STATISTICS_VERSION,
            Self::CandidateS1 => CONFIRMATION_CANDIDATE_S1,
            Self::CandidateS2 => CONFIRMATION_CANDIDATE_S2,
        }
    }

    /// The widest block the statistic reads, in block lengths: S2 also uses
    /// blocks of `2L`.
    fn widest_block(self) -> u128 {
        match self {
            Self::ConfirmationV1 | Self::CandidateS1 => 1,
            Self::CandidateS2 => 2,
        }
    }

    fn evaluate(
        self,
        declaration: &ConfirmationDeclaration,
        family_tests: u64,
        candidates: &[CandidateSeries<'_>],
    ) -> Result<ConfirmationReport, ConfirmationError> {
        match self {
            Self::ConfirmationV1 => evaluate_confirmation(declaration, family_tests, candidates),
            Self::CandidateS1 => {
                evaluate_confirmation_candidate_s1(declaration, family_tests, candidates)
            }
            Self::CandidateS2 => {
                evaluate_confirmation_candidate_s2(declaration, family_tests, candidates)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoiseCheckRule {
    /// Counts only; required for the shifted (power) scenario.
    None,
    /// `count / simulations <= limit`: the plan's diagnostic screen.
    PointScreen,
    /// The upper end of the two-sided 95% Wilson interval is at most the
    /// limit: the plan's acceptance rule.
    WilsonUpperBound,
}

impl NoiseCheckRule {
    fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PointScreen => "point-screen",
            Self::WilsonUpperBound => "wilson-upper-bound",
        }
    }
}

/// How the rate of every confirmation and of the family is judged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoiseCheck {
    pub rule: NoiseCheckRule,
    /// `limit = nominal × multiplier / 1e6`; `None` exactly when the rule is
    /// [`NoiseCheckRule::None`].
    pub limit_multiplier_ppm: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoiseSimulationV2Declaration {
    pub statistic: SimulatedStatistic,
    /// AR(1) coefficient, ppm, `[0, 900_000]` as in v1.
    pub autocorrelation_ppm: u64,
    /// Added to every bar of the strategy's returns, in millionths. `0` is the
    /// null scenario, where every tested hypothesis is true.
    pub effect_millionths: u64,
    pub bars: u64,
    pub candidates_per_confirmation: u64,
    pub prior_trials: u64,
    pub block_length: u64,
    pub bootstrap_samples: u64,
    pub simulations: u64,
    pub seed: u64,
    pub check: NoiseCheck,
    /// Simulation counts, strictly increasing and below `simulations`, at
    /// which the running counts are recorded.
    pub checkpoints: Vec<u64>,
    pub allocation: AlphaAllocationDeclaration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoiseSimulationV2Status {
    /// No rule was declared; the report only counts.
    Measured,
    /// Every confirmation and the family are within their limits.
    WithinLimits,
    ExceedsLimits,
}

/// One observed rate against its nominal value and, if a rule is declared,
/// its limit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseRateCheck {
    pub nominal_ppm: u64,
    pub limit_ppm: Option<u64>,
    /// Simulations in which the event occurred.
    pub count: u64,
    /// `floor(count × 1e6 / simulations)`.
    pub rate_ppm: u64,
    /// The two-sided 95% Wilson interval, rounded outwards to whole ppm.
    pub wilson_lower_ppm: u64,
    pub wilson_upper_ppm: u64,
    pub within_limit: Option<bool>,
}

/// Running counts of one confirmation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseConfirmationCounts {
    /// Simulations in which it rejected at least one null.
    pub rejecting_simulations: u64,
    pub net_return_rejecting_simulations: u64,
    pub benchmark_excess_rejecting_simulations: u64,
    /// Sum of the bootstrap extreme counts of every test, by test: a digest
    /// that changes with any change to the data, the resampling or the
    /// statistic, even when nothing is rejected.
    pub net_return_extreme_total: u64,
    pub benchmark_excess_extreme_total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseConfirmationV2Result {
    pub confirmation_number: u64,
    pub alpha_ppm: u64,
    pub family_tests: u64,
    /// `rejectingSimulations` against the confirmation's own share.
    pub check: NoiseRateCheck,
    pub counts: NoiseConfirmationCounts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseCheckpoint {
    /// The first this-many simulations.
    pub simulations: u64,
    pub family_rejecting_simulations: u64,
    pub confirmations: Vec<NoiseConfirmationCounts>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseSimulationV2Report {
    pub contract_version: &'static str,
    pub statistic: SimulatedStatistic,
    pub allocation_id: String,
    pub status: NoiseSimulationV2Status,
    pub simulations: u64,
    pub effect_millionths: u64,
    pub check_rule: NoiseCheckRule,
    pub limit_multiplier_ppm: Option<u64>,
    /// Simulations in which any confirmation rejected, against the schedule's
    /// total alpha.
    pub family: NoiseRateCheck,
    pub confirmations: Vec<NoiseConfirmationV2Result>,
    pub checkpoints: Vec<NoiseCheckpoint>,
}

fn family_tests_at(declaration: &NoiseSimulationV2Declaration, confirmation_number: u64) -> u128 {
    (u128::from(declaration.prior_trials)
        + u128::from(confirmation_number) * u128::from(declaration.candidates_per_confirmation))
        * u128::from(CONFIRMATION_TESTS_PER_TRIAL)
}

fn wrap(error: impl std::fmt::Display) -> NoiseSimulationError {
    NoiseSimulationError(format!("simulation.{error}"))
}

/// The limit of one nominal rate, when it is a whole number of ppm below one.
fn limit_ppm(nominal_ppm: u64, multiplier_ppm: u64) -> Result<u64, NoiseSimulationError> {
    let product = u128::from(nominal_ppm) * u128::from(multiplier_ppm);
    if !product.is_multiple_of(PPM) || product / PPM >= PPM {
        return fail(format!(
            "simulation.check.limitMultiplierPpm: the limit of a nominal {nominal_ppm} ppm must be a whole number of ppm below 1000000"
        ));
    }
    Ok((product / PPM) as u64)
}

/// Re-checks a declaration against the contract domain; every public entry
/// point goes through it. Returns the alpha the schedule hands out.
fn validated(declaration: &NoiseSimulationV2Declaration) -> Result<u64, NoiseSimulationError> {
    AUTOCORRELATION.check(declaration.autocorrelation_ppm)?;
    EFFECT.check(declaration.effect_millionths)?;
    BARS.check(declaration.bars)?;
    CANDIDATES.check(declaration.candidates_per_confirmation)?;
    PRIOR_TRIALS.check(declaration.prior_trials)?;
    BLOCK_LENGTH.check(declaration.block_length)?;
    BOOTSTRAP_SAMPLES.check(declaration.bootstrap_samples)?;
    SIMULATIONS.check(declaration.simulations)?;
    SEED.check(declaration.seed)?;
    match (
        declaration.check.rule,
        declaration.check.limit_multiplier_ppm,
    ) {
        (NoiseCheckRule::None, None) => {}
        (NoiseCheckRule::None, Some(_)) => {
            return fail("simulation.check.limitMultiplierPpm: not allowed with rule \"none\"")
        }
        (_, None) => return fail("simulation.check.limitMultiplierPpm: required"),
        (_, Some(multiplier)) => {
            LIMIT_MULTIPLIER.check(multiplier)?;
        }
    }
    if declaration.checkpoints.len() > NOISE_V2_MAX_CHECKPOINTS {
        return fail(format!(
            "simulation.checkpoints: at most {NOISE_V2_MAX_CHECKPOINTS} entries"
        ));
    }
    let mut previous = 0u64;
    for (index, &checkpoint) in declaration.checkpoints.iter().enumerate() {
        if checkpoint <= previous || checkpoint >= declaration.simulations {
            return fail(format!(
                "simulation.checkpoints[{index}]: must be above the previous entry and below simulations"
            ));
        }
        previous = checkpoint;
    }
    alpha_allocation_id(&declaration.allocation).map_err(wrap)?;

    let widest = declaration.statistic.widest_block();
    let block = u128::from(declaration.block_length) * widest;
    if block * block > u128::from(declaration.bars) {
        return fail(format!(
            "simulation.blockLength: {}{} squared exceeds bars {}",
            if widest == 1 { "" } else { "twice " },
            declaration.block_length,
            declaration.bars
        ));
    }
    let confirmations = declaration.allocation.schedule.len() as u64;
    if family_tests_at(declaration, confirmations) > u128::from(PRECISION_MAX_COUNT) {
        return fail(format!(
            "simulation: family tests at confirmation {confirmations} exceed {PRECISION_MAX_COUNT}"
        ));
    }
    let nominal: u64 = declaration.allocation.schedule.iter().sum();
    if let Some(multiplier) = declaration.check.limit_multiplier_ppm {
        // A rate check needs true nulls: under a shift a rejection is not a
        // false positive.
        if declaration.effect_millionths != 0 {
            return fail("simulation.check.rule: must be \"none\" when effectMillionths is not 0");
        }
        for &share in &declaration.allocation.schedule {
            limit_ppm(share, multiplier)?;
        }
        limit_ppm(nominal, multiplier)?;
    }
    Ok(nominal)
}

fn parse_check(raw: Option<&Value>) -> Result<NoiseCheck, NoiseSimulationError> {
    let Some(raw) = raw else {
        return fail("simulation.check: required");
    };
    let Some(object) = raw.as_object() else {
        return fail("simulation.check: must be an object");
    };
    let mut unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !CHECK_FIELDS.contains(key))
        .collect();
    unknown.sort_unstable();
    if let Some(first) = unknown.first() {
        return fail(format!("simulation.check.{first}: unknown field"));
    }
    let rule = match object.get("rule") {
        None => return fail("simulation.check.rule: required"),
        Some(Value::String(name)) => [
            NoiseCheckRule::None,
            NoiseCheckRule::PointScreen,
            NoiseCheckRule::WilsonUpperBound,
        ]
        .into_iter()
        .find(|rule| rule.name() == name),
        Some(_) => None,
    };
    let Some(rule) = rule else {
        return fail(
            "simulation.check.rule: must be \"none\", \"point-screen\" or \"wilson-upper-bound\"",
        );
    };
    let limit_multiplier_ppm = match (rule, object.get("limitMultiplierPpm")) {
        (NoiseCheckRule::None, None) => None,
        (NoiseCheckRule::None, Some(_)) => {
            return fail("simulation.check.limitMultiplierPpm: not allowed with rule \"none\"")
        }
        (_, None) => return fail("simulation.check.limitMultiplierPpm: required"),
        (_, Some(value)) => Some(LIMIT_MULTIPLIER.check(value.as_u64().unwrap_or(u64::MAX))?),
    };
    Ok(NoiseCheck {
        rule,
        limit_multiplier_ppm,
    })
}

fn parse_checkpoints(
    object: &Map<String, Value>,
    simulations: u64,
) -> Result<Vec<u64>, NoiseSimulationError> {
    let entries = match object.get("checkpoints") {
        None => return fail("simulation.checkpoints: required"),
        Some(Value::Array(entries)) => entries,
        Some(_) => return fail("simulation.checkpoints: must be an array"),
    };
    if entries.len() > NOISE_V2_MAX_CHECKPOINTS {
        return fail(format!(
            "simulation.checkpoints: at most {NOISE_V2_MAX_CHECKPOINTS} entries"
        ));
    }
    let mut checkpoints = Vec::with_capacity(entries.len());
    let mut previous = 0u64;
    for (index, entry) in entries.iter().enumerate() {
        match entry.as_u64() {
            Some(checkpoint) if checkpoint > previous && checkpoint < simulations => {
                checkpoints.push(checkpoint);
                previous = checkpoint;
            }
            _ => {
                return fail(format!(
                    "simulation.checkpoints[{index}]: must be above the previous entry and below simulations"
                ))
            }
        }
    }
    Ok(checkpoints)
}

/// Strictly parses a declaration. Rejection order: not an object, unknown
/// fields (sorted), [`FIELDS`] in order (`check` and `allocation` by their own
/// order), then the cross-field rules of [`validated`].
pub fn parse_noise_simulation_v2(
    raw: &Value,
) -> Result<NoiseSimulationV2Declaration, NoiseSimulationError> {
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
    read_literal(object, "contractVersion", NOISE_SIMULATION_V2_VERSION)?;
    let statistic = match object.get("statistic") {
        None => return fail("simulation.statistic: required"),
        Some(Value::String(name)) => SimulatedStatistic::ALL
            .into_iter()
            .find(|statistic| statistic.contract() == name),
        Some(_) => None,
    };
    let Some(statistic) = statistic else {
        return fail(format!(
            "simulation.statistic: must be one of \"{CONFIRMATION_STATISTICS_VERSION}\", \"{CONFIRMATION_CANDIDATE_S1}\", \"{CONFIRMATION_CANDIDATE_S2}\""
        ));
    };
    read_literal(object, "noiseModel", NOISE_MODEL_AR1)?;
    let autocorrelation_ppm = AUTOCORRELATION.read(object)?;
    let effect_millionths = EFFECT.read(object)?;
    let bars = BARS.read(object)?;
    let candidates_per_confirmation = CANDIDATES.read(object)?;
    let prior_trials = PRIOR_TRIALS.read(object)?;
    let block_length = BLOCK_LENGTH.read(object)?;
    let bootstrap_samples = BOOTSTRAP_SAMPLES.read(object)?;
    let simulations = SIMULATIONS.read(object)?;
    let seed = SEED.read(object)?;
    let check = parse_check(object.get("check"))?;
    let checkpoints = parse_checkpoints(object, simulations)?;
    let allocation = match object.get("allocation") {
        None => return fail("simulation.allocation: required"),
        Some(raw) => parse_alpha_allocation(raw).map_err(wrap)?,
    };
    let declaration = NoiseSimulationV2Declaration {
        statistic,
        autocorrelation_ppm,
        effect_millionths,
        bars,
        candidates_per_confirmation,
        prior_trials,
        block_length,
        bootstrap_samples,
        simulations,
        seed,
        check,
        checkpoints,
        allocation,
    };
    validated(&declaration)?;
    Ok(declaration)
}

/// `ceil(sqrt(value))`.
fn ceil_sqrt(value: u128) -> u128 {
    let floor = value.isqrt();
    if floor * floor == value {
        floor
    } else {
        floor + 1
    }
}

/// The two-sided 95% Wilson interval of `count / simulations`, in ppm and
/// rounded outwards (lower bound down, upper bound up), in exact integers.
/// With `z² = 2401/625`:
/// `bounds = (N(1250x + 2401) ∓ sqrt(2401·N·(2500·x(N − x) + 2401·N))) / (N(1250N + 4802))`.
/// Requires `1 <= simulations <= 1_000_000` and `count <= simulations`.
pub fn wilson_bounds_ppm(count: u64, simulations: u64) -> (u64, u64) {
    let (x, n) = (u128::from(count), u128::from(simulations));
    let centre = n * (1_250 * x + 2_401);
    let radicand = 2_401 * n * (2_500 * x * (n - x) + 2_401 * n);
    let denominator = n * (1_250 * n + 4_802);
    let spread = ceil_sqrt(PPM * PPM * radicand);
    let lower = (PPM * centre).saturating_sub(spread) / denominator;
    let upper = (PPM * centre + spread).div_ceil(denominator);
    (lower as u64, upper.min(PPM) as u64)
}

/// The plan's acceptance rule: the Wilson upper bound of `count / simulations`
/// is at most `limit_ppm`, without a square root:
/// `x·1e6 < l·N` and `(l·N − x·1e6)² · 10000 >= 38416 · l · (1e6 − l) · N`.
pub fn wilson_upper_within(count: u64, simulations: u64, limit_ppm: u64) -> bool {
    let (x, n, l) = (
        u128::from(count),
        u128::from(simulations),
        u128::from(limit_ppm),
    );
    if l >= PPM || x * PPM >= l * n {
        return false;
    }
    let gap = l * n - x * PPM;
    gap * gap * Z_SQUARED_DENOMINATOR >= Z_SQUARED_NUMERATOR * l * (PPM - l) * n
}

fn rate_check(
    nominal_ppm: u64,
    count: u64,
    simulations: u64,
    check: &NoiseCheck,
) -> Result<NoiseRateCheck, NoiseSimulationError> {
    let limit = match check.limit_multiplier_ppm {
        Some(multiplier) => Some(limit_ppm(nominal_ppm, multiplier)?),
        None => None,
    };
    let within_limit = limit.map(|limit| match check.rule {
        NoiseCheckRule::WilsonUpperBound => wilson_upper_within(count, simulations, limit),
        // count / simulations <= limit / 1e6, in exact integers.
        _ => u128::from(count) * PPM <= u128::from(limit) * u128::from(simulations),
    });
    let (wilson_lower_ppm, wilson_upper_ppm) = wilson_bounds_ppm(count, simulations);
    Ok(NoiseRateCheck {
        nominal_ppm,
        limit_ppm: limit,
        count,
        rate_ppm: (u128::from(count) * PPM / u128::from(simulations)) as u64,
        wilson_lower_ppm,
        wilson_upper_ppm,
        within_limit,
    })
}

/// Runs the declared simulation. Deterministic; simulation `s` does not
/// depend on how many simulations follow it, so a run cut at a checkpoint
/// reproduces that checkpoint's counts.
pub fn simulate_noise_v2(
    declaration: &NoiseSimulationV2Declaration,
) -> Result<NoiseSimulationV2Report, NoiseSimulationError> {
    let nominal_alpha_ppm = validated(declaration)?;
    let allocation_id = alpha_allocation_id(&declaration.allocation).map_err(wrap)?;
    let schedule = &declaration.allocation.schedule;
    let candidates = declaration.candidates_per_confirmation;
    let bars = declaration.bars as usize;
    let phi = declaration.autocorrelation_ppm as f64 / 1_000_000.0;
    let shift = declaration.effect_millionths as f64 / 1_000_000.0;

    // The production allocator, fed the family's own history.
    let mut alphas: Vec<u64> = Vec::with_capacity(schedule.len());
    for _ in schedule {
        let alpha = allocate_confirmation_alpha(&declaration.allocation, &alphas)
            .map_err(wrap)?
            .alpha_ppm
            .ok_or_else(|| NoiseSimulationError("simulation: the schedule ran out early".into()))?;
        alphas.push(alpha);
    }

    let mut counts = vec![NoiseConfirmationCounts::default(); schedule.len()];
    let mut family_rejecting = 0u64;
    let mut checkpoints = Vec::with_capacity(declaration.checkpoints.len());
    let mut pending = declaration.checkpoints.iter().copied().peekable();
    for simulation in 0..declaration.simulations {
        let mut family_rejected = false;
        for (position, &alpha_ppm) in alphas.iter().enumerate() {
            let confirmation_number = position as u64 + 1;
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
                let mut returns = data(ROLE_RETURNS);
                // Nothing is added in the null scenario, so it is v1's noise
                // bit for bit.
                if declaration.effect_millionths != 0 {
                    for value in &mut returns {
                        *value += shift;
                    }
                }
                series.push((
                    position as u64 * candidates + candidate,
                    returns,
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
            let report = declaration
                .statistic
                .evaluate(
                    &confirmation,
                    family_tests_at(declaration, confirmation_number) as u64,
                    &inputs,
                )
                .map_err(wrap)?;

            let row = &mut counts[position];
            let (mut net_rejected, mut excess_rejected) = (false, false);
            for test in &report.tests {
                match test.test {
                    ConfirmationTest::NetReturn => {
                        row.net_return_extreme_total += test.extreme_count;
                        net_rejected |= test.rejects_null;
                    }
                    ConfirmationTest::BenchmarkExcess => {
                        row.benchmark_excess_extreme_total += test.extreme_count;
                        excess_rejected |= test.rejects_null;
                    }
                }
            }
            row.net_return_rejecting_simulations += u64::from(net_rejected);
            row.benchmark_excess_rejecting_simulations += u64::from(excess_rejected);
            if net_rejected || excess_rejected {
                row.rejecting_simulations += 1;
                family_rejected = true;
            }
        }
        family_rejecting += u64::from(family_rejected);
        if pending.next_if_eq(&(simulation + 1)).is_some() {
            checkpoints.push(NoiseCheckpoint {
                simulations: simulation + 1,
                family_rejecting_simulations: family_rejecting,
                confirmations: counts.clone(),
            });
        }
    }

    let simulations = declaration.simulations;
    let family = rate_check(
        nominal_alpha_ppm,
        family_rejecting,
        simulations,
        &declaration.check,
    )?;
    let mut confirmations = Vec::with_capacity(schedule.len());
    for (position, (row, &alpha_ppm)) in counts.into_iter().zip(&alphas).enumerate() {
        let confirmation_number = position as u64 + 1;
        confirmations.push(NoiseConfirmationV2Result {
            confirmation_number,
            alpha_ppm,
            family_tests: family_tests_at(declaration, confirmation_number) as u64,
            check: rate_check(
                alpha_ppm,
                row.rejecting_simulations,
                simulations,
                &declaration.check,
            )?,
            counts: row,
        });
    }
    let status = match declaration.check.rule {
        NoiseCheckRule::None => NoiseSimulationV2Status::Measured,
        _ if family.within_limit == Some(true)
            && confirmations
                .iter()
                .all(|row| row.check.within_limit == Some(true)) =>
        {
            NoiseSimulationV2Status::WithinLimits
        }
        _ => NoiseSimulationV2Status::ExceedsLimits,
    };
    Ok(NoiseSimulationV2Report {
        contract_version: NOISE_SIMULATION_V2_VERSION,
        statistic: declaration.statistic,
        allocation_id,
        status,
        simulations,
        effect_millionths: declaration.effect_millionths,
        check_rule: declaration.check.rule,
        limit_multiplier_ppm: declaration.check.limit_multiplier_ppm,
        family,
        confirmations,
        checkpoints,
    })
}

#[cfg(test)]
mod tests;
