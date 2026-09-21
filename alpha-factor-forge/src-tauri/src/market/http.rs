//! P07 — the one place that talks to the network.
//!
//! Everything above this module works against the `HttpFetcher` trait, so
//! the ingest path is exercised offline in the test suite and only the
//! transport itself needs a real network (the phase's real-run evidence).
//! It is also the only module that names the HTTP client crate, so swapping
//! it is a one-file change.
//!
//! The policy here is deliberately narrow, because a market data fetch that
//! quietly went somewhere else is a source-confusion bug that no downstream
//! check can catch:
//!
//! * HTTPS only, and only to a host the caller allowed;
//! * redirects are refused rather than followed — a redirect to another
//!   host is a changed source, not a detail;
//! * a response larger than the caller's limit is an error, never a
//!   truncated body;
//! * retries are bounded and only for failures that can plausibly pass
//!   (transport, timeout, 429, 5xx). A 404 is an answer, not a failure.

use std::fmt;
use std::time::Duration;

use crate::error::AppError;

/// The public archive of monthly/daily kline files.
pub const BINANCE_ARCHIVE_HOST: &str = "data.binance.vision";
/// The exchange's REST API, used for the same instrument's recent bars.
pub const BINANCE_REST_HOST: &str = "api.binance.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchLimits {
    pub timeout: Duration,
    pub max_bytes: u64,
    /// Total attempts, including the first one.
    pub attempts: u32,
    pub backoff: Duration,
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            // A monthly 1m archive is a few MB; 64 MiB is far above anything
            // this contract asks for and far below "whatever arrives".
            max_bytes: 64 * 1024 * 1024,
            attempts: 3,
            backoff: Duration::from_millis(500),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchError {
    /// The resource does not exist. For an archive unit that means "not
    /// published (yet)", which the ingest treats as information, not failure.
    NotFound(String),
    Status { url: String, status: u16 },
    TooLarge { url: String, limit: u64 },
    /// Refused before any request, or refused what came back: not HTTPS, a
    /// host the caller did not allow, or a redirect.
    Refused(String),
    Transport { url: String, attempts: u32, message: String },
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::NotFound(url) => write!(f, "{url} is not published"),
            FetchError::Status { url, status } => write!(f, "{url} answered HTTP {status}"),
            FetchError::TooLarge { url, limit } => {
                write!(f, "{url} is larger than the {limit} byte limit")
            }
            FetchError::Refused(reason) => write!(f, "refused: {reason}"),
            FetchError::Transport { url, attempts, message } => {
                write!(f, "{url} failed after {attempts} attempt(s): {message}")
            }
        }
    }
}

impl std::error::Error for FetchError {}

impl From<FetchError> for AppError {
    fn from(error: FetchError) -> Self {
        AppError::Other(error.to_string())
    }
}

/// One GET, with the caller's limits already applied.
pub trait HttpFetcher: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, FetchError>;
}

/// Refuse anything that is not `https://<allowed host>/...` before a
/// request is made. Returns the host on success.
pub fn check_url<'a>(url: &'a str, allowed_hosts: &[String]) -> Result<&'a str, FetchError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| FetchError::Refused(format!("{url} is not an https URL")))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(FetchError::Refused(format!("{url} has no host")));
    }
    if authority.contains('@') {
        return Err(FetchError::Refused(format!("{url} carries credentials")));
    }
    // A port is allowed only as the default one; anything else is a
    // different endpoint than the contract names.
    let host = match authority.split_once(':') {
        Some((host, "443")) => host,
        Some(_) => return Err(FetchError::Refused(format!("{url} names a non-default port"))),
        None => authority,
    };
    if !allowed_hosts.iter().any(|allowed| allowed == host) {
        return Err(FetchError::Refused(format!(
            "{host} is not one of the hosts this source may use"
        )));
    }
    Ok(host)
}

/// Whether another attempt could plausibly succeed. A 404 or any other 4xx
/// is an answer about the resource, so repeating it only wastes the
/// source's quota.
pub fn status_is_retryable(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

/// The real transport. Constructed once and shared; the agent keeps its
/// connection pool.
pub struct UreqFetcher {
    agent: ureq::Agent,
    limits: FetchLimits,
    allowed_hosts: Vec<String>,
    tiingo_token: Option<super::tiingo_credentials::TiingoToken>,
}

impl UreqFetcher {
    pub fn new(limits: FetchLimits, allowed_hosts: Vec<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(limits.timeout))
            // Refused rather than followed: see the module note.
            .max_redirects(0)
            .max_redirects_will_error(true)
            .https_only(true)
            .user_agent(USER_AGENT)
            .build()
            .new_agent();
        Self { agent, limits, allowed_hosts, tiingo_token: None }
    }

    /// The default limits, for the two Binance hosts.
    pub fn for_binance() -> Self {
        Self::new(
            FetchLimits::default(),
            vec![BINANCE_ARCHIVE_HOST.to_string(), BINANCE_REST_HOST.to_string()],
        )
    }

    pub fn for_tiingo(token: super::tiingo_credentials::TiingoToken) -> Self {
        let mut fetcher = Self::new(FetchLimits {
            timeout: Duration::from_secs(30), max_bytes: 8 * 1024 * 1024,
            // An entitlement or quota failure must be visible, not multiplied
            // across the five-symbol batch. Operator may retry later.
            attempts: 1, backoff: Duration::ZERO,
        }, vec!["api.tiingo.com".into()]);
        fetcher.tiingo_token = Some(token);
        fetcher
    }

    /// P10 public endpoints. One attempt keeps a quota/availability failure
    /// visible and bounds the five-instrument batch; neither host needs a
    /// credential or receives one.
    pub fn for_tw_etf() -> Self {
        Self::new(
            FetchLimits {
                timeout: Duration::from_secs(30),
                max_bytes: 8 * 1024 * 1024,
                attempts: 1,
                backoff: Duration::ZERO,
            },
            vec!["api.finmindtrade.com".into(), "www.twse.com.tw".into()],
        )
    }

    fn attempt(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        let request = self.agent.get(url);
        let request = if let Some(token) = &self.tiingo_token {
            request.header("Authorization", token.header())
        } else { request };
        match request.call() {
            Ok(mut response) => response
                .body_mut()
                .with_config()
                .limit(self.limits.max_bytes)
                .read_to_vec()
                .map_err(|error| match error {
                    ureq::Error::BodyExceedsLimit(limit) => {
                        FetchError::TooLarge { url: url.to_string(), limit }
                    }
                    other => FetchError::Transport {
                        url: url.to_string(),
                        attempts: 1,
                        message: if self.tiingo_token.is_some() { "authenticated response read failed".into() } else { other.to_string() },
                    },
                }),
            Err(ureq::Error::StatusCode(404)) => Err(FetchError::NotFound(url.to_string())),
            Err(ureq::Error::StatusCode(status)) => {
                Err(FetchError::Status { url: url.to_string(), status })
            }
            Err(ureq::Error::TooManyRedirects) => Err(FetchError::Refused(format!(
                "{url} redirected; a redirect can change the source"
            ))),
            Err(ureq::Error::RequireHttpsOnly(reason)) => Err(FetchError::Refused(reason)),
            Err(other) => Err(FetchError::Transport {
                url: url.to_string(),
                attempts: 1,
                message: if self.tiingo_token.is_some() { "authenticated transport failed".into() } else { other.to_string() },
            }),
        }
    }
}

/// Identifies this workspace to the source. A public archive is still
/// someone else's service.
const USER_AGENT: &str = concat!("AlphaFactorForge/", env!("CARGO_PKG_VERSION"), " (research workstation)");

impl HttpFetcher for UreqFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        check_url(url, &self.allowed_hosts)?;
        let mut last = None;
        for attempt in 1..=self.limits.attempts.max(1) {
            match self.attempt(url) {
                Ok(bytes) => return Ok(bytes),
                Err(error) => {
                    let retryable = match &error {
                        FetchError::Transport { .. } => true,
                        FetchError::Status { status, .. } => status_is_retryable(*status),
                        _ => false,
                    };
                    if !retryable || attempt == self.limits.attempts.max(1) {
                        return Err(match error {
                            FetchError::Transport { url, message, .. } => {
                                FetchError::Transport { url, attempts: attempt, message }
                            }
                            other => other,
                        });
                    }
                    last = Some(error);
                    std::thread::sleep(self.limits.backoff * attempt);
                }
            }
        }
        Err(last.unwrap_or_else(|| FetchError::Transport {
            url: url.to_string(),
            attempts: 0,
            message: "no attempt was made".into(),
        }))
    }
}

#[cfg(test)]
pub mod testing {
    //! A fetcher the ingest tests drive: every response is authored, so the
    //! whole path above the transport runs with no network at all.

    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::{FetchError, HttpFetcher};

    #[derive(Default)]
    pub struct FakeFetcher {
        responses: Mutex<HashMap<String, Result<Vec<u8>, FetchError>>>,
        /// Every URL asked for, in order, including repeats.
        pub requested: Mutex<Vec<String>>,
    }

    impl FakeFetcher {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with(self, url: &str, bytes: &[u8]) -> Self {
            self.responses.lock().unwrap().insert(url.to_string(), Ok(bytes.to_vec()));
            self
        }

        pub fn failing(self, url: &str, error: FetchError) -> Self {
            self.responses.lock().unwrap().insert(url.to_string(), Err(error));
            self
        }

        pub fn requested(&self) -> Vec<String> {
            self.requested.lock().unwrap().clone()
        }
    }

    impl HttpFetcher for FakeFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
            self.requested.lock().unwrap().push(url.to_string());
            match self.responses.lock().unwrap().get(url) {
                Some(Ok(bytes)) => Ok(bytes.clone()),
                Some(Err(error)) => Err(error.clone()),
                // Nothing authored for this URL: the archive has not
                // published it. That is what the real source answers too.
                None => Err(FetchError::NotFound(url.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts() -> Vec<String> {
        vec![BINANCE_ARCHIVE_HOST.to_string(), BINANCE_REST_HOST.to_string()]
    }

    #[test]
    fn only_https_to_an_allowed_host_is_ever_requested() {
        assert_eq!(
            check_url(
                "https://data.binance.vision/data/spot/monthly/klines/BTCUSDT/1h/BTCUSDT-1h-2024-07.zip",
                &hosts()
            ),
            Ok("data.binance.vision")
        );
        assert_eq!(
            check_url("https://api.binance.com/api/v3/klines?symbol=BTCUSDT", &hosts()),
            Ok("api.binance.com")
        );
        assert_eq!(check_url("https://data.binance.vision:443/x", &hosts()), Ok("data.binance.vision"));

        for (url, expected) in [
            ("http://data.binance.vision/x", "not an https URL"),
            ("https://evil.example/data/spot/x.zip", "not one of the hosts"),
            ("https://data.binance.vision:8443/x", "non-default port"),
            ("https://user:pass@data.binance.vision/x", "carries credentials"),
            ("https:///x", "has no host"),
            ("data.binance.vision/x", "not an https URL"),
        ] {
            let error = check_url(url, &hosts()).unwrap_err().to_string();
            assert!(error.contains(expected), "{url}: {error}");
        }
    }

    #[test]
    fn only_failures_that_could_pass_are_retried() {
        assert!(status_is_retryable(429) && status_is_retryable(500) && status_is_retryable(503));
        for status in [400, 401, 403, 404, 410, 418, 200, 301] {
            assert!(!status_is_retryable(status), "{status} must not be retried");
        }
    }

    #[test]
    fn the_fake_fetcher_answers_only_what_was_authored() {
        use super::testing::FakeFetcher;
        let fetcher = FakeFetcher::new().with("https://data.binance.vision/a", b"bytes");
        assert_eq!(fetcher.get("https://data.binance.vision/a").unwrap(), b"bytes");
        assert!(matches!(
            fetcher.get("https://data.binance.vision/b"),
            Err(FetchError::NotFound(_))
        ));
        assert_eq!(
            fetcher.requested(),
            vec!["https://data.binance.vision/a", "https://data.binance.vision/b"]
        );
    }
}
