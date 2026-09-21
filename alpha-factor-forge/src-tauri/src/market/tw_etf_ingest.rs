//! P10 bounded, operator-triggered Taiwan ETF retrieval.
//!
//! FinMind raw daily rows are the only dataset input. Official TWSE monthly
//! quotes are independently reconciled and retained as comparison evidence;
//! they never fill a missing FinMind row. P06 provenance/revisions/snapshots
//! and P08 TWD/calendar/action/cost gates remain the admission boundary.
use super::{
    canonical_json,
    http::{FetchError, HttpFetcher, UreqFetcher},
    provenance::{self, RawObservation},
    registry::{self, InstrumentDraft},
    sha256_hex,
    snapshot::{self, SnapshotKind, SnapshotOutcome, SnapshotRequest, SnapshotRow},
    sources::{finmind, twse},
};
use crate::{
    db::{
        self,
        ownership::HolderKind,
        repositories::{self, Dataset},
    },
    error::{AppError, AppResult},
    research::artifacts::ArtifactStore,
};
use alpha_factor_forge::discovery_core::{
    etf::{
        self, ActionEvidence, DividendAction, EtfCostProfile, EtfMarket, NativeCurrency,
        SplitAction,
    },
    market_foundation::{
        audit_coverage, parse_instrument_id, utc_date_to_ms, AssetType, CoverageRequest, Market,
        PriceBasis, SeriesRole, Suspension,
    },
};
use chrono::{Datelike, NaiveDate, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

const DAY: i64 = 86_400_000;
pub const SETTINGS_VERSION: &str = "tw-etf-settings-v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuspensionEvidence {
    pub from: String,
    pub to_exclusive: String,
    pub evidence: String,
}

impl SuspensionEvidence {
    fn suspension(&self) -> Result<Suspension, &'static str> {
        let from_ms = utc_date_to_ms(&self.from).ok_or("invalid_suspension")?;
        let to_ms_exclusive = utc_date_to_ms(&self.to_exclusive).ok_or("invalid_suspension")?;
        if from_ms >= to_ms_exclusive || self.evidence.trim().is_empty() {
            return Err("invalid_suspension");
        }
        Ok(Suspension {
            from_ms,
            to_ms_exclusive,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SplitEvidence {
    pub effective_date: String,
    pub ratio: f64,
    pub evidence: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DividendEvidence {
    pub ex_date: String,
    pub payment_date: String,
    pub amount_per_share: f64,
    pub evidence: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentSettings {
    pub market: EtfMarket,
    pub listed_from: String,
    pub listing_evidence: String,
    pub calendar_evidence: String,
    pub suspensions: Vec<SuspensionEvidence>,
    pub confirmed_dividends: Vec<DividendEvidence>,
    pub confirmed_splits: Vec<SplitEvidence>,
    /// The operator checked dividend/split completeness against the named
    /// TWSE evidence interval. A successful FinMind response alone is not
    /// this confirmation.
    pub corporate_actions_confirmed: bool,
    pub action_evidence: String,
    pub costs: Option<EtfCostProfile>,
}

impl InstrumentSettings {
    fn suspension_rows(&self) -> Result<Vec<Suspension>, &'static str> {
        let rows: Vec<_> = self
            .suspensions
            .iter()
            .map(SuspensionEvidence::suspension)
            .collect::<Result<_, _>>()?;
        if rows
            .windows(2)
            .any(|pair| pair[0].to_ms_exclusive > pair[1].from_ms)
        {
            return Err("invalid_suspension");
        }
        Ok(rows)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub version: String,
    pub instruments: Vec<InstrumentSettings>,
}

impl Settings {
    pub fn validate(&self) -> AppResult<()> {
        if self.version != SETTINGS_VERSION || self.instruments.len() > finmind::SYMBOLS.len() {
            return Err(error("invalid_settings_version_or_count"));
        }
        let mut seen = HashSet::new();
        for settings in &self.instruments {
            etf::validate_etf_market(&settings.market).map_err(error)?;
            let id = parse_instrument_id(&settings.market.instrument_id)
                .map_err(|_| error("invalid_instrument"))?;
            if id.market != Market::TwEtf
                || id.venue != "twse"
                || !finmind::SYMBOLS.contains(&id.symbol.as_str())
                || !seen.insert(id.symbol.clone())
            {
                return Err(error("unsupported_or_duplicate_instrument"));
            }
            if settings.market.currency != NativeCurrency::TWD
                || utc_date_to_ms(&settings.listed_from).is_none()
                || settings.listing_evidence.trim().is_empty()
                || settings.calendar_evidence.trim().is_empty()
                || settings.action_evidence.trim().is_empty()
            {
                return Err(error("market_evidence_missing"));
            }
            settings.suspension_rows().map_err(error)?;
            let mut previous = None;
            for dividend in &settings.confirmed_dividends {
                let ex_date = utc_date_to_ms(&dividend.ex_date)
                    .ok_or_else(|| error("invalid_dividend_evidence"))?;
                let payment_date = utc_date_to_ms(&dividend.payment_date)
                    .ok_or_else(|| error("invalid_dividend_evidence"))?;
                if payment_date < ex_date
                    || !dividend.amount_per_share.is_finite()
                    || dividend.amount_per_share <= 0.0
                    || dividend.evidence.trim().is_empty()
                    || previous.is_some_and(|old| ex_date <= old)
                {
                    return Err(error("invalid_dividend_evidence"));
                }
                previous = Some(ex_date);
            }
            let mut previous = None;
            for split in &settings.confirmed_splits {
                let date = utc_date_to_ms(&split.effective_date)
                    .ok_or_else(|| error("invalid_split_evidence"))?;
                if !split.ratio.is_finite()
                    || split.ratio <= 0.0
                    || split.evidence.trim().is_empty()
                    || previous.is_some_and(|old| date <= old)
                {
                    return Err(error("invalid_split_evidence"));
                }
                previous = Some(date);
            }
            if let Some(costs) = &settings.costs {
                etf::validate_etf_costs(costs, NativeCurrency::TWD).map_err(error)?;
            }
        }
        Ok(())
    }
}

fn error(code: &str) -> AppError {
    AppError::Other(code.into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub settings: PathBuf,
    pub data_dir: Option<PathBuf>,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

pub fn parse_args<S: AsRef<str>>(args: &[S]) -> Result<Options, String> {
    let mut flags = std::collections::HashMap::new();
    let mut index = 0;
    while index < args.len() {
        let text = args[index].as_ref();
        index += 1;
        let (flag, value) = if let Some(pair) = text.split_once('=') {
            pair
        } else {
            let value = args
                .get(index)
                .ok_or("TW ETF option needs a value")?
                .as_ref();
            index += 1;
            (text, value)
        };
        if !["--settings", "--data-dir", "--from", "--to"].contains(&flag)
            || value.is_empty()
            || flags.insert(flag, value).is_some()
        {
            return Err("unknown, empty or duplicate TW ETF option".into());
        }
    }
    Ok(Options {
        settings: PathBuf::from(
            *flags
                .get("--settings")
                .ok_or("fetch-tw-etf needs --settings")?,
        ),
        data_dir: flags.get("--data-dir").map(PathBuf::from),
        from: super::fetch::parse_date(flags.get("--from").ok_or("fetch-tw-etf needs --from")?)?,
        to: super::fetch::parse_date(flags.get("--to").ok_or("fetch-tw-etf needs --to")?)?,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: &'static str,
    pub symbol: String,
    pub instrument_id: Option<String>,
    pub from: String,
    pub to_exclusive: String,
    pub status: &'static str,
    pub qualification_eligible: bool,
    pub reasons: Vec<String>,
    pub provenance_ids: Vec<i64>,
    pub bars: usize,
    pub twse_reconciled_months: usize,
    pub coverage: Value,
    pub dividends: Vec<DividendAction>,
    pub splits: Vec<SplitAction>,
    pub dataset_id: Option<i64>,
    pub snapshot: Option<SnapshotRow>,
}

impl Report {
    fn new(symbol: &str, from: &str, to: &str) -> Self {
        Self {
            version: finmind::VERSION,
            symbol: symbol.into(),
            instrument_id: None,
            from: from.into(),
            to_exclusive: to.into(),
            status: "BLOCKED",
            qualification_eligible: false,
            reasons: Vec::new(),
            provenance_ids: Vec::new(),
            bars: 0,
            twse_reconciled_months: 0,
            coverage: Value::Null,
            dividends: Vec::new(),
            splits: Vec::new(),
            dataset_id: None,
            snapshot: None,
        }
    }

    fn reason(&mut self, code: &str) {
        if !self.reasons.iter().any(|reason| reason == code) {
            self.reasons.push(code.into());
        }
    }
}

pub fn run(data_dir: &Path, options: &Options) -> AppResult<Vec<Report>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(&options.settings)?
        .take(262_145)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 262_144 {
        return Err(error("settings_too_large"));
    }
    let settings: Settings =
        serde_json::from_slice(&bytes).map_err(|_| error("invalid_tw_etf_settings"))?;
    settings.validate()?;
    let from = options.from.to_string();
    let to = options.to.to_string();
    let as_of = Utc::now().timestamp_millis();
    validate_range(&from, &to, as_of)?;
    let fetcher = UreqFetcher::for_tw_etf();
    let workspace =
        crate::runtime::open_workspace(&data_dir.join(db::DB_FILE_NAME), HolderKind::Service)?;
    let store = ArtifactStore::in_data_dir(data_dir);
    let mut conn = workspace.db.lock().map_err(|_| error("db lock poisoned"))?;
    ingest(&mut conn, &store, &fetcher, &settings, &from, &to, as_of)
}

fn validate_range(from: &str, to: &str, as_of: i64) -> AppResult<(i64, i64)> {
    let start = utc_date_to_ms(from).ok_or_else(|| error("invalid_from_date"))?;
    let end = utc_date_to_ms(to).ok_or_else(|| error("invalid_to_date"))?;
    if end <= start || end - start > 31 * DAY {
        return Err(error("range_must_be_1_to_31_days"));
    }
    // The requested final date is the day before `to`; wait through the next
    // Taiwan morning before treating all official rows as final.
    if end + 8 * 3_600_000 > as_of {
        return Err(error("range_not_finalized"));
    }
    Ok((start, end))
}

pub fn ingest(
    conn: &mut Connection,
    store: &ArtifactStore,
    fetcher: &dyn HttpFetcher,
    settings: &Settings,
    from: &str,
    to: &str,
    as_of: i64,
) -> AppResult<Vec<Report>> {
    settings.validate()?;
    let (start, end) = validate_range(from, to, as_of)?;
    let mut reports = Vec::new();
    let mut batch_stop = None;
    for symbol in finmind::SYMBOLS {
        let mut report = Report::new(symbol, from, to);
        let configured = settings
            .instruments
            .iter()
            .find(|item| item.market.instrument_id.ends_with(&format!(":{symbol}")));
        if let Some(item) = configured {
            report.instrument_id = Some(item.market.instrument_id.clone());
            if let Some(reason) = batch_stop {
                report.reason(reason);
            } else {
                ingest_symbol(
                    conn,
                    store,
                    fetcher,
                    item,
                    &mut report,
                    start,
                    end,
                    as_of,
                    &mut batch_stop,
                )?;
            }
            let body = canonical_json(&serde_json::to_value(&report)?)?;
            let observed = Utc::now().to_rfc3339();
            provenance::record_raw(
                conn,
                store,
                &RawObservation {
                    instrument_id: item.market.instrument_id.clone(),
                    interval: "1d".into(),
                    source: "tw-etf-report".into(),
                    role: SeriesRole::Comparison,
                    request_scope: json!({"version":finmind::VERSION,"from":from,"toExclusive":to,"settings":item}),
                    retrieved_at: observed.clone(),
                    available_at: Some(observed),
                    accepted: true,
                    rejection_reason: None,
                    revision_of: None,
                    media_type: "application/json".into(),
                },
                &body,
            )?;
        } else {
            report.reason("source_settings_missing");
        }
        reports.push(report);
    }
    Ok(reports)
}

fn fetch_code(error: &FetchError) -> &'static str {
    match error {
        FetchError::Status { status: 429, .. } => "quota_exhausted",
        FetchError::NotFound(_) => "source_not_found",
        FetchError::TooLarge { .. } => "response_too_large",
        FetchError::Refused(_) => "source_request_refused",
        _ => "source_unavailable",
    }
}

#[allow(clippy::too_many_arguments)]
fn retrieve<T>(
    conn: &Connection,
    store: &ArtifactStore,
    fetcher: &dyn HttpFetcher,
    settings: &InstrumentSettings,
    source: &str,
    role: SeriesRole,
    url: &str,
    report: &mut Report,
    batch_stop: &mut Option<&'static str>,
    parse: impl FnOnce(&[u8]) -> Result<T, &'static str>,
) -> AppResult<Option<T>> {
    let (bytes, result, transport) = match fetcher.get(url) {
        Ok(bytes) => {
            let result = parse(&bytes);
            (bytes, result, false)
        }
        Err(error) => {
            let code = fetch_code(&error);
            if code == "quota_exhausted" {
                *batch_stop = Some(code);
            }
            (
                serde_json::to_vec(&json!({"localFailureReceipt":code}))?,
                Err(code),
                true,
            )
        }
    };
    let previous = provenance::list_provenance(conn, &settings.market.instrument_id, "1d")?
        .into_iter()
        .rev()
        .find(|row| {
            row.source == source
                && row.request_scope["endpoint"] == url
                && !row.request_scope["localFailureReceipt"]
                    .as_bool()
                    .unwrap_or(false)
        });
    let revision = (!transport).then(|| previous.map(|row| row.id)).flatten();
    let now = Utc::now().to_rfc3339();
    let (id, _) = provenance::record_raw(
        conn,
        store,
        &RawObservation {
            instrument_id: settings.market.instrument_id.clone(),
            interval: "1d".into(),
            source: source.into(),
            role,
            request_scope: json!({"version":finmind::VERSION,"endpoint":url,"settings":settings,"localFailureReceipt":transport,"availabilityBasis":"response-completed"}),
            retrieved_at: now.clone(),
            available_at: (!transport).then_some(now),
            accepted: result.is_ok(),
            rejection_reason: result.as_ref().err().map(|code| (*code).into()),
            revision_of: revision,
            media_type: "application/json".into(),
        },
        &bytes,
    )?;
    report.provenance_ids.push(id);
    match result {
        Ok(value) => Ok(Some(value)),
        Err(code) => {
            report.reason(code);
            Ok(None)
        }
    }
}

fn months(from: NaiveDate, last: NaiveDate) -> Vec<(i32, u32)> {
    let mut result = Vec::new();
    let (mut year, mut month) = (from.year(), from.month());
    loop {
        result.push((year, month));
        if year == last.year() && month == last.month() {
            break;
        }
        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }
    result
}

fn reconcile_splits(
    settings: &InstrumentSettings,
    observations: &[finmind::SplitObservation],
    start: i64,
    end: i64,
    observed_at: i64,
) -> Result<Vec<SplitAction>, &'static str> {
    let expected: Vec<_> = settings
        .confirmed_splits
        .iter()
        .filter(|split| {
            utc_date_to_ms(&split.effective_date)
                .is_some_and(|timestamp| (start..end).contains(&timestamp))
        })
        .collect();
    if expected.len() != observations.len() {
        return Err("split_evidence_mismatch");
    }
    let mut actions = Vec::new();
    for (official, source) in expected.into_iter().zip(observations) {
        let published_ratio = source.before_price / source.after_price;
        let expected_kind = if official.ratio >= 1.0 {
            "分割"
        } else {
            "反分割"
        };
        if official.effective_date != source.date
            || source.kind != expected_kind
            || (published_ratio - official.ratio).abs() > 0.01 * official.ratio.max(1.0)
        {
            return Err("split_evidence_mismatch");
        }
        actions.push(SplitAction {
            id: format!(
                "twse:{}:split:{}",
                settings.market.instrument_id, official.effective_date
            ),
            instrument_id: settings.market.instrument_id.clone(),
            currency: NativeCurrency::TWD,
            effective_date: official.effective_date.clone(),
            ratio: official.ratio,
            available_at_ms: observed_at,
        });
    }
    Ok(actions)
}

fn reconcile_dividends(
    settings: &InstrumentSettings,
    observations: &[DividendAction],
    start: i64,
    end: i64,
) -> Result<(), &'static str> {
    let expected: Vec<_> = settings
        .confirmed_dividends
        .iter()
        .filter(|dividend| {
            utc_date_to_ms(&dividend.ex_date)
                .is_some_and(|timestamp| (start..end).contains(&timestamp))
        })
        .collect();
    if expected.len() != observations.len() {
        return Err("dividend_evidence_mismatch");
    }
    for (official, source) in expected.into_iter().zip(observations) {
        if official.ex_date != source.ex_date
            || source.payment_date.as_deref() != Some(official.payment_date.as_str())
            || (official.amount_per_share - source.amount_per_share).abs()
                > 1e-10 * official.amount_per_share.max(1.0)
        {
            return Err("dividend_evidence_mismatch");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ingest_symbol(
    conn: &mut Connection,
    store: &ArtifactStore,
    fetcher: &dyn HttpFetcher,
    settings: &InstrumentSettings,
    report: &mut Report,
    start: i64,
    end: i64,
    as_of: i64,
    batch_stop: &mut Option<&'static str>,
) -> AppResult<()> {
    if start < utc_date_to_ms(&settings.listed_from).expect("validated listing") {
        report.reason("before_listing");
        return Ok(());
    }
    let suspensions = settings.suspension_rows().map_err(error)?;
    let exchange_sessions =
        match etf::etf_session_dates(&settings.market, &report.from, &report.to_exclusive, &[]) {
            Ok(value) => value,
            Err(code) => {
                report.reason(code);
                return Ok(());
            }
        };
    let expected = etf::etf_session_dates(
        &settings.market,
        &report.from,
        &report.to_exclusive,
        &suspensions,
    )
    .map_err(error)?;
    let Some(&first) = expected.first() else {
        report.reason("no_sessions");
        return Ok(());
    };
    let last = *expected.last().expect("nonempty sessions");
    let final_date = finmind::date(end - DAY);
    let symbol = report.symbol.clone();
    let event_from = format!("{:04}-01-01", report.from[0..4].parse::<i32>().unwrap());
    let event_to = format!("{:04}-12-31", final_date[0..4].parse::<i32>().unwrap());
    let urls = finmind::urls(&symbol, &report.from, &final_date, &event_from, &event_to);
    let Some(published_sessions) = retrieve(
        conn,
        store,
        fetcher,
        settings,
        finmind::SOURCE,
        SeriesRole::Primary,
        &urls[0],
        report,
        batch_stop,
        |bytes| finmind::parse_trading_dates(bytes, start, end),
    )?
    else {
        return Ok(());
    };
    if published_sessions != exchange_sessions {
        report.reason("trading_calendar_mismatch");
        return Ok(());
    }
    let Some(rows) = retrieve(
        conn,
        store,
        fetcher,
        settings,
        finmind::SOURCE,
        SeriesRole::Primary,
        &urls[1],
        report,
        batch_stop,
        |bytes| finmind::parse_prices(bytes, &symbol, start, end),
    )?
    else {
        return Ok(());
    };
    let observed: Vec<i64> = rows.iter().map(|row| row.candle().timestamp).collect();
    let coverage = audit_coverage(&CoverageRequest {
        interval: "1d",
        expected: &expected,
        observed: &observed,
        as_of_ms: as_of,
    });
    report.bars = rows.len();
    report.coverage = serde_json::to_value(&coverage)?;
    if coverage.blocking || observed != expected {
        report.reason("requested_range_incomplete_or_off_session");
        return Ok(());
    }

    let from_date = NaiveDate::parse_from_str(&report.from, "%Y-%m-%d")
        .map_err(|_| error("invalid_from_date"))?;
    let last_date =
        NaiveDate::parse_from_str(&final_date, "%Y-%m-%d").map_err(|_| error("invalid_to_date"))?;
    let mut official = Vec::new();
    for (year, month) in months(from_date, last_date) {
        let url = twse::month_url(&symbol, year, month);
        let Some(mut quotes) = retrieve(
            conn,
            store,
            fetcher,
            settings,
            twse::SOURCE,
            SeriesRole::Comparison,
            &url,
            report,
            batch_stop,
            |bytes| twse::parse_month(bytes, &symbol),
        )?
        else {
            return Ok(());
        };
        report.twse_reconciled_months += 1;
        official.append(&mut quotes);
    }
    if let Err(code) = twse::reconcile(&rows, &official, start, end) {
        report.reason(code);
        return Ok(());
    }

    let Some(dividends) = retrieve(
        conn,
        store,
        fetcher,
        settings,
        finmind::SOURCE,
        SeriesRole::Primary,
        &urls[2],
        report,
        batch_stop,
        |bytes| {
            finmind::parse_dividends(bytes, &symbol, &settings.market.instrument_id, start, end)
        },
    )?
    else {
        return Ok(());
    };
    if settings.corporate_actions_confirmed {
        if let Err(code) = reconcile_dividends(settings, &dividends, start, end) {
            report.reason(code);
            return Ok(());
        }
    }
    report.dividends = dividends;
    let Some(split_observations) = retrieve(
        conn,
        store,
        fetcher,
        settings,
        finmind::SOURCE,
        SeriesRole::Primary,
        &urls[3],
        report,
        batch_stop,
        |bytes| finmind::parse_splits(bytes, &symbol, start, end),
    )?
    else {
        return Ok(());
    };
    report.splits = match reconcile_splits(
        settings,
        &split_observations,
        start,
        end,
        Utc::now().timestamp_millis(),
    ) {
        Ok(actions) => actions,
        Err(code) => {
            report.reason(code);
            return Ok(());
        }
    };

    let unconfirmed_costs = EtfCostProfile {
        version: "unconfirmed-v1".into(),
        currency: NativeCurrency::TWD,
        confirmed: false,
        commission_rate: 0.0,
        minimum_commission: 0.0,
        slippage_rate: 0.0,
        buy_tax_rate: 0.0,
        sell_tax_rate: 0.0,
    };
    let costs = settings.costs.as_ref().unwrap_or(&unconfirmed_costs);
    let evidence = ActionEvidence {
        version: finmind::VERSION.into(),
        complete: settings.corporate_actions_confirmed,
        from: report.from.clone(),
        to_exclusive: report.to_exclusive.clone(),
        dividends: report.dividends.clone(),
    };
    let verdict = etf::assess_etf_semantics(
        &settings.market,
        costs,
        &evidence,
        &report.from,
        &report.to_exclusive,
    )
    .map_err(error)?;
    for reason in &verdict.reasons {
        report.reason(reason);
    }

    registry::register_calendar(conn, &settings.market.calendar)?;
    let parsed = parse_instrument_id(&settings.market.instrument_id)
        .map_err(|_| error("invalid_instrument"))?;
    let existing = registry::latest_instrument(conn, &settings.market.instrument_id)?;
    if let Some(ref old) = existing {
        if old.session_calendar_id != settings.market.calendar.calendar_id
            || old.quote != "TWD"
            || old.base != parsed.symbol
            || old.timezone != settings.market.calendar.timezone
            || old.listed_from != utc_date_to_ms(&settings.listed_from)
            || old.delisted_at.is_some()
            || old.suspensions != suspensions
        {
            report.reason("registered_market_requires_reconciliation");
            return Ok(());
        }
    } else {
        registry::register_instrument(
            conn,
            &InstrumentDraft {
                instrument_id: settings.market.instrument_id.clone(),
                base: parsed.symbol.clone(),
                quote: "TWD".into(),
                asset_type: AssetType::Etf,
                session_calendar_id: settings.market.calendar.calendar_id.clone(),
                timezone: settings.market.calendar.timezone.clone(),
                lot_size: Some(1.0),
                price_step: None,
                min_notional: None,
                listed_from: utc_date_to_ms(&settings.listed_from),
                delisted_at: None,
                suspensions: suspensions.clone(),
                source_capabilities: json!({
                    "source":finmind::SOURCE,"raw":true,"adjusted":false,
                    "twseComparison":true,"tradingCalendar":true,"dividends":true,
                    "splits":true,"forwardObserved":false
                }),
            },
        )?;
    }

    let candles: Vec<_> = rows.iter().map(finmind::PriceRow::candle).collect();
    let mut dataset = Dataset {
        id: None,
        exchange: parsed.venue,
        symbol: report.symbol.clone(),
        interval: "1d".into(),
        start_time: first,
        end_time: last,
        candle_count: candles.len() as i64,
        source: "exchange".into(),
        dataset_hash: String::new(),
    };
    dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, &candles)?;
    let dataset_id = repositories::import_dataset_with_candles(conn, &dataset, &candles)?;
    report.dataset_id = Some(dataset_id);
    let mut primary = Vec::new();
    for id in &report.provenance_ids {
        let row =
            provenance::get_provenance(conn, *id)?.ok_or_else(|| error("missing_provenance"))?;
        if row.accepted && row.role == SeriesRole::Primary {
            primary.push(*id);
        }
    }
    let action_hash = sha256_hex(&canonical_json(&json!({
        "evidence":evidence,"splits":report.splits,"sources":report.provenance_ids,
        "settings":settings,"twseReconciledMonths":report.twse_reconciled_months
    }))?);
    let cost_hash = sha256_hex(&canonical_json(&serde_json::to_value(costs)?)?);
    let action_ok =
        settings.corporate_actions_confirmed && !verdict.reasons.contains(&"unknown_payment_date");
    let outcome = snapshot::build_snapshot(
        conn,
        &SnapshotRequest {
            instrument_id: settings.market.instrument_id.clone(),
            interval: "1d".into(),
            dataset_id,
            price_basis: PriceBasis::Raw,
            corporate_action_version: action_ok.then(|| format!("tw-etf-actions-v1:{action_hash}")),
            cost_profile_version: costs
                .confirmed
                .then(|| format!("{}:{cost_hash}", costs.version)),
            kind: SnapshotKind::Historical,
            provenance_ids: primary,
            as_of_ms: as_of,
        },
    )?;
    match outcome {
        SnapshotOutcome::Created(row) | SnapshotOutcome::Existing(row) => {
            report.qualification_eligible =
                row.qualification_eligible() && report.reasons.is_empty();
            report.status = if report.qualification_eligible {
                "OK"
            } else {
                "DEGRADED"
            };
            report.snapshot = Some(row);
        }
        SnapshotOutcome::Blocked { .. } => report.reason("snapshot_blocked"),
    }
    Ok(())
}

#[cfg(test)]
mod tests;
