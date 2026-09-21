use super::*;
use crate::market::http::testing::FakeFetcher;
use std::sync::atomic::{AtomicU64, Ordering};

fn settings() -> Settings {
    serde_json::from_value(json!({"version":SETTINGS_VERSION,"instruments":[{
        "market":{"instrumentId":"us-etf:nyse-arca:SPY","currency":"USD","calendar":{
            "calendarId":"test-us-v1","kind":"trading-days","timezone":"America/New_York",
            "tradingWeekdays":[1,2,3,4,5],"holidays":["2026-07-03"],"earlyCloses":[]},
            "calendarFrom":"2026-07-01","calendarToExclusive":"2026-08-01","sessionsPerYear":252},
        "exchangeCode":"NYSE ARCA","listedFrom":"1993-01-22","listingEvidence":"synthetic listing fixture",
        "calendarEvidence":"synthetic calendar fixture","corporateActionsConfirmed":true,
        "costs":{"version":"fixture-v1","currency":"USD","confirmed":true,"commissionRate":0.001,
            "minimumCommission":0,"slippageRate":0.001,"buyTaxRate":0,"sellTaxRate":0}
    }]})).unwrap()
}
fn rows() -> Value {
    json!([
        {"date":"2026-07-02T00:00:00.000Z","open":100,"high":102,"low":98,"close":100,"volume":1000,
         "adjOpen":50,"adjHigh":51,"adjLow":49,"adjClose":50,"adjVolume":2000,"divCash":1.25,"splitFactor":1},
        {"date":"2026-07-06T00:00:00.000Z","open":50,"high":51,"low":49,"close":50,"volume":2000,
         "adjOpen":50,"adjHigh":51,"adjLow":49,"adjClose":50,"adjVolume":2000,"divCash":0,"splitFactor":2}
    ])
}
fn dividends(payment: Value) -> Value {
    json!([{"ticker":"spy","exDate":"2026-07-02T00:00:00Z","paymentDate":payment,"distribution":1.25,"distributionFrequency":"q"}])
}
fn fetcher(eod: Value, events: Value) -> FakeFetcher {
    let u = tiingo::urls("SPY", "2026-07-02", "2026-07-06");
    FakeFetcher::new().with(&u[0],br#"{"ticker":"SPY","exchangeCode":"NYSE ARCA","startDate":"1993-01-22","endDate":"2026-07-06"}"#)
        .with(&u[1],&serde_json::to_vec(&eod).unwrap()).with(&u[2],&serde_json::to_vec(&events).unwrap())
}
struct Workspace {
    conn: Connection,
    store: ArtifactStore,
    dir: PathBuf,
}
impl Workspace {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "aff-tiingo-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        db::apply_migrations(&conn).unwrap();
        Self {
            conn,
            store: ArtifactStore::in_data_dir(&dir),
            dir,
        }
    }
    fn run(&mut self, f: &dyn HttpFetcher, s: &Settings) -> Vec<Report> {
        ingest(
            &mut self.conn,
            &self.store,
            Some(f),
            None,
            s,
            "2026-07-02",
            "2026-07-07",
            utc_date_to_ms("2026-07-08").unwrap(),
        )
        .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        if self.dir.exists() {
            std::fs::remove_dir_all(&self.dir).unwrap();
        }
    }
}

#[test]
fn complete_flow_keeps_raw_prices_holidays_actions_and_reopenable_evidence() {
    let mut w = Workspace::new();
    let f = fetcher(rows(), dividends(json!("2026-07-15")));
    let reports = w.run(&f, &settings());
    let r = &reports[0];
    assert_eq!(reports.len(), 5);
    assert_eq!(r.status, "OK");
    assert!(r.qualification_eligible);
    assert_eq!(r.bars, 2);
    assert_eq!(r.dividends[0].payment_date.as_deref(), Some("2026-07-15"));
    assert_eq!(r.splits[0].ratio, 2.0);
    assert!(r.splits[0].available_at_ms > utc_date_to_ms("2026-07-06").unwrap());
    let candles = repositories::get_candles(&w.conn, r.dataset_id.unwrap(), 0, i64::MAX).unwrap();
    assert_eq!(candles[0].close, 100.0);
    assert_eq!(candles[0].volume, 1000.0);
    assert!(reports[1..]
        .iter()
        .all(|r| r.reasons == ["source_settings_missing"]));
    let originals =
        provenance::list_provenance(&w.conn, r.instrument_id.as_deref().unwrap(), "1d").unwrap();
    assert_eq!(originals.len(), 4);
    for p in originals {
        assert!(!w.store.read(&p.artifact_ref()).unwrap().is_empty());
    }
    let restored = snapshot::get_snapshot(&w.conn, r.snapshot.as_ref().unwrap().id)
        .unwrap()
        .unwrap();
    assert!(restored.qualification_eligible());
}

#[test]
fn payment_unknown_and_unconfirmed_evidence_or_costs_degrade_snapshot() {
    for kind in ["payment", "actions", "costs"] {
        let mut w = Workspace::new();
        let mut s = settings();
        if kind == "actions" {
            s.instruments[0].corporate_actions_confirmed = false;
        }
        if kind == "costs" {
            s.instruments[0].costs = None;
        }
        let payment = if kind == "payment" {
            Value::Null
        } else {
            json!("2026-07-15")
        };
        let r = w.run(&fetcher(rows(), dividends(payment)), &s);
        assert_eq!(r[0].status, "DEGRADED");
        assert!(!r[0].qualification_eligible);
        assert!(!r[0].snapshot.as_ref().unwrap().qualification_eligible());
        assert!(r[0].reasons.iter().any(|r| r
            == match kind {
                "payment" => "unknown_payment_date",
                "actions" => "corporate_actions_unconfirmed",
                _ => "costs_unconfirmed",
            }));
    }
}

#[test]
fn entitlement_denial_keeps_eod_and_explicit_missing_payment_blockers() {
    let u = tiingo::urls("SPY", "2026-07-02", "2026-07-06");
    let f = fetcher(rows(), json!([])).failing(
        &u[2],
        FetchError::Status {
            url: u[2].clone(),
            status: 403,
        },
    );
    let mut w = Workspace::new();
    let r = w.run(&f, &settings());
    assert_eq!(r[0].status, "DEGRADED");
    assert!(r[0].reasons.contains(&"entitlement_denied".into()));
    assert!(r[0].reasons.contains(&"unknown_payment_date".into()));
    assert!(r[0].dataset_id.is_some());
    assert!(!r[0].qualification_eligible);
}

#[test]
fn gaps_duplicates_off_session_invalid_values_and_identity_never_create_snapshot() {
    for kind in [
        "leading-gap",
        "trailing-gap",
        "duplicate",
        "weekend",
        "negative",
        "adjusted",
        "date",
    ] {
        let mut values = rows();
        let a = values.as_array_mut().unwrap();
        match kind {
            "leading-gap" => {
                a.remove(0);
            }
            "trailing-gap" => {
                a.pop();
            }
            "duplicate" => a.push(a[1].clone()),
            "weekend" => a[1]["date"] = json!("2026-07-05"),
            "negative" => a[0]["volume"] = json!(-1),
            "adjusted" => a[0]["adjClose"] = json!(900),
            _ => a[0]["date"] = json!("2026-07-02T13:00:00Z"),
        }
        let mut w = Workspace::new();
        let r = w.run(
            &fetcher(values, dividends(json!("2026-07-15"))),
            &settings(),
        );
        assert!(r[0].snapshot.is_none(), "{kind}");
        assert!(!r[0].reasons.is_empty());
    }
}

#[test]
fn company_event_conflicts_preserve_originals_and_never_promote() {
    for kind in [
        "missing",
        "extra",
        "amount",
        "ticker",
        "early",
        "duplicate",
        "cancelled",
    ] {
        let mut e = dividends(json!("2026-07-15"));
        match kind {
            "missing" => e = json!([]),
            "extra" => e[0]["exDate"] = json!("2026-07-06"),
            "amount" => e[0]["distribution"] = json!(2),
            "ticker" => e[0]["ticker"] = json!("QQQ"),
            "early" => e[0]["paymentDate"] = json!("2026-07-01"),
            "duplicate" => {
                let copy = e[0].clone();
                e.as_array_mut().unwrap().push(copy);
            }
            _ => e[0]["distributionFrequency"] = json!("c"),
        }
        let mut w = Workspace::new();
        let r = w.run(&fetcher(rows(), e), &settings());
        assert_eq!(r[0].status, "DEGRADED", "{kind}");
        assert!(!r[0].qualification_eligible);
        let raw = provenance::get_provenance(&w.conn, r[0].provenance_ids[2])
            .unwrap()
            .unwrap();
        assert!(!raw.accepted);
        assert!(w.store.read(&raw.artifact_ref()).is_ok());
    }
}

#[test]
fn revision_is_linked_and_old_bytes_are_not_overwritten() {
    let mut w = Workspace::new();
    let s = settings();
    let first = w.run(&fetcher(rows(), dividends(json!("2026-07-15"))), &s);
    let mut changed = rows();
    changed[0]["volume"] = json!(1100);
    let second = w.run(&fetcher(changed, dividends(json!("2026-07-15"))), &s);
    assert_ne!(first[0].dataset_id, second[0].dataset_id);
    let old = provenance::get_provenance(&w.conn, first[0].provenance_ids[1])
        .unwrap()
        .unwrap();
    let new = provenance::get_provenance(&w.conn, second[0].provenance_ids[1])
        .unwrap()
        .unwrap();
    assert_eq!(new.revision_of, Some(old.id));
    assert_ne!(old.raw_sha256, new.raw_sha256);
    assert!(w.store.read(&old.artifact_ref()).is_ok());
    assert!(w.store.read(&new.artifact_ref()).is_ok());
}

#[test]
fn absent_credential_produces_five_independent_reports_without_network() {
    let mut w = Workspace::new();
    let mut s = settings();
    for symbol in &tiingo::SYMBOLS[1..] {
        let mut item = s.instruments[0].clone();
        item.market.instrument_id = format!("us-etf:nyse-arca:{symbol}");
        s.instruments.push(item);
    }
    let reports = ingest(
        &mut w.conn,
        &w.store,
        None,
        Some("credential_missing"),
        &s,
        "2026-07-02",
        "2026-07-07",
        utc_date_to_ms("2026-07-08").unwrap(),
    )
    .unwrap();
    assert_eq!(reports.len(), 5);
    assert!(reports.iter().all(|r| r.reasons == ["credential_missing"]));
}

#[test]
fn quota_stops_batch_without_retrying_other_symbols() {
    let mut w = Workspace::new();
    let mut s = settings();
    let mut q = s.instruments[0].clone();
    q.market.instrument_id = "us-etf:nasdaq:QQQ".into();
    q.exchange_code = "NASDAQ".into();
    s.instruments.push(q);
    let u = tiingo::urls("SPY", "2026-07-02", "2026-07-06");
    let f = FakeFetcher::new().failing(
        &u[0],
        FetchError::Status {
            url: u[0].clone(),
            status: 429,
        },
    );
    let r = w.run(&f, &s);
    assert_eq!(f.requested().len(), 1);
    assert_eq!(r[0].reasons, ["quota_exhausted"]);
    assert_eq!(r[1].reasons, ["quota_exhausted"]);
}

#[test]
fn settings_and_range_fail_before_any_network_or_import() {
    let mut s = settings();
    s.instruments[0].market.currency = NativeCurrency::USDT;
    assert!(s.validate().is_err());
    assert!(serde_json::from_value::<Settings>(
        json!({"version":SETTINGS_VERSION,"instruments":[],"token":"never stored"})
    )
    .is_err());
    let now = utc_date_to_ms("2026-07-08").unwrap();
    for (from, to) in [
        ("2026-07-07", "2026-07-07"),
        ("2026-07-01", "2026-07-08"),
        ("2024-01-01", "2026-01-01"),
    ] {
        assert!(validate_range(from, to, now).is_err());
    }
    let mut w = Workspace::new();
    let mut s = settings();
    s.instruments[0].market.calendar_to_exclusive = "2026-07-06".into();
    let f = FakeFetcher::new();
    let r = w.run(&f, &s);
    assert_eq!(r[0].reasons, ["calendar_out_of_range"]);
    assert!(f.requested().is_empty());
}

#[test]
fn strict_cli_never_accepts_tokens_or_ignored_options() {
    let args = [
        "--settings",
        "s.json",
        "--from",
        "2026-07-02",
        "--to",
        "2026-07-07",
    ];
    assert!(parse_args(&args).is_ok());
    for extra in [
        ["--token", "secret"],
        ["--from", "2026-07-03"],
        ["--interval", "1h"],
    ] {
        let mut values = args.to_vec();
        values.extend(extra);
        let e = parse_args(&values).unwrap_err();
        assert!(!e.contains("secret"));
    }
}

#[test]
fn checked_in_example_covers_all_defaults_but_cannot_claim_qualification() {
    let s: Settings =
        serde_json::from_str(include_str!("../../../../config/tiingo-2026.example.json")).unwrap();
    s.validate().unwrap();
    assert_eq!(s.instruments.len(), 5);
    assert!(s
        .instruments
        .iter()
        .all(|i| !i.corporate_actions_confirmed && i.costs.is_none()));
}

#[test]
fn all_five_symbols_can_independently_complete_without_cross_filling() {
    let mut w = Workspace::new();
    let mut s = settings();
    for symbol in &tiingo::SYMBOLS[1..] {
        let mut i = s.instruments[0].clone();
        i.market.instrument_id = format!("us-etf:nyse-arca:{symbol}");
        s.instruments.push(i);
    }
    let mut f = FakeFetcher::new();
    for symbol in tiingo::SYMBOLS {
        let u = tiingo::urls(symbol, "2026-07-02", "2026-07-06");
        let mut events = dividends(json!("2026-07-15"));
        events[0]["ticker"] = json!(symbol);
        f=f.with(&u[0],&serde_json::to_vec(&json!({"ticker":symbol,"exchangeCode":"NYSE ARCA","startDate":"1990-01-01","endDate":"2026-07-06"})).unwrap())
            .with(&u[1],&serde_json::to_vec(&rows()).unwrap()).with(&u[2],&serde_json::to_vec(&events).unwrap());
    }
    let reports = w.run(&f, &s);
    assert_eq!(reports.len(), 5);
    assert!(reports.iter().all(|r| r.qualification_eligible));
    assert_eq!(f.requested().len(), 15);
    let datasets: HashSet<_> = reports.iter().map(|r| r.dataset_id).collect();
    assert_eq!(datasets.len(), 5);
}

#[test]
fn metadata_identity_and_coverage_are_not_inferred_from_price_rows() {
    for (ticker, exchange, start, end) in [
        ("QQQ", "NYSE ARCA", "1990-01-01", "2026-07-06"),
        ("SPY", "NASDAQ", "1990-01-01", "2026-07-06"),
        ("SPY", "NYSE ARCA", "2026-07-06", "2026-07-06"),
        ("SPY", "NYSE ARCA", "1990-01-01", "2026-07-02"),
    ] {
        let u = tiingo::urls("SPY", "2026-07-02", "2026-07-06");
        let f = fetcher(rows(), dividends(json!("2026-07-15"))).with(
            &u[0],
            &serde_json::to_vec(&json!({
            "ticker":ticker,"exchangeCode":exchange,"startDate":start,"endDate":end}))
            .unwrap(),
        );
        let mut w = Workspace::new();
        let reports = w.run(&f, &settings());
        assert_eq!(reports[0].status, "BLOCKED");
        assert!(reports[0].dataset_id.is_none());
        assert_eq!(f.requested().len(), 1);
    }
}

#[test]
fn identical_retrievals_form_one_linear_revision_history() {
    let mut w = Workspace::new();
    let f = fetcher(rows(), dividends(json!("2026-07-15")));
    let s = settings();
    let a = w.run(&f, &s);
    let b = w.run(&f, &s);
    for index in 0..3 {
        let next = provenance::get_provenance(&w.conn, b[0].provenance_ids[index])
            .unwrap()
            .unwrap();
        assert_eq!(next.revision_of, Some(a[0].provenance_ids[index]));
    }
    assert_eq!(a[0].dataset_id, b[0].dataset_id);
    let mut modified = rows();
    modified[0]["volume"] = json!(1900);
    let c = w.run(&fetcher(modified, dividends(json!("2026-07-15"))), &s);
    let last = provenance::get_provenance(&w.conn, c[0].provenance_ids[1])
        .unwrap()
        .unwrap();
    assert_eq!(last.revision_of, Some(b[0].provenance_ids[1]));
}

#[test]
fn existing_currency_conflict_is_refused_and_richer_specs_are_preserved() {
    for currency in ["USD", "EUR"] {
        let mut w = Workspace::new();
        let s = settings();
        let i = &s.instruments[0];
        registry::register_calendar(&w.conn, &i.market.calendar).unwrap();
        registry::register_instrument(
            &w.conn,
            &InstrumentDraft {
                instrument_id: i.market.instrument_id.clone(),
                base: "SPY".into(),
                quote: currency.into(),
                asset_type: AssetType::Etf,
                session_calendar_id: i.market.calendar.calendar_id.clone(),
                timezone: "America/New_York".into(),
                lot_size: Some(1.0),
                price_step: Some(0.01),
                min_notional: None,
                listed_from: utc_date_to_ms(&i.listed_from),
                delisted_at: None,
                suspensions: vec![],
                source_capabilities: json!({"richer":true}),
            },
        )
        .unwrap();
        let reports = w.run(&fetcher(rows(), dividends(json!("2026-07-15"))), &s);
        let registered = registry::latest_instrument(&w.conn, &i.market.instrument_id)
            .unwrap()
            .unwrap();
        assert_eq!(registered.revision, 1);
        assert_eq!(registered.lot_size, Some(1.0));
        assert_eq!(registered.price_step, Some(0.01));
        if currency == "EUR" {
            assert_eq!(
                reports[0].reasons,
                ["registered_market_requires_reconciliation"]
            );
            assert!(reports[0].snapshot.is_none());
        } else {
            assert!(reports[0].qualification_eligible);
        }
    }
}
