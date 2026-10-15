//! P13a: validate alpha reservation evidence before projecting amounts.
//!
//! Pure data integrity and conflict review, not a registry, reservation,
//! completeness proof, admission permit or confirmation result.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    alpha_allocation::{
        allocate_confirmation_alpha, alpha_allocation_id, AlphaAllocationDeclaration,
        AlphaAllocationReport, ALPHA_PPM_MAX,
    },
    identity::canonical_bytes,
};

pub const ALPHA_RESERVATION_VERSION: &str = "research-alpha-reservation-v1";
pub const ALPHA_RESERVATION_HISTORY_VERSION: &str = "research-alpha-reservation-history-v1";

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("alpha reservation: {0}")]
pub struct AlphaReservationError(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReservationBody {
    contract_version: String,
    family_id: String,
    allocation_id: String,
    origin_registry_id: String,
    reservation_key: String,
    confirmation_batch_id: String,
    confirmation_number: u64,
    alpha_ppm: u64,
}

/// Only constructors below can create this checked record. Its checksum
/// proves content integrity, never who created it or whether alpha was spent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlphaReservation {
    reservation_id: String,
    body: ReservationBody,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredReservation {
    reservation_id: String,
    body: ReservationBody,
}

impl AlphaReservation {
    pub fn family_id(&self) -> &str {
        &self.body.family_id
    }
    pub fn allocation_id(&self) -> &str {
        &self.body.allocation_id
    }
    pub fn origin_registry_id(&self) -> &str {
        &self.body.origin_registry_id
    }
    pub fn reservation_key(&self) -> &str {
        &self.body.reservation_key
    }
    pub fn confirmation_batch_id(&self) -> &str {
        &self.body.confirmation_batch_id
    }
    pub fn confirmation_number(&self) -> u64 {
        self.body.confirmation_number
    }
    pub fn alpha_ppm(&self) -> u64 {
        self.body.alpha_ppm
    }

    pub fn reservation_id(&self) -> &str {
        &self.reservation_id
    }

    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("reservation contains only strings and bounded integers")
    }
}

fn require(valid: bool, message: impl Into<String>) -> Result<(), AlphaReservationError> {
    if valid {
        Ok(())
    } else {
        Err(AlphaReservationError(message.into()))
    }
}

fn lowercase_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_family(family_id: &str) -> Result<(), AlphaReservationError> {
    require(
        family_id
            .strip_prefix("trial-family-v1:")
            .is_some_and(|hash| lowercase_hex(hash, 64)),
        "familyId must be trial-family-v1:<64 lowercase hex digits>",
    )
}

fn checked(body: ReservationBody) -> Result<AlphaReservation, AlphaReservationError> {
    require(
        body.contract_version == ALPHA_RESERVATION_VERSION,
        "unsupported contractVersion",
    )?;
    validate_family(&body.family_id)?;
    for (name, value, length) in [
        ("allocationId", &body.allocation_id, 64),
        ("originRegistryId", &body.origin_registry_id, 32),
        ("reservationKey", &body.reservation_key, 32),
        ("confirmationBatchId", &body.confirmation_batch_id, 64),
    ] {
        require(
            lowercase_hex(value, length),
            format!("{name} must be {length} lowercase hex digits"),
        )?;
    }
    // The existing schedule has positive integer-ppm shares, so at most
    // ALPHA_PPM_MAX entries; all hashed numbers are exactly representable.
    for (name, value) in [
        ("confirmationNumber", body.confirmation_number),
        ("alphaPpm", body.alpha_ppm),
    ] {
        require(
            (1..=ALPHA_PPM_MAX).contains(&value),
            format!("{name} must be an integer in [1, {ALPHA_PPM_MAX}]"),
        )?;
    }
    let document =
        serde_json::to_value(&body).map_err(|error| AlphaReservationError(error.to_string()))?;
    let bytes =
        canonical_bytes(&document).map_err(|error| AlphaReservationError(error.to_string()))?;
    let mut hash = Sha256::new();
    hash.update(ALPHA_RESERVATION_VERSION.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    Ok(AlphaReservation {
        reservation_id: hex::encode(hash.finalize()),
        body,
    })
}

/// Freeze a proposed body's content identity. The caller supplies a stable
/// operation key and registry identity; this function spends nothing.
/// Parsing text directly rejects duplicate object fields as well as unknown
/// fields, floats, absent fields and unsupported versions.
pub fn freeze_alpha_reservation(raw: &str) -> Result<AlphaReservation, AlphaReservationError> {
    let body =
        serde_json::from_str(raw).map_err(|error| AlphaReservationError(error.to_string()))?;
    checked(body)
}

/// Read stored evidence and recompute its content identity before review.
pub fn parse_alpha_reservation(raw: &str) -> Result<AlphaReservation, AlphaReservationError> {
    let stored: StoredReservation =
        serde_json::from_str(raw).map_err(|error| AlphaReservationError(error.to_string()))?;
    let checked = checked(stored.body)?;
    require(
        stored.reservation_id == checked.reservation_id,
        "reservationId does not reproduce the body",
    )?;
    Ok(checked)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlphaReservationHistoryStatus {
    Consistent,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaReservationConflictKind {
    ConfirmationNumber,
    OriginKey,
}

/// Every distinct checked record remains in the report's evidence. Conflict
/// groups name the records; they never renumber them or choose a winner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlphaReservationConflict {
    pub kind: AlphaReservationConflictKind,
    pub reservation_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlphaReservationHistoryReport {
    pub contract_version: &'static str,
    pub family_id: String,
    pub allocation_id: String,
    pub status: AlphaReservationHistoryStatus,
    pub duplicate_replays: usize,
    pub evidence: Vec<AlphaReservation>,
    pub conflicts: Vec<AlphaReservationConflict>,
    /// None on conflict. An exhausted, consistent history instead contains
    /// the existing allocator's NOT_ELIGIBLE report.
    pub allocation: Option<AlphaAllocationReport>,
}

/// Review one family's complete observed history. The registry caller must
/// establish completeness and atomicity; a truncated prefix cannot be
/// detected here. Every reservation counts, including failed/unfinished work.
pub fn review_alpha_reservations(
    family_id: &str,
    declaration: &AlphaAllocationDeclaration,
    history: &[AlphaReservation],
) -> Result<AlphaReservationHistoryReport, AlphaReservationError> {
    validate_family(family_id)?;
    let allocation_id = alpha_allocation_id(declaration)
        .map_err(|error| AlphaReservationError(error.to_string()))?;
    let mut unique = BTreeMap::new();
    for record in history {
        require(
            record.body.family_id == family_id,
            "history contains another family",
        )?;
        require(
            record.body.allocation_id == allocation_id,
            "history contains another allocation declaration",
        )?;
        let index = record.body.confirmation_number as usize - 1;
        require(
            declaration.schedule.get(index) == Some(&record.body.alpha_ppm),
            "confirmation number/share contradicts the declared schedule",
        )?;
        // Checked records cannot have one hash with two different bodies.
        unique.insert(record.reservation_id.clone(), record.clone());
    }
    let duplicate_replays = history.len() - unique.len();
    let mut evidence: Vec<_> = unique.into_values().collect();
    evidence.sort_by(|left, right| {
        (left.body.confirmation_number, &left.reservation_id)
            .cmp(&(right.body.confirmation_number, &right.reservation_id))
    });
    let mut by_number: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
    let mut by_origin: BTreeMap<(&str, &str), BTreeSet<String>> = BTreeMap::new();
    for record in &evidence {
        by_number
            .entry(record.body.confirmation_number)
            .or_default()
            .insert(record.reservation_id.clone());
        by_origin
            .entry((
                &record.body.origin_registry_id,
                &record.body.reservation_key,
            ))
            .or_default()
            .insert(record.reservation_id.clone());
    }
    let mut conflicts = Vec::new();
    for (kind, groups) in [
        (
            AlphaReservationConflictKind::ConfirmationNumber,
            by_number.values().collect::<Vec<_>>(),
        ),
        (
            AlphaReservationConflictKind::OriginKey,
            by_origin.values().collect(),
        ),
    ] {
        for group in groups.into_iter().filter(|group| group.len() > 1) {
            conflicts.push(AlphaReservationConflict {
                kind,
                reservation_ids: group.iter().cloned().collect(),
            });
        }
    }
    let allocation = if conflicts.is_empty() {
        for (index, record) in evidence.iter().enumerate() {
            require(
                record.body.confirmation_number == index as u64 + 1,
                "history must be a contiguous prefix starting at confirmation 1",
            )?;
        }
        let amounts: Vec<_> = evidence
            .iter()
            .map(|record| record.body.alpha_ppm)
            .collect();
        Some(
            allocate_confirmation_alpha(declaration, &amounts)
                .map_err(|error| AlphaReservationError(error.to_string()))?,
        )
    } else {
        None
    };
    Ok(AlphaReservationHistoryReport {
        contract_version: ALPHA_RESERVATION_HISTORY_VERSION,
        family_id: family_id.into(),
        allocation_id,
        status: if conflicts.is_empty() {
            AlphaReservationHistoryStatus::Consistent
        } else {
            AlphaReservationHistoryStatus::Blocked
        },
        duplicate_replays,
        evidence,
        conflicts,
        allocation,
    })
}

#[cfg(test)]
mod tests;
