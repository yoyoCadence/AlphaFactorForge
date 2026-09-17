//! Desktop (Tauri) adapters over the host-agnostic runtime.
//!
//! Anything that needs an `AppHandle`, a window, or the Tauri event bus lives
//! here or in `commands`; the runtime, runner, and database never see them
//! (`runtime::boundary_tests`). `single_instance.rs` predates this module and
//! stays where the PR #103 smoke lane knows it.

pub mod discovery_events;
