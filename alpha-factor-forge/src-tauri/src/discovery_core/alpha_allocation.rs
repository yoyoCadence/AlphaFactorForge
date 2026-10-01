//! `research-alpha-allocation-v1`: the P12e-2 pure cross-batch alpha budget
//! (docs/research-alpha-allocation-v1.md).
//!
//! A trial family declares, before its first confirmation, one total
//! family-wise alpha and how much of it each successive confirmation may use.
//! The family's k-th confirmation gets the k-th entry of that schedule. The
//! entries sum to at most the total, so the chance of any false rejection
//! across every confirmation the family ever runs stays within the total
//! (union bound), provided each confirmation controls its own family-wise
//! error at its share — which `research-confirmation-statistics-v1` does.
//!
//! A reserved share is spent whether or not the confirmation rejected
//! anything or even finished. Nothing is carried over, recycled or rounded
//! up, and a family whose schedule is used up gets no further alpha.
//!
//! All arithmetic is integer parts-per-million. Pure: no Tauri, rusqlite,
//! threads, events, UI, clock or IO, and not called by any runtime path. It
//! cannot see the registry: the caller must pass every confirmation the
//! family has already reserved (P13 owns reserving and storing them), and an
//! allocation is never a confirmation `PASS`.

use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use super::identity::canonical_bytes;

pub const ALPHA_ALLOCATION_VERSION: &str = "research-alpha-allocation-v1";
pub const ALPHA_ALLOCATION_RULE: &str = "declared-schedule";
/// The budget belongs to one trial family (one instrument), like the trial
/// count: a new workspace or campaign does not get a new one.
pub const ALPHA_ALLOCATION_SCOPE: &str = "trial-family";
/// Upper end of the alpha domain shared with `research-precision-v1` and
/// `research-confirmation-statistics-v1`.
pub const ALPHA_PPM_MAX: u64 = 999_999;

const FIELDS: [&str; 5] = [
    "contractVersion",
    "rule",
    "scope",
    "totalAlphaPpm",
    "schedule",
];

/// A family's whole alpha budget, frozen before its first confirmation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaAllocationDeclaration {
    /// Family-wise alpha for every confirmation the family will ever run,
    /// ppm, `[1, 999_999]`.
    pub total_alpha_ppm: u64,
    /// `schedule[k - 1]` is the alpha of the family's k-th confirmation, ppm.
    /// Non-empty, every entry in `[1, 999_999]`, summing to at most the total.
    pub schedule: Vec<u64>,
}

impl AlphaAllocationDeclaration {
    /// The canonical document the identity covers.
    pub fn document(&self) -> Value {
        json!({
            "contractVersion": ALPHA_ALLOCATION_VERSION,
            "rule": ALPHA_ALLOCATION_RULE,
            "scope": ALPHA_ALLOCATION_SCOPE,
            "totalAlphaPpm": self.total_alpha_ppm,
            "schedule": self.schedule,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlphaAllocationStatus {
    /// The family may reserve `alphaPpm` for its next confirmation.
    Eligible,
    NotEligible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaAllocationReason {
    /// Every scheduled confirmation has already been reserved.
    AlphaBudgetExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlphaAllocationReport {
    pub contract_version: &'static str,
    pub rule: &'static str,
    pub scope: &'static str,
    pub allocation_id: String,
    pub status: AlphaAllocationStatus,
    pub reasons: Vec<AlphaAllocationReason>,
    pub total_alpha_ppm: u64,
    pub scheduled_confirmations: u64,
    /// Confirmations the family had reserved before this one.
    pub reserved_confirmations: u64,
    /// Alpha those reservations spent.
    pub spent_alpha_ppm: u64,
    /// 1-based number of the confirmation this allocation is for.
    pub confirmation_number: Option<u64>,
    /// The alpha that confirmation must declare; `None` when exhausted.
    pub alpha_ppm: Option<u64>,
    /// Scheduled confirmations left after this one.
    pub remaining_confirmations: u64,
    /// Alpha scheduled for those remaining confirmations.
    pub remaining_scheduled_alpha_ppm: u64,
    /// Budget the schedule never assigns; it cannot be used later.
    pub unscheduled_alpha_ppm: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaAllocationError(pub String);

impl std::fmt::Display for AlphaAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AlphaAllocationError {}

fn fail<T>(message: impl Into<String>) -> Result<T, AlphaAllocationError> {
    Err(AlphaAllocationError(message.into()))
}

fn alpha_in_domain(alpha_ppm: u64) -> bool {
    (1..=ALPHA_PPM_MAX).contains(&alpha_ppm)
}

fn total_error<T>() -> Result<T, AlphaAllocationError> {
    fail(format!(
        "allocation.totalAlphaPpm: must be an integer in [1, {ALPHA_PPM_MAX}]"
    ))
}

fn share_error<T>(index: usize) -> Result<T, AlphaAllocationError> {
    fail(format!(
        "allocation.schedule[{index}]: must be an integer in [1, {ALPHA_PPM_MAX}]"
    ))
}

fn read_literal(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), AlphaAllocationError> {
    match object.get(field) {
        None => fail(format!("allocation.{field}: required")),
        Some(Value::String(text)) if text == expected => Ok(()),
        Some(_) => fail(format!("allocation.{field}: must be \"{expected}\"")),
    }
}

/// Re-checks a declaration against the contract domain and returns the alpha
/// its schedule assigns. Every public entry point goes through it, so a
/// directly constructed declaration cannot bypass the budget bound.
fn scheduled_alpha(declaration: &AlphaAllocationDeclaration) -> Result<u64, AlphaAllocationError> {
    let total = declaration.total_alpha_ppm;
    if !alpha_in_domain(total) {
        return total_error();
    }
    if declaration.schedule.is_empty() {
        return fail("allocation.schedule: must be a non-empty array");
    }
    let mut scheduled = 0u128;
    for (index, &share) in declaration.schedule.iter().enumerate() {
        if !alpha_in_domain(share) {
            return share_error(index);
        }
        scheduled += u128::from(share);
    }
    // Exact: one ppm over the total is refused, never rounded away.
    if scheduled > u128::from(total) {
        return fail(format!(
            "allocation.schedule: allocates {scheduled} ppm, above totalAlphaPpm {total}"
        ));
    }
    Ok(scheduled as u64)
}

/// Strictly parses a declaration. Rejection order: not an object, unknown
/// fields (sorted), [`FIELDS`] in order with schedule entries in array order,
/// then the schedule's sum against the total.
pub fn parse_alpha_allocation(
    raw: &Value,
) -> Result<AlphaAllocationDeclaration, AlphaAllocationError> {
    let Some(object) = raw.as_object() else {
        return fail("allocation: must be an object");
    };
    let mut unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !FIELDS.contains(key))
        .collect();
    unknown.sort_unstable();
    if let Some(first) = unknown.first() {
        return fail(format!("allocation.{first}: unknown field"));
    }
    read_literal(object, "contractVersion", ALPHA_ALLOCATION_VERSION)?;
    read_literal(object, "rule", ALPHA_ALLOCATION_RULE)?;
    read_literal(object, "scope", ALPHA_ALLOCATION_SCOPE)?;
    let total_alpha_ppm = match object.get("totalAlphaPpm") {
        None => return fail("allocation.totalAlphaPpm: required"),
        // `as_u64` is `None` for negatives and for any float literal.
        Some(value) => match value.as_u64() {
            Some(total) if alpha_in_domain(total) => total,
            _ => return total_error(),
        },
    };
    let entries = match object.get("schedule") {
        None => return fail("allocation.schedule: required"),
        Some(Value::Array(entries)) if !entries.is_empty() => entries,
        Some(_) => return fail("allocation.schedule: must be a non-empty array"),
    };
    let mut schedule = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        match entry.as_u64() {
            Some(share) if alpha_in_domain(share) => schedule.push(share),
            _ => return share_error(index),
        }
    }
    let declaration = AlphaAllocationDeclaration {
        total_alpha_ppm,
        schedule,
    };
    scheduled_alpha(&declaration)?;
    Ok(declaration)
}

fn identity_of(declaration: &AlphaAllocationDeclaration) -> Result<String, AlphaAllocationError> {
    let encoded = canonical_bytes(&declaration.document())
        .map_err(|error| AlphaAllocationError(error.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(ALPHA_ALLOCATION_VERSION.as_bytes());
    digest.update([0]);
    digest.update(encoded);
    Ok(hex::encode(digest.finalize()))
}

/// Content identity of a valid declaration: SHA-256 over the contract
/// version, a zero byte and the canonical encoding of its document — the
/// campaign declaration's construction. Schedule order is part of it.
pub fn alpha_allocation_id(
    declaration: &AlphaAllocationDeclaration,
) -> Result<String, AlphaAllocationError> {
    scheduled_alpha(declaration)?;
    identity_of(declaration)
}

/// The equal-share schedule: `confirmations` entries of
/// `floor(total / confirmations)` ppm. The remainder stays unscheduled — it is
/// never rounded up or added to an entry — and a split whose share would
/// floor to zero is refused.
pub fn equal_alpha_schedule(
    total_alpha_ppm: u64,
    confirmations: u64,
) -> Result<Vec<u64>, AlphaAllocationError> {
    if !alpha_in_domain(total_alpha_ppm) {
        return total_error();
    }
    if !(1..=total_alpha_ppm).contains(&confirmations) {
        return fail(format!(
            "allocation: confirmations must be in [1, {total_alpha_ppm}] so every share is at least 1 ppm"
        ));
    }
    Ok(vec![
        total_alpha_ppm / confirmations;
        confirmations as usize
    ])
}

/// The alpha of the family's next confirmation. `reserved` is the alpha of
/// every confirmation the family has already reserved, oldest first, counting
/// ones that failed or never finished. It must equal the schedule's prefix:
/// a history that contradicts the declaration is an error, not an allocation.
pub fn allocate_confirmation_alpha(
    declaration: &AlphaAllocationDeclaration,
    reserved: &[u64],
) -> Result<AlphaAllocationReport, AlphaAllocationError> {
    let scheduled = scheduled_alpha(declaration)?;
    let schedule = &declaration.schedule;
    if reserved.len() > schedule.len() {
        return fail(format!(
            "allocation: {} confirmations are reserved but the schedule declares {}",
            reserved.len(),
            schedule.len()
        ));
    }
    for (index, (&was, &declared)) in reserved.iter().zip(schedule).enumerate() {
        if was != declared {
            return fail(format!(
                "allocation: confirmation {} reserved {was} ppm but the schedule declares {declared}",
                index + 1
            ));
        }
    }
    // A verified prefix of the schedule, so it is within the total.
    let spent: u64 = reserved.iter().sum();
    let alpha_ppm = schedule.get(reserved.len()).copied();
    let reserved_confirmations = reserved.len() as u64;
    let scheduled_confirmations = schedule.len() as u64;
    let taken = u64::from(alpha_ppm.is_some());
    Ok(AlphaAllocationReport {
        contract_version: ALPHA_ALLOCATION_VERSION,
        rule: ALPHA_ALLOCATION_RULE,
        scope: ALPHA_ALLOCATION_SCOPE,
        allocation_id: identity_of(declaration)?,
        status: match alpha_ppm {
            Some(_) => AlphaAllocationStatus::Eligible,
            None => AlphaAllocationStatus::NotEligible,
        },
        reasons: match alpha_ppm {
            Some(_) => Vec::new(),
            None => vec![AlphaAllocationReason::AlphaBudgetExhausted],
        },
        total_alpha_ppm: declaration.total_alpha_ppm,
        scheduled_confirmations,
        reserved_confirmations,
        spent_alpha_ppm: spent,
        confirmation_number: alpha_ppm.map(|_| reserved_confirmations + 1),
        alpha_ppm,
        remaining_confirmations: scheduled_confirmations - reserved_confirmations - taken,
        remaining_scheduled_alpha_ppm: scheduled - spent - alpha_ppm.unwrap_or(0),
        unscheduled_alpha_ppm: declaration.total_alpha_ppm - scheduled,
    })
}

#[cfg(test)]
mod tests;
