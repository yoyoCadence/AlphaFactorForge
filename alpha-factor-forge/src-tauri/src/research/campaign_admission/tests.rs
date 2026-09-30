use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use super::*;
use crate::research::trial_ledger::{
    BindingCheck, TrialBatchInput, TrialEventInput, TrialKind, TrialLedger, TrialOrigin,
};
use alpha_factor_forge::discovery_core::{
    campaign::freeze_campaign,
    walk_forward::{evaluate_walk_forward_plan, WalkForwardPlan},
};

const BTC: &str = "crypto:binance:BTCUSDT";
const FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-campaign-declaration-v1.json");
/// The fixture's sample policy.
const BARS: u64 = 8_760;
const MIN_TRAIN: u64 = 1_000;
const FOLD_VALIDATION: u64 = 500;
const FOLDS: u64 = 3;

struct Registry {
    root: PathBuf,
    ledger: TrialLedger,
}

impl Registry {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "aff-campaign-admission-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let ledger = TrialLedger::open(&root.join("registry"), &workspace).unwrap();
        Self { root, ledger }
    }

    /// `count` effective variants for BTC under the A31/A32 protocol (m = 2).
    fn register(
        &self,
        request: &str,
        count: u64,
    ) -> crate::research::trial_ledger::BatchRegistration {
        self.ledger
            .register_batch(&TrialBatchInput {
                workspace_id: "ws".into(),
                instrument_id: Some(BTC.into()),
                tests_per_trial: 2,
                events: (0..count)
                    .map(|index| TrialEventInput {
                        kind: TrialKind::Variant,
                        origin: TrialOrigin::Request {
                            request_id: request.into(),
                            candidate_index: index,
                        },
                        hypothesis_hash: None,
                        strategy_hash: Some(format!("strategy-{request}-{index}")),
                        dataset_hash: Some("dataset".into()),
                        snapshot_id: Some("snapshot".into()),
                        split_hash: Some("split".into()),
                        seeds_hash: Some("seeds".into()),
                        engine_fingerprint_hash: Some("engine".into()),
                        benchmark_id: None,
                        benchmark_params_hash: None,
                        reproduction_of: None,
                        benchmark_evidence: None,
                    })
                    .collect(),
            })
            .unwrap()
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn campaign_with(bootstrap_samples: u64, max_bootstrap_samples: u64) -> FrozenCampaignDeclaration {
    let mut raw: Value = serde_json::from_str(FIXTURE).unwrap();
    raw["sampling"]["bootstrapSamples"] = json!(bootstrap_samples);
    raw["sampling"]["maxBootstrapSamples"] = json!(max_bootstrap_samples);
    freeze_campaign(&raw).unwrap()
}

fn fixture_dataset_hash() -> String {
    let raw: Value = serde_json::from_str(FIXTURE).unwrap();
    raw["instruments"][0]["datasetHash"]
        .as_str()
        .unwrap()
        .to_string()
}

fn resolved(bar_count: u64) -> ResolvedInstrument {
    ResolvedInstrument {
        instrument_id: BTC.into(),
        snapshot_id: serde_json::from_str::<Value>(FIXTURE).unwrap()["instruments"][0]
            ["snapshotId"]
            .as_str()
            .unwrap()
            .to_string(),
        snapshot_row_id: 7,
        instrument_row_id: 3,
        dataset_id: 11,
        dataset_hash: fixture_dataset_hash(),
        bar_count,
        cost_profile_version: "cost-profile-v1".into(),
    }
}

fn candidate(index: i64, bar_count: u64, embargo_bars: u64) -> CandidateFeasibility {
    let report = evaluate_walk_forward_plan(&WalkForwardPlan {
        total_bars: bar_count,
        embargo_bars,
        minimum_train_bars: MIN_TRAIN,
        fold_validation_bars: FOLD_VALIDATION,
        fold_count: FOLDS,
    })
    .unwrap();
    CandidateFeasibility {
        candidate_index: index,
        strategy_hash: format!("strategy-{index}"),
        report: Some(report),
        error: None,
    }
}

struct Case<'a> {
    campaign: &'a FrozenCampaignDeclaration,
    resolved: ResolvedInstrument,
    earlier_snapshot: Option<SnapshotObservation>,
    legacy: Vec<String>,
    candidates: Vec<CandidateFeasibility>,
}

impl<'a> Case<'a> {
    fn new(campaign: &'a FrozenCampaignDeclaration) -> Self {
        Self {
            campaign,
            resolved: resolved(BARS),
            earlier_snapshot: None,
            legacy: Vec::new(),
            candidates: vec![candidate(0, BARS, 24), candidate(1, BARS, 48)],
        }
    }

    fn evaluate(&self, ledger: LedgerInput<'_>) -> AppResult<CampaignAdmissionReport> {
        let batch_id = match ledger {
            LedgerInput::Registered(Admission::Count(count))
            | LedgerInput::Fenced(AdmissionFence::Unchanged(count) | AdmissionFence::Grew(count)) => {
                count.batch_id()
            }
            _ => "trial-batch-v1:blocked",
        };
        evaluate_campaign_admission(&CampaignAdmissionInput {
            campaign: self.campaign,
            instrument_id: BTC,
            resolved: &self.resolved,
            earlier_snapshot: self.earlier_snapshot.as_ref(),
            batch_id,
            ledger,
            legacy_trials_unknown: &self.legacy,
            candidates: &self.candidates,
        })
    }
}

use CampaignAdmissionReason as R;

#[test]
fn admission_rejects_a_different_declared_snapshot_for_the_same_dataset() {
    let registry = Registry::new();
    let batch = registry.register("snapshot-binding", 1);
    let original = campaign_with(100_000, 200_000);
    let mut declaration = original.document().clone();
    let old_id = declaration["instruments"][0]["snapshotId"]
        .as_str()
        .unwrap();
    let other_id = if old_id == "a".repeat(64) { "b" } else { "a" }.repeat(64);
    declaration["instruments"][0]["snapshotId"] = json!(other_id);
    let other = freeze_campaign(&declaration).unwrap();
    let mut case = Case::new(&original);
    assert!(case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .is_ok());
    // Reusing A's resolution for B used to relabel the observation as B.
    case.campaign = &other;
    assert!(case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .is_err());
}

#[test]
fn admission_rejects_a_count_from_another_batch_in_the_same_family() {
    let registry = Registry::new();
    let first = registry.register("batch-a", 1);
    let second = registry.register("batch-b", 1);
    let campaign = campaign_with(100_000, 200_000);
    let case = Case::new(&campaign);
    let input = CampaignAdmissionInput {
        campaign: &campaign,
        instrument_id: BTC,
        resolved: &case.resolved,
        earlier_snapshot: None,
        batch_id: &first.batch_id,
        ledger: LedgerInput::Registered(&second.admission),
        legacy_trials_unknown: &[],
        candidates: &case.candidates,
    };
    assert!(evaluate_campaign_admission(&input).is_err());
}

#[test]
fn admission_rejects_a_walk_forward_status_that_disagrees_with_its_plan() {
    let registry = Registry::new();
    let batch = registry.register("report-binding", 1);
    let campaign = campaign_with(100_000, 200_000);
    let mut case = Case::new(&campaign);
    case.candidates[0] = candidate(0, BARS, 3_000);
    let report = case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .unwrap();
    assert_eq!(report.reasons, vec![R::WalkForwardNotEligible]);
    case.candidates[0].report.as_mut().unwrap().status = WalkForwardStatus::Eligible;
    assert!(case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .is_err());

    // An eligible label also cannot hide missing or altered fold evidence.
    case.candidates[0] = candidate(0, BARS, 24);
    case.candidates[0].report.as_mut().unwrap().folds.clear();
    assert!(case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .is_err());
    case.candidates[0] = candidate(0, BARS, 24);
    case.candidates[0].report.as_mut().unwrap().folds[0]
        .validation
        .to += 1;
    assert!(case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .is_err());
}

#[test]
fn a20_alphabtc_equivalent_counts_are_not_eligible_through_campaign_admission() {
    // trial-ledger-v1 A20: the plan P12d assembles from the ledger gives the
    // AlphaBTC 146/1001 counterexample and blocks it.
    let registry = Registry::new();
    registry.register("earlier", 49);
    let batch = registry.register("next", 24);
    let campaign = campaign_with(1_000, 100_000);
    let report = Case::new(&campaign)
        .evaluate(LedgerInput::Registered(&batch.admission))
        .unwrap();
    let precision = report.precision.as_ref().unwrap();
    assert_eq!(
        (
            precision.best_adjusted_p.numerator,
            precision.best_adjusted_p.denominator
        ),
        (146, 1001)
    );
    assert_eq!(report.status, CampaignAdmissionStatus::NotEligible);
    assert_eq!(report.reasons, vec![R::PrecisionNotEligible]);
    let ledger = report.ledger.as_ref().unwrap();
    assert_eq!(
        (
            ledger.family_effective_trials,
            ledger.batch_effective_trials
        ),
        (73, 24)
    );
}

#[test]
fn a32_an_eligible_decision_is_fenced_before_it_is_acted_on() {
    // trial-ledger-v1 A32 with the A31 parameters: family 11 (m = 22) is
    // ELIGIBLE at B = 10,975; five more trials make it m = 32 (needs 15,975).
    let registry = Registry::new();
    registry.register("reqB", 10);
    let batch = registry.register("reqA", 1);
    let campaign = campaign_with(10_975, 100_000);
    let case = Case::new(&campaign);
    let first = case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .unwrap();
    assert_eq!(
        first.status,
        CampaignAdmissionStatus::Eligible,
        "{:?}",
        first.reasons
    );
    assert_eq!(first.ledger_fence, None);
    let stored = first.ledger.clone().unwrap();
    assert_eq!(stored.family_effective_trials, 11);

    let mut refreshed = Case::new(&campaign);
    refreshed.earlier_snapshot = Some(first.snapshot.clone());
    let fence = registry
        .ledger
        .fence_admission(&batch.batch_id, &stored)
        .unwrap();
    assert!(matches!(fence, AdmissionFence::Unchanged(_)));
    let still = refreshed.evaluate(LedgerInput::Fenced(&fence)).unwrap();
    assert_eq!(
        still.status,
        CampaignAdmissionStatus::Eligible,
        "the decision stands"
    );
    assert_eq!(still.ledger_fence, Some("unchanged"));
    assert_eq!(still.ledger, first.ledger);

    registry.register("reqC", 5);
    let fence = registry
        .ledger
        .fence_admission(&batch.batch_id, &stored)
        .unwrap();
    let AdmissionFence::Grew(count) = &fence else {
        panic!("expected a grown family, got {fence:?}");
    };
    assert_eq!(count.family_effective_trials(), 16);
    let stale = refreshed.evaluate(LedgerInput::Fenced(&fence)).unwrap();
    assert_eq!(stale.status, CampaignAdmissionStatus::NotEligible);
    assert_eq!(stale.reasons, vec![R::PrecisionNotEligible]);
    assert_eq!(stale.ledger_fence, Some("grew"));
    let precision = stale.precision.as_ref().unwrap();
    assert_eq!(precision.family_tests, 32);
    assert_eq!(precision.required_samples_for_precision, Some(15_975));

    let json = serde_json::to_value(&stale).unwrap();
    assert_eq!(json["contractVersion"], CAMPAIGN_ADMISSION_VERSION);
    assert_eq!(json["status"], "NOT_ELIGIBLE");
    assert_eq!(json["reasons"], json!(["precision_not_eligible"]));
    assert_eq!(json["ledger"]["familyEffectiveTrials"], 16);
    assert_eq!(json["ledger"]["batchId"], batch.batch_id);
    assert_eq!(json["snapshot"]["snapshotRowId"], 7);
    assert_eq!(json["walkForward"][0]["report"]["status"], "ELIGIBLE");
    assert!(json.get("pass").is_none() && json["status"] != "PASS");
}

#[test]
fn every_failing_check_is_reported_once_in_declaration_order() {
    let campaign = campaign_with(100_000, 200_000);
    let family = family_id_for(BTC).unwrap();
    let short = BARS - 1;
    let mut case = Case::new(&campaign);
    case.resolved = resolved(short);
    case.legacy = vec!["trial-family-v1:other".into(), family];
    case.candidates = vec![
        candidate(0, short, 24),
        // Folds no longer fit inside the outer Train range.
        candidate(1, short, 3_000),
        CandidateFeasibility {
            candidate_index: 2,
            strategy_hash: "strategy-2".into(),
            report: None,
            error: Some("minimumTrainBars is below the candidate signal lookback".into()),
        },
    ];
    assert_eq!(
        case.candidates[1].report.as_ref().unwrap().status,
        WalkForwardStatus::NotEligible
    );
    let quarantined = Admission::Blocked(AdmissionBlocked::FamilyQuarantined);
    let report = case
        .evaluate(LedgerInput::Registered(&quarantined))
        .unwrap();
    assert_eq!(
        report.reasons,
        vec![
            R::FamilyQuarantined,
            R::LegacyTrialsUnknown,
            R::InsufficientTotalBars,
            R::WalkForwardNotEligible
        ]
    );
    assert_eq!(
        (report.ledger.as_ref(), report.precision.as_ref()),
        (None, None)
    );

    let case = Case::new(&campaign);
    for (blocked, reason) in [
        (AdmissionBlocked::FamilyUnknown, R::FamilyUnknown),
        (AdmissionBlocked::FamilyQuarantined, R::FamilyQuarantined),
        (AdmissionBlocked::ChainBroken, R::RegistryChainBroken),
        (AdmissionBlocked::NoEffectiveTrials, R::NoEffectiveTrials),
    ] {
        let admission = Admission::Blocked(blocked.clone());
        let report = case.evaluate(LedgerInput::Registered(&admission)).unwrap();
        assert_eq!(report.reasons, vec![reason], "{blocked:?}");
        let fenced = AdmissionFence::Blocked(FenceBlocked::Admission(blocked.clone()));
        let report = case.evaluate(LedgerInput::Fenced(&fenced)).unwrap();
        assert_eq!(report.reasons, vec![reason], "fenced {blocked:?}");
        assert_eq!(report.ledger_fence, Some(blocked.code()));
    }
}

#[test]
fn an_unproven_fence_or_a_changed_snapshot_stops_the_decision() {
    let campaign = campaign_with(100_000, 200_000);
    let registry = Registry::new();
    let batch = registry.register("req", 1);
    let mut case = Case::new(&campaign);
    let first = case
        .evaluate(LedgerInput::Registered(&batch.admission))
        .unwrap();
    assert_eq!(
        first.status,
        CampaignAdmissionStatus::Eligible,
        "{:?}",
        first.reasons
    );

    for (fence, reason, code) in [
        (
            AdmissionFence::Blocked(FenceBlocked::Prefix(BindingCheck::RegistryRolledBack)),
            R::LedgerPrefixUnproven,
            "registry_rolled_back",
        ),
        (
            AdmissionFence::Blocked(FenceBlocked::Prefix(BindingCheck::RegistryReplaced)),
            R::LedgerPrefixUnproven,
            "registry_replaced",
        ),
        (
            AdmissionFence::Blocked(FenceBlocked::Inconsistent),
            R::LedgerCountInconsistent,
            "admission_count_inconsistent",
        ),
    ] {
        let report = case.evaluate(LedgerInput::Fenced(&fence)).unwrap();
        assert_eq!(report.reasons, vec![reason], "{code}");
        assert_eq!(report.ledger_fence, Some(code));
        assert_eq!(report.ledger, None);
    }

    let mut moved = first.snapshot.clone();
    moved.snapshot_row_id += 1;
    case.earlier_snapshot = Some(moved);
    let fence = registry
        .ledger
        .fence_admission(&batch.batch_id, first.ledger.as_ref().unwrap())
        .unwrap();
    let report = case.evaluate(LedgerInput::Fenced(&fence)).unwrap();
    assert_eq!(report.reasons, vec![R::SnapshotChanged]);
}

#[test]
fn a_binding_that_contradicts_the_campaign_is_an_error() {
    let campaign = campaign_with(100_000, 200_000);
    let registry = Registry::new();
    let batch = registry.register("req", 1);
    let admission = LedgerInput::Registered(&batch.admission);
    let error = |case: &Case<'_>, instrument: &str, batch_id: &str| {
        evaluate_campaign_admission(&CampaignAdmissionInput {
            campaign: case.campaign,
            instrument_id: instrument,
            resolved: &case.resolved,
            earlier_snapshot: None,
            batch_id,
            ledger: admission,
            legacy_trials_unknown: &case.legacy,
            candidates: &case.candidates,
        })
        .err()
        .map(|error| error.to_string())
    };
    let base = Case::new(&campaign);
    assert_eq!(error(&base, BTC, &batch.batch_id), None);
    assert!(error(&base, "crypto:binance:ETHUSDT", "b")
        .unwrap()
        .contains("not declared"));
    assert!(error(&base, BTC, " ").unwrap().contains("batch id"));

    type Change = Box<dyn Fn(&mut Case<'_>)>;
    let mut changes: Vec<(&str, Change)> = vec![
        (
            "dataset",
            Box::new(|case| case.resolved.dataset_hash.push('0')),
        ),
        (
            "instrument",
            Box::new(|case| case.resolved.instrument_id = "crypto:binance:ETHUSDT".into()),
        ),
        ("no candidates", Box::new(|case| case.candidates.clear())),
        (
            "duplicate",
            Box::new(|case| case.candidates[1].candidate_index = 0),
        ),
        (
            "negative index",
            Box::new(|case| case.candidates[0].candidate_index = -1),
        ),
        (
            "blank strategy",
            Box::new(|case| case.candidates[0].strategy_hash.clear()),
        ),
        (
            "both",
            Box::new(|case| case.candidates[0].error = Some("x".into())),
        ),
        ("neither", Box::new(|case| case.candidates[0].report = None)),
        (
            "blank error",
            Box::new(|case| {
                case.candidates[0].report = None;
                case.candidates[0].error = Some(" ".into());
            }),
        ),
        (
            "total bars",
            Box::new(|case| case.candidates[0] = candidate(0, BARS + 1, 24)),
        ),
    ];
    for (field, value) in [
        ("minimum train", MIN_TRAIN + 1),
        ("fold validation", FOLD_VALIDATION + 1),
        ("fold count", FOLDS - 1),
    ] {
        changes.push((
            field,
            Box::new(move |case: &mut Case<'_>| {
                let plan = &mut case.candidates[0].report.as_mut().unwrap().plan;
                match field {
                    "minimum train" => plan.minimum_train_bars = value,
                    "fold validation" => plan.fold_validation_bars = value,
                    _ => plan.fold_count = value,
                }
            }),
        ));
    }
    for (label, change) in changes {
        let mut case = Case::new(&campaign);
        change(&mut case);
        assert!(error(&case, BTC, &batch.batch_id).is_some(), "{label}");
    }

    // A count read for another instrument's family is not this decision's.
    let other = registry
        .ledger
        .register_batch(&TrialBatchInput {
            instrument_id: Some("crypto:binance:ETHUSDT".into()),
            ..TrialBatchInput {
                workspace_id: "ws".into(),
                instrument_id: None,
                tests_per_trial: 2,
                events: vec![TrialEventInput {
                    kind: TrialKind::Variant,
                    origin: TrialOrigin::Request {
                        request_id: "eth".into(),
                        candidate_index: 0,
                    },
                    hypothesis_hash: None,
                    strategy_hash: Some("strategy".into()),
                    dataset_hash: Some("dataset".into()),
                    snapshot_id: Some("snapshot".into()),
                    split_hash: Some("split".into()),
                    seeds_hash: Some("seeds".into()),
                    engine_fingerprint_hash: Some("engine".into()),
                    benchmark_id: None,
                    benchmark_params_hash: None,
                    reproduction_of: None,
                    benchmark_evidence: None,
                }],
            }
        })
        .unwrap();
    let error = Case::new(&campaign)
        .evaluate(LedgerInput::Registered(&other.admission))
        .err()
        .unwrap();
    assert!(error.to_string().contains("another family"), "{error}");
}
