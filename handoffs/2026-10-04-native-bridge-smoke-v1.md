# Handoff: native invoke/event CI smoke

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `ci/native-bridge-smoke`
Status: In Progress; scoped plan recorded before implementation.

## Scope / dependency reassessment

The maintainer authorized continued task-board implementation, PRs and checked
merges. CI-TAURI-SMOKE-001b specifies one native invoke/event round trip but was
blocked on new WebDriver dependencies. [Official Playwright WebView2 support](https://playwright.dev/docs/webview2)
uses connectOverCDP and environment-only debugging with an isolated browser
profile. The repository already has Playwright 1.61.1; no dependency, manifest,
capability, product-command or lockfile change is needed.

The Browser runtime was initialized and inspected: only the Chrome-extension
browser is registered, with no native WebView target. This task authors a CI
harness for the separately launched native WebView, not a control path for the
registered Chrome browser. It does not change any rendered product behavior.

## Agreed implementation plan

1. Extend the existing debug/no-bundle native smoke lane with a tracked Windows
   launcher. Start the expected executable hidden with unique temp workspace,
   registry and browser directories. Use AFF_DATA_DIR and the existing debug-only
   AFF_TEST_TRIAL_REGISTRY_DIR seam; never touch the user's workspace/registry.
2. Enable CDP only in the launched test process on loopback at an unused port.
   Verify the listening WebView's process owns the isolated profile. Preserve
   startup/SQLite/WAL assertions and use bounded readiness instead of a fixed
   25-second delay. Clean up only verified owned processes and temp directories.
3. Use existing Playwright to attach to that WebView, require a rendered native
   page without the mock seam, call existing SQLite-backed commands, and verify
   a unique payload through the native event plugin's listen/emit_to/unlisten
   path allowed by current capabilities. Unregister callback/listener and close
   the CDP connection on both success and failure.
4. Reuse existing CI lanes, run the harness locally, and require final-head native
   CI before merging. Capture a failure screenshot for diagnosis if useful.

The event test checks the builtin Tauri event bridge, not discovery ordering,
reconnection, service hand-over or P12 campaign operator acceptance. No API keys,
network data providers or statistical simulations are involved.

## Initial native evidence (2026-10-05)

The existing tauri build --debug --no-bundle command linked the real binary and
built the production frontend. A local run at http://tauri.localhost/ rendered
the app and returned real embedded workspace identity, empty SQLite dataset/
candle results and the exact nonce/nested event payload with no console/runtime
errors. The first run exposed a WebView profile cleanup race after the bridge
assertions passed. The launcher now snapshots verified profile descendants,
checks process identity before stopping them and retries only a verified owned
temp path for at most five seconds on IOException. The next full run, including
cleanup, exited successfully in 5.7 s. One wrong camelCase argument rejection
check was then added; final verification follows after rebase onto PR #156.

Screenshot/result/log artifacts are under ignored test-results/native-smoke;
CI uploads them on success or failure. No artifact or user data is committed.
