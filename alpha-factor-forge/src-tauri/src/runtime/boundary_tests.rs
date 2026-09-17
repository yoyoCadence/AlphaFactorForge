//! P02 boundary guard (docs/research-runtime-contract.md §0).
//!
//! The host-agnostic modules — the runtime, the runner orchestration, and the
//! database layer — must compile without the desktop: that is what lets a
//! headless service (P04) reuse them unchanged. One Cargo package cannot
//! express "these modules may not use `tauri`", so this test reads the
//! sources and asserts it, the same way `exprInterpreter.test.ts` scans its
//! own file for dynamic code. Line comments and doc comments are stripped
//! first, so the word may still appear in prose (this file included).

/// Every source file that must stay free of the desktop framework.
const HOST_AGNOSTIC_SOURCES: &[(&str, &str)] = &[
    ("runtime/mod.rs", include_str!("mod.rs")),
    ("runtime/lease.rs", include_str!("lease.rs")),
    ("db/mod.rs", include_str!("../db/mod.rs")),
    ("db/ownership.rs", include_str!("../db/ownership.rs")),
    ("db/repositories.rs", include_str!("../db/repositories.rs")),
    ("db/discovery.rs", include_str!("../db/discovery.rs")),
    ("db/validation_record.rs", include_str!("../db/validation_record.rs")),
    ("discovery_runner/mod.rs", include_str!("../discovery_runner/mod.rs")),
    ("discovery_runner/execution.rs", include_str!("../discovery_runner/execution.rs")),
    ("identity.rs", include_str!("../identity.rs")),
    ("error.rs", include_str!("../error.rs")),
];

/// Code only: `//` line comments (including `///` and `//!`) removed.
fn without_line_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(index) => &line[..index],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn host_agnostic_modules_never_name_the_desktop_framework() {
    let mut offenders = Vec::new();
    for (name, source) in HOST_AGNOSTIC_SOURCES {
        let code = without_line_comments(source);
        for (line_number, line) in code.lines().enumerate() {
            if line.contains("tauri") {
                offenders.push(format!("{name}:{}: {}", line_number + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "host-agnostic modules reference tauri:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_desktop_adapter_is_where_tauri_events_live() {
    // The inverse: the adapter exists and is the one place that emits through
    // the framework, so a reviewer looking for the sink finds exactly one.
    let adapter = without_line_comments(include_str!("../desktop/discovery_events.rs"));
    assert!(adapter.contains("use tauri::"), "the desktop adapter imports tauri");
    assert!(adapter.contains("impl DiscoveryEventSink for TauriDiscoveryEventSink"));
}

#[test]
fn the_comment_stripper_keeps_code_and_drops_doc_prose() {
    let stripped = without_line_comments("let a = 1; // tauri in a comment\n/// tauri doc\nlet b = 2;");
    assert_eq!(stripped, "let a = 1; \n\nlet b = 2;");
}
