# Handoff: RUNNER-OWNERSHIP-001 single-instance guard

Date: 2026-08-09
Repo: yoyoCadence/AlphaFactorForge
Branch: fix/runner-single-instance-ownership
PR: #90
Status: resolved and merged

## Summary

The desktop backend now rejects a secondary process before `setup`, SQLite initialization, or orphan recovery. This closes the split-brain path where process B could pause/requeue process A's live discovery run.

## Required Action / Decision

Review the plugin-first registration and Windows native smoke evidence. No owner-generation/lease schema task is proposed: v1 is intentionally a single-process desktop application, and the official plugin now enforces that boundary. Add a lease only if supported multi-process execution becomes a product requirement.

## Review Notes

- `tauri-plugin-single-instance` 2.4.3 is scoped to Windows, macOS, and Linux only.
- The plugin is the first `Builder` plugin, before `setup` and `recover_orphans`, as required by the [official Tauri single-instance documentation](https://v2.tauri.app/plugin/single-instance/).
- A secondary launch restores the primary window in the explicit order show → unminimize → focus. Callback failures are reported to stderr without panicking the primary process.
- The plugin requires Rust 1.77.2, so `Cargo.toml` and all three README language sections now declare 1.77.2+.
- The initial smoke attempt showed that overriding `APPDATA` does not override Windows Known Folder resolution. Before proceeding, a read-only query confirmed the normal database contained no discovery runs. The successful test then used a temporary Tauri config identifier, and both its Roaming and Local app-data directories were deleted after all owned processes exited.

## Verification

- `npm run test` — 690 passed
- `npm run typecheck`
- `npm run build`
- `cargo check --locked`
- `cargo test --locked` — 135 passed
- `cargo clippy --locked --all-targets` — no new warnings; four pre-existing warnings remain in `backtest.rs` / `score.rs`
- Targeted `rustfmt --check`
- `git diff --check`
- Windows native two-launch smoke:
  - secondary exit code: 0
  - live same-binary process count after launch: 1
  - injected run status after secondary launch: `running`
  - minimized after callback: false
  - primary is foreground after callback: true

## Resolution (added when acted on)

Implemented the official plugin guard, deterministic focus behavior, dependency/MSRV alignment, regression tests, and isolated native verification. PR and commit identifiers will be added after publication.

Published as PR #90 from commit `acabec5` and merged into `main` as `a451f0d` on 2026-08-09. All five required GitHub Actions jobs passed (`typecheck`, `test`, `build`, `cargo-check`, and `e2e`), and the PR closed without additional review comments.
