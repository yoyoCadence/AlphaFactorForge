//! Pure Rust contracts used by the backend discovery engine.
//!
//! Tauri commands, SQLite repositories, threading, and events must not enter
//! this library boundary. They remain orchestration concerns in the binary.

pub mod discovery_core;
