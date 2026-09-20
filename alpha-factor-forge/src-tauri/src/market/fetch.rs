//! P07 — running one retrieval against a real workspace.
//!
//! The operator-facing half of `ingest`: it owns the workspace exactly as
//! any other host does (OS lock → open → migrate → ownership epoch →
//! recovery → heartbeat), does one bounded retrieval, and releases it. That
//! is why a fetch while the service is running is refused rather than
//! queued — two owners writing the same workspace is the failure P03a
//! exists to prevent, and the answer is to stop the service or, once P17
//! schedules retrievals, to ask the service to do it.

use std::path::Path;

use chrono::{NaiveDate, TimeZone, Utc};

use crate::db::ownership::HolderKind;
use crate::db::DB_FILE_NAME;
use crate::error::{AppError, AppResult};
use crate::research::artifacts::ArtifactStore;
use crate::runtime;

use super::http::UreqFetcher;
use super::ingest::{self, IngestReport, IngestRequest};

/// What the command line asks for. Dates are UTC days: `from` is inclusive
/// and `to` is exclusive, so a single day is `--from D --to D+1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchOptions {
    pub instrument_id: String,
    pub interval: String,
    pub from: NaiveDate,
    pub to_exclusive: NaiveDate,
    pub allow_rest: bool,
    pub cost_profile_version: Option<String>,
}

/// `YYYY-MM-DD` as the start of that UTC day.
///
/// Deliberately the CONTRACT's date rule (`market-foundation-v1`), not
/// chrono's lenient one: a calendar would refuse `2024-7-15` as an
/// `invalid_date`, so the command line refuses it too rather than letting
/// two spellings of a date exist in the same workspace.
pub fn parse_date(value: &str) -> Result<NaiveDate, String> {
    let ms = alpha_factor_forge::discovery_core::market_foundation::utc_date_to_ms(value)
        .ok_or_else(|| format!("{value:?} is not a YYYY-MM-DD date"))?;
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|moment| moment.date_naive())
        .ok_or_else(|| format!("{value:?} is not a representable date"))
}

fn start_of_day_ms(date: NaiveDate) -> AppResult<i64> {
    date.and_hms_opt(0, 0, 0)
        .map(|moment| moment.and_utc().timestamp_millis())
        .ok_or_else(|| AppError::Other(format!("{date} has no midnight")))
}

/// Own the workspace at `data_dir`, retrieve the requested range, release.
pub fn fetch(data_dir: &Path, options: &FetchOptions) -> AppResult<IngestReport> {
    let from_ms = start_of_day_ms(options.from)?;
    let to_ms_exclusive = start_of_day_ms(options.to_exclusive)?;
    if to_ms_exclusive <= from_ms {
        return Err(AppError::Other("--to must be after --from".into()));
    }
    let as_of_ms = Utc::now().timestamp_millis();
    if from_ms > as_of_ms {
        return Err(AppError::Other("--from is in the future".into()));
    }
    let request = IngestRequest {
        instrument_id: options.instrument_id.clone(),
        interval: options.interval.clone(),
        from_ms,
        // Never ask a source for bars that cannot exist yet.
        to_ms_exclusive: to_ms_exclusive.min(as_of_ms),
        as_of_ms,
        cost_profile_version: options.cost_profile_version.clone(),
        allow_rest: options.allow_rest,
    };

    let workspace = runtime::open_workspace(&data_dir.join(DB_FILE_NAME), HolderKind::Service)?;
    let store = ArtifactStore::in_data_dir(data_dir);
    let fetcher = UreqFetcher::for_binance();
    let mut guard = workspace
        .db
        .lock()
        .map_err(|_| AppError::Other("db lock poisoned".into()))?;
    register_known_instrument(&guard, &options.instrument_id)?;
    let report = ingest::ingest(&mut guard, &store, &fetcher, &request);
    drop(guard);
    drop(workspace);
    report
}

/// Register an instrument this adapter can describe from the contract
/// alone — but only if the workspace does not know it yet.
///
/// The guard is the point: a later phase (or a source that reports a lot
/// size) registers a richer revision, and re-registering this phase's
/// spec-less draft would add a newer revision that *loses* it. An
/// instrument the adapter does not know stays unregistered, and the ingest
/// says so rather than inventing one.
fn register_known_instrument(conn: &rusqlite::Connection, instrument_id: &str) -> AppResult<()> {
    use super::registry;
    if registry::latest_instrument(conn, instrument_id)?.is_some() {
        return Ok(());
    }
    if let Some(draft) = registry::default_crypto_instruments()
        .into_iter()
        .find(|draft| draft.instrument_id == instrument_id)
    {
        registry::register_instrument(conn, &draft)?;
    }
    Ok(())
}

/// A UTC instant as a `YYYY-MM-DD HH:MM` for an operator reading a report.
pub fn format_instant(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|moment| moment.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| ms.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_utc_days_and_a_bad_one_is_refused() {
        assert_eq!(parse_date("2024-07-15").unwrap(), NaiveDate::from_ymd_opt(2024, 7, 15).unwrap());
        assert_eq!(start_of_day_ms(parse_date("2024-07-15").unwrap()).unwrap(), 1_721_001_600_000);
        for bad in ["2024-7-15", "15/07/2024", "2024-02-30", "today", ""] {
            assert!(parse_date(bad).is_err(), "{bad:?}");
        }
        assert_eq!(format_instant(1_721_001_600_000), "2024-07-15 00:00");
    }

    #[test]
    fn a_range_that_ends_before_it_starts_is_refused_before_the_workspace_is_touched() {
        let options = FetchOptions {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            from: parse_date("2024-07-16").unwrap(),
            to_exclusive: parse_date("2024-07-15").unwrap(),
            allow_rest: false,
            cost_profile_version: None,
        };
        // The path does not exist; the range is refused before it matters.
        let error = fetch(Path::new("C:/nonexistent-workspace-p07"), &options).unwrap_err();
        assert!(error.to_string().contains("--to must be after --from"), "{error}");
    }

    #[test]
    fn a_future_range_is_refused_rather_than_asked_for() {
        let tomorrow = Utc::now().date_naive().succ_opt().unwrap();
        let options = FetchOptions {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            interval: "1h".into(),
            from: tomorrow,
            to_exclusive: tomorrow.succ_opt().unwrap(),
            allow_rest: false,
            cost_profile_version: None,
        };
        let error = fetch(Path::new("C:/nonexistent-workspace-p07"), &options).unwrap_err();
        assert!(error.to_string().contains("--from is in the future"), "{error}");
    }
}
