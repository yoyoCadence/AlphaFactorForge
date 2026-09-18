//! P04a — `alpha-factor-forge-service`, the headless research service.
//!
//! The second binary of this package. It declares the same orchestration
//! modules as the desktop (`main.rs`) — the database layer, the discovery
//! runner, and the host-agnostic runtime — and none of the desktop-only
//! ones (`commands`, `desktop`, `single_instance`); `tauri` is never named
//! here or in anything this file pulls in (`runtime::boundary_tests`).
//! The whole lifecycle is `runtime::service`; this file only hands it the
//! arguments and the exit code. A console program on every platform, so a
//! scheduler or an operator sees its log.

// The shared modules also serve the desktop; what only the desktop uses is
// dead here, and that is expected rather than a warning to chase.
#![allow(dead_code)]

mod db;
mod discovery_runner;
mod error;
mod identity;
mod runtime;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(runtime::service::main(args));
}
