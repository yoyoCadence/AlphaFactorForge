//! P07 — reading what Binance publishes, and nothing else.
//!
//! Two same-exchange sources for one instrument:
//!
//! * `binance-archive` — the public monthly/daily kline files at
//!   `data.binance.vision`, each with a published SHA-256 CHECKSUM.
//! * `binance-rest` — the exchange's own REST klines, for the bars the
//!   archive has not published yet.
//!
//! Everything here is pure: bytes in, rows or a stable rejection code out.
//! The caller owns the network, the provenance record, and the decision of
//! what to do with a rejection.
//!
//! **The time-unit rule.** The archive changed from milliseconds to
//! microseconds at the start of 2025 (`docs/market-contract.md` §4), which
//! this phase verified against the real files. `market-foundation-v1`
//! deliberately has no conversion function, because a *silent* conversion is
//! the failure it exists to prevent. The conversion therefore lives here, in
//! the source adapter, under conditions that make it evidence rather than a
//! guess:
//!
//! 1. the unit is resolved over the whole file, never per row;
//! 2. a file whose rows disagree is rejected, not averaged;
//! 3. the conversion must be exact — a microsecond value that is not a whole
//!    millisecond is rejected rather than rounded;
//! 4. the resolved unit is returned so the caller records it in the
//!    provenance for that file.

use alpha_factor_forge::discovery_core::market_foundation::{detect_time_unit, TimeUnit};
use chrono::{Datelike, NaiveDate};
use flate2::read::DeflateDecoder;
use std::io::Read;

/// The two source ids this adapter records in provenance.
pub const SOURCE_ARCHIVE: &str = "binance-archive";
pub const SOURCE_REST: &str = "binance-rest";
/// The venue half of every instrument id this adapter serves.
pub const VENUE: &str = "binance";

const ARCHIVE_BASE: &str = "https://data.binance.vision/data/spot";
const REST_BASE: &str = "https://api.binance.com/api/v3/klines";
/// The archive's REST endpoint caps a page at 1000 klines.
pub const REST_MAX_LIMIT: usize = 1000;
/// An inflated archive entry larger than this is refused rather than read.
pub const MAX_ENTRY_BYTES: usize = 256 * 1024 * 1024;

/// Every way this adapter can refuse bytes. The `code` is what a provenance
/// rejection reason and a test both name, so it never drifts into prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceError {
    pub code: &'static str,
    pub detail: String,
}

impl SourceError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self { code, detail: detail.into() }
    }
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for SourceError {}

/// Stable rejection codes, in one list so the contract can be read at a
/// glance and a new one cannot be introduced by accident.
pub const SOURCE_ERROR_CODES: [&str; 14] = [
    "checksum_malformed",
    "checksum_file_name_mismatch",
    "checksum_mismatch",
    "zip_not_an_archive",
    "zip_unsupported_entry",
    "zip_multiple_entries",
    "zip_entry_too_large",
    "zip_corrupt_entry",
    "csv_empty",
    "csv_column_count",
    "csv_not_a_number",
    "time_unit_unresolved",
    "time_unit_not_exact",
    "rest_shape",
];

// ------------------------------------------------------------------ URLs

/// `<symbol>-<interval>-<YYYY-MM>.zip` under the monthly archive.
pub fn monthly_url(symbol: &str, interval: &str, year: i32, month: u32) -> String {
    let file = monthly_file_name(symbol, interval, year, month);
    format!("{ARCHIVE_BASE}/monthly/klines/{symbol}/{interval}/{file}")
}

pub fn monthly_file_name(symbol: &str, interval: &str, year: i32, month: u32) -> String {
    format!("{symbol}-{interval}-{year:04}-{month:02}.zip")
}

/// `<symbol>-<interval>-<YYYY-MM-DD>.zip` under the daily archive.
pub fn daily_url(symbol: &str, interval: &str, date: NaiveDate) -> String {
    let file = daily_file_name(symbol, interval, date);
    format!("{ARCHIVE_BASE}/daily/klines/{symbol}/{interval}/{file}")
}

pub fn daily_file_name(symbol: &str, interval: &str, date: NaiveDate) -> String {
    format!(
        "{symbol}-{interval}-{:04}-{:02}-{:02}.zip",
        date.year(),
        date.month(),
        date.day()
    )
}

/// The published checksum beside an archive file.
pub fn checksum_url(archive_url: &str) -> String {
    format!("{archive_url}.CHECKSUM")
}

/// One REST page. `start_ms` is inclusive and `end_ms` exclusive, which is
/// how this workspace states ranges; the exchange's `endTime` is inclusive,
/// so the last millisecond is dropped here rather than at the call site.
pub fn rest_klines_url(symbol: &str, interval: &str, start_ms: i64, end_ms_exclusive: i64, limit: usize) -> String {
    let limit = limit.clamp(1, REST_MAX_LIMIT);
    format!(
        "{REST_BASE}?symbol={symbol}&interval={interval}&startTime={start_ms}&endTime={}&limit={limit}",
        end_ms_exclusive.saturating_sub(1)
    )
}

// -------------------------------------------------------------- checksum

/// Parse `<sha256>  <file name>` and check it names the file we asked for.
pub fn parse_checksum(bytes: &[u8], expected_file_name: &str) -> Result<String, SourceError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| SourceError::new("checksum_malformed", "the checksum file is not UTF-8"))?;
    let line = text.lines().next().unwrap_or("").trim();
    let mut parts = line.split_whitespace();
    let (Some(digest), Some(name)) = (parts.next(), parts.next()) else {
        return Err(SourceError::new(
            "checksum_malformed",
            format!("expected `<sha256>  <file>`, got {line:?}"),
        ));
    };
    if parts.next().is_some() {
        return Err(SourceError::new("checksum_malformed", "more than two fields"));
    }
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(SourceError::new(
            "checksum_malformed",
            format!("{digest:?} is not a SHA-256 digest"),
        ));
    }
    // A checksum that names another file proves nothing about this one.
    if name != expected_file_name {
        return Err(SourceError::new(
            "checksum_file_name_mismatch",
            format!("the checksum is for {name:?}, not {expected_file_name:?}"),
        ));
    }
    Ok(digest.to_ascii_lowercase())
}

/// Compare the published digest with what actually arrived.
pub fn verify_checksum(bytes: &[u8], expected_digest: &str) -> Result<(), SourceError> {
    let actual = super::super::sha256_hex(bytes);
    if actual != expected_digest.to_ascii_lowercase() {
        return Err(SourceError::new(
            "checksum_mismatch",
            format!("published {expected_digest}, received {actual}"),
        ));
    }
    Ok(())
}

// ------------------------------------------------------------------- zip

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// The single entry of a Binance archive file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipEntry {
    pub name: String,
    pub content: Vec<u8>,
}

/// Read the one entry a Binance archive holds.
///
/// A deliberately small reader for a deliberately narrow input: one local
/// file header at offset zero, stored or deflated, with its sizes in the
/// header. The end-of-central-directory record must agree that there is
/// exactly ONE entry, so a second file cannot ride along unnoticed, and the
/// entry's CRC-32 and uncompressed size are both checked — cheap, and they
/// catch a truncated member that the archive-level checksum would only
/// catch if the publisher's digest were also wrong.
pub fn read_single_zip_entry(bytes: &[u8]) -> Result<ZipEntry, SourceError> {
    const LOCAL_HEADER: u32 = 0x0403_4b50;
    const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;
    if bytes.len() < 30 || u32_at(bytes, 0) != LOCAL_HEADER {
        return Err(SourceError::new("zip_not_an_archive", "no local file header"));
    }

    // Exactly one member, per the central directory.
    let eocd = (0..bytes.len().saturating_sub(21))
        .rev()
        .find(|at| u32_at(bytes, *at) == END_OF_CENTRAL_DIRECTORY)
        .ok_or_else(|| SourceError::new("zip_not_an_archive", "no end-of-central-directory record"))?;
    let entries = u16_at(bytes, eocd + 10);
    if entries != 1 {
        return Err(SourceError::new(
            "zip_multiple_entries",
            format!("the archive holds {entries} entries; exactly one is expected"),
        ));
    }

    let flags = u16_at(bytes, 6);
    let method = u16_at(bytes, 8);
    let crc32 = u32_at(bytes, 14);
    let compressed_size = u32_at(bytes, 18) as usize;
    let uncompressed_size = u32_at(bytes, 22) as usize;
    let name_len = u16_at(bytes, 26) as usize;
    let extra_len = u16_at(bytes, 28) as usize;
    if flags & 0x0008 != 0 {
        // Sizes in a trailing data descriptor: not what the archive
        // publishes, and guessing the extent of the member is exactly the
        // kind of guess this module refuses to make.
        return Err(SourceError::new(
            "zip_unsupported_entry",
            "the entry's sizes are in a data descriptor",
        ));
    }
    if flags & 0x0001 != 0 {
        return Err(SourceError::new("zip_unsupported_entry", "the entry is encrypted"));
    }
    if uncompressed_size > MAX_ENTRY_BYTES {
        return Err(SourceError::new(
            "zip_entry_too_large",
            format!("the entry declares {uncompressed_size} bytes"),
        ));
    }
    let data_at = 30 + name_len + extra_len;
    let data_end = data_at.saturating_add(compressed_size);
    if data_end > bytes.len() {
        return Err(SourceError::new("zip_corrupt_entry", "the entry runs past the file"));
    }
    let name = String::from_utf8_lossy(&bytes[30..30 + name_len]).into_owned();
    let raw = &bytes[data_at..data_end];

    let content = match method {
        0 => raw.to_vec(),
        8 => {
            let mut out = Vec::with_capacity(uncompressed_size);
            DeflateDecoder::new(raw)
                .take(MAX_ENTRY_BYTES as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|error| SourceError::new("zip_corrupt_entry", error.to_string()))?;
            out
        }
        other => {
            return Err(SourceError::new(
                "zip_unsupported_entry",
                format!("compression method {other} is not stored or deflate"),
            ))
        }
    };
    if content.len() != uncompressed_size {
        return Err(SourceError::new(
            "zip_corrupt_entry",
            format!("the entry inflated to {} bytes, not the declared {uncompressed_size}", content.len()),
        ));
    }
    let mut crc = flate2::Crc::new();
    crc.update(&content);
    if crc.sum() != crc32 {
        return Err(SourceError::new("zip_corrupt_entry", "the entry's CRC-32 does not match"));
    }
    Ok(ZipEntry { name, content })
}

// ------------------------------------------------------------------ rows

/// One kline, already in this workspace's units: milliseconds and f64.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kline {
    pub open_time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time_ms: i64,
}

/// Rows plus what had to be resolved to read them — the caller records this
/// in the provenance for the file it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedKlines {
    pub rows: Vec<Kline>,
    /// The unit the source actually used for this file.
    pub source_time_unit: TimeUnit,
    /// True when the timestamps had to be converted to milliseconds.
    pub converted_to_milliseconds: bool,
    /// True when a column-name header row was present and skipped.
    pub header_row_skipped: bool,
}

/// The archive's kline CSV: 12 columns, no quoting, one bar per line.
const ARCHIVE_COLUMNS: usize = 12;
/// The only header line this adapter will skip; anything else that fails to
/// parse is an error, not a header.
const HEADER_FIRST_FIELD: &str = "open_time";

pub fn parse_archive_csv(bytes: &[u8]) -> Result<ParsedKlines, SourceError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| SourceError::new("csv_not_a_number", "the file is not UTF-8"))?;
    let mut raw_open: Vec<f64> = Vec::new();
    let mut raw_close: Vec<f64> = Vec::new();
    let mut prices: Vec<[f64; 5]> = Vec::new();
    let mut header_row_skipped = false;

    for (index, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        if index == 0 && fields.first().map(|first| first.trim()) == Some(HEADER_FIRST_FIELD) {
            header_row_skipped = true;
            continue;
        }
        if fields.len() != ARCHIVE_COLUMNS {
            return Err(SourceError::new(
                "csv_column_count",
                format!("line {} has {} fields, not {ARCHIVE_COLUMNS}", index + 1, fields.len()),
            ));
        }
        let number = |at: usize| -> Result<f64, SourceError> {
            fields[at].trim().parse::<f64>().map_err(|_| {
                SourceError::new(
                    "csv_not_a_number",
                    format!("line {} field {} is {:?}", index + 1, at + 1, fields[at]),
                )
            })
        };
        raw_open.push(number(0)?);
        raw_close.push(number(6)?);
        prices.push([number(1)?, number(2)?, number(3)?, number(4)?, number(5)?]);
    }
    assemble(raw_open, raw_close, prices, header_row_skipped)
}

/// The REST endpoint's JSON: an array of 12-element arrays, numbers for the
/// times and strings for the prices.
pub fn parse_rest_klines(bytes: &[u8]) -> Result<ParsedKlines, SourceError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| SourceError::new("rest_shape", error.to_string()))?;
    let rows = value
        .as_array()
        .ok_or_else(|| SourceError::new("rest_shape", "the response is not an array"))?;
    let mut raw_open: Vec<f64> = Vec::new();
    let mut raw_close: Vec<f64> = Vec::new();
    let mut prices: Vec<[f64; 5]> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let row = row
            .as_array()
            .ok_or_else(|| SourceError::new("rest_shape", format!("row {index} is not an array")))?;
        if row.len() != ARCHIVE_COLUMNS {
            return Err(SourceError::new(
                "csv_column_count",
                format!("row {index} has {} fields, not {ARCHIVE_COLUMNS}", row.len()),
            ));
        }
        let number = |at: usize| -> Result<f64, SourceError> {
            match &row[at] {
                serde_json::Value::Number(value) => value.as_f64().ok_or_else(|| {
                    SourceError::new("csv_not_a_number", format!("row {index} field {at} is not a f64"))
                }),
                serde_json::Value::String(text) => text.trim().parse::<f64>().map_err(|_| {
                    SourceError::new("csv_not_a_number", format!("row {index} field {at} is {text:?}"))
                }),
                other => Err(SourceError::new(
                    "csv_not_a_number",
                    format!("row {index} field {at} is {other}"),
                )),
            }
        };
        raw_open.push(number(0)?);
        raw_close.push(number(6)?);
        prices.push([number(1)?, number(2)?, number(3)?, number(4)?, number(5)?]);
    }
    assemble(raw_open, raw_close, prices, false)
}

/// Resolve the file's time unit once, convert exactly or refuse, and pair
/// the timestamps back up with their prices.
fn assemble(
    raw_open: Vec<f64>,
    raw_close: Vec<f64>,
    prices: Vec<[f64; 5]>,
    header_row_skipped: bool,
) -> Result<ParsedKlines, SourceError> {
    if prices.is_empty() {
        return Err(SourceError::new("csv_empty", "the file holds no rows"));
    }
    // Open and close times are resolved together: a file whose two time
    // columns disagree about their unit is not a file we can read.
    let mut all_times = raw_open.clone();
    all_times.extend_from_slice(&raw_close);
    let verdict = detect_time_unit(&all_times);
    let Some(unit) = verdict.unit else {
        return Err(SourceError::new(
            "time_unit_unresolved",
            format!(
                "{} at index {}",
                verdict.issue.map(|issue| issue.as_str()).unwrap_or("unresolved"),
                verdict.index.unwrap_or(0)
            ),
        ));
    };
    let mut rows = Vec::with_capacity(prices.len());
    for (index, [open, high, low, close, volume]) in prices.into_iter().enumerate() {
        rows.push(Kline {
            open_time_ms: to_milliseconds(raw_open[index], unit, index, "open time")?,
            open,
            high,
            low,
            close,
            volume,
            close_time_ms: to_milliseconds(raw_close[index], unit, index, "close time")?,
        });
    }
    Ok(ParsedKlines {
        rows,
        source_time_unit: unit,
        converted_to_milliseconds: unit != TimeUnit::Milliseconds,
        header_row_skipped,
    })
}

/// Exact conversion or refusal. Rounding a timestamp would move a bar.
fn to_milliseconds(value: f64, unit: TimeUnit, index: usize, what: &str) -> Result<i64, SourceError> {
    let refuse = |detail: String| SourceError::new("time_unit_not_exact", detail);
    if !value.is_finite() || value.fract() != 0.0 {
        return Err(refuse(format!("row {index} {what} {value} is not a whole number")));
    }
    let raw = value as i64;
    let millis = match unit {
        TimeUnit::Milliseconds => raw,
        TimeUnit::Seconds => raw
            .checked_mul(1000)
            .ok_or_else(|| refuse(format!("row {index} {what} {raw} overflows in milliseconds")))?,
        TimeUnit::Microseconds => exact_division(raw, 1_000, index, what)?,
        TimeUnit::Nanoseconds => exact_division(raw, 1_000_000, index, what)?,
    };
    Ok(millis)
}

fn exact_division(raw: i64, divisor: i64, index: usize, what: &str) -> Result<i64, SourceError> {
    // A close time is the last instant of the bar (…:59.999999), so the
    // sub-millisecond remainder is expected there and is truncated toward
    // the bar it belongs to; an OPEN time must land exactly on a
    // millisecond, because that is the bar's identity.
    if what == "open time" && raw % divisor != 0 {
        return Err(SourceError::new(
            "time_unit_not_exact",
            format!("row {index} {what} {raw} is not a whole millisecond"),
        ));
    }
    Ok(raw.div_euclid(divisor))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real files, downloaded on 2026-09-20 with their published CHECKSUMs
    /// (`fixtures/binance/README.md`). One from each side of the archive's
    /// millisecond → microsecond change.
    const BTC_MS_ZIP: &[u8] = include_bytes!("../../../../fixtures/binance/BTCUSDT-1h-2024-07-15.zip");
    const BTC_MS_CHECKSUM: &[u8] =
        include_bytes!("../../../../fixtures/binance/BTCUSDT-1h-2024-07-15.zip.CHECKSUM");
    const BTC_US_ZIP: &[u8] = include_bytes!("../../../../fixtures/binance/BTCUSDT-1h-2026-09-18.zip");
    const BTC_US_CHECKSUM: &[u8] =
        include_bytes!("../../../../fixtures/binance/BTCUSDT-1h-2026-09-18.zip.CHECKSUM");
    const ETH_US_ZIP: &[u8] = include_bytes!("../../../../fixtures/binance/ETHUSDT-1h-2026-09-18.zip");

    const HOUR: i64 = 3_600_000;

    #[test]
    fn urls_are_built_exactly_as_the_archive_publishes_them() {
        assert_eq!(
            monthly_url("BTCUSDT", "1h", 2024, 7),
            "https://data.binance.vision/data/spot/monthly/klines/BTCUSDT/1h/BTCUSDT-1h-2024-07.zip"
        );
        let date = NaiveDate::from_ymd_opt(2026, 9, 18).unwrap();
        assert_eq!(
            daily_url("ETHUSDT", "1h", date),
            "https://data.binance.vision/data/spot/daily/klines/ETHUSDT/1h/ETHUSDT-1h-2026-09-18.zip"
        );
        assert_eq!(
            checksum_url(&monthly_url("BTCUSDT", "1h", 2024, 7)),
            "https://data.binance.vision/data/spot/monthly/klines/BTCUSDT/1h/BTCUSDT-1h-2024-07.zip.CHECKSUM"
        );
        // The exchange's endTime is inclusive; this workspace's is not.
        assert_eq!(
            rest_klines_url("BTCUSDT", "1h", 1_721_001_600_000, 1_721_005_200_000, 5000),
            "https://api.binance.com/api/v3/klines?symbol=BTCUSDT&interval=1h\
             &startTime=1721001600000&endTime=1721005199999&limit=1000"
                .replace(char::is_whitespace, "")
        );
    }

    #[test]
    fn a_published_checksum_is_parsed_and_enforced() {
        let digest = parse_checksum(BTC_MS_CHECKSUM, "BTCUSDT-1h-2024-07-15.zip").unwrap();
        assert_eq!(digest, "e6fbeb74d3bd74a85bcc42c7d63376a50bf851f6edf9462cb0b486e9cd3a4e0d");
        assert!(verify_checksum(BTC_MS_ZIP, &digest).is_ok());

        // The published digest of a different file proves nothing here.
        let other = parse_checksum(BTC_US_CHECKSUM, "BTCUSDT-1h-2026-09-18.zip").unwrap();
        assert_eq!(
            verify_checksum(BTC_MS_ZIP, &other).unwrap_err().code,
            "checksum_mismatch"
        );
        assert_eq!(
            parse_checksum(BTC_MS_CHECKSUM, "BTCUSDT-1h-2024-07-16.zip").unwrap_err().code,
            "checksum_file_name_mismatch"
        );
        for (bytes, code) in [
            (&b"not a checksum"[..], "checksum_malformed"),
            (&b"zz  BTCUSDT-1h-2024-07-15.zip"[..], "checksum_malformed"),
            (&b"e6fbeb74  BTCUSDT-1h-2024-07-15.zip"[..], "checksum_malformed"),
            (&b""[..], "checksum_malformed"),
        ] {
            assert_eq!(
                parse_checksum(bytes, "BTCUSDT-1h-2024-07-15.zip").unwrap_err().code,
                code
            );
        }
        // A single altered byte is caught.
        let mut tampered = BTC_MS_ZIP.to_vec();
        *tampered.last_mut().unwrap() ^= 0x01;
        assert_eq!(verify_checksum(&tampered, &digest).unwrap_err().code, "checksum_mismatch");
    }

    #[test]
    fn the_single_archive_entry_is_read_and_a_damaged_one_is_refused() {
        let entry = read_single_zip_entry(BTC_MS_ZIP).unwrap();
        assert_eq!(entry.name, "BTCUSDT-1h-2024-07-15.csv");
        assert_eq!(entry.content.lines().count(), 24, "24 hourly bars in a day");

        for (bytes, code) in [
            (&b"not a zip at all, really"[..], "zip_not_an_archive"),
            (&BTC_MS_ZIP[..20], "zip_not_an_archive"),
        ] {
            assert_eq!(read_single_zip_entry(bytes).unwrap_err().code, code);
        }
        // A corrupted deflate stream fails the CRC or the inflate itself.
        let mut damaged = BTC_MS_ZIP.to_vec();
        damaged[80] ^= 0xff;
        assert_eq!(read_single_zip_entry(&damaged).unwrap_err().code, "zip_corrupt_entry");
        // A second member is refused even though the first is readable.
        let mut two_entries = BTC_MS_ZIP.to_vec();
        let eocd = (0..two_entries.len() - 21)
            .rev()
            .find(|at| u32_at(&two_entries, *at) == 0x0605_4b50)
            .unwrap();
        two_entries[eocd + 10] = 2;
        assert_eq!(
            read_single_zip_entry(&two_entries).unwrap_err().code,
            "zip_multiple_entries"
        );
    }

    #[test]
    fn a_millisecond_era_file_is_read_without_conversion() {
        let entry = read_single_zip_entry(BTC_MS_ZIP).unwrap();
        let parsed = parse_archive_csv(&entry.content).unwrap();
        assert_eq!(parsed.source_time_unit, TimeUnit::Milliseconds);
        assert!(!parsed.converted_to_milliseconds && !parsed.header_row_skipped);
        assert_eq!(parsed.rows.len(), 24);
        // 2024-07-15T00:00:00Z, and one bar per hour after it.
        assert_eq!(parsed.rows[0].open_time_ms, 1_721_001_600_000);
        assert_eq!(parsed.rows[0].close_time_ms, 1_721_001_600_000 + HOUR - 1);
        assert_eq!(parsed.rows[23].open_time_ms, 1_721_001_600_000 + 23 * HOUR);
        for (index, row) in parsed.rows.iter().enumerate() {
            assert_eq!(row.open_time_ms, 1_721_001_600_000 + index as i64 * HOUR);
            assert!(row.high >= row.low && row.open > 0.0 && row.volume >= 0.0);
        }
    }

    #[test]
    fn a_microsecond_era_file_is_converted_exactly_and_lands_on_the_same_grid() {
        for zip in [BTC_US_ZIP, ETH_US_ZIP] {
            let entry = read_single_zip_entry(zip).unwrap();
            let parsed = parse_archive_csv(&entry.content).unwrap();
            assert_eq!(
                parsed.source_time_unit,
                TimeUnit::Microseconds,
                "the archive switched to microseconds in 2025"
            );
            assert!(parsed.converted_to_milliseconds);
            assert_eq!(parsed.rows.len(), 24);
            // 2026-09-18T00:00:00Z in milliseconds, on the hourly grid.
            assert_eq!(parsed.rows[0].open_time_ms, 1_789_689_600_000);
            for (index, row) in parsed.rows.iter().enumerate() {
                assert_eq!(row.open_time_ms, 1_789_689_600_000 + index as i64 * HOUR);
                assert_eq!(row.open_time_ms % HOUR, 0, "a converted open time is still a whole bar");
                // The close time is the bar's last microsecond; truncation
                // keeps it inside the bar it belongs to.
                assert_eq!(row.close_time_ms, row.open_time_ms + HOUR - 1);
            }
        }
    }

    #[test]
    fn a_file_that_changes_unit_or_cannot_convert_exactly_is_refused() {
        // Two bars, the second one microseconds: the archive's own change,
        // as it would look INSIDE one file.
        let mixed = b"1721001600000,1,2,0.5,1.5,10,1721005199999,0,1,0,0,0\n\
                      1721005200000000,1,2,0.5,1.5,10,1721008799999999,0,1,0,0,0\n";
        assert_eq!(parse_archive_csv(mixed).unwrap_err().code, "time_unit_unresolved");

        // Microseconds that are not a whole millisecond: refused, never rounded.
        let ragged = b"1721001600000500,1,2,0.5,1.5,10,1721005199999999,0,1,0,0,0\n";
        assert_eq!(parse_archive_csv(ragged).unwrap_err().code, "time_unit_not_exact");

        for (csv, code) in [
            (&b""[..], "csv_empty"),
            (&b"\n\n"[..], "csv_empty"),
            (&b"1721001600000,1,2,0.5,1.5,10\n"[..], "csv_column_count"),
            (&b"1721001600000,x,2,0.5,1.5,10,1721005199999,0,1,0,0,0\n"[..], "csv_not_a_number"),
            (&b"0,1,2,0.5,1.5,10,1,0,1,0,0,0\n"[..], "time_unit_unresolved"),
        ] {
            assert_eq!(parse_archive_csv(csv).unwrap_err().code, code, "{:?}", String::from_utf8_lossy(csv));
        }
    }

    #[test]
    fn a_column_header_is_skipped_only_when_it_really_is_one() {
        let with_header = b"open_time,open,high,low,close,volume,close_time,qav,trades,tbv,tqv,ignore\n\
                            1721001600000,1,2,0.5,1.5,10,1721005199999,0,1,0,0,0\n";
        let parsed = parse_archive_csv(with_header).unwrap();
        assert!(parsed.header_row_skipped && parsed.rows.len() == 1);

        // Anything else that fails to parse is an error, not a header.
        let not_a_header = b"timestamp,open,high,low,close,volume,close_time,qav,trades,tbv,tqv,ignore\n";
        assert_eq!(parse_archive_csv(not_a_header).unwrap_err().code, "csv_not_a_number");
    }

    #[test]
    fn the_rest_response_reads_into_the_same_rows() {
        // The exact shape the endpoint returned on 2026-09-20.
        let body = br#"[[1721001600000,"60797.91000000","61324.01000000","60632.30000000","61211.99000000","1116.66990000",1721005199999,"68065049.70836240",83744,"516.48376000","31492867.75182780","0"]]"#;
        let parsed = parse_rest_klines(body).unwrap();
        assert_eq!(parsed.source_time_unit, TimeUnit::Milliseconds);
        assert_eq!(parsed.rows.len(), 1);
        assert_eq!(parsed.rows[0].open_time_ms, 1_721_001_600_000);
        assert_eq!(parsed.rows[0].close, 61_211.99);
        assert_eq!(parsed.rows[0].volume, 1116.6699);

        for (body, code) in [
            (&br#"{"code":-1121,"msg":"Invalid symbol."}"#[..], "rest_shape"),
            (&b"not json"[..], "rest_shape"),
            (&br#"[[1721001600000,"1"]]"#[..], "csv_column_count"),
            (&b"[]"[..], "csv_empty"),
        ] {
            assert_eq!(parse_rest_klines(body).unwrap_err().code, code);
        }
    }

    trait LineCount {
        fn lines(&self) -> std::str::Lines<'_>;
    }

    impl LineCount for Vec<u8> {
        fn lines(&self) -> std::str::Lines<'_> {
            std::str::from_utf8(self).expect("the archive entry is UTF-8").lines()
        }
    }
}
