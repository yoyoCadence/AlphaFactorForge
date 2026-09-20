//! P06 parity: the Rust market-foundation contract is checked against the
//! same committed matrix the vitest suite reads.
//!
//! The fixture is authored from `docs/market-contract.md` /
//! `docs/market-foundation-v1.md` rather than recorded from either runtime,
//! so agreeing with it is a real constraint on both sides and not a
//! restatement of whichever implementation was written first. Every leaf
//! compares exactly (`expectedNumericPolicy: exact-v1`): this contract
//! classifies, so there is no tolerance to spend.

use serde_json::Value;

use super::market_foundation::{
    audit_coverage, detect_time_unit, expected_bar_starts, interval_ms, parse_instrument_id,
    series_conflicts, source_origin, sources_share_origin, validate_calendar, CoverageRequest,
    ExpectedRangeRequest, SeriesIdentity,
    SessionCalendar, Suspension, ACTION_CODES, CALENDAR_RULE_IDS, COVERAGE_CODES,
    EXPECTED_RANGE_ISSUE_IDS, INSTRUMENT_ID_RULE_IDS, INTERVAL_MS, MARKET_FOUNDATION_VERSION,
    MAX_EXPECTED_BARS, SERIES_CONFLICT_CODES, TIME_UNIT_IDS, TIME_UNIT_ISSUE_IDS,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/rs-core/market-foundation-v1.json"
    ))
    .expect("market-foundation fixture parses")
}

fn strings(value: &Value, path: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{path} is an array"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("{path} holds strings"))
                .to_string()
        })
        .collect()
}

fn numbers(value: &Value, path: &str) -> Vec<i64> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{path} is an array"))
        .iter()
        .map(|entry| {
            entry
                .as_i64()
                .unwrap_or_else(|| panic!("{path} holds integers"))
        })
        .collect()
}

/// One input leaf, honouring `explicit-numeric-status-v1` tags for the
/// non-finite values JSON cannot hold.
fn decode_number(value: &Value, path: &str) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().unwrap_or_else(|| panic!("{path} is a f64")),
        Value::String(tag) => match tag.as_str() {
            "nan" => f64::NAN,
            "positive_infinity" => f64::INFINITY,
            "negative_infinity" => f64::NEG_INFINITY,
            other => panic!("{path}: unknown numeric tag {other}"),
        },
        other => panic!("{path}: unsupported numeric leaf {other}"),
    }
}

fn calendar_of(fixture: &Value, calendar_id: &str) -> SessionCalendar {
    let calendars = fixture["calendars"].as_array().expect("calendars array");
    let found = calendars
        .iter()
        .find(|calendar| calendar["calendarId"] == calendar_id)
        .unwrap_or_else(|| panic!("fixture defines calendar {calendar_id}"));
    serde_json::from_value(found.clone()).expect("calendar decodes")
}

#[test]
fn the_envelope_constants_and_every_inventory_match_the_fixture() {
    let fixture = fixture();
    assert_eq!(fixture["schemaVersion"], "rs-core-parity-fixture-v1");
    assert_eq!(fixture["fixtureVersion"], "market-foundation-parity-v1");
    assert_eq!(
        fixture["contracts"]["marketFoundation"],
        MARKET_FOUNDATION_VERSION
    );
    assert_eq!(
        fixture["contracts"]["marketDataQuality"],
        super::market_data::MARKET_DATA_QUALITY_VERSION
    );
    assert_eq!(
        fixture["constants"]["minTimestampMs"].as_i64(),
        Some(super::market_data::MIN_MARKET_TIMESTAMP_MS)
    );
    assert_eq!(
        fixture["constants"]["maxTimestampMsExclusive"].as_i64(),
        Some(super::market_data::MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE)
    );
    assert_eq!(
        fixture["constants"]["maxExpectedBars"].as_u64(),
        Some(MAX_EXPECTED_BARS as u64)
    );

    let inventories = &fixture["inventories"];
    assert_eq!(
        strings(&inventories["instrumentIdRuleIds"], "instrumentIdRuleIds"),
        INSTRUMENT_ID_RULE_IDS.to_vec()
    );
    assert_eq!(
        strings(&inventories["calendarRuleIds"], "calendarRuleIds"),
        CALENDAR_RULE_IDS.to_vec()
    );
    assert_eq!(
        strings(&inventories["timeUnitIds"], "timeUnitIds"),
        TIME_UNIT_IDS.to_vec()
    );
    assert_eq!(
        strings(&inventories["timeUnitIssueIds"], "timeUnitIssueIds"),
        TIME_UNIT_ISSUE_IDS.to_vec()
    );
    assert_eq!(
        strings(&inventories["expectedRangeIssueIds"], "expectedRangeIssueIds"),
        EXPECTED_RANGE_ISSUE_IDS.to_vec()
    );
    assert_eq!(
        strings(&inventories["coverageCodes"], "coverageCodes"),
        COVERAGE_CODES.to_vec()
    );
    assert_eq!(
        strings(&inventories["actionCodes"], "actionCodes"),
        ACTION_CODES.to_vec()
    );
    assert_eq!(
        strings(&inventories["seriesConflictCodes"], "seriesConflictCodes"),
        SERIES_CONFLICT_CODES.to_vec()
    );

    // `unknown_kind` is unreachable here: the kind is an enum, so an unknown
    // value never reaches the validator. It therefore has no shared row.
    assert_eq!(
        strings(
            &inventories["runtimeSpecificCalendarRuleIds"],
            "runtimeSpecificCalendarRuleIds"
        ),
        vec!["unknown_kind".to_string()]
    );

    let intervals = fixture["intervals"].as_array().expect("intervals array");
    assert_eq!(intervals.len(), INTERVAL_MS.len());
    for entry in intervals {
        let name = entry["interval"].as_str().expect("interval name");
        assert_eq!(
            interval_ms(name),
            entry["ms"].as_i64(),
            "interval {name} cadence"
        );
    }
}

#[test]
fn every_instrument_id_is_parsed_exactly_as_specified() {
    let fixture = fixture();
    let cases = fixture["cases"]["instrumentIds"]
        .as_array()
        .expect("instrumentIds array");
    assert!(!cases.is_empty());
    for case in cases {
        let id = case["id"].as_str().expect("case id");
        let value = case["value"].as_str().expect("instrument id value");
        let expected = &case["expected"];
        match parse_instrument_id(value) {
            Ok(parsed) => {
                assert!(expected["rule"].is_null(), "{id} must be rejected");
                assert_eq!(expected["parsed"]["market"], parsed.market.as_str(), "{id}");
                assert_eq!(expected["parsed"]["venue"], parsed.venue, "{id}");
                assert_eq!(expected["parsed"]["symbol"], parsed.symbol, "{id}");
            }
            Err(rule) => {
                assert!(expected["parsed"].is_null(), "{id} must be accepted");
                assert_eq!(expected["rule"], rule.as_str(), "{id}");
            }
        }
    }
}

#[test]
fn every_raw_timestamp_batch_is_classified_exactly_as_specified() {
    let fixture = fixture();
    for case in fixture["cases"]["timeUnits"]
        .as_array()
        .expect("timeUnits array")
    {
        let id = case["id"].as_str().expect("case id");
        let timestamps: Vec<f64> = case["timestamps"]
            .as_array()
            .expect("timestamps array")
            .iter()
            .enumerate()
            .map(|(index, value)| decode_number(value, &format!("{id}.timestamps[{index}]")))
            .collect();
        let verdict = detect_time_unit(&timestamps);
        let expected = &case["expected"];
        assert_eq!(
            verdict.unit.map(|unit| unit.as_str()),
            expected["unit"].as_str(),
            "{id}: unit"
        );
        assert_eq!(
            verdict.issue.map(|issue| issue.as_str()),
            expected["issue"].as_str(),
            "{id}: issue"
        );
        assert_eq!(
            verdict.index.map(|index| index as u64),
            expected["index"].as_u64(),
            "{id}: index"
        );
    }
}

#[test]
fn every_calendar_is_validated_exactly_as_specified() {
    let fixture = fixture();
    for case in fixture["cases"]["calendarValidation"]
        .as_array()
        .expect("calendarValidation array")
    {
        let id = case["id"].as_str().expect("case id");
        let calendar: SessionCalendar =
            serde_json::from_value(case["calendar"].clone()).expect("calendar decodes");
        assert_eq!(
            validate_calendar(&calendar).map(|rule| rule.as_str()),
            case["expected"]["rule"].as_str(),
            "{id}"
        );
    }
}

#[test]
fn every_expected_range_is_derived_exactly_as_specified() {
    let fixture = fixture();
    for case in fixture["cases"]["expectedRanges"]
        .as_array()
        .expect("expectedRanges array")
    {
        let id = case["id"].as_str().expect("case id");
        let calendar = calendar_of(&fixture, case["calendarId"].as_str().expect("calendar id"));
        let suspensions: Vec<Suspension> =
            serde_json::from_value(case["suspensions"].clone()).expect("suspensions decode");
        let result = expected_bar_starts(&ExpectedRangeRequest {
            calendar: &calendar,
            interval: case["interval"].as_str().expect("interval"),
            from_ms: case["fromMs"].as_i64().expect("fromMs"),
            to_ms_exclusive: case["toMsExclusive"].as_i64().expect("toMsExclusive"),
            listed_from_ms: case["listedFromMs"].as_i64(),
            delisted_at_ms: case["delistedAtMs"].as_i64(),
            suspensions: &suspensions,
        });
        assert_eq!(
            result.timestamps,
            numbers(&case["expected"]["timestamps"], "expected.timestamps"),
            "{id}: timestamps"
        );
        assert_eq!(
            result.issue.map(|issue| issue.as_str()),
            case["expected"]["issue"].as_str(),
            "{id}: issue"
        );
    }
}

#[test]
fn every_coverage_case_is_audited_exactly_as_specified() {
    let fixture = fixture();
    for case in fixture["cases"]["coverage"]
        .as_array()
        .expect("coverage array")
    {
        let id = case["id"].as_str().expect("case id");
        let expected_bars = numbers(&case["expected"], "expected");
        let observed = numbers(&case["observed"], "observed");
        let report = audit_coverage(&CoverageRequest {
            interval: case["interval"].as_str().expect("interval"),
            expected: &expected_bars,
            observed: &observed,
            as_of_ms: case["asOfMs"].as_i64().expect("asOfMs"),
        });
        let expected_report = &case["expectedReport"];
        assert_eq!(report.version, MARKET_FOUNDATION_VERSION, "{id}: version");
        assert_eq!(
            report.blocking,
            expected_report["blocking"].as_bool().expect("blocking"),
            "{id}: blocking"
        );
        assert_eq!(
            report.expected_count,
            expected_report["expectedCount"].as_i64().expect("count"),
            "{id}: expectedCount"
        );
        assert_eq!(
            report.not_due_count,
            expected_report["notDueCount"].as_i64().expect("count"),
            "{id}: notDueCount"
        );
        assert_eq!(
            report.observed_count,
            expected_report["observedCount"].as_i64().expect("count"),
            "{id}: observedCount"
        );
        assert_eq!(
            report.matched_count,
            expected_report["matchedCount"].as_i64().expect("count"),
            "{id}: matchedCount"
        );
        assert_eq!(
            report.issue.map(|issue| issue.as_str()),
            expected_report["issue"].as_str(),
            "{id}: issue"
        );

        let events = expected_report["events"].as_array().expect("events array");
        assert_eq!(report.events.len(), events.len(), "{id}: event count");
        for (index, expected_event) in events.iter().enumerate() {
            let event = &report.events[index];
            let at = format!("{id}.events[{index}]");
            assert_eq!(event.code.as_str(), expected_event["code"], "{at}: code");
            assert_eq!(
                event.severity.as_str(),
                expected_event["severity"],
                "{at}: severity"
            );
            assert_eq!(
                event.range_start,
                expected_event["rangeStart"].as_i64().expect("rangeStart"),
                "{at}: rangeStart"
            );
            assert_eq!(
                event.range_end,
                expected_event["rangeEnd"].as_i64().expect("rangeEnd"),
                "{at}: rangeEnd"
            );
            assert_eq!(
                event.count,
                expected_event["count"].as_i64().expect("count"),
                "{at}: count"
            );
            assert_eq!(event.action, expected_event["action"], "{at}: action");
        }
    }
}

#[test]
fn two_endpoints_of_one_publisher_are_told_from_two_publishers() {
    let fixture = fixture();
    let cases = fixture["cases"]["sourceOrigins"]
        .as_array()
        .expect("sourceOrigins array");
    assert!(!cases.is_empty());
    for case in cases {
        let id = case["id"].as_str().expect("case id");
        let a = case["a"].as_str().expect("a");
        let b = case["b"].as_str().expect("b");
        assert_eq!(
            source_origin(a),
            case["expected"]["origin"].as_str().expect("origin"),
            "{id}: origin"
        );
        assert_eq!(
            sources_share_origin(a, b),
            case["expected"]["shareOrigin"].as_bool().expect("shareOrigin"),
            "{id}: shared"
        );
    }
}

#[test]
fn every_series_combination_case_is_answered_exactly_as_specified() {
    let fixture = fixture();
    for case in fixture["cases"]["seriesCombination"]
        .as_array()
        .expect("seriesCombination array")
    {
        let id = case["id"].as_str().expect("case id");
        let a: SeriesIdentity = serde_json::from_value(case["a"].clone()).expect("a decodes");
        let b: SeriesIdentity = serde_json::from_value(case["b"].clone()).expect("b decodes");
        assert_eq!(
            series_conflicts(&a, &b),
            strings(&case["expected"]["conflicts"], "expected.conflicts"),
            "{id}"
        );
    }
}

/// The storage half's action codes (snapshot composition, costs, corporate
/// actions, revisions) are Rust-only and must never come out of a coverage
/// report; asserted rather than left unstated.
#[test]
fn a_coverage_report_never_asks_for_a_storage_only_action() {
    let fixture = fixture();
    let storage_only = strings(
        &fixture["inventories"]["storageOnlyActionCodes"],
        "storageOnlyActionCodes",
    );
    assert!(!storage_only.is_empty());
    for case in fixture["cases"]["coverage"]
        .as_array()
        .expect("coverage array")
    {
        let expected_bars = numbers(&case["expected"], "expected");
        let observed = numbers(&case["observed"], "observed");
        let report = audit_coverage(&CoverageRequest {
            interval: case["interval"].as_str().expect("interval"),
            expected: &expected_bars,
            observed: &observed,
            as_of_ms: case["asOfMs"].as_i64().expect("asOfMs"),
        });
        for event in &report.events {
            assert!(
                !storage_only.iter().any(|action| action == event.action),
                "{} produced the storage-only action {}",
                case["id"],
                event.action
            );
            assert!(
                ACTION_CODES.contains(&event.action),
                "{} produced an unknown action {}",
                case["id"],
                event.action
            );
        }
    }
}
