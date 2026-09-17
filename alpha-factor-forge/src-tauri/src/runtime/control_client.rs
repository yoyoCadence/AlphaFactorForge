//! P04a — the client half of the local control interface (contract §4).
//!
//! Used by the service's own `stop`/`status` subcommands, by the tests of
//! the server, and — next phase — by the desktop in connect mode. One
//! request per connection, `Connection: close`, JSON in and out; nothing
//! more than the server in `control_api` speaks.

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::time::Duration;

use serde_json::Value;

use super::commands::CommandError;
use super::control_api::{ControlToken, MAX_LONG_POLL};

/// A parsed response: status, lower-cased header names, body bytes.
#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(&self) -> Result<Value, ClientError> {
        serde_json::from_slice(&self.body)
            .map_err(|error| ClientError::Protocol(format!("response body is not JSON: {error}")))
    }
}

#[derive(Debug)]
pub enum ClientError {
    /// The socket: nothing listening, a timeout, a reset.
    Io(io::Error),
    /// The bytes came back but are not the protocol this client speaks.
    Protocol(String),
    /// A complete HTTP answer that is not a command outcome (401, 403, 404
    /// for an unknown route, 503 for too many connections, ...).
    Http { status: u16, body: String },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Io(error) => write!(f, "control api connection: {error}"),
            ClientError::Protocol(message) => write!(f, "control api protocol: {message}"),
            ClientError::Http { status, body } => write!(f, "control api answered {status}: {body}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<io::Error> for ClientError {
    fn from(error: io::Error) -> Self {
        ClientError::Io(error)
    }
}

/// A client for one published endpoint.
#[derive(Clone)]
pub struct ControlClient {
    port: u16,
    token: ControlToken,
    /// Applies to the connect, the write, and every read. `events` extends
    /// it by the requested wait so a long-poll is never cut short.
    timeout: Duration,
}

impl ControlClient {
    pub fn new(port: u16, token: ControlToken) -> Self {
        Self { port, token, timeout: Duration::from_secs(10) }
    }

    /// `GET /v1/info`.
    pub fn info(&self) -> Result<Value, ClientError> {
        let response = self.request("GET", "/v1/info", None, &[])?;
        expect_ok(&response)?;
        response.json()
    }

    /// `POST /v1/commands`. The outer error is the transport; the inner
    /// result is the command's own outcome, exactly as the dispatcher
    /// produced it (a rejection is a `CommandError` with its code).
    pub fn dispatch(&self, envelope: &Value) -> Result<Result<Value, CommandError>, ClientError> {
        let body = serde_json::to_vec(envelope).map_err(|error| ClientError::Protocol(error.to_string()))?;
        let response = self.request("POST", "/v1/commands", Some(&body), &[])?;
        let json = response.json()?;
        if let Some(error) = json.get("error") {
            let error: CommandError = serde_json::from_value(error.clone())
                .map_err(|error| ClientError::Protocol(format!("error body is not a CommandError: {error}")))?;
            return Ok(Err(error));
        }
        match json.get("result") {
            Some(result) if response.status == 200 => Ok(Ok(result.clone())),
            _ => Err(ClientError::Http { status: response.status, body: String::from_utf8_lossy(&response.body).into_owned() }),
        }
    }

    /// `GET /v1/events`: the page after `after`, waiting up to `wait` (the
    /// server clamps it to `MAX_LONG_POLL`) for the ledger to grow.
    pub fn events(&self, after: i64, limit: Option<usize>, wait: Duration) -> Result<Value, ClientError> {
        let wait = wait.min(MAX_LONG_POLL);
        let mut path = format!("/v1/events?afterEventId={after}&waitMs={}", wait.as_millis());
        if let Some(limit) = limit {
            path.push_str(&format!("&limit={limit}"));
        }
        let response = self.send(&self.timeout.saturating_add(wait), "GET", &path, None, &[])?;
        expect_ok(&response)?;
        response.json()
    }

    /// `POST /v1/shutdown`: ask the service to drain and stop. Returns once
    /// the request was accepted, not once the service exited.
    pub fn shutdown(&self) -> Result<(), ClientError> {
        let response = self.request("POST", "/v1/shutdown", Some(b"{}"), &[])?;
        if response.status == 202 {
            Ok(())
        } else {
            Err(ClientError::Http { status: response.status, body: String::from_utf8_lossy(&response.body).into_owned() })
        }
    }

    /// One authenticated request; `extra_headers` are appended verbatim.
    pub fn request(
        &self,
        method: &str,
        path_and_query: &str,
        body: Option<&[u8]>,
        extra_headers: &[(&str, &str)],
    ) -> Result<HttpResponse, ClientError> {
        self.send(&self.timeout, method, path_and_query, body, extra_headers)
    }

    fn send(
        &self,
        timeout: &Duration,
        method: &str,
        path_and_query: &str,
        body: Option<&[u8]>,
        extra_headers: &[(&str, &str)],
    ) -> Result<HttpResponse, ClientError> {
        let mut head = format!("{method} {path_and_query} HTTP/1.1\r\n");
        head.push_str(&format!("Host: 127.0.0.1:{}\r\n", self.port));
        head.push_str(&format!("Authorization: Bearer {}\r\n", self.token.as_str()));
        head.push_str("Accept: application/json\r\n");
        head.push_str("Connection: close\r\n");
        if body.is_some() || method == "POST" {
            head.push_str("Content-Type: application/json\r\n");
            head.push_str(&format!("Content-Length: {}\r\n", body.map_or(0, <[u8]>::len)));
        }
        for (name, value) in extra_headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        let mut bytes = head.into_bytes();
        if let Some(body) = body {
            bytes.extend_from_slice(body);
        }
        Ok(send_raw(self.port, &bytes, *timeout)?)
    }
}

fn expect_ok(response: &HttpResponse) -> Result<(), ClientError> {
    if response.status == 200 {
        Ok(())
    } else {
        Err(ClientError::Http { status: response.status, body: String::from_utf8_lossy(&response.body).into_owned() })
    }
}

/// Write `request` to the loopback port and read the whole response (the
/// server closes the connection after it). Public so tests can send
/// deliberately malformed requests; nothing else should need it.
pub fn send_raw(port: u16, request: &[u8], timeout: Duration) -> io::Result<HttpResponse> {
    let mut stream = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(request)?;
    stream.flush()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    parse_response(&raw).map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))
}

fn parse_response(raw: &[u8]) -> Result<HttpResponse, String> {
    let head_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "response has no header terminator".to_string())?;
    let head = String::from_utf8_lossy(&raw[..head_end]);
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|text| text.parse::<u16>().ok())
        .ok_or_else(|| format!("malformed status line: {status_line:?}"))?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let body_start = head_end + 4;
    let declared = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .map(|(_, value)| value.parse::<usize>().map_err(|_| "Content-Length is not a number".to_string()))
        .transpose()?;
    let body = match declared {
        Some(length) if raw.len() >= body_start + length => raw[body_start..body_start + length].to_vec(),
        Some(length) => return Err(format!("body truncated: {} of {length} bytes", raw.len() - body_start)),
        None => raw[body_start..].to_vec(),
    };
    Ok(HttpResponse { status, headers, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_response_is_split_into_status_headers_and_the_declared_body() {
        let response = parse_response(b"HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}").unwrap();
        assert_eq!(response.status, 404);
        assert_eq!(response.headers[0], ("content-type".to_string(), "application/json".to_string()));
        assert_eq!(response.body, b"{}");
        assert!(parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n{}").unwrap_err().contains("truncated"));
        assert!(parse_response(b"garbage").unwrap_err().contains("terminator"));
        assert!(parse_response(b"HTTP/1.1 abc\r\n\r\n").unwrap_err().contains("status line"));
    }

    #[test]
    fn nothing_listening_is_an_io_error_not_a_hang() {
        let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let client = ControlClient::new(port, ControlToken::parse(&"a".repeat(64)).unwrap());
        let error = client.info().unwrap_err();
        assert!(matches!(error, ClientError::Io(_)), "{error}");
    }
}
