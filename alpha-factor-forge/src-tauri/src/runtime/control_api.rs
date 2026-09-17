//! P04a — the local control interface
//! (docs/research-runtime-contract.md §4, `control-endpoint-v1`).
//!
//! A headless service owns the workspace and answers `research-command-v1`
//! envelopes over HTTP/1.1 JSON on a loopback socket. This module is the
//! server half: the endpoint manifest and control token it publishes for
//! other hosts, the request rules (loopback `Host`, no browser `Origin`,
//! bearer token compared in constant time, bounded bodies), the four routes,
//! and the long-poll that lets a reader wait for the ledger to grow instead
//! of spinning.
//!
//! It is written on `std::net` on purpose. The only clients are this
//! project's own hosts (the desktop bridge, the MCP adapter, the `stop`
//! subcommand) and they need nothing an HTTP framework adds; keeping the
//! surface to "one request, one JSON response, `Connection: close`" means
//! there is no keep-alive, pipelining, chunked-encoding, or upgrade path to
//! get wrong. Every limit is a constant here and every rule has a test.
//!
//! Not on the wire, by contract: the token never appears in a response,
//! the database, or a log line; there is no shell, SQL, file, or live
//! trading endpoint; and nothing binds outside `127.0.0.1`.

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::commands::{CommandError, Dispatcher, ErrorCode, COMMAND_PROTOCOL_VERSION, EVENT_PROTOCOL_VERSION};
use crate::discovery_runner::{DiscoveryEvent, DiscoveryEventSink};

/// The manifest's own version; a reader refuses any other.
pub const MANIFEST_VERSION: &str = "control-endpoint-v1";
/// Where the service publishes its endpoint, inside the workspace data
/// directory (beside the database and the lock file).
pub const MANIFEST_FILE_NAME: &str = "control-endpoint.json";
/// The control token, same directory, same permissions, never inside the
/// manifest so the manifest can be shown or logged without leaking it.
pub const TOKEN_FILE_NAME: &str = "control-token";
/// 32 random bytes as lowercase hex.
pub const TOKEN_HEX_LEN: usize = 64;
/// The largest request body accepted; an envelope is a few hundred bytes.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;
/// The largest request head (request line + headers) accepted.
pub const MAX_HEAD_BYTES: usize = 16 * 1024;
/// The longest a `GET /v1/events` may wait for new events (`waitMs` is
/// clamped to this). A client's read timeout must exceed it.
pub const MAX_LONG_POLL: Duration = Duration::from_secs(30);
/// Socket read/write timeout for the request head and body and for the
/// response; the long-poll wait happens after the request was fully read.
pub const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections handled at once; beyond it the server answers 503 at once.
pub const MAX_CONNECTIONS: usize = 32;
/// After answering, how long and how much the server keeps reading what a
/// client is still sending (a refused over-long request), so the refusal
/// reaches the client instead of a connection reset.
pub const LINGER_TIMEOUT: Duration = Duration::from_secs(1);
pub const MAX_LINGER_BYTES: usize = MAX_BODY_BYTES + MAX_HEAD_BYTES;
/// Reported in the manifest and `/v1/info`: the package version.
pub const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The header that carries the token: `Authorization: Bearer <token>`.
pub const AUTH_HEADER: &str = "authorization";

// ---------- token ----------

/// The random control token. `Debug` redacts it and there is no `Display`,
/// so it cannot reach a log line by accident; the only way to read it is
/// `as_str`, used when writing the token file and by a client sending it.
#[derive(Clone, PartialEq, Eq)]
pub struct ControlToken(String);

impl ControlToken {
    /// 32 bytes from the OS CSPRNG (`getrandom`), as lowercase hex.
    pub fn generate() -> io::Result<Self> {
        let mut bytes = [0u8; TOKEN_HEX_LEN / 2];
        getrandom::fill(&mut bytes).map_err(|error| io::Error::other(format!("os randomness: {error}")))?;
        Ok(Self(hex::encode(bytes)))
    }

    /// Accept exactly what `generate` produces (after trimming whitespace).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let well_formed = text.len() == TOKEN_HEX_LEN
            && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        well_formed.then(|| Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Constant-time comparison against a presented credential.
    pub fn matches(&self, presented: &str) -> bool {
        constant_time_eq(self.0.as_bytes(), presented.as_bytes())
    }
}

impl fmt::Debug for ControlToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ControlToken(<redacted>)")
    }
}

/// Byte-wise comparison whose duration does not depend on where the inputs
/// first differ. A length mismatch is folded in rather than returned early;
/// the token length is public anyway.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for (index, byte) in a.iter().enumerate() {
        diff |= usize::from(byte ^ b.get(index).copied().unwrap_or(0));
    }
    diff == 0
}

// ---------- endpoint manifest ----------

/// What a connecting host needs to find and trust the service (contract §4):
/// the port, and the identity to verify against `/v1/info` before use (a
/// manifest left behind by a crashed service names a dead instance).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EndpointManifest {
    pub manifest_version: String,
    pub port: u16,
    pub workspace_id: String,
    pub epoch: i64,
    pub holder_kind: String,
    pub instance_id: String,
    pub pid: u32,
    pub service_version: String,
    /// RFC 3339, UTC.
    pub started_at: String,
}

pub fn manifest_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MANIFEST_FILE_NAME)
}

pub fn token_path(data_dir: &Path) -> PathBuf {
    data_dir.join(TOKEN_FILE_NAME)
}

/// Publish the endpoint: each file is staged beside its final name and
/// renamed into place, so a reader sees either the previous file or the
/// complete new one. On Unix both files are created `0600`; on Windows they
/// inherit the data directory's ACL, which under the user's profile
/// (`%APPDATA%`) is already private to that user.
pub fn write_endpoint_files(data_dir: &Path, manifest: &EndpointManifest, token: &ControlToken) -> io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let manifest_json = serde_json::to_vec_pretty(manifest).map_err(io::Error::other)?;
    write_private_atomically(&token_path(data_dir), token.as_str().as_bytes())?;
    write_private_atomically(&manifest_path(data_dir), &manifest_json)
}

fn write_private_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let staging = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&staging)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    std::fs::rename(&staging, path)
}

/// Read a published endpoint. `None` when there is no manifest; an error
/// when there is one that cannot be trusted (unreadable, another version,
/// or a token file that is missing or malformed).
pub fn read_endpoint_files(data_dir: &Path) -> io::Result<Option<(EndpointManifest, ControlToken)>> {
    let manifest_text = match std::fs::read_to_string(manifest_path(data_dir)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let manifest: EndpointManifest = serde_json::from_str(&manifest_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("endpoint manifest: {error}")))?;
    if manifest.manifest_version != MANIFEST_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("endpoint manifest version {} is not {MANIFEST_VERSION}", manifest.manifest_version),
        ));
    }
    let token_text = std::fs::read_to_string(token_path(data_dir))?;
    let token = ControlToken::parse(&token_text)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "control token file is malformed"))?;
    Ok(Some((manifest, token)))
}

/// Withdraw the endpoint. Missing files are not an error.
pub fn remove_endpoint_files(data_dir: &Path) -> io::Result<()> {
    for path in [manifest_path(data_dir), token_path(data_dir)] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

// ---------- event notifier ----------

/// Wakes long-poll readers whenever the ledger grew. The service's host
/// sink bumps it AFTER `LedgerSink` appended the row, so a woken reader
/// always finds what it was woken for.
#[derive(Default)]
pub struct EventNotifier {
    seq: Mutex<u64>,
    changed: Condvar,
}

impl EventNotifier {
    pub fn notify(&self) {
        if let Ok(mut seq) = self.seq.lock() {
            *seq = seq.wrapping_add(1);
        }
        self.changed.notify_all();
    }

    pub fn current(&self) -> u64 {
        self.seq.lock().map(|seq| *seq).unwrap_or(0)
    }

    /// Block until the sequence moved past `seen` or `timeout` elapsed;
    /// returns the sequence observed on return.
    pub fn wait_past(&self, seen: u64, timeout: Duration) -> u64 {
        let Ok(guard) = self.seq.lock() else { return seen };
        match self.changed.wait_timeout_while(guard, timeout, |seq| *seq == seen) {
            Ok((guard, _)) => *guard,
            Err(_) => seen,
        }
    }
}

/// The host sink of a headless service: there is no window to post to, so
/// an event only wakes whoever is long-polling the ledger.
pub struct NotifyingSink(pub Arc<EventNotifier>);

impl DiscoveryEventSink for NotifyingSink {
    fn emit(&self, _event: &DiscoveryEvent) -> Result<(), String> {
        self.0.notify();
        Ok(())
    }
}

// ---------- server ----------

/// Who is answering, for `/v1/info` and the manifest.
#[derive(Clone, Debug)]
pub struct ServiceIdentity {
    pub workspace_id: String,
    pub epoch: i64,
    pub holder_kind: &'static str,
    pub instance_id: String,
    pub pid: u32,
}

struct Shared {
    dispatcher: Arc<Dispatcher>,
    token: ControlToken,
    identity: ServiceIdentity,
    notifier: Arc<EventNotifier>,
    port: u16,
    /// `POST /v1/shutdown` was accepted: mutating commands are refused from
    /// now on and the host is expected to drain and stop.
    shutdown_requested: AtomicBool,
    shutdown_signal: (Mutex<bool>, Condvar),
    /// `stop()` was called: the accept loop exits.
    stopping: AtomicBool,
    connections: AtomicUsize,
}

impl Shared {
    fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        if let Ok(mut flag) = self.shutdown_signal.0.lock() {
            *flag = true;
        }
        self.shutdown_signal.1.notify_all();
        // Long-polls return at once so a client following the run sees the
        // drain begin instead of waiting out its timeout.
        self.notifier.notify();
    }

    fn shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }
}

/// A bound, listening control server. Dropping it (or `stop`) ends the
/// accept loop; connections already being answered finish on their own.
pub struct ControlServer {
    shared: Arc<Shared>,
    accept_thread: Option<JoinHandle<()>>,
}

impl ControlServer {
    /// Bind `127.0.0.1` on a port the OS picks and start answering.
    pub fn bind(
        dispatcher: Arc<Dispatcher>,
        token: ControlToken,
        identity: ServiceIdentity,
        notifier: Arc<EventNotifier>,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let shared = Arc::new(Shared {
            dispatcher,
            token,
            identity,
            notifier,
            port,
            shutdown_requested: AtomicBool::new(false),
            shutdown_signal: (Mutex::new(false), Condvar::new()),
            stopping: AtomicBool::new(false),
            connections: AtomicUsize::new(0),
        });
        let accept_thread = {
            let shared = shared.clone();
            thread::Builder::new()
                .name("control-api-accept".into())
                .spawn(move || accept_loop(listener, shared))?
        };
        Ok(Self { shared, accept_thread: Some(accept_thread) })
    }

    pub fn port(&self) -> u16 {
        self.shared.port
    }

    pub fn shutdown_requested(&self) -> bool {
        self.shared.shutdown_requested()
    }

    /// Wait up to `timeout` for a `POST /v1/shutdown`; true once one arrived.
    pub fn wait_for_shutdown_request(&self, timeout: Duration) -> bool {
        let (flag, changed) = &self.shared.shutdown_signal;
        let Ok(guard) = flag.lock() else { return self.shutdown_requested() };
        match changed.wait_timeout_while(guard, timeout, |requested| !*requested) {
            Ok((guard, _)) => *guard,
            Err(_) => self.shutdown_requested(),
        }
    }

    /// As if `POST /v1/shutdown` arrived (the host decided to stop itself).
    pub fn request_shutdown(&self) {
        self.shared.request_shutdown();
    }

    /// Stop accepting. Returns once the accept loop has exited.
    pub fn stop(mut self) {
        self.stop_accepting();
    }

    fn stop_accepting(&mut self) {
        if self.shared.stopping.swap(true, Ordering::SeqCst) {
            return;
        }
        // Wake the blocking accept with one connection to ourselves.
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.shared.port));
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop_accepting();
    }
}

fn accept_loop(listener: TcpListener, shared: Arc<Shared>) {
    for incoming in listener.incoming() {
        if shared.stopping.load(Ordering::SeqCst) {
            break;
        }
        let stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("control api: accept failed: {error}");
                continue;
            }
        };
        if shared.connections.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            shared.connections.fetch_sub(1, Ordering::SeqCst);
            let mut stream = stream;
            let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
            let _ = write_response(&mut stream, &error_response(503, ErrorCode::Busy, "too many connections", true));
            continue;
        }
        let for_thread = shared.clone();
        let spawned = thread::Builder::new().name("control-api-conn".into()).spawn(move || {
            handle_connection(stream, &for_thread);
            for_thread.connections.fetch_sub(1, Ordering::SeqCst);
        });
        if let Err(error) = spawned {
            eprintln!("control api: cannot spawn a connection thread: {error}");
            shared.connections.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

// ---------- one request ----------

#[derive(Debug)]
struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct Response {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

fn json_response(status: u16, body: &Value) -> Response {
    Response {
        status,
        headers: Vec::new(),
        body: serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec()),
    }
}

fn error_response(status: u16, code: ErrorCode, message: impl Into<String>, retryable: bool) -> Response {
    json_response(status, &json!({ "error": CommandError::new(code, message, retryable) }))
}

/// The HTTP status a command error travels under. The body carries the
/// contract's code either way; the status is for clients that look no
/// further than the status line.
fn status_for(code: &ErrorCode) -> u16 {
    match code {
        ErrorCode::UnsupportedProtocol | ErrorCode::WorkspaceMismatch | ErrorCode::Validation => 400,
        ErrorCode::Unauthorized => 401,
        ErrorCode::NotFound => 404,
        ErrorCode::DuplicateRequest | ErrorCode::NotOwner | ErrorCode::StaleOwner => 409,
        ErrorCode::Busy => 503,
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        505 => "HTTP Version Not Supported",
        _ => "Unknown",
    }
}

fn handle_connection(mut stream: TcpStream, shared: &Shared) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let response = match read_request(&mut stream) {
        Ok(request) => route(&request, shared),
        Err(response) => response,
    };
    let _ = write_response(&mut stream, &response);
    let _ = stream.shutdown(Shutdown::Write);
    // Lingering close: a request refused before it was fully read (431, 413)
    // is still being sent; closing with unread bytes would reset the
    // connection and lose the answer. Drain until the client closes, briefly.
    let _ = stream.set_read_timeout(Some(LINGER_TIMEOUT));
    let mut discard = [0u8; 4096];
    let mut drained = 0usize;
    while drained < MAX_LINGER_BYTES {
        match stream.read(&mut discard) {
            Ok(0) | Err(_) => break,
            Ok(read) => drained += read,
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}

fn write_response(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    let mut head = format!("HTTP/1.1 {} {}\r\n", response.status, reason(response.status));
    head.push_str("Content-Type: application/json\r\n");
    head.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
    head.push_str("Cache-Control: no-store\r\n");
    head.push_str("Connection: close\r\n");
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

/// Read and parse one request. Every refusal is a complete response.
fn read_request(stream: &mut TcpStream) -> Result<Request, Response> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0u8; 2048];
    let head_end = loop {
        if let Some(end) = find_head_end(&buffer) {
            break end;
        }
        if buffer.len() >= MAX_HEAD_BYTES {
            return Err(error_response(431, ErrorCode::Validation, "request head exceeds 16 KiB", false));
        }
        let read = stream
            .read(&mut chunk)
            .map_err(|error| error_response(400, ErrorCode::Validation, format!("cannot read request: {error}"), false))?;
        if read == 0 {
            return Err(error_response(400, ErrorCode::Validation, "connection closed before the request head ended", false));
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
    let mut request = parse_head(&head)?;
    let mut body = buffer[head_end + 4..].to_vec();

    if request.headers.contains_key("transfer-encoding") {
        return Err(error_response(411, ErrorCode::Validation, "chunked bodies are not accepted; send Content-Length", false));
    }
    let content_length = match request.headers.get("content-length") {
        Some(text) => text
            .trim()
            .parse::<usize>()
            .map_err(|_| error_response(400, ErrorCode::Validation, "Content-Length is not a number", false))?,
        None if request.method == "POST" => {
            return Err(error_response(411, ErrorCode::Validation, "POST requires Content-Length", false))
        }
        None => 0,
    };
    if content_length > MAX_BODY_BYTES {
        return Err(error_response(413, ErrorCode::Validation, "request body exceeds 1 MiB", false));
    }
    if body.len() > content_length {
        return Err(error_response(400, ErrorCode::Validation, "more body bytes than Content-Length", false));
    }
    while body.len() < content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| error_response(400, ErrorCode::Validation, format!("cannot read request body: {error}"), false))?;
        if read == 0 {
            return Err(error_response(400, ErrorCode::Validation, "connection closed before the body ended", false));
        }
        let wanted = (content_length - body.len()).min(read);
        body.extend_from_slice(&chunk[..wanted]);
        if wanted < read {
            return Err(error_response(400, ErrorCode::Validation, "more body bytes than Content-Length", false));
        }
    }
    request.body = body;
    Ok(request)
}

fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Request line and headers. Names are lower-cased; a duplicated `Host` or
/// `Content-Length` is refused rather than picked from.
fn parse_head(head: &str) -> Result<Request, Response> {
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(error_response(400, ErrorCode::Validation, "malformed request line", false));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(error_response(505, ErrorCode::Validation, "HTTP/1.x only", false));
    }
    if !target.starts_with('/') {
        return Err(error_response(400, ErrorCode::Validation, "request target must be origin-form", false));
    }
    let (path, query_text) = match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    };
    let mut query = HashMap::new();
    if let Some(query_text) = query_text {
        for pair in query_text.split('&').filter(|pair| !pair.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            query.insert(key.to_string(), value.to_string());
        }
    }
    let mut headers: HashMap<String, String> = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(error_response(400, ErrorCode::Validation, "malformed header line", false));
        };
        let name = name.trim().to_ascii_lowercase();
        if (name == "host" || name == "content-length") && headers.contains_key(&name) {
            return Err(error_response(400, ErrorCode::Validation, format!("duplicate {name} header"), false));
        }
        headers.entry(name).or_insert_with(|| value.trim().to_string());
    }
    Ok(Request {
        method: method.to_string(),
        path: path.to_string(),
        query,
        headers,
        body: Vec::new(),
    })
}

/// `Host` must name this loopback endpoint (contract §4). The port, when
/// given, must be ours: a request steered here through another name or port
/// is not one of our clients.
fn host_is_this_loopback(host: &str, port: u16) -> bool {
    let host = host.trim().to_ascii_lowercase();
    let (name, given_port) = match host.rsplit_once(':') {
        Some((name, port_text)) => (name, Some(port_text)),
        None => (host.as_str(), None),
    };
    let name_ok = name == "127.0.0.1" || name == "localhost";
    let port_ok = match given_port {
        Some(text) => text.parse::<u16>().is_ok_and(|given| given == port),
        None => true,
    };
    name_ok && port_ok
}

fn route(request: &Request, shared: &Shared) -> Response {
    // Origin/Host before authentication: a browser page must learn nothing,
    // not even that a token is required.
    if request.headers.contains_key("origin") {
        return error_response(403, ErrorCode::Unauthorized, "browser origins may not use the control interface", false);
    }
    match request.headers.get("host") {
        Some(host) if host_is_this_loopback(host, shared.port) => {}
        Some(_) => return error_response(403, ErrorCode::Unauthorized, "Host must be this loopback endpoint", false),
        None => return error_response(400, ErrorCode::Validation, "Host header is required", false),
    }
    let presented = request
        .headers
        .get(AUTH_HEADER)
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim());
    if !presented.is_some_and(|token| shared.token.matches(token)) {
        let mut response = error_response(401, ErrorCode::Unauthorized, "a valid control token is required", false);
        response.headers.push(("WWW-Authenticate", "Bearer".to_string()));
        return response;
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/info") => json_response(200, &info(shared)),
        ("POST", "/v1/commands") => commands(request, shared),
        ("GET", "/v1/events") => events(request, shared),
        ("POST", "/v1/shutdown") => {
            shared.request_shutdown();
            json_response(202, &json!({ "accepted": true, "shuttingDown": true }))
        }
        (_, "/v1/info" | "/v1/events") => method_not_allowed("GET"),
        (_, "/v1/commands" | "/v1/shutdown") => method_not_allowed("POST"),
        _ => error_response(404, ErrorCode::NotFound, format!("no route {} {}", request.method, request.path), false),
    }
}

fn method_not_allowed(allow: &'static str) -> Response {
    let mut response = error_response(405, ErrorCode::Validation, format!("method not allowed; use {allow}"), false);
    response.headers.push(("Allow", allow.to_string()));
    response
}

fn info(shared: &Shared) -> Value {
    json!({
        "manifestVersion": MANIFEST_VERSION,
        "workspaceId": shared.identity.workspace_id,
        "epoch": shared.identity.epoch,
        "holderKind": shared.identity.holder_kind,
        "instanceId": shared.identity.instance_id,
        "pid": shared.identity.pid,
        "serviceVersion": SERVICE_VERSION,
        "commandProtocolVersion": COMMAND_PROTOCOL_VERSION,
        "eventProtocolVersion": EVENT_PROTOCOL_VERSION,
        "shuttingDown": shared.shutdown_requested(),
    })
}

/// Commands that change nothing and stay available while draining.
const READ_COMMANDS: &[&str] = &["discovery.progress", "discovery.active", "events.read", "ownership.read"];

fn commands(request: &Request, shared: &Shared) -> Response {
    let envelope: Value = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(error) => return error_response(400, ErrorCode::Validation, format!("body is not JSON: {error}"), false),
    };
    let is_read = envelope
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| READ_COMMANDS.contains(&command));
    if shared.shutdown_requested() && !is_read {
        // Never reserved, so the same requestId can be sent to the next owner.
        return error_response(503, ErrorCode::Busy, "the service is shutting down; retry against the next owner", true);
    }
    match shared.dispatcher.dispatch(envelope) {
        Ok(result) => json_response(200, &json!({ "result": result })),
        Err(error) => json_response(status_for(&error.code), &json!({ "error": error })),
    }
}

fn events(request: &Request, shared: &Shared) -> Response {
    let number = |key: &str| -> Result<Option<i64>, Response> {
        match request.query.get(key) {
            None => Ok(None),
            Some(text) => text
                .parse::<i64>()
                .map(Some)
                .map_err(|_| error_response(400, ErrorCode::Validation, format!("{key} must be an integer"), false)),
        }
    };
    let after = match number("afterEventId") {
        Ok(value) => value.unwrap_or(0),
        Err(response) => return response,
    };
    let limit = match number("limit") {
        Ok(value) => value.map(|n| n.max(0) as usize),
        Err(response) => return response,
    };
    let wait = match number("waitMs") {
        Ok(value) => Duration::from_millis(value.unwrap_or(0).max(0) as u64).min(MAX_LONG_POLL),
        Err(response) => return response,
    };
    match events_long_poll(shared, after, limit, wait) {
        Ok(page) => json_response(200, &page),
        Err(error) => json_response(status_for(&error.code), &json!({ "error": error })),
    }
}

/// Answer at once when there is something to say — events after the cursor,
/// a state version that moved (the reader must re-snapshot, contract §3), or
/// a shutdown under way — and otherwise wait for the ledger to grow, at most
/// `wait`. The notifier sequence is sampled BEFORE each page read so an
/// append between the read and the wait is not missed.
fn events_long_poll(shared: &Shared, after: i64, limit: Option<usize>, wait: Duration) -> Result<Value, CommandError> {
    let deadline = Instant::now() + wait;
    let mut seen = shared.notifier.current();
    let mut page = shared.dispatcher.events_page(after, limit)?;
    let first_version = page["stateVersion"].clone();
    loop {
        let has_events = page["events"].as_array().is_some_and(|events| !events.is_empty());
        if has_events || page["stateVersion"] != first_version || shared.shutdown_requested() {
            return Ok(page);
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(page);
        }
        seen = shared.notifier.wait_past(seen, deadline - now);
        page = shared.dispatcher.events_page(after, limit)?;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::db::ownership::{acquire, HolderKind};
    use crate::db::runtime_ledger;
    use crate::discovery_runner::DiscoveryRunner;
    use crate::runtime::commands::InFlightRequests;
    use crate::runtime::control_client::{send_raw, ControlClient};
    use crate::runtime::SharedDb;

    // ---------- token ----------

    #[test]
    fn a_generated_token_is_64_lowercase_hex_and_round_trips_through_parse() {
        let token = ControlToken::generate().unwrap();
        assert_eq!(token.as_str().len(), TOKEN_HEX_LEN);
        assert!(token.as_str().bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(ControlToken::parse(&format!("  {}\n", token.as_str())).unwrap(), token);
        assert_ne!(ControlToken::generate().unwrap(), token, "two tokens differ");
        assert_eq!(format!("{token:?}"), "ControlToken(<redacted>)");
    }

    #[test]
    fn parse_refuses_anything_but_the_generated_shape() {
        assert!(ControlToken::parse("").is_none());
        assert!(ControlToken::parse(&"a".repeat(63)).is_none());
        assert!(ControlToken::parse(&"A".repeat(64)).is_none(), "upper-case hex is not what we write");
        assert!(ControlToken::parse(&"g".repeat(64)).is_none());
        assert!(ControlToken::parse(&"a".repeat(64)).is_some());
    }

    #[test]
    fn constant_time_eq_compares_whole_inputs() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"ab", b"abc"));
        assert!(constant_time_eq(b"", b""));
    }

    // ---------- manifest ----------

    fn fresh_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("aff-control-api-test-{}-{n}", std::process::id()))
    }

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            if self.0.exists() {
                std::fs::remove_dir_all(&self.0)
                    .unwrap_or_else(|error| panic!("temp dir {} not removed: {error}", self.0.display()));
            }
        }
    }

    fn manifest(port: u16) -> EndpointManifest {
        EndpointManifest {
            manifest_version: MANIFEST_VERSION.into(),
            port,
            workspace_id: "w".repeat(32),
            epoch: 3,
            holder_kind: "service".into(),
            instance_id: "i".repeat(32),
            pid: std::process::id(),
            service_version: SERVICE_VERSION.into(),
            started_at: "2026-09-17T00:00:00Z".into(),
        }
    }

    #[test]
    fn endpoint_files_round_trip_and_the_token_stays_out_of_the_manifest() {
        let dir = fresh_dir();
        let _guard = TempDir(dir.clone());
        assert_eq!(read_endpoint_files(&dir).unwrap(), None, "nothing published yet");

        let token = ControlToken::generate().unwrap();
        write_endpoint_files(&dir, &manifest(4242), &token).unwrap();
        let (read_manifest, read_token) = read_endpoint_files(&dir).unwrap().expect("published");
        assert_eq!(read_manifest, manifest(4242));
        assert_eq!(read_token, token);
        let manifest_text = std::fs::read_to_string(manifest_path(&dir)).unwrap();
        assert!(!manifest_text.contains(token.as_str()), "the manifest never carries the token");
        assert!(std::fs::read_dir(&dir).unwrap().all(|entry| {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            !name.contains("tmp-")
        }), "no staging file left behind");

        // Re-publishing replaces atomically; withdrawing removes both files.
        let second = ControlToken::generate().unwrap();
        write_endpoint_files(&dir, &manifest(4343), &second).unwrap();
        assert_eq!(read_endpoint_files(&dir).unwrap().unwrap().1, second);
        remove_endpoint_files(&dir).unwrap();
        assert_eq!(read_endpoint_files(&dir).unwrap(), None);
        remove_endpoint_files(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn endpoint_files_are_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = fresh_dir();
        let _guard = TempDir(dir.clone());
        write_endpoint_files(&dir, &manifest(1), &ControlToken::generate().unwrap()).unwrap();
        for path in [manifest_path(&dir), token_path(&dir)] {
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{}", path.display());
        }
    }

    #[test]
    fn a_manifest_of_another_version_or_without_its_token_is_refused() {
        let dir = fresh_dir();
        let _guard = TempDir(dir.clone());
        let mut other = manifest(1);
        other.manifest_version = "control-endpoint-v2".into();
        write_endpoint_files(&dir, &other, &ControlToken::generate().unwrap()).unwrap();
        let error = read_endpoint_files(&dir).unwrap_err();
        assert!(error.to_string().contains("control-endpoint-v2"), "{error}");

        write_endpoint_files(&dir, &manifest(1), &ControlToken::generate().unwrap()).unwrap();
        std::fs::write(token_path(&dir), "not a token").unwrap();
        assert!(read_endpoint_files(&dir).unwrap_err().to_string().contains("malformed"));
        std::fs::remove_file(token_path(&dir)).unwrap();
        assert_eq!(read_endpoint_files(&dir).unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    // ---------- request rules ----------

    fn parse(head: &str) -> Result<Request, u16> {
        parse_head(head).map_err(|response| response.status)
    }

    #[test]
    fn the_head_parser_lower_cases_names_splits_the_query_and_refuses_ambiguity() {
        let request = parse("GET /v1/events?afterEventId=7&waitMs=10&flag HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization:  Bearer x \r\n").unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/v1/events");
        assert_eq!(request.query.get("afterEventId").map(String::as_str), Some("7"));
        assert_eq!(request.query.get("flag").map(String::as_str), Some(""));
        assert_eq!(request.headers.get("authorization").map(String::as_str), Some("Bearer x"));

        assert_eq!(parse("GET /v1/info\r\n").unwrap_err(), 400, "no version");
        assert_eq!(parse("GET /v1/info HTTP/2.0\r\n").unwrap_err(), 505);
        assert_eq!(parse("GET http://127.0.0.1/v1/info HTTP/1.1\r\n").unwrap_err(), 400, "absolute-form");
        assert_eq!(parse("GET /v1/info HTTP/1.1\r\nHost: a\r\nHost: b\r\n").unwrap_err(), 400, "two hosts");
        assert_eq!(parse("GET /v1/info HTTP/1.1\r\nContent-Length: 1\r\ncontent-length: 2\r\n").unwrap_err(), 400);
        assert_eq!(parse("GET /v1/info HTTP/1.1\r\nno colon\r\n").unwrap_err(), 400);
    }

    #[test]
    fn host_must_be_this_loopback_endpoint() {
        assert!(host_is_this_loopback("127.0.0.1", 5000));
        assert!(host_is_this_loopback("127.0.0.1:5000", 5000));
        assert!(host_is_this_loopback("LocalHost:5000", 5000));
        assert!(host_is_this_loopback("localhost", 5000));
        assert!(!host_is_this_loopback("127.0.0.1:5001", 5000), "another port");
        assert!(!host_is_this_loopback("127.0.0.2", 5000));
        assert!(!host_is_this_loopback("[::1]:5000", 5000), "we never bind v6");
        assert!(!host_is_this_loopback("example.com:5000", 5000));
        assert!(!host_is_this_loopback("", 5000));
    }

    // ---------- the server over real sockets ----------

    struct TestServer {
        server: ControlServer,
        token: ControlToken,
        workspace_id: String,
        notifier: Arc<EventNotifier>,
        _db: SharedDb,
    }

    fn test_server() -> TestServer {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        crate::db::apply_migrations(&conn).unwrap();
        let db: SharedDb = Arc::new(Mutex::new(conn));
        let (epoch, instance_id, workspace_id) = {
            let mut conn = db.lock().unwrap();
            let acquired = acquire(&mut conn, HolderKind::Service, std::process::id()).unwrap();
            (acquired.epoch, acquired.instance_id, runtime_ledger::workspace_id(&conn).unwrap())
        };
        let notifier = Arc::new(EventNotifier::default());
        let dispatcher = Arc::new(Dispatcher::new(
            db.clone(),
            DiscoveryRunner::with_epoch(epoch),
            epoch,
            workspace_id.clone(),
            Arc::new(NotifyingSink(notifier.clone())),
            Arc::new(InFlightRequests::default()),
        ));
        let token = ControlToken::generate().unwrap();
        let identity = ServiceIdentity {
            workspace_id: workspace_id.clone(),
            epoch,
            holder_kind: HolderKind::Service.as_str(),
            instance_id,
            pid: std::process::id(),
        };
        let server = ControlServer::bind(dispatcher, token.clone(), identity, notifier.clone()).unwrap();
        TestServer { server, token, workspace_id, notifier, _db: db }
    }

    impl TestServer {
        fn client(&self) -> ControlClient {
            ControlClient::new(self.server.port(), self.token.clone())
        }

        fn raw(&self, request: String) -> (u16, Value) {
            let response = send_raw(self.server.port(), request.as_bytes(), Duration::from_secs(5)).unwrap();
            let body: Value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
            (response.status, body)
        }

        fn envelope(&self, request_id: &str, command: &str, payload: Value) -> Value {
            json!({
                "protocolVersion": COMMAND_PROTOCOL_VERSION,
                "workspaceId": self.workspace_id,
                "requestId": request_id,
                "command": command,
                "payload": payload,
            })
        }
    }

    fn error_code(body: &Value) -> &str {
        body["error"]["code"].as_str().unwrap_or("<no error>")
    }

    #[test]
    fn info_and_a_read_command_answer_with_the_bearer_token() {
        let server = test_server();
        let client = server.client();
        let info = client.info().unwrap();
        assert_eq!(info["workspaceId"], server.workspace_id);
        assert_eq!(info["holderKind"], "service");
        assert_eq!(info["epoch"], 1);
        assert_eq!(info["manifestVersion"], MANIFEST_VERSION);
        assert_eq!(info["commandProtocolVersion"], COMMAND_PROTOCOL_VERSION);
        assert_eq!(info["shuttingDown"], false);
        assert!(!serde_json::to_string(&info).unwrap().contains(server.token.as_str()), "no token in info");

        let result = client.dispatch(&server.envelope("r1", "ownership.read", json!({}))).unwrap().unwrap();
        assert_eq!(result["workspaceId"], server.workspace_id);
        let active = client.dispatch(&server.envelope("r2", "discovery.active", json!({}))).unwrap().unwrap();
        assert_eq!(active["run"], Value::Null);
    }

    #[test]
    fn a_request_without_a_valid_token_is_refused_before_any_route() {
        let server = test_server();
        let port = server.server.port();
        let (status, body) = server.raw(format!("GET /v1/info HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"));
        assert_eq!((status, error_code(&body)), (401, "Unauthorized"));
        let wrong = "0".repeat(TOKEN_HEX_LEN);
        let (status, _) = server.raw(format!("GET /v1/info HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {wrong}\r\n\r\n"));
        assert_eq!(status, 401, "a well-formed but wrong token");
        let (status, _) = server.raw(format!("GET /v1/info HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Basic {}\r\n\r\n", server.token.as_str()));
        assert_eq!(status, 401, "another scheme");
        let (status, _) = server.raw("GET /nowhere HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n".to_string());
        assert_eq!(status, 401, "an unknown route is not even revealed without the token");
    }

    #[test]
    fn a_browser_origin_or_a_foreign_host_is_refused_even_with_the_token() {
        let server = test_server();
        let token = server.token.as_str();
        let port = server.server.port();
        let (status, body) = server.raw(format!(
            "GET /v1/info HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://localhost:5173\r\nAuthorization: Bearer {token}\r\n\r\n"
        ));
        assert_eq!((status, error_code(&body)), (403, "Unauthorized"));
        let (status, _) = server.raw(format!(
            "GET /v1/info HTTP/1.1\r\nHost: example.com\r\nAuthorization: Bearer {token}\r\n\r\n"
        ));
        assert_eq!(status, 403);
        let (status, _) = server.raw(format!(
            "GET /v1/info HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {token}\r\n\r\n",
            port.wrapping_add(1)
        ));
        assert_eq!(status, 403, "our address, another port");
        let (status, _) = server.raw(format!("GET /v1/info HTTP/1.1\r\nAuthorization: Bearer {token}\r\n\r\n"));
        assert_eq!(status, 400, "no Host at all");
    }

    #[test]
    fn routes_methods_and_bodies_are_bounded() {
        let server = test_server();
        let token = server.token.as_str();
        let auth = format!("Host: 127.0.0.1\r\nAuthorization: Bearer {token}\r\n");
        let (status, body) = server.raw(format!("GET /v1/nope HTTP/1.1\r\n{auth}\r\n"));
        assert_eq!((status, error_code(&body)), (404, "NotFound"));
        let (status, _) = server.raw(format!("POST /v1/info HTTP/1.1\r\n{auth}Content-Length: 0\r\n\r\n"));
        assert_eq!(status, 405);
        let (status, _) = server.raw(format!("GET /v1/commands HTTP/1.1\r\n{auth}\r\n"));
        assert_eq!(status, 405);
        let (status, _) = server.raw(format!("POST /v1/commands HTTP/1.1\r\n{auth}\r\n"));
        assert_eq!(status, 411, "POST without Content-Length");
        let (status, _) = server.raw(format!("POST /v1/commands HTTP/1.1\r\n{auth}Transfer-Encoding: chunked\r\n\r\n"));
        assert_eq!(status, 411, "chunked is not accepted");
        let (status, _) = server.raw(format!("POST /v1/commands HTTP/1.1\r\n{auth}Content-Length: {}\r\n\r\n", MAX_BODY_BYTES + 1));
        assert_eq!(status, 413);
        let (status, _) = server.raw(format!("POST /v1/commands HTTP/1.1\r\n{auth}Content-Length: 2\r\n\r\n{{}}extra"));
        assert_eq!(status, 400, "more bytes than announced");
        let (status, body) = server.raw(format!("POST /v1/commands HTTP/1.1\r\n{auth}Content-Length: 3\r\n\r\nnot"));
        assert_eq!((status, error_code(&body)), (400, "Validation"));
        let (status, body) = server.raw(format!("POST /v1/commands HTTP/1.1\r\n{auth}Content-Length: 2\r\n\r\n{{}}"));
        assert_eq!((status, error_code(&body)), (400, "Validation"), "not an envelope");
        let (status, _) = server.raw(format!("GET /v1/events?afterEventId=x HTTP/1.1\r\n{auth}\r\n"));
        assert_eq!(status, 400);
        let head = format!("GET /v1/info HTTP/1.1\r\n{auth}X-Pad: {}\r\n\r\n", "p".repeat(MAX_HEAD_BYTES));
        let (status, _) = server.raw(head);
        assert_eq!(status, 431);
    }

    #[test]
    fn command_errors_travel_under_a_matching_status_with_the_contract_body() {
        let server = test_server();
        let client = server.client();
        let mut foreign = server.envelope("r1", "ownership.read", json!({}));
        foreign["workspaceId"] = json!("f".repeat(32));
        let error = client.dispatch(&foreign).unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::WorkspaceMismatch);
        let missing = server.envelope("r2", "discovery.progress", json!({ "runId": 999 }));
        let error = client.dispatch(&missing).unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::NotFound);
        let response = client
            .request("POST", "/v1/commands", Some(serde_json::to_vec(&missing).unwrap().as_slice()), &[])
            .unwrap();
        assert_eq!(response.status, 404, "the status line agrees with the code");
    }

    #[test]
    fn events_long_poll_waits_for_the_ledger_and_returns_at_once_when_woken() {
        let server = test_server();
        let client = server.client();
        // Nothing to read: the poll waits out (a bounded) `waitMs`.
        let started = Instant::now();
        let page = client.events(0, None, Duration::from_millis(300)).unwrap();
        assert!(page["events"].as_array().unwrap().is_empty());
        assert_eq!(page["lastEventId"], 0);
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(300) && waited < Duration::from_secs(5), "{waited:?}");
        // `waitMs=0` returns immediately.
        let started = Instant::now();
        client.events(0, None, Duration::ZERO).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));

        // A waiting poll is woken by the notifier (what the ledger sink does
        // after appending) and re-reads instead of sleeping out its wait.
        let notifier = server.notifier.clone();
        let port = server.server.port();
        let token = server.token.clone();
        let poller = thread::spawn(move || {
            let client = ControlClient::new(port, token);
            let started = Instant::now();
            let page = client.events(0, None, MAX_LONG_POLL).unwrap();
            (page, started.elapsed())
        });
        thread::sleep(Duration::from_millis(200));
        // No event row was appended (only the wake-up), so the page is still
        // empty, but the poll must not have waited for the full 30 s.
        notifier.notify();
        thread::sleep(Duration::from_millis(50));
        notifier.notify();
        // With nothing new to return, the loop keeps waiting; end it early.
        server.server.request_shutdown();
        let (page, elapsed) = poller.join().unwrap();
        assert!(page["events"].as_array().unwrap().is_empty());
        assert!(elapsed < Duration::from_secs(5), "woken, not timed out: {elapsed:?}");
    }

    #[test]
    fn shutdown_refuses_new_work_keeps_reads_and_wakes_the_host() {
        let server = test_server();
        let client = server.client();
        assert!(!server.server.wait_for_shutdown_request(Duration::from_millis(50)));
        client.shutdown().unwrap();
        assert!(server.server.wait_for_shutdown_request(Duration::from_secs(5)));
        assert!(server.server.shutdown_requested());
        assert_eq!(client.info().unwrap()["shuttingDown"], true);

        let start = server.envelope("s1", "discovery.start", json!({}));
        let error = client.dispatch(&start).unwrap().unwrap_err();
        assert_eq!(error.code, ErrorCode::Busy);
        assert!(error.retryable, "never reserved: the same id may go to the next owner");
        let read = client.dispatch(&server.envelope("s2", "ownership.read", json!({}))).unwrap();
        assert!(read.is_ok(), "reads continue while draining: {read:?}");
        // A second shutdown is idempotent.
        client.shutdown().unwrap();
    }

    #[test]
    fn stop_ends_the_accept_loop_and_the_port_stops_answering() {
        let server = test_server();
        let port = server.server.port();
        let TestServer { server, .. } = server;
        server.stop();
        let refused = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(2));
        assert!(refused.is_err(), "nothing listens after stop");
    }
}
