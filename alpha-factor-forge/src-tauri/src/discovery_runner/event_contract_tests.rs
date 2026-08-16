//! RUNNER-UI-001a — the Rust half of the `discovery-event-v1` payload contract.
//!
//! The frontend consumes these events, so their serialized JSON is a public
//! contract, not an implementation detail. Before this module, the only
//! description of that contract on the TypeScript side was written BEFORE the
//! runner existed and matched none of the emitted fields — and nothing failed,
//! because no test compared the two.
//!
//! Both languages now assert the same authored fixture:
//!   - here: `serde_json::to_value(struct) == sample`
//!   - there: `src/tauri-client/events.test.ts` parses the same samples
//!
//! So adding, renaming, or re-typing a field on either side fails the other
//! side's test. The samples are constructed literally, never by running a
//! discovery, so this file stays a pure statement about serialization.

use serde_json::Value;

use super::{
    DiscoveryCandidateDigest, DiscoveryDoneEvent, DiscoveryJobIds, DiscoveryProgressCounts,
    DiscoveryProgressEvent, DiscoveryResultEvent, DISCOVERY_DONE_EVENT, DISCOVERY_EVENT_VERSION,
    DISCOVERY_PROGRESS_EVENT, DISCOVERY_RESULT_EVENT,
};
use crate::db::discovery::RunStatus;

const CONTRACT_FIXTURE: &str = include_str!("../../../fixtures/rs-core/discovery-event-v1.json");

fn fixture() -> Value {
    serde_json::from_str(CONTRACT_FIXTURE).expect("parse authored discovery-event fixture")
}

fn sample(name: &str) -> Value {
    fixture()
        .get("samples")
        .and_then(|samples| samples.get(name))
        .cloned()
        .unwrap_or_else(|| panic!("fixture sample \"{name}\" is missing"))
}

fn counts() -> DiscoveryProgressCounts {
    DiscoveryProgressCounts {
        total_candidates: 4,
        queued_candidates: 1,
        running_candidates: 1,
        completed_candidates: 2,
        failed_candidates: 0,
        skipped_candidates: 0,
    }
}

#[test]
fn fixture_pins_the_version_and_channel_names() {
    let fixture = fixture();
    assert_eq!(
        fixture["eventVersion"].as_str(),
        Some(DISCOVERY_EVENT_VERSION)
    );
    assert_eq!(
        fixture["channels"]["progress"].as_str(),
        Some(DISCOVERY_PROGRESS_EVENT)
    );
    assert_eq!(
        fixture["channels"]["result"].as_str(),
        Some(DISCOVERY_RESULT_EVENT)
    );
    assert_eq!(
        fixture["channels"]["done"].as_str(),
        Some(DISCOVERY_DONE_EVENT)
    );
    // The fixture must stay hand-authored: a generator would let one side define
    // both halves of the contract and the drift guard would prove nothing.
    assert_eq!(fixture["authored"].as_bool(), Some(true));
}

#[test]
fn progress_event_serializes_to_the_authored_sample() {
    let event = DiscoveryProgressEvent {
        event_version: DISCOVERY_EVENT_VERSION,
        sequence: 7,
        run_id: 12,
        status: RunStatus::Running,
        counts: counts(),
        candidate: Some(DiscoveryCandidateDigest {
            candidate_index: 2,
            strategy_id: 31,
            dataset_id: 5,
            job_ids: DiscoveryJobIds {
                train: 60,
                validation: 61,
            },
        }),
        best_strategy_id: Some(31),
    };
    assert_eq!(
        serde_json::to_value(&event).expect("serialize progress event"),
        sample("progressWithCandidate")
    );
}

/// `candidate` and `best_strategy_id` are `skip_serializing_if`, so the keys are
/// ABSENT rather than null. The frontend parser has to accept that.
#[test]
fn progress_event_omits_absent_optionals_instead_of_nulling_them() {
    let event = DiscoveryProgressEvent {
        event_version: DISCOVERY_EVENT_VERSION,
        sequence: 1,
        run_id: 12,
        status: RunStatus::Paused,
        counts: DiscoveryProgressCounts {
            total_candidates: 4,
            queued_candidates: 4,
            running_candidates: 0,
            completed_candidates: 0,
            failed_candidates: 0,
            skipped_candidates: 0,
        },
        candidate: None,
        best_strategy_id: None,
    };
    let value = serde_json::to_value(&event).expect("serialize minimal progress event");
    assert_eq!(value, sample("progressMinimal"));
    let object = value.as_object().expect("progress event is an object");
    assert!(!object.contains_key("candidate"));
    assert!(!object.contains_key("bestStrategyId"));
}

#[test]
fn result_event_serializes_to_the_authored_sample() {
    let event = DiscoveryResultEvent {
        event_version: DISCOVERY_EVENT_VERSION,
        sequence: 8,
        run_id: 12,
        candidate_index: 2,
        job_ids: DiscoveryJobIds {
            train: 60,
            validation: 61,
        },
        strategy_id: 31,
        strategy_hash: "strategy-v2:4f1c2d3e5a6b7c8d9e0f1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f"
            .to_string(),
        dataset_id: 5,
        validation_record_id: 44,
        gate_passed: true,
        score: Some(0.7351),
    };
    assert_eq!(
        serde_json::to_value(&event).expect("serialize result event"),
        sample("resultGatePassed")
    );
}

/// `score` has no `skip_serializing_if`, so a gate-failed candidate emits an
/// explicit `null` — the opposite convention from the progress optionals, and
/// exactly the kind of asymmetry a hand-written frontend DTO gets wrong.
#[test]
fn gate_failed_result_emits_an_explicit_null_score() {
    let event = DiscoveryResultEvent {
        event_version: DISCOVERY_EVENT_VERSION,
        sequence: 9,
        run_id: 12,
        candidate_index: 3,
        job_ids: DiscoveryJobIds {
            train: 62,
            validation: 63,
        },
        strategy_id: 32,
        strategy_hash: "strategy-v2:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
            .to_string(),
        dataset_id: 5,
        validation_record_id: 45,
        gate_passed: false,
        score: None,
    };
    let value = serde_json::to_value(&event).expect("serialize gate-failed result event");
    assert_eq!(value, sample("resultGateFailed"));
    assert!(value
        .as_object()
        .expect("result event is an object")
        .contains_key("score"));
    assert!(value["score"].is_null());
}

#[test]
fn done_event_serializes_both_terminal_shapes() {
    let completed = DiscoveryDoneEvent {
        event_version: DISCOVERY_EVENT_VERSION,
        sequence: 10,
        run_id: 12,
        status: RunStatus::Completed,
        best_strategy_id: Some(31),
        error_message: None,
    };
    assert_eq!(
        serde_json::to_value(&completed).expect("serialize completed done event"),
        sample("doneCompleted")
    );

    let failed = DiscoveryDoneEvent {
        event_version: DISCOVERY_EVENT_VERSION,
        sequence: 11,
        run_id: 13,
        status: RunStatus::Failed,
        best_strategy_id: None,
        error_message: Some(
            "candidate 3 failed: dataset 5 candle 12 violates market-data rule price_not_positive"
                .to_string(),
        ),
    };
    assert_eq!(
        serde_json::to_value(&failed).expect("serialize failed done event"),
        sample("doneFailed")
    );
}

/// Every run status the frontend can receive must keep its lowercase spelling:
/// the UI switches on these strings.
#[test]
fn run_status_serializes_lowercase_for_every_variant() {
    for (status, expected) in [
        (RunStatus::Idle, "idle"),
        (RunStatus::Running, "running"),
        (RunStatus::Paused, "paused"),
        (RunStatus::Completed, "completed"),
        (RunStatus::Failed, "failed"),
        (RunStatus::Cancelled, "cancelled"),
    ] {
        assert_eq!(
            serde_json::to_value(status).expect("serialize run status"),
            Value::String(expected.to_string())
        );
        assert_eq!(status.as_str(), expected);
    }
}
