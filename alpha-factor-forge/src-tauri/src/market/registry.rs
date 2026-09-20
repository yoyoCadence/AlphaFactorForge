//! P06 — the instrument and calendar registry (`market_instruments`,
//! `market_calendars` in migration 0008).
//!
//! Two append-only registries with one rule each:
//!
//! * a **calendar id carries its version**, so a calendar row is written
//!   once and never edited. A corrected calendar is a new id, and every
//!   snapshot that named the old one still means what it meant.
//! * an **instrument is a chain of revisions**. Re-registering identical
//!   content is the same row; a change adds revision n+1 and leaves n for
//!   whatever already referenced it.
//!
//! Registering an instrument requires its calendar to exist. That is the
//! honest blocking point for the ETF markets: `nyse-v1` and `twse-v1` hold
//! real holiday data that this phase does not have and does not invent, so
//! those instruments cannot be registered until P09/P10 supply it.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use alpha_factor_forge::discovery_core::market_foundation::{
    parse_instrument_id, validate_calendar, AssetType, CalendarKind, Market, SessionCalendar,
    Suspension, MARKET_INSTRUMENT_VERSION, SESSION_CALENDAR_VERSION,
};
use crate::error::{AppError, AppResult};

use super::{canonical_json, sha256_hex};

// ------------------------------------------------------------- calendars

/// The one calendar this phase can define from the contract alone: crypto
/// spot trades continuously, so it needs no holiday source.
pub const CRYPTO_24X7_CALENDAR_ID: &str = "crypto-24x7-v1";

pub fn crypto_24x7_calendar() -> SessionCalendar {
    SessionCalendar {
        calendar_id: CRYPTO_24X7_CALENDAR_ID.to_string(),
        kind: CalendarKind::Continuous,
        timezone: "UTC".to_string(),
        trading_weekdays: Vec::new(),
        holidays: Vec::new(),
        early_closes: Vec::new(),
    }
}

fn calendar_content(calendar: &SessionCalendar) -> AppResult<Value> {
    let mut content = serde_json::to_value(calendar)?;
    content["version"] = json!(SESSION_CALENDAR_VERSION);
    Ok(content)
}

/// Register a calendar version. Returns its row id and whether this call
/// created it. Registering the same id with different content is refused:
/// a calendar version is immutable, and a correction is a new id.
pub fn register_calendar(conn: &Connection, calendar: &SessionCalendar) -> AppResult<(i64, bool)> {
    if let Some(rule) = validate_calendar(calendar) {
        return Err(AppError::Other(format!(
            "calendar {} is not valid: {}",
            calendar.calendar_id,
            rule.as_str()
        )));
    }
    let content = calendar_content(calendar)?;
    let hash = sha256_hex(&canonical_json(&content)?);
    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, content_hash FROM market_calendars WHERE calendar_id = ?1",
            [&calendar.calendar_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((id, existing_hash)) = existing {
        if existing_hash != hash {
            return Err(AppError::Other(format!(
                "calendar {} already exists with different content; register a new calendar id",
                calendar.calendar_id
            )));
        }
        return Ok((id, false));
    }
    conn.execute(
        "INSERT INTO market_calendars (calendar_id, version, kind, timezone, content_hash, definition_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            calendar.calendar_id,
            SESSION_CALENDAR_VERSION,
            calendar.kind.as_str(),
            calendar.timezone,
            hash,
            String::from_utf8(canonical_json(&content)?).map_err(|error| AppError::Other(error.to_string()))?,
        ],
    )?;
    Ok((conn.last_insert_rowid(), true))
}

/// The built-in calendars a workspace always has. Idempotent, and called by
/// the owner right after migration — a second call registers nothing.
pub fn ensure_builtin_calendars(conn: &Connection) -> AppResult<()> {
    register_calendar(conn, &crypto_24x7_calendar())?;
    Ok(())
}

fn calendar_from_row(row: &Row<'_>) -> rusqlite::Result<SessionCalendar> {
    let definition: String = row.get(0)?;
    serde_json::from_str(&definition).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

pub fn get_calendar(conn: &Connection, calendar_id: &str) -> AppResult<Option<SessionCalendar>> {
    Ok(conn
        .query_row(
            "SELECT definition_json FROM market_calendars WHERE calendar_id = ?1",
            [calendar_id],
            calendar_from_row,
        )
        .optional()?)
}

pub fn list_calendars(conn: &Connection) -> AppResult<Vec<SessionCalendar>> {
    let mut stmt =
        conn.prepare("SELECT definition_json FROM market_calendars ORDER BY calendar_id")?;
    let rows = stmt
        .query_map([], calendar_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ------------------------------------------------------------ instruments

/// What an instrument is, as registered (`market-instrument-v1`).
///
/// The trading specification is optional because a source reports it; an
/// instrument without one is researchable and NOT paper-tradable (P19/P20
/// enforce that, this contract records it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentDraft {
    pub instrument_id: String,
    pub base: String,
    pub quote: String,
    pub asset_type: AssetType,
    pub session_calendar_id: String,
    pub timezone: String,
    pub lot_size: Option<f64>,
    pub price_step: Option<f64>,
    pub min_notional: Option<f64>,
    pub listed_from: Option<i64>,
    pub delisted_at: Option<i64>,
    /// Halted periods; bars inside them are not expected to exist.
    pub suspensions: Vec<Suspension>,
    /// What each source can actually provide (raw / adjusted / dividend /
    /// split) and under which entitlement. `{}` means "nothing verified".
    pub source_capabilities: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentRow {
    pub id: i64,
    pub instrument_id: String,
    pub revision: i64,
    pub content_hash: String,
    pub version: String,
    pub market: Market,
    pub venue: String,
    pub symbol: String,
    pub base: String,
    pub quote: String,
    pub asset_type: AssetType,
    pub session_calendar_id: String,
    pub timezone: String,
    pub lot_size: Option<f64>,
    pub price_step: Option<f64>,
    pub min_notional: Option<f64>,
    pub listed_from: Option<i64>,
    pub delisted_at: Option<i64>,
    pub suspensions: Vec<Suspension>,
    pub source_capabilities: Value,
    pub created_at: String,
}

impl InstrumentRow {
    /// Paper trading needs a trading specification; research does not.
    /// Recorded here so P19/P20 read one answer instead of re-deriving it.
    pub fn has_trading_specification(&self) -> bool {
        self.lot_size.is_some() && self.price_step.is_some()
    }
}

impl InstrumentDraft {
    fn content(&self) -> AppResult<Value> {
        let mut content = serde_json::to_value(self)?;
        content["version"] = json!(MARKET_INSTRUMENT_VERSION);
        Ok(content)
    }

    fn hash(&self) -> AppResult<String> {
        Ok(sha256_hex(&canonical_json(&self.content()?)?))
    }
}

fn positive_spec(name: &str, value: Option<f64>) -> AppResult<()> {
    match value {
        Some(number) if !number.is_finite() || number <= 0.0 => Err(AppError::Other(format!(
            "instrument {name} must be a finite positive number when it is known"
        ))),
        _ => Ok(()),
    }
}

/// Register an instrument revision. Returns its row id and whether this
/// call created it; identical content re-registers as the same row.
pub fn register_instrument(conn: &Connection, draft: &InstrumentDraft) -> AppResult<(i64, bool)> {
    let parsed = parse_instrument_id(&draft.instrument_id).map_err(|rule| {
        AppError::Other(format!(
            "instrument id {:?} is not valid: {}",
            draft.instrument_id,
            rule.as_str()
        ))
    })?;
    let expected_asset_type = match parsed.market {
        Market::Crypto => AssetType::SpotCrypto,
        Market::UsEtf | Market::TwEtf => AssetType::Etf,
    };
    if draft.asset_type != expected_asset_type {
        return Err(AppError::Other(format!(
            "market {} holds {} instruments, not {}",
            parsed.market.as_str(),
            expected_asset_type.as_str(),
            draft.asset_type.as_str()
        )));
    }
    for (name, value) in [("base", &draft.base), ("quote", &draft.quote), ("timezone", &draft.timezone)] {
        if value.trim().is_empty() || value.trim() != value {
            return Err(AppError::Other(format!(
                "instrument {name} must be a non-empty value without surrounding whitespace"
            )));
        }
    }
    positive_spec("lotSize", draft.lot_size)?;
    positive_spec("priceStep", draft.price_step)?;
    positive_spec("minNotional", draft.min_notional)?;
    if let (Some(listed), Some(delisted)) = (draft.listed_from, draft.delisted_at) {
        if delisted <= listed {
            return Err(AppError::Other(
                "an instrument's delisting must come after its listing".into(),
            ));
        }
    }
    let mut previous_end: Option<i64> = None;
    for suspension in &draft.suspensions {
        if suspension.to_ms_exclusive <= suspension.from_ms {
            return Err(AppError::Other("a suspension ends after it starts".into()));
        }
        if previous_end.is_some_and(|end| suspension.from_ms < end) {
            return Err(AppError::Other(
                "suspensions must be sorted and must not overlap".into(),
            ));
        }
        previous_end = Some(suspension.to_ms_exclusive);
    }
    if !draft.source_capabilities.is_object() {
        return Err(AppError::Other(
            "sourceCapabilities is an object describing what each source provides".into(),
        ));
    }
    let Some(calendar) = get_calendar(conn, &draft.session_calendar_id)? else {
        return Err(AppError::Other(format!(
            "calendar {} is not registered; register its version before the instruments that use it",
            draft.session_calendar_id
        )));
    };
    let expected_kind = match draft.asset_type {
        AssetType::SpotCrypto => CalendarKind::Continuous,
        AssetType::Etf => CalendarKind::TradingDays,
    };
    if calendar.kind != expected_kind {
        return Err(AppError::Other(format!(
            "{} instruments need a {} calendar, but {} is {}",
            draft.asset_type.as_str(),
            expected_kind.as_str(),
            calendar.calendar_id,
            calendar.kind.as_str()
        )));
    }

    let hash = draft.hash()?;
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM market_instruments WHERE content_hash = ?1",
            [&hash],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok((id, false));
    }
    let revision: i64 = conn.query_row(
        "SELECT COALESCE(MAX(revision), 0) + 1 FROM market_instruments WHERE instrument_id = ?1",
        [&draft.instrument_id],
        |row| row.get(0),
    )?;
    let content = draft.content()?;
    conn.execute(
        "INSERT INTO market_instruments
            (instrument_id, revision, content_hash, version, market, venue, symbol, base, quote,
             asset_type, session_calendar_id, timezone, lot_size, price_step, min_notional,
             listed_from, delisted_at, suspensions_json, source_capabilities_json, content_json)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
        params![
            draft.instrument_id,
            revision,
            hash,
            MARKET_INSTRUMENT_VERSION,
            parsed.market.as_str(),
            parsed.venue,
            parsed.symbol,
            draft.base,
            draft.quote,
            draft.asset_type.as_str(),
            draft.session_calendar_id,
            draft.timezone,
            draft.lot_size,
            draft.price_step,
            draft.min_notional,
            draft.listed_from,
            draft.delisted_at,
            serde_json::to_string(&draft.suspensions)?,
            serde_json::to_string(&draft.source_capabilities)?,
            serde_json::to_string(&content)?,
        ],
    )?;
    Ok((conn.last_insert_rowid(), true))
}

const INSTRUMENT_COLUMNS: &str = "id, instrument_id, revision, content_hash, version, market, venue, symbol, base, quote,
    asset_type, session_calendar_id, timezone, lot_size, price_step, min_notional, listed_from,
    delisted_at, suspensions_json, source_capabilities_json, created_at";

fn instrument_from_row(row: &Row<'_>) -> rusqlite::Result<InstrumentRow> {
    let market: String = row.get(5)?;
    let asset_type: String = row.get(10)?;
    let suspensions: String = row.get(18)?;
    let capabilities: String = row.get(19)?;
    Ok(InstrumentRow {
        id: row.get(0)?,
        instrument_id: row.get(1)?,
        revision: row.get(2)?,
        content_hash: row.get(3)?,
        version: row.get(4)?,
        market: Market::parse(&market).unwrap_or(Market::Crypto),
        venue: row.get(6)?,
        symbol: row.get(7)?,
        base: row.get(8)?,
        quote: row.get(9)?,
        asset_type: AssetType::parse(&asset_type).unwrap_or(AssetType::Etf),
        session_calendar_id: row.get(11)?,
        timezone: row.get(12)?,
        lot_size: row.get(13)?,
        price_step: row.get(14)?,
        min_notional: row.get(15)?,
        listed_from: row.get(16)?,
        delisted_at: row.get(17)?,
        suspensions: serde_json::from_str(&suspensions).unwrap_or_default(),
        source_capabilities: serde_json::from_str(&capabilities).unwrap_or(Value::Null),
        created_at: row.get(20)?,
    })
}

/// The newest revision of one instrument.
pub fn latest_instrument(conn: &Connection, instrument_id: &str) -> AppResult<Option<InstrumentRow>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {INSTRUMENT_COLUMNS} FROM market_instruments
                 WHERE instrument_id = ?1 ORDER BY revision DESC LIMIT 1"
            ),
            [instrument_id],
            instrument_from_row,
        )
        .optional()?)
}

/// One exact revision, by row id — what a snapshot froze.
pub fn get_instrument(conn: &Connection, row_id: i64) -> AppResult<Option<InstrumentRow>> {
    Ok(conn
        .query_row(
            &format!("SELECT {INSTRUMENT_COLUMNS} FROM market_instruments WHERE id = ?1"),
            [row_id],
            instrument_from_row,
        )
        .optional()?)
}

/// Every instrument's newest revision, by instrument id.
pub fn list_instruments(conn: &Connection) -> AppResult<Vec<InstrumentRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {INSTRUMENT_COLUMNS} FROM market_instruments
         WHERE (instrument_id, revision) IN
             (SELECT instrument_id, MAX(revision) FROM market_instruments GROUP BY instrument_id)
         ORDER BY instrument_id"
    ))?;
    let rows = stmt
        .query_map([], instrument_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Every revision of one instrument, oldest first.
pub fn instrument_revisions(conn: &Connection, instrument_id: &str) -> AppResult<Vec<InstrumentRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {INSTRUMENT_COLUMNS} FROM market_instruments
         WHERE instrument_id = ?1 ORDER BY revision ASC"
    ))?;
    let rows = stmt
        .query_map([instrument_id], instrument_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// --------------------------------------------------- the research shortlist

/// The editable default research list (`docs/market-contract.md` §1.1). It
/// is a starting scope, **not** an investment recommendation, and it is data
/// rather than a registration: only the crypto pair's venue is established
/// by this phase's own source decision.
pub const DEFAULT_RESEARCH_SYMBOLS: [(&str, &str, &str); 12] = [
    ("crypto", "BTCUSDT", "1h"),
    ("crypto", "ETHUSDT", "1h"),
    ("us-etf", "SPY", "1d"),
    ("us-etf", "QQQ", "1d"),
    ("us-etf", "VTI", "1d"),
    ("us-etf", "TLT", "1d"),
    ("us-etf", "GLD", "1d"),
    ("tw-etf", "0050", "1d"),
    ("tw-etf", "006208", "1d"),
    ("tw-etf", "0056", "1d"),
    ("tw-etf", "00878", "1d"),
    ("tw-etf", "00713", "1d"),
];

/// The two instruments this phase can describe completely: the Binance spot
/// pairs the crypto adapter (P07) will fetch. The ETF rows of
/// `DEFAULT_RESEARCH_SYMBOLS` are deliberately absent — their venue, listing
/// dates, and trading specification come from their own sources (P09/P10),
/// and inventing them here would be a fabricated market fact.
pub fn default_crypto_instruments() -> Vec<InstrumentDraft> {
    ["BTC", "ETH"]
        .into_iter()
        .map(|base| InstrumentDraft {
            instrument_id: format!("crypto:binance:{base}USDT"),
            base: base.to_string(),
            quote: "USDT".to_string(),
            asset_type: AssetType::SpotCrypto,
            session_calendar_id: CRYPTO_24X7_CALENDAR_ID.to_string(),
            timezone: "UTC".to_string(),
            lot_size: None,
            price_step: None,
            min_notional: None,
            listed_from: None,
            delisted_at: None,
            suspensions: Vec::new(),
            source_capabilities: json!({}),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON").expect("foreign keys");
        db::apply_migrations(&conn).expect("migrations");
        conn
    }

    fn etf_calendar(id: &str) -> SessionCalendar {
        SessionCalendar {
            calendar_id: id.to_string(),
            kind: CalendarKind::TradingDays,
            timezone: "America/New_York".to_string(),
            trading_weekdays: vec![1, 2, 3, 4, 5],
            holidays: vec!["2024-07-04".to_string()],
            early_closes: Vec::new(),
        }
    }

    #[test]
    fn a_calendar_version_is_registered_once_and_never_edited() {
        let conn = memory_db();
        ensure_builtin_calendars(&conn).unwrap();
        // The built-in is there, and a second ensure adds nothing.
        ensure_builtin_calendars(&conn).unwrap();
        let (id, created) = register_calendar(&conn, &crypto_24x7_calendar()).unwrap();
        assert!(!created, "the same content is the same row");
        assert_eq!(
            get_calendar(&conn, CRYPTO_24X7_CALENDAR_ID).unwrap(),
            Some(crypto_24x7_calendar())
        );

        let corrected = SessionCalendar {
            timezone: "Etc/UTC".to_string(),
            ..crypto_24x7_calendar()
        };
        let error = register_calendar(&conn, &corrected).unwrap_err().to_string();
        assert!(error.contains("register a new calendar id"), "{error}");
        // ... and the row really cannot be edited underneath it.
        let refused = conn
            .execute(
                "UPDATE market_calendars SET timezone = 'Etc/UTC' WHERE id = ?1",
                [id],
            )
            .unwrap_err()
            .to_string();
        assert!(refused.contains("immutable"), "{refused}");
        assert!(conn
            .execute("DELETE FROM market_calendars WHERE id = ?1", [id])
            .unwrap_err()
            .to_string()
            .contains("never deleted"));

        let invalid = SessionCalendar {
            calendar_id: "nyse".to_string(),
            ..etf_calendar("nyse-v1")
        };
        assert!(register_calendar(&conn, &invalid)
            .unwrap_err()
            .to_string()
            .contains("calendar_id_shape"));
    }

    #[test]
    fn an_instrument_is_a_chain_of_revisions_and_identical_content_is_one_row() {
        let conn = memory_db();
        ensure_builtin_calendars(&conn).unwrap();
        let draft = default_crypto_instruments()
            .into_iter()
            .next()
            .expect("BTCUSDT");
        let (first, created) = register_instrument(&conn, &draft).unwrap();
        assert!(created);
        assert_eq!(register_instrument(&conn, &draft).unwrap(), (first, false));

        // A source later reports the trading specification: revision 2.
        let specified = InstrumentDraft {
            lot_size: Some(0.00001),
            price_step: Some(0.01),
            min_notional: Some(5.0),
            source_capabilities: json!({ "binance-archive": { "raw": true } }),
            ..draft.clone()
        };
        let (second, created) = register_instrument(&conn, &specified).unwrap();
        assert!(created && second != first);
        let latest = latest_instrument(&conn, &draft.instrument_id).unwrap().expect("latest");
        assert_eq!((latest.revision, latest.id), (2, second));
        assert!(latest.has_trading_specification());
        let revisions = instrument_revisions(&conn, &draft.instrument_id).unwrap();
        assert_eq!(revisions.len(), 2);
        assert!(!revisions[0].has_trading_specification(), "revision 1 is unchanged");
        assert_eq!(revisions[0].market, Market::Crypto);
        assert_eq!(revisions[0].venue, "binance");
        assert_eq!(revisions[0].symbol, "BTCUSDT");
        assert_eq!(revisions[0].quote, "USDT", "USDT is never renamed USD");
        assert_eq!(list_instruments(&conn).unwrap().len(), 1, "the newest revision only");

        assert!(conn
            .execute("UPDATE market_instruments SET quote = 'USD' WHERE id = ?1", [first])
            .unwrap_err()
            .to_string()
            .contains("immutable"));
    }

    #[test]
    fn an_instrument_without_its_calendar_is_refused_rather_than_invented() {
        let conn = memory_db();
        ensure_builtin_calendars(&conn).unwrap();
        let spy = InstrumentDraft {
            instrument_id: "us-etf:nyse-arca:SPY".to_string(),
            base: "SPY".to_string(),
            quote: "USD".to_string(),
            asset_type: AssetType::Etf,
            session_calendar_id: "nyse-v1".to_string(),
            timezone: "America/New_York".to_string(),
            lot_size: None,
            price_step: None,
            min_notional: None,
            listed_from: None,
            delisted_at: None,
            suspensions: Vec::new(),
            source_capabilities: json!({}),
        };
        let error = register_instrument(&conn, &spy).unwrap_err().to_string();
        assert!(error.contains("nyse-v1 is not registered"), "{error}");

        // With the calendar registered it goes in; with the wrong KIND of
        // calendar it does not.
        register_calendar(&conn, &etf_calendar("nyse-v1")).unwrap();
        assert!(register_instrument(&conn, &spy).unwrap().1);
        let confused = InstrumentDraft {
            session_calendar_id: CRYPTO_24X7_CALENDAR_ID.to_string(),
            ..spy.clone()
        };
        assert!(register_instrument(&conn, &confused)
            .unwrap_err()
            .to_string()
            .contains("need a trading-days calendar"));

        // The default research list names the ETFs, but only the crypto
        // instruments are describable in this phase.
        assert_eq!(DEFAULT_RESEARCH_SYMBOLS.len(), 12);
        assert_eq!(default_crypto_instruments().len(), 2);
        assert!(DEFAULT_RESEARCH_SYMBOLS
            .iter()
            .any(|(market, symbol, interval)| *market == "tw-etf" && *symbol == "0050" && *interval == "1d"));
    }

    #[test]
    fn a_malformed_instrument_is_refused_by_rule() {
        let conn = memory_db();
        ensure_builtin_calendars(&conn).unwrap();
        let base = default_crypto_instruments().into_iter().next().unwrap();
        let cases: Vec<(InstrumentDraft, &str)> = vec![
            (
                InstrumentDraft { instrument_id: "binance:BTCUSDT".into(), ..base.clone() },
                "instrument_id_shape",
            ),
            (
                InstrumentDraft { asset_type: AssetType::Etf, ..base.clone() },
                "holds spot-crypto instruments",
            ),
            (InstrumentDraft { quote: " ".into(), ..base.clone() }, "non-empty value"),
            (InstrumentDraft { lot_size: Some(0.0), ..base.clone() }, "finite positive"),
            (
                InstrumentDraft { listed_from: Some(10), delisted_at: Some(5), ..base.clone() },
                "delisting must come after",
            ),
            (
                InstrumentDraft {
                    suspensions: vec![
                        Suspension { from_ms: 100, to_ms_exclusive: 200 },
                        Suspension { from_ms: 150, to_ms_exclusive: 300 },
                    ],
                    ..base.clone()
                },
                "must not overlap",
            ),
            (
                InstrumentDraft { source_capabilities: json!([]), ..base.clone() },
                "sourceCapabilities is an object",
            ),
        ];
        for (draft, expected) in cases {
            let error = register_instrument(&conn, &draft).unwrap_err().to_string();
            assert!(error.contains(expected), "expected {expected:?} in {error:?}");
        }
        assert_eq!(list_instruments(&conn).unwrap().len(), 0, "nothing was written");
    }
}
