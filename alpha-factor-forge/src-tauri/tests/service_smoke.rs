//! P04a — the REAL `alpha-factor-forge-service` binary, spawned as a
//! process on an isolated workspace: it publishes its endpoint, answers the
//! control API with the bearer token and refuses without it, refuses a
//! second service on the same workspace with the documented exit code, and
//! `stop` drains it, withdraws the endpoint, and lets the next host in.
//!
//! An integration test sees only the package's library (the pure core), so
//! the few lines of HTTP it needs are here; the unit tests inside the
//! binary cover the protocol itself. The binary path comes from Cargo
//! (`CARGO_BIN_EXE_alpha-factor-forge-service`), so this runs wherever
//! `cargo test` does — including the CI cargo-check lane on Windows.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const SERVICE: &str = env!("CARGO_BIN_EXE_alpha-factor-forge-service");
const TIMEOUT: Duration = Duration::from_secs(60);

/// The isolated workspace; removed afterwards. Nothing here touches the
/// user's data directory: every invocation passes `--data-dir`. Declared
/// before any `Spawned` so it is dropped after them (services killed first).
struct Workspace(PathBuf);

impl Drop for Workspace {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0)
                .unwrap_or_else(|error| panic!("workspace {} not removed: {error}", self.0.display()));
        }
        let registry = self.0.with_extension("registry");
        if registry.exists() {
            std::fs::remove_dir_all(&registry).unwrap_or_else(|error| {
                panic!("registry {} not removed: {error}", registry.display())
            });
        }
    }
}

/// Kills the service if the test fails before `stop` did.
struct Spawned {
    child: Child,
}

impl Drop for Spawned {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fresh_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("aff-service-smoke-{}-{label}", std::process::id()))
}

fn service(args: &[&str], dir: &Path) -> Command {
    let mut command = Command::new(SERVICE);
    command.env("AFF_TEST_TRIAL_REGISTRY_DIR", dir.with_extension("registry"));
    command.args(args).arg("--data-dir").arg(dir);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    command
}

fn wait_for_manifest(dir: &Path) -> (Value, String) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let (Ok(manifest), Ok(token)) = (
            std::fs::read_to_string(dir.join("control-endpoint.json")),
            std::fs::read_to_string(dir.join("control-token")),
        ) {
            if let Ok(manifest) = serde_json::from_str::<Value>(&manifest) {
                if manifest["port"].as_u64().is_some() && token.trim().len() == 64 {
                    return (manifest, token.trim().to_string());
                }
            }
        }
        assert!(Instant::now() < deadline, "the service never published an endpoint in {}", dir.display());
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// One HTTP/1.1 request; returns (status, JSON body).
fn http(port: u16, method: &str, path: &str, token: Option<&str>, body: Option<&Value>) -> (u16, Value) {
    let payload = body.map(|value| serde_json::to_vec(value).unwrap()).unwrap_or_default();
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    if let Some(token) = token {
        head.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    if method == "POST" {
        head.push_str(&format!("Content-Type: application/json\r\nContent-Length: {}\r\n", payload.len()));
    }
    head.push_str("\r\n");
    let mut stream = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(5)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(&payload).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text.split_once("\r\n\r\n").expect("a complete response");
    let status: u16 = head.split(' ').nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

fn port_answers(port: u16) -> bool {
    TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(1)).is_ok()
}

#[test]
fn the_service_binary_owns_a_workspace_serves_the_control_api_and_stops_cleanly() {
    let workspace = Workspace(fresh_dir("lifecycle"));
    let dir = workspace.0.clone();
    let child = service(&["run"], &dir).spawn().expect("spawn the service binary");
    let mut spawned = Spawned { child };

    let (manifest, token) = wait_for_manifest(&dir);
    let port = manifest["port"].as_u64().unwrap() as u16;
    assert_eq!(manifest["manifestVersion"], "control-endpoint-v1");
    assert_eq!(manifest["holderKind"], "service");
    assert_eq!(manifest["pid"], spawned.child.id());
    assert!(dir.join("alphafactorforge.sqlite3").is_file(), "the workspace database was created");
    assert!(dir.join("ownership.lock").is_file());

    // The token gates every route.
    let (status, body) = http(port, "GET", "/v1/info", None, None);
    assert_eq!((status, body["error"]["code"].as_str()), (401, Some("Unauthorized")));
    let (status, info) = http(port, "GET", "/v1/info", Some(&token), None);
    assert_eq!(status, 200, "{info}");
    assert_eq!(info["workspaceId"], manifest["workspaceId"]);
    assert_eq!(info["instanceId"], manifest["instanceId"]);
    assert_eq!(info["pid"], spawned.child.id());
    assert_eq!(info["commandProtocolVersion"], "research-command-v1");

    // A real envelope through the same dispatcher the desktop uses.
    let envelope = json!({
        "protocolVersion": "research-command-v1",
        "workspaceId": manifest["workspaceId"],
        "requestId": "smoke-ownership",
        "command": "ownership.read",
        "payload": {},
    });
    let (status, answer) = http(port, "POST", "/v1/commands", Some(&token), Some(&envelope));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["result"]["workspaceId"], manifest["workspaceId"]);
    assert_eq!(answer["result"]["ownership"]["holderKind"], "service");
    let mut foreign = envelope.clone();
    foreign["workspaceId"] = json!("0".repeat(32));
    let (status, answer) = http(port, "POST", "/v1/commands", Some(&token), Some(&foreign));
    assert_eq!((status, answer["error"]["code"].as_str()), (400, Some("WorkspaceMismatch")));
    let (status, page) = http(port, "GET", "/v1/events?afterEventId=0&waitMs=0", Some(&token), None);
    assert_eq!(status, 200);
    assert_eq!(page["events"], json!([]));

    // 雙啟: a second service on the same workspace exits with code 2 and
    // leaves the first untouched.
    let second = service(&["run"], &dir).output().expect("run a second service");
    assert_eq!(second.status.code(), Some(2), "stderr: {}", String::from_utf8_lossy(&second.stderr));
    assert!(String::from_utf8_lossy(&second.stderr).contains("owns this workspace"));
    assert_eq!(http(port, "GET", "/v1/info", Some(&token), None).0, 200, "the first still answers");

    // `status` sees it live; `stop` drains it and waits for it to be gone.
    let status_output = service(&["status"], &dir).output().unwrap();
    assert_eq!(status_output.status.code(), Some(0));
    let printed: Value = serde_json::from_slice(&status_output.stdout).unwrap();
    assert_eq!(printed["live"]["pid"], spawned.child.id());

    let stop = service(&["stop"], &dir).output().unwrap();
    assert_eq!(stop.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&stop.stderr));
    assert!(String::from_utf8_lossy(&stop.stdout).contains("service stopped"));
    let exit = spawned.child.wait().expect("the service exits after stop");
    assert_eq!(exit.code(), Some(0));
    assert!(!dir.join("control-endpoint.json").exists(), "endpoint withdrawn");
    assert!(!dir.join("control-token").exists());
    assert!(!port_answers(port));
    let mut log = String::new();
    spawned.child.stdout.take().unwrap().read_to_string(&mut log).unwrap();
    assert!(log.contains("listening on 127.0.0.1:"), "{log}");
    assert!(log.contains("shutdown requested"), "{log}");
    assert!(log.contains("stopped"), "{log}");
    assert!(!log.contains(&token), "the token is never logged");

    // With the lock released, the next host gets in; without a service,
    // `stop` and `status` say so with exit code 4.
    assert_eq!(service(&["stop"], &dir).output().unwrap().status.code(), Some(4));
    assert_eq!(service(&["status"], &dir).output().unwrap().status.code(), Some(4));
    let mut again = Spawned { child: service(&["run"], &dir).spawn().unwrap() };
    let (manifest, _) = wait_for_manifest(&dir);
    assert_eq!(manifest["epoch"], 2, "the next owner bumped the epoch");
    assert_eq!(service(&["stop"], &dir).output().unwrap().status.code(), Some(0));
    assert_eq!(again.child.wait().unwrap().code(), Some(0));
    drop(again);
    drop(spawned);
    drop(workspace);
}

#[test]
fn usage_errors_and_help_have_their_exit_codes() {
    let help = Command::new(SERVICE).arg("--help").output().unwrap();
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).contains("USAGE"));
    let bad = Command::new(SERVICE).arg("serve").output().unwrap();
    assert_eq!(bad.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("unexpected argument"));
}
