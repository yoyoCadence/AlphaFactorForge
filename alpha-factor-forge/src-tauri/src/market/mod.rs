//! P06 — the market data foundation's storage half (migration 0008,
//! `docs/market-foundation-v1.md`).
//!
//! The pure half is `discovery_core::market_foundation`, mirrored in
//! TypeScript and pinned by `fixtures/rs-core/market-foundation-v1.json`.
//! This module is what persists: which instruments and calendar versions
//! exist (`registry`), what was actually retrieved and what was rejected or
//! revised (`provenance`), and which dataset is admissible for research
//! under which semantics (`snapshot`).
//!
//! What it deliberately does NOT do: reach the network (P07/P09/P10 add the
//! adapters), compute ETF corporate-action semantics (P08), or change how
//! anything existing runs. A dataset without a snapshot stays legacy: it
//! still imports and backtests exactly as before, and it simply cannot
//! acquire new qualification on its own.
//!
//! Like `research`, nothing here names a host or a framework.

// A foundation with more shape than callers, on purpose. P07 gave the
// writers a real caller (`ingest`, reached from `service fetch`), but the
// readers — listing instruments, walking a revision chain, listing
// snapshots — are for the ETF sources (P09/P10) and the work centre
// (P16/P21). Until those land the compiler cannot see a caller outside this
// module's own tests, and that is expected rather than a warning to chase.
// It is also why no Tauri command surface is exposed yet: a command nothing
// calls would be an untested boundary, not a feature.
#![allow(dead_code)]

pub mod http;
pub mod fetch;
pub mod ingest;
pub mod provenance;
pub mod registry;
pub mod sources;
pub mod snapshot;

use serde_json::Value;

use alpha_factor_forge::discovery_core::market_foundation::{Severity, ACTION_CODES, COVERAGE_CODES};
use crate::error::{AppError, AppResult};

/// Canonical JSON and hashing come from `research` rather than a second
/// codec: one encoding for every content hash in the workspace (the P05
/// review's "no second drifting codec" rule).
pub use crate::research::{canonical_json, sha256_hex};

/// The artifact-store kind of a raw market response.
pub const MARKET_RAW_ARTIFACT_KIND: &str = "market-raw-v1";

/// Event codes the storage half adds to the pure contract's
/// `COVERAGE_CODES`. They are about a snapshot's composition rather than a
/// series' shape, so they cannot come out of a coverage audit.
pub const SNAPSHOT_EVENT_CODES: [&str; 10] = [
    "dataset_instrument_mismatch",
    "expected_range_unavailable",
    "source_conflict",
    "superseded_source",
    "time_unit_mismatch",
    "availability_unknown",
    "corporate_actions_unverified",
    "cost_profile_unconfirmed",
    "missing_source",
    "availability_after_cut",
];

/// One row of `market_quality_events`: structured, append-only evidence.
/// A blocking event is why something was refused and names what to do about
/// it; a degraded one is why a snapshot exists but cannot qualify.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityEvent {
    pub instrument_id: String,
    pub interval: String,
    pub code: String,
    pub severity: Severity,
    /// One of `ACTION_CODES`; the UI owns the wording it shows.
    pub action: String,
    pub range_start: Option<i64>,
    pub range_end: Option<i64>,
    pub bar_count: Option<i64>,
    pub dataset_id: Option<i64>,
    pub provenance_id: Option<i64>,
    pub snapshot_row_id: Option<i64>,
    pub detail: Value,
}

impl QualityEvent {
    /// A minimal event; the callers add what they know.
    pub fn new(
        instrument_id: &str,
        interval: &str,
        code: &str,
        severity: Severity,
        action: &str,
    ) -> Self {
        Self {
            instrument_id: instrument_id.to_string(),
            interval: interval.to_string(),
            code: code.to_string(),
            severity,
            action: action.to_string(),
            range_start: None,
            range_end: None,
            bar_count: None,
            dataset_id: None,
            provenance_id: None,
            snapshot_row_id: None,
            detail: Value::Object(Default::default()),
        }
    }

    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = detail;
        self
    }

    pub fn with_range(mut self, start: i64, end: i64, bars: i64) -> Self {
        self.range_start = Some(start);
        self.range_end = Some(end);
        self.bar_count = Some(bars);
        self
    }

    pub fn for_dataset(mut self, dataset_id: i64) -> Self {
        self.dataset_id = Some(dataset_id);
        self
    }

    pub fn for_provenance(mut self, provenance_id: i64) -> Self {
        self.provenance_id = Some(provenance_id);
        self
    }

    /// An unknown code or action is refused rather than stored: an event
    /// nothing can act on is not evidence, and a typo would quietly widen
    /// the vocabulary both runtimes share.
    pub fn validate(&self) -> AppResult<()> {
        let known_code = COVERAGE_CODES.contains(&self.code.as_str())
            || SNAPSHOT_EVENT_CODES.contains(&self.code.as_str());
        if !known_code {
            return Err(AppError::Other(format!(
                "quality event code {:?} is not in the contract",
                self.code
            )));
        }
        if !ACTION_CODES.contains(&self.action.as_str()) {
            return Err(AppError::Other(format!(
                "quality event action {:?} is not in the contract",
                self.action
            )));
        }
        Ok(())
    }
}

/// True when every blocking event is absent — the one place that decides
/// what "blocking" means for the storage half.
pub fn is_blocked(events: &[QualityEvent]) -> bool {
    events
        .iter()
        .any(|event| event.severity == Severity::Blocking)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_outside_the_contract_vocabulary_is_refused() {
        let ok = QualityEvent::new(
            "crypto:binance:BTCUSDT",
            "1h",
            "missing_bar",
            Severity::Blocking,
            "refetch_range",
        );
        assert!(ok.validate().is_ok());
        let snapshot_level = QualityEvent::new(
            "crypto:binance:BTCUSDT",
            "1h",
            "cost_profile_unconfirmed",
            Severity::Degraded,
            "confirm_costs",
        );
        assert!(snapshot_level.validate().is_ok());
        let bad_code = QualityEvent::new("crypto:binance:BTCUSDT", "1h", "looks_wrong", Severity::Info, "refetch_range");
        assert!(bad_code.validate().unwrap_err().to_string().contains("not in the contract"));
        let bad_action = QualityEvent::new("crypto:binance:BTCUSDT", "1h", "missing_bar", Severity::Blocking, "call_support");
        assert!(bad_action.validate().unwrap_err().to_string().contains("action"));
        assert!(is_blocked(&[ok]));
        assert!(!is_blocked(&[snapshot_level]));
    }
}
