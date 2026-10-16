//! Verify existing integer decision evidence, never run a bootstrap or change
//! the frozen statistic. Seal authenticity belongs to the future runtime host.
use super::*;
use crate::discovery_core::alpha_reservations::AlphaReservation;

pub struct SealedValidationInput<'a> {
    pub receipt_id: &'a str,
    pub reservation: &'a AlphaReservation,
    pub report_json: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Report {
    contract_version: String,
    correction: String,
    scheme: String,
    prng: String,
    alpha_ppm: u64,
    block_length: u64,
    bootstrap_samples: u64,
    seed: u64,
    family_tests: u64,
    tests: Vec<TestRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TestKind {
    NetReturn,
    BenchmarkExcess,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ratio {
    numerator: u64,
    denominator: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TestRow {
    candidate_index: u64,
    test: TestKind,
    observations: u64,
    observed_mean: f64,
    extreme_count: u64,
    raw_p: Ratio,
    raw_p_value: f64,
    holm_rank: u64,
    adjusted_p: Ratio,
    adjusted_p_value: f64,
    rejects_null: bool,
}

pub(crate) fn verify_report_shape(raw: &str) -> Result<(), ContractError> {
    let report: Report = read(raw)?;
    require(
        report.contract_version == CONFIRMATION_STATISTICS_V2_VERSION
            && report.correction == "holm"
            && report.scheme == "circular-block"
            && report.prng == "splitmix64",
        "unsupported report blob",
    )
}

fn checked_report(
    parent: &FrozenBatch,
    reservation: &AlphaReservation,
    report_json: &str,
    expected_family_tests: Option<u64>,
) -> Result<Report, ContractError> {
    let report: Report = read(report_json)?;
    let s = &parent.body.statistics;
    let settings = if parent.is_validation() {
        &s.validation
    } else {
        &s.test
    };
    require(
        reservation.family_id() == parent.body.source.family_id
            && reservation.allocation_id() == s.allocation_id
            && reservation.confirmation_batch_id() == parent.id,
        "confirmation reservation binding differs",
    )?;
    require(
        report.contract_version == CONFIRMATION_STATISTICS_V2_VERSION
            && report.correction == "holm"
            && report.scheme == "circular-block"
            && report.prng == "splitmix64",
        "unsupported sealed report contract",
    )?;
    require(
        report.alpha_ppm == reservation.alpha_ppm()
            && report.block_length == settings.block_length
            && report.bootstrap_samples == s.bootstrap_samples
            && report.seed == settings.seed,
        "sealed report settings differ from frozen batch/reservation",
    )?;
    count(
        report.family_tests,
        parent.body.candidates.len() as u64 * 2,
        "familyTests",
    )?;
    require(
        report.family_tests.is_multiple_of(2),
        "familyTests must be even",
    )?;
    if let Some(expected) = expected_family_tests {
        require(
            report.family_tests == expected,
            "report differs from admission whole-family m",
        )?;
    }
    require(
        report.tests.len() == parent.body.candidates.len() * 2,
        "sealed report does not contain every candidate/test",
    )?;
    let (validation_bars, test_bars) = stage_bars(&parent.body)?;
    let bars = if parent.is_validation() {
        validation_bars
    } else {
        test_bars
    };
    let denominator = s.bootstrap_samples + 1;
    for (i, row) in report.tests.iter().enumerate() {
        let expected = &parent.body.candidates[i / 2];
        let kind = if i % 2 == 0 {
            TestKind::NetReturn
        } else {
            TestKind::BenchmarkExcess
        };
        require(
            row.candidate_index == expected.candidate_index
                && row.test == kind
                && row.observations == bars,
            "sealed report candidate/test/observations differ",
        )?;
        require(
            row.extreme_count <= s.bootstrap_samples
                && row.raw_p.numerator == row.extreme_count + 1
                && row.raw_p.denominator == denominator,
            "sealed report raw ratio differs",
        )?;
        // Existing display floats remain original bytes and never drive the rule.
        require(
            row.observed_mean.is_finite()
                && row.raw_p_value.is_finite()
                && (0.0..=1.0).contains(&row.raw_p_value)
                && row.adjusted_p_value.is_finite()
                && (0.0..=1.0).contains(&row.adjusted_p_value),
            "non-finite/out-of-domain display field",
        )?;
    }
    let mut order: Vec<usize> = (0..report.tests.len()).collect();
    order.sort_by_key(|&i| {
        let row = &report.tests[i];
        (row.extreme_count, row.candidate_index, row.test)
    });
    let mut running = 0u128;
    for (position, i) in order.into_iter().enumerate() {
        let row = &report.tests[i];
        let rank = position as u64 + 1;
        let numerator = (u128::from(report.family_tests - rank + 1)
            * u128::from(row.extreme_count + 1))
        .min(u128::from(denominator));
        running = running.max(numerator);
        let rejects = running * 1_000_000 <= u128::from(report.alpha_ppm) * u128::from(denominator);
        require(
            row.holm_rank == rank
                && row.adjusted_p.denominator == denominator
                && u128::from(row.adjusted_p.numerator) == running
                && row.rejects_null == rejects,
            "sealed report Holm decision does not reproduce",
        )?;
    }
    Ok(report)
}

/// Check original v2 report decisions for either frozen stage, without running
/// a bootstrap or granting execution/release rights. The admission supplies m.
pub fn verify_confirmation_report(
    report_json: &str,
    batch: &FrozenBatch,
    reservation: &AlphaReservation,
    family_tests: u64,
) -> Result<(), ContractError> {
    checked_report(batch, reservation, report_json, Some(family_tests))?;
    Ok(())
}

fn reproduce_selection(
    parent: &FrozenBatch,
    input: &SealedValidationInput<'_>,
) -> Result<Vec<u64>, ContractError> {
    let report = checked_report(parent, input.reservation, input.report_json, None)?;
    Ok(report
        .tests
        .chunks_exact(2)
        .filter(|pair| pair.iter().all(|row| row.rejects_null))
        .map(|pair| pair[0].candidate_index)
        .take(parent.body.selection_rule.max_candidates as usize)
        .collect())
}

fn checked(
    body: SelectionBody,
    parent: &FrozenBatch,
    input: &SealedValidationInput<'_>,
) -> Result<FrozenSelection, ContractError> {
    require(
        body.contract_version == SELECTION_VERSION && parent.is_validation(),
        "selection requires version v1 and Validation parent",
    )?;
    for (name, value) in [
        ("validationBatchId", &body.validation_batch_id),
        ("alphaReservationId", &body.alpha_reservation_id),
        ("sealedReceiptId", &body.sealed_receipt_id),
        ("reportSha256", &body.report_sha256),
    ] {
        hash_field(value, name)?;
    }
    hash_field(input.receipt_id, "supplied seal receipt ID")?;
    require(
        body.selected_candidates.len() <= MAX_CANDIDATES,
        "selection exceeds 4096 candidates",
    )?;
    require(
        body.selected_candidates.windows(2).all(|p| p[0] < p[1]),
        "selected candidates must be sorted and unique",
    )?;
    for &index in &body.selected_candidates {
        count(index, 0, "selected candidate index")?;
    }
    require(
        body.validation_batch_id == parent.id
            && body.alpha_reservation_id == input.reservation.reservation_id()
            && body.sealed_receipt_id == input.receipt_id,
        "selection parent/reservation/seal differs",
    )?;
    require(
        input.report_json.len() <= MAX_DOCUMENT_BYTES,
        "report exceeds 4 MiB",
    )?;
    require(
        body.report_sha256 == hex::encode(Sha256::digest(input.report_json.as_bytes())),
        "original report hash differs",
    )?;
    require(
        body.selected_candidates == reproduce_selection(parent, input)?,
        "selection does not reproduce frozen rule",
    )?;
    Ok(FrozenSelection {
        id: identity(SELECTION_VERSION, &body)?,
        body,
    })
}

pub fn freeze_selection(
    raw: &str,
    parent: &FrozenBatch,
    input: &SealedValidationInput<'_>,
) -> Result<FrozenSelection, ContractError> {
    checked(read(raw)?, parent, input)
}

pub fn parse_selection(
    raw: &str,
    parent: &FrozenBatch,
    input: &SealedValidationInput<'_>,
) -> Result<FrozenSelection, ContractError> {
    let stored: Stored<SelectionBody> = read(raw)?;
    let result = checked(stored.body, parent, input)?;
    require(stored.id == result.id, "selection ID does not reproduce")?;
    Ok(result)
}
