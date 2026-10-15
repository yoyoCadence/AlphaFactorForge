use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use super::*;
use crate::db::{self, repositories};
use crate::market::provenance::{self, RawObservation};
use crate::market::snapshot::{self, SnapshotKind, SnapshotOutcome, SnapshotRequest};
use crate::research::artifacts::ArtifactStore;
use alpha_factor_forge::discovery_core::market_foundation::{PriceBasis, SeriesRole, Suspension};

const NOW: i64 = 1_796_400_000_000;
const LISTED: i64 = 1_502_942_400_000;
const SOURCE: &str = "Synthetic operator-reviewed fixture:\r\nBTC/USDT and ETH/USDT trade from 2017-08-17 12:00 UTC+8.\r\n保留原始位元組。\r\n";
const EXCERPT: &str = "BTC/USDT and ETH/USDT trade from 2017-08-17 12:00 UTC+8.";

struct TempDir(PathBuf);
impl TempDir {
    fn fresh() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "aff-crypto-listing-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn settings() -> Value {
    json!({
        "version": VERSION, "instrumentId": "crypto:binance:BTCUSDT",
        "listedFrom": "2017-08-17T04:00:00Z",
        "sourceUrl": "https://www.binance.com/ru/support/announcement/detail/115001094331",
        "retrievedAt": "2026-10-07T00:00:00Z", "rawFile": "source.txt",
        "rawSha256": sha256_hex(SOURCE.as_bytes()), "mediaType": "text/plain",
        "operatorConfirmed": true, "evidenceExcerpt": EXCERPT,
    })
}

fn write_settings(dir: &Path, value: &Value) -> PathBuf {
    let path = dir.join("settings.json");
    std::fs::write(dir.join("source.txt"), SOURCE.as_bytes()).unwrap();
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

fn evidence() -> ListingEvidence {
    let dir = TempDir::fresh();
    load_evidence(&write_settings(&dir.0, &settings()), NOW).unwrap()
}

fn memory_db() -> (Connection, i64) {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    db::apply_migrations(&conn).unwrap();
    registry::ensure_builtin_calendars(&conn).unwrap();
    let acquired = ownership::acquire(
        &mut conn,
        ownership::HolderKind::Service,
        std::process::id(),
    )
    .unwrap();
    (conn, acquired.epoch)
}

fn domain_state(conn: &Connection) -> (Vec<InstrumentRow>, i64) {
    (
        registry::instrument_revisions(conn, "crypto:binance:BTCUSDT").unwrap(),
        ownership::state_version(conn).unwrap(),
    )
}

#[test]
fn source_capture_round_trips_exactly_and_times_have_canonical_integer_milliseconds() {
    let capture = evidence();
    assert_eq!(capture.validate(NOW).unwrap(), LISTED);
    assert_eq!(capture.raw_content.as_bytes(), SOURCE.as_bytes());
    assert_eq!(capture.raw_byte_len, SOURCE.len() as u64);
    assert_eq!(capture.raw_sha256, sha256_hex(SOURCE.as_bytes()));
    assert_eq!(
        utc_millis("time", "2017-08-17T04:00:00.123Z").unwrap(),
        LISTED + 123
    );
    for bad in [
        "2017-08-17T12:00:00+08:00",
        "2017-08-17T04:00:00+00:00",
        "2017-08-17t04:00:00z",
        "2017-08-17T04:00:00.000Z",
        "2017-08-17T04:00:00.1234Z",
        "2017-08-17T04:00:60Z",
        "2017-8-17T04:00:00Z",
    ] {
        assert!(utc_millis("time", bad).is_err(), "{bad}");
    }
}

#[test]
fn committed_operator_example_has_the_exact_source_hash_and_registers_supported_defaults() {
    const RAW: &[u8] =
        include_bytes!("../../../../fixtures/binance/listing-usdt-announcement-excerpt.txt");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/binance/listing-btcusdt-settings-v1.json");
    let mut capture = load_evidence(&path, NOW).unwrap();
    assert_eq!(capture.raw_content.as_bytes(), RAW);
    assert_eq!(capture.raw_sha256, sha256_hex(RAW));
    let (conn, epoch) = memory_db();
    for instrument_id in ["crypto:binance:BTCUSDT", "crypto:binance:ETHUSDT"] {
        capture.instrument_id = instrument_id.into();
        let receipt = register_evidence(&conn, Some(epoch), &capture, NOW).unwrap();
        assert!(receipt.created);
        assert_eq!((receipt.revision, receipt.listed_from), (1, LISTED));
    }
    assert_eq!(registry::list_instruments(&conn).unwrap().len(), 2);
}

#[test]
fn invalid_inputs_refuse_before_creating_any_workspace() {
    let dir = TempDir::fresh();
    let mutations = [
        ("version", json!("future-version")),
        ("instrumentId", json!("crypto:binance:SOLUSDT")),
        ("instrumentId", json!("crypto:other:BTCUSDT")),
        ("listedFrom", json!("1999-12-31T00:00:00Z")),
        ("listedFrom", json!("2100-01-01T00:00:00Z")),
        ("listedFrom", json!("2027-01-01T00:00:00Z")),
        ("listedFrom", json!("2017-08-17T12:00:00+08:00")),
        ("listedFrom", json!("2016-12-31T23:59:60Z")),
        ("retrievedAt", json!("2017-08-16T00:00:00Z")),
        ("retrievedAt", json!("2016-12-31T23:59:60Z")),
        ("retrievedAt", json!("2099-01-01T00:00:00Z")),
        (
            "sourceUrl",
            json!("https://api.binance.com/api/v3/exchangeInfo"),
        ),
        (
            "sourceUrl",
            json!("https://data.binance.vision/?prefix=data/spot/monthly"),
        ),
        (
            "sourceUrl",
            json!("https://www.binance.com.evil/en/support/announcement/123"),
        ),
        (
            "sourceUrl",
            json!("https://evil@www.binance.com/en/support/announcement/123"),
        ),
        (
            "sourceUrl",
            json!("https://www.binance.com:8443/en/support/announcement/123"),
        ),
        (
            "sourceUrl",
            json!("https://www.binance.com/en/support/announcement/123?q=1"),
        ),
        (
            "sourceUrl",
            json!("https://www.binance.com/en/support/announcement/123#x"),
        ),
        (
            "sourceUrl",
            json!("https://www.binance.com/en/support/announcement/%2e%2e"),
        ),
        (
            "sourceUrl",
            json!("https://www.binance.com/en/support/announcement/../anything"),
        ),
        ("rawSha256", json!("0".repeat(64))),
        (
            "rawSha256",
            json!(sha256_hex(SOURCE.as_bytes()).to_uppercase()),
        ),
        ("mediaType", json!("application/zip")),
        ("operatorConfirmed", json!(false)),
        ("evidenceExcerpt", json!("")),
        ("evidenceExcerpt", json!("ETH/USDT")),
        (
            "evidenceExcerpt",
            json!("BTCUSDT does not appear in source"),
        ),
        ("unexpected", json!(true)),
        ("rawFile", json!("missing.txt")),
    ];
    for (index, (field, value)) in mutations.into_iter().enumerate() {
        let mut value_settings = settings();
        value_settings[field] = value;
        let settings = write_settings(&dir.0, &value_settings);
        let workspace = dir.0.join(format!("workspace-{index}"));
        assert!(
            run(
                &workspace,
                &Options {
                    settings,
                    data_dir: None
                }
            )
            .is_err(),
            "{field}"
        );
        assert!(!workspace.exists(), "invalid {field} opened the workspace");
    }
    let settings_path = write_settings(&dir.0, &settings());
    for invalid in [
        format!(
            "{{\"version\":\"{VERSION}\",{}",
            &serde_json::to_string(&settings()).unwrap()[1..]
        ),
        " ".repeat(MAX_SETTINGS_BYTES as usize + 1),
    ] {
        std::fs::write(&settings_path, invalid).unwrap();
        let workspace = dir.0.join("invalid-json-workspace");
        assert!(run(
            &workspace,
            &Options {
                settings: settings_path.clone(),
                data_dir: None
            }
        )
        .is_err());
        assert!(!workspace.exists());
    }
    for bytes in [
        vec![0xff, 0xfe],
        vec![b'x'; MAX_SOURCE_BYTES as usize + 1],
        vec![],
    ] {
        write_settings(&dir.0, &settings());
        std::fs::write(dir.0.join("source.txt"), bytes).unwrap();
        let workspace = dir.0.join("invalid-source-workspace");
        assert!(run(
            &workspace,
            &Options {
                settings: settings_path.clone(),
                data_dir: None
            }
        )
        .is_err());
        assert!(!workspace.exists());
    }
}

#[test]
fn append_retry_and_same_date_new_evidence_preserve_all_prior_market_metadata() {
    let (conn, epoch) = memory_db();
    let mut original = supported_draft("crypto:binance:BTCUSDT").unwrap();
    original.lot_size = Some(0.001);
    original.price_step = Some(0.01);
    original.min_notional = Some(5.0);
    original.delisted_at = Some(1_900_000_000_000);
    original.suspensions = vec![Suspension {
        from_ms: LISTED + 86_400_000,
        to_ms_exclusive: LISTED + 86_400_000 + 3_600_000,
    }];
    original.source_capabilities = json!({"binance": {"raw": true}, "operatorNote": "retained"});
    let (first_id, _) = registry::register_instrument(&conn, &original).unwrap();
    let first = registry::get_instrument(&conn, first_id).unwrap().unwrap();
    let before_seq = ownership::state_version(&conn).unwrap();
    let evidence = evidence();
    let receipt = register_evidence(&conn, Some(epoch), &evidence, NOW).unwrap();
    assert!(receipt.created);
    assert_eq!((receipt.revision, receipt.listed_from), (2, LISTED));
    assert_ne!(receipt.content_hash, first.content_hash);
    assert_eq!(ownership::state_version(&conn).unwrap(), before_seq + 1);
    let row = registry::get_instrument(&conn, receipt.row_id)
        .unwrap()
        .unwrap();
    let mut expected = original;
    expected.listed_from = Some(LISTED);
    expected.source_capabilities[EVIDENCE_KEY] = serde_json::to_value(&evidence).unwrap();
    assert_eq!(draft_from_row(&row), expected);
    assert_eq!(
        registry::get_instrument(&conn, first_id).unwrap().unwrap(),
        first
    );
    assert_eq!(
        row.source_capabilities[EVIDENCE_KEY]["rawContent"]
            .as_str()
            .unwrap()
            .as_bytes(),
        SOURCE.as_bytes()
    );
    let state = domain_state(&conn);
    let retry = register_evidence(&conn, Some(epoch), &evidence, NOW).unwrap();
    assert!(!retry.created);
    assert_eq!(
        (retry.row_id, retry.content_hash),
        (receipt.row_id, receipt.content_hash)
    );
    assert_eq!(
        domain_state(&conn),
        state,
        "exact retry is not a domain mutation"
    );
    let mut additional = evidence.clone();
    additional
        .raw_content
        .push_str("Additional operator capture.\r\n");
    additional.raw_byte_len = additional.raw_content.len() as u64;
    additional.raw_sha256 = sha256_hex(additional.raw_content.as_bytes());
    let second = register_evidence(&conn, Some(epoch), &additional, NOW).unwrap();
    assert!(second.created);
    assert_eq!(second.revision, 3);
    assert_ne!(second.content_hash, row.content_hash);
    assert_eq!(
        registry::get_instrument(&conn, row.id).unwrap().unwrap(),
        row
    );
    let state = domain_state(&conn);
    assert!(register_evidence(&conn, Some(epoch), &evidence, NOW)
        .unwrap_err()
        .to_string()
        .contains("superseded revision"));
    assert_eq!(domain_state(&conn), state);
    assert_eq!(
        registry::latest_instrument(&conn, &evidence.instrument_id)
            .unwrap()
            .unwrap()
            .id,
        second.row_id
    );
}

#[test]
fn stale_epoch_conflicting_listing_and_invalid_prior_evidence_rollback_domain_changes() {
    let evidence = evidence();
    let (conn, epoch) = memory_db();
    let initial = domain_state(&conn);
    assert!(matches!(
        register_evidence(&conn, Some(epoch + 1), &evidence, NOW),
        Err(AppError::StaleOwner(_))
    ));
    assert_eq!(domain_state(&conn), initial);
    let mut base = supported_draft(&evidence.instrument_id).unwrap();
    base.listed_from = Some(LISTED - 1);
    registry::register_instrument(&conn, &base).unwrap();
    let state = domain_state(&conn);
    assert!(register_evidence(&conn, Some(epoch), &evidence, NOW)
        .unwrap_err()
        .to_string()
        .contains("known listing differs"));
    assert_eq!(domain_state(&conn), state);
    for prior in [
        json!(null),
        json!({"version": "unknown"}),
        {
            let mut malformed = serde_json::to_value(&evidence).unwrap();
            malformed["rawSha256"] = json!("0".repeat(64));
            malformed
        },
        {
            let mut mismatched = serde_json::to_value(&evidence).unwrap();
            mismatched["instrumentId"] = json!("crypto:binance:ETHUSDT");
            mismatched
        },
    ] {
        let (conn, epoch) = memory_db();
        let mut base = supported_draft(&evidence.instrument_id).unwrap();
        base.listed_from = Some(LISTED);
        base.source_capabilities[EVIDENCE_KEY] = prior;
        registry::register_instrument(&conn, &base).unwrap();
        let state = domain_state(&conn);
        assert!(register_evidence(&conn, Some(epoch), &evidence, NOW).is_err());
        assert_eq!(domain_state(&conn), state);
    }
}

#[test]
fn a_live_workspace_owner_refuses_registration_before_database_opening() {
    let dir = TempDir::fresh();
    let settings = write_settings(&dir.0, &settings());
    let workspace = dir.0.join("workspace");
    let lock = runtime::lease::try_lock_workspace(&workspace).unwrap();
    assert!(matches!(
        run(
            &workspace,
            &Options {
                settings,
                data_dir: None
            }
        ),
        Err(AppError::NotOwner(_))
    ));
    assert!(!workspace.join(DB_FILE_NAME).exists());
    drop(lock);
}

#[test]
fn new_snapshot_identity_pins_listing_evidence_and_old_snapshot_stays_frozen() {
    let dir = TempDir::fresh();
    let store = ArtifactStore::in_data_dir(&dir.0);
    let (mut conn, epoch) = memory_db();
    let (old_id, _) =
        registry::register_instrument(&conn, &supported_draft("crypto:binance:BTCUSDT").unwrap())
            .unwrap();
    let old_instrument = registry::get_instrument(&conn, old_id).unwrap().unwrap();
    const START: i64 = 1_721_001_600_000;
    const HOUR: i64 = 3_600_000;
    let candles: Vec<_> = (0..4)
        .map(|index| repositories::Candle {
            timestamp: START + index * HOUR,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 10.0,
        })
        .collect();
    let mut dataset = repositories::Dataset {
        id: None,
        exchange: "binance".into(),
        symbol: "BTCUSDT".into(),
        interval: "1h".into(),
        start_time: START,
        end_time: START + 3 * HOUR,
        candle_count: 4,
        source: "exchange".into(),
        dataset_hash: String::new(),
    };
    dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, &candles).unwrap();
    let dataset_id =
        repositories::import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap();
    let (provenance_id, _) = provenance::record_raw(
        &conn,
        &store,
        &RawObservation {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            source: "binance-archive".into(),
            role: SeriesRole::Primary,
            request_scope: json!({"fixture": "synthetic offline candle evidence"}),
            retrieved_at: "2024-08-01T00:00:00Z".into(),
            available_at: Some("2024-08-01T00:00:00Z".into()),
            accepted: true,
            rejection_reason: None,
            revision_of: None,
            media_type: "text/csv".into(),
        },
        b"synthetic fixture, not real market data\n",
    )
    .unwrap();
    let request = SnapshotRequest {
        instrument_id: "crypto:binance:BTCUSDT".into(),
        interval: "1h".into(),
        dataset_id,
        price_basis: PriceBasis::Raw,
        corporate_action_version: None,
        cost_profile_version: Some("synthetic-smoke-only".into()),
        kind: SnapshotKind::Historical,
        provenance_ids: vec![provenance_id],
        as_of_ms: START + 4 * HOUR,
    };
    let SnapshotOutcome::Created(old_snapshot) =
        snapshot::build_snapshot(&mut conn, &request).unwrap()
    else {
        panic!("initial snapshot must be created");
    };
    let receipt = register_evidence(&conn, Some(epoch), &evidence(), NOW).unwrap();
    let SnapshotOutcome::Created(new_snapshot) =
        snapshot::build_snapshot(&mut conn, &request).unwrap()
    else {
        panic!("new instrument revision must create a distinct snapshot");
    };
    assert_ne!(new_snapshot.snapshot_id, old_snapshot.snapshot_id);
    assert_eq!(old_snapshot.instrument_row_id, old_id);
    assert_eq!(new_snapshot.instrument_row_id, receipt.row_id);
    assert_eq!(old_snapshot.dataset_hash, new_snapshot.dataset_hash);
    assert_eq!(
        snapshot::get_snapshot_by_id(&conn, &old_snapshot.snapshot_id)
            .unwrap()
            .unwrap(),
        old_snapshot
    );
    assert_eq!(
        registry::get_instrument(&conn, old_id).unwrap().unwrap(),
        old_instrument
    );
}
