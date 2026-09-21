use super::*;
use crate::market::http::testing::FakeFetcher;
use std::sync::atomic::{AtomicU64, Ordering};

fn settings() -> Settings {
    serde_json::from_value(json!({
        "version":SETTINGS_VERSION,
        "instruments":[{
            "market":{
                "instrumentId":"tw-etf:twse:0050","currency":"TWD",
                "calendar":{
                    "calendarId":"test-twse-2025-v1","kind":"trading-days","timezone":"Asia/Taipei",
                    "tradingWeekdays":[1,2,3,4,5],"holidays":[],"earlyCloses":[]
                },
                "calendarFrom":"2025-06-01","calendarToExclusive":"2025-07-01","sessionsPerYear":244
            },
            "listedFrom":"2003-06-30","listingEvidence":"synthetic TWSE listing fixture",
            "calendarEvidence":"synthetic TWSE calendar fixture",
            "suspensions":[{
                "from":"2025-06-11","toExclusive":"2025-06-18","evidence":"synthetic split halt fixture"
            }],
            "confirmedDividends":[{
                "exDate":"2025-06-19","paymentDate":"2025-07-10","amountPerShare":0.36,
                "evidence":"synthetic official dividend fixture"
            }],
            "confirmedSplits":[{
                "effectiveDate":"2025-06-18","ratio":4,"evidence":"synthetic official split fixture"
            }],
            "corporateActionsConfirmed":true,"actionEvidence":"synthetic official action fixture",
            "costs":{"version":"fixture-twd-v1","currency":"TWD","confirmed":true,
                "commissionRate":0.001425,"minimumCommission":1,"slippageRate":0.001,
                "buyTaxRate":0,"sellTaxRate":0.001}
        }]
    })).unwrap()
}

fn envelope(data: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"msg":"success","status":200,"data":data})).unwrap()
}

fn calendar() -> Vec<u8> {
    envelope(json!([
        {"date":"2025-06-09"},{"date":"2025-06-10"},{"date":"2025-06-11"},
        {"date":"2025-06-12"},{"date":"2025-06-13"},{"date":"2025-06-16"},
        {"date":"2025-06-17"},{"date":"2025-06-18"},{"date":"2025-06-19"},
        {"date":"2025-06-20"}
    ]))
}

fn price_values() -> Vec<Value> {
    [
        (
            "2025-06-09",
            188.0,
            190.0,
            187.0,
            189.0,
            1.0,
            1_000u64,
            189_000u64,
            100u64,
        ),
        (
            "2025-06-10",
            189.0,
            191.0,
            188.0,
            190.0,
            1.0,
            1_100,
            209_000,
            110,
        ),
        (
            "2025-06-18",
            47.0,
            48.25,
            46.95,
            47.65,
            0.49,
            291_042_332,
            13_878_058_322,
            242_995,
        ),
        (
            "2025-06-19",
            47.8,
            48.0,
            47.2,
            47.5,
            -0.15,
            2_000,
            95_000,
            200,
        ),
        (
            "2025-06-20",
            47.6,
            48.1,
            47.4,
            48.0,
            0.5,
            2_100,
            100_800,
            210,
        ),
    ]
    .into_iter()
    .map(
        |(date, open, high, low, close, spread, volume, money, turnover)| {
            json!({
                "date":date,"stock_id":"0050","Trading_Volume":volume,"Trading_money":money,
                "open":open,"max":high,"min":low,"close":close,"spread":spread,
                "Trading_turnover":turnover
            })
        },
    )
    .collect()
}

fn prices() -> Vec<u8> {
    envelope(Value::Array(price_values()))
}

fn twse_rows() -> Vec<Value> {
    price_values()
        .into_iter()
        .map(|row| {
            let date = row["date"].as_str().unwrap();
            let roc = format!("114/06/{}", &date[8..10]);
            json!([
                roc,
                format!("{}", row["Trading_Volume"].as_u64().unwrap()),
                format!("{}", row["Trading_money"].as_u64().unwrap()),
                format!("{:.2}", row["open"].as_f64().unwrap()),
                format!("{:.2}", row["max"].as_f64().unwrap()),
                format!("{:.2}", row["min"].as_f64().unwrap()),
                format!("{:.2}", row["close"].as_f64().unwrap()),
                format!("{:+.2}", row["spread"].as_f64().unwrap()),
                format!("{}", row["Trading_turnover"].as_u64().unwrap()),
                ""
            ])
        })
        .collect()
}

fn twse_response(rows: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "stat":"OK","date":"20250601","title":"114年06月 0050 元大台灣50 各日成交資訊",
        "fields":["日期","成交股數","成交金額","開盤價","最高價","最低價","收盤價","漲跌價差","成交筆數","註記"],
        "data":rows
    })).unwrap()
}

fn dividends(payment: Option<&str>) -> Vec<u8> {
    envelope(json!([{
        "stock_id":"0050","StockEarningsDistribution":0,"StockStatutorySurplus":0,
        "CashEarningsDistribution":0.36,"CashStatutorySurplus":0,
        "CashExDividendTradingDate":"2025-06-19","CashDividendPaymentDate":payment
    }]))
}

fn splits(present: bool) -> Vec<u8> {
    envelope(if present {
        json!([{
            "date":"2025-06-18","stock_id":"0050","type":"分割",
            "before_price":188.65,"after_price":47.16
        }])
    } else {
        json!([])
    })
}

fn fetcher(
    price_bytes: Vec<u8>,
    twse_bytes: Vec<u8>,
    dividend_bytes: Vec<u8>,
    split_bytes: Vec<u8>,
) -> FakeFetcher {
    let urls = finmind::urls(
        "0050",
        "2025-06-09",
        "2025-06-20",
        "2025-01-01",
        "2025-12-31",
    );
    FakeFetcher::new()
        .with(&urls[0], &calendar())
        .with(&urls[1], &price_bytes)
        .with(&urls[2], &dividend_bytes)
        .with(&urls[3], &split_bytes)
        .with(&twse::month_url("0050", 2025, 6), &twse_bytes)
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
            "aff-tw-etf-test-{}-{}",
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

    fn run(&mut self, fetcher: &dyn HttpFetcher, settings: &Settings) -> Vec<Report> {
        ingest(
            &mut self.conn,
            &self.store,
            fetcher,
            settings,
            "2025-06-09",
            "2025-06-21",
            utc_date_to_ms("2025-06-22").unwrap() + 9 * 3_600_000,
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
fn complete_0050_flow_handles_halt_split_dividend_roc_date_and_share_volume() {
    let mut workspace = Workspace::new();
    let fetcher = fetcher(
        prices(),
        twse_response(twse_rows()),
        dividends(Some("2025-07-10")),
        splits(true),
    );
    let reports = workspace.run(&fetcher, &settings());
    let report = &reports[0];
    assert_eq!(reports.len(), 5);
    assert_eq!(report.status, "OK");
    assert!(report.qualification_eligible);
    assert_eq!(report.bars, 5);
    assert_eq!(report.twse_reconciled_months, 1);
    assert_eq!(
        report.dividends[0].payment_date.as_deref(),
        Some("2025-07-10")
    );
    assert_eq!(report.splits[0].ratio, 4.0);
    assert!(report.splits[0].available_at_ms > utc_date_to_ms("2025-06-18").unwrap());
    let candles =
        repositories::get_candles(&workspace.conn, report.dataset_id.unwrap(), 0, i64::MAX)
            .unwrap();
    assert_eq!(candles.len(), 5);
    assert_eq!(candles[0].close, 189.0);
    assert_eq!(candles[2].close, 47.65);
    assert_eq!(candles[2].volume, 291_042_332.0);
    let snapshot = report.snapshot.as_ref().unwrap();
    let restored = snapshot::get_snapshot(&workspace.conn, snapshot.id)
        .unwrap()
        .unwrap();
    assert_eq!(restored.price_basis, PriceBasis::Raw);
    assert!(reports[1..]
        .iter()
        .all(|item| item.reasons == ["source_settings_missing"]));
    assert_eq!(fetcher.requested().len(), 5);
}

#[test]
fn twse_mismatch_and_missing_finmind_bar_are_never_cross_filled() {
    let mut changed = twse_rows();
    changed[2][1] = json!("291042");
    let mut workspace = Workspace::new();
    let reports = workspace.run(
        &fetcher(
            prices(),
            twse_response(changed),
            dividends(Some("2025-07-10")),
            splits(true),
        ),
        &settings(),
    );
    assert_eq!(reports[0].reasons, ["twse_value_mismatch"]);
    assert!(reports[0].dataset_id.is_none());

    let mut missing = price_values();
    missing.remove(3);
    let mut workspace = Workspace::new();
    let fetcher = fetcher(
        envelope(Value::Array(missing)),
        twse_response(twse_rows()),
        dividends(Some("2025-07-10")),
        splits(true),
    );
    let reports = workspace.run(&fetcher, &settings());
    assert_eq!(
        reports[0].reasons,
        ["requested_range_incomplete_or_off_session"]
    );
    assert!(reports[0].dataset_id.is_none());
    assert_eq!(fetcher.requested().len(), 2);
}

#[test]
fn missing_split_evidence_or_payment_date_degrades_or_blocks_explicitly() {
    let mut workspace = Workspace::new();
    let reports = workspace.run(
        &fetcher(
            prices(),
            twse_response(twse_rows()),
            dividends(Some("2025-07-10")),
            splits(false),
        ),
        &settings(),
    );
    assert_eq!(reports[0].reasons, ["split_evidence_mismatch"]);
    assert!(reports[0].dataset_id.is_none());

    let mut settings_without_confirmed_payment = settings();
    settings_without_confirmed_payment.instruments[0]
        .confirmed_dividends
        .clear();
    settings_without_confirmed_payment.instruments[0].corporate_actions_confirmed = false;
    let mut workspace = Workspace::new();
    let reports = workspace.run(
        &fetcher(
            prices(),
            twse_response(twse_rows()),
            dividends(None),
            splits(true),
        ),
        &settings_without_confirmed_payment,
    );
    assert_eq!(reports[0].status, "DEGRADED");
    assert_eq!(
        reports[0].reasons,
        ["corporate_actions_unconfirmed", "unknown_payment_date"]
    );
    assert!(reports[0].snapshot.is_some());
    assert!(!reports[0].qualification_eligible);
}

#[test]
fn a_dividend_missing_from_either_event_list_is_blocked() {
    let mut workspace = Workspace::new();
    let reports = workspace.run(
        &fetcher(
            prices(),
            twse_response(twse_rows()),
            envelope(json!([])),
            splits(true),
        ),
        &settings(),
    );
    assert_eq!(reports[0].reasons, ["dividend_evidence_mismatch"]);
    assert!(reports[0].dataset_id.is_none());

    let mut unexpected = settings();
    unexpected.instruments[0].confirmed_dividends.clear();
    let mut workspace = Workspace::new();
    let reports = workspace.run(
        &fetcher(
            prices(),
            twse_response(twse_rows()),
            dividends(Some("2025-07-10")),
            splits(true),
        ),
        &unexpected,
    );
    assert_eq!(reports[0].reasons, ["dividend_evidence_mismatch"]);
}

#[test]
fn halt_evidence_is_required_to_explain_0050_absence() {
    let mut settings = settings();
    settings.instruments[0].suspensions.clear();
    let mut workspace = Workspace::new();
    let reports = workspace.run(
        &fetcher(
            prices(),
            twse_response(twse_rows()),
            dividends(Some("2025-07-10")),
            splits(true),
        ),
        &settings,
    );
    assert_eq!(
        reports[0].reasons,
        ["requested_range_incomplete_or_off_session"]
    );
    assert!(reports[0].dataset_id.is_none());
}

#[test]
fn revisions_are_linked_and_originals_remain_reopenable() {
    let mut workspace = Workspace::new();
    let settings = settings();
    let first = workspace.run(
        &fetcher(
            prices(),
            twse_response(twse_rows()),
            dividends(Some("2025-07-10")),
            splits(true),
        ),
        &settings,
    );
    let mut changed = price_values();
    changed[0]["Trading_Volume"] = json!(1_001);
    let mut official = twse_rows();
    official[0][1] = json!("1001");
    let second = workspace.run(
        &fetcher(
            envelope(Value::Array(changed)),
            twse_response(official),
            dividends(Some("2025-07-10")),
            splits(true),
        ),
        &settings,
    );
    assert_ne!(first[0].dataset_id, second[0].dataset_id);
    for index in 0..5 {
        let old = provenance::get_provenance(&workspace.conn, first[0].provenance_ids[index])
            .unwrap()
            .unwrap();
        let new = provenance::get_provenance(&workspace.conn, second[0].provenance_ids[index])
            .unwrap()
            .unwrap();
        assert_eq!(new.revision_of, Some(old.id));
        assert!(workspace.store.read(&old.artifact_ref()).is_ok());
        assert!(workspace.store.read(&new.artifact_ref()).is_ok());
    }
}

#[test]
fn settings_range_and_cli_fail_closed_before_ignored_options() {
    let mut settings = settings();
    settings.instruments[0].market.currency = NativeCurrency::USD;
    assert!(settings.validate().is_err());
    assert!(serde_json::from_value::<Settings>(json!({
        "version":SETTINGS_VERSION,"instruments":[],"token":"not accepted"
    }))
    .is_err());
    let now = utc_date_to_ms("2025-07-10").unwrap();
    for (from, to) in [
        ("2025-06-09", "2025-06-09"),
        ("2025-06-01", "2025-07-03"),
        ("2025-07-09", "2025-07-10"),
    ] {
        assert!(validate_range(from, to, now).is_err());
    }
    let args = [
        "--settings",
        "s.json",
        "--from",
        "2025-06-09",
        "--to",
        "2025-06-21",
    ];
    assert!(parse_args(&args).is_ok());
    for extra in [
        ["--interval", "1h"],
        ["--token", "secret"],
        ["--from", "2025-06-10"],
    ] {
        let mut values = args.to_vec();
        values.extend(extra);
        let message = parse_args(&values).unwrap_err();
        assert!(!message.contains("secret"));
    }
}

#[test]
fn checked_in_2025_settings_cover_all_defaults_and_keep_costs_explicitly_unconfirmed() {
    let settings: Settings =
        serde_json::from_str(include_str!("../../../../config/tw-etf-2025.example.json")).unwrap();
    settings.validate().unwrap();
    assert_eq!(settings.instruments.len(), finmind::SYMBOLS.len());
    assert_eq!(settings.instruments[0].confirmed_splits[0].ratio, 4.0);
    assert_eq!(settings.instruments[0].suspensions[0].from, "2025-06-11");
    assert!(settings.instruments.iter().all(|item| {
        item.corporate_actions_confirmed
            && !item.confirmed_dividends.is_empty()
            && item.costs.is_none()
    }));
}
