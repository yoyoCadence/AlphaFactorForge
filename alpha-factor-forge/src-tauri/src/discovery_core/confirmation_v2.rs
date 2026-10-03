//! `research-confirmation-statistics-v2`: the method the recalibration plan
//! selected (docs/research-confirmation-statistics-v2.md;
//! docs/plans/confirmation-recalibration-plan-v1.md §6, P12e-6b), frozen
//! before its final acceptance (§7).
//!
//! It is the draft candidate S2 — v1's circular block bootstrap with the
//! centred deviation stretched by a flat-top over Bartlett long-run variance
//! ratio — with block rule R3 fixed: the declared block length must be
//! `round(n^(1/3))` for every series (maintainer decision 2026-10-03). The
//! computation is S2's own code path
//! (`confirmation_candidates::bootstrap_s2`), so the frozen method is the one
//! the diagnostics measured.
//!
//! NOT YET ACCEPTED: until the plan's final acceptance passes, nothing may
//! describe it as controlling its false-positive rate, and no runtime path
//! calls it. Pure: no Tauri, rusqlite, threads, events, UI, clock or IO.

use serde_json::Value;

use super::confirmation::{
    evaluate_with, fail, parse_declaration_as, CandidateSeries, ConfirmationDeclaration,
    ConfirmationError, ConfirmationReport, Observed,
};
use super::confirmation_candidates::bootstrap_s2;

pub const CONFIRMATION_STATISTICS_V2_VERSION: &str = "research-confirmation-statistics-v2";

/// Block rule R3, `round(n^(1/3))` for `n >= 1`, in exact integers: the
/// smallest `L >= 1` with `(2L + 1)^3 > 8n`. There is no tie to break: an odd
/// cube is never `8n`.
pub fn r3_block_length(n: u64) -> u64 {
    let eight_n = 8 * u128::from(n);
    let above = |length: u64| {
        let odd = 2 * u128::from(length) + 1;
        odd * odd * odd > eight_n
    };
    // (2^22)^3 = 2^66 > 2^64 > n, so the answer is below 2^22.
    let (mut low, mut high) = (1u64, 1u64 << 22);
    while low < high {
        let middle = low + (high - low) / 2;
        if above(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// Strictly parses a v2 declaration: v1's fields, rules and rejection order,
/// with `contractVersion` `research-confirmation-statistics-v2`.
pub fn parse_confirmation_declaration_v2(
    raw: &Value,
) -> Result<ConfirmationDeclaration, ConfirmationError> {
    parse_declaration_as(raw, CONFIRMATION_STATISTICS_V2_VERSION)
}

/// S2 on one candidate, after the block rule. v1's own input checks (equal
/// lengths, at least two bars) keep their errors and come first.
fn bootstrap_v2(
    declaration: &ConfirmationDeclaration,
    candidate: &CandidateSeries<'_>,
) -> Result<[Observed; 2], ConfirmationError> {
    let n = candidate.returns.len();
    if n == candidate.benchmark_returns.len() && n >= 2 {
        let required = r3_block_length(n as u64);
        if declaration.block_length != required {
            return fail(format!(
                "confirmation: {CONFIRMATION_STATISTICS_V2_VERSION} requires blockLength {required} (round of the cube root of candidate {}'s {n} bars), not {}",
                candidate.candidate_index, declaration.block_length
            ));
        }
    }
    bootstrap_s2(declaration, candidate)
}

/// The frozen statistic. Same inputs and report as `evaluate_confirmation`;
/// every series must have `blockLength = round(n^(1/3))` and
/// `(2 · blockLength)^2 <= n`.
pub fn evaluate_confirmation_v2(
    declaration: &ConfirmationDeclaration,
    family_tests: u64,
    candidates: &[CandidateSeries<'_>],
) -> Result<ConfirmationReport, ConfirmationError> {
    evaluate_with(
        CONFIRMATION_STATISTICS_V2_VERSION,
        declaration,
        family_tests,
        candidates,
        bootstrap_v2,
    )
}

#[cfg(test)]
mod tests;
