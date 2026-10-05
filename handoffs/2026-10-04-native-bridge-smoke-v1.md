# Handoff: native invoke/event CI smoke

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `ci/native-bridge-smoke`
Status: Resolved; PR #157 merged after six green CI jobs.

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

## Final local verification (2026-10-05)

Rebased onto merged PR #156 (`863adaf7`), preserving its DB-ASYNC-001c task
status and this task's dependency reassessment in the task-board conflict.
Rebuilt the real debug binary with the same CI command (production typecheck/
build also pass). The final native smoke passed in 8.9 s, including the
wrong-key error `missing required key datasetId`, full event payload equality,
zero console/runtime errors, verified process shutdown and temp deletion.

A temporary test-only assertion mutation then forced a genuine failure. The
launcher propagated failure, captured its screenshot/JSON and left no native
app or new aff-native-* temp directory. The mutation was immediately restored;
no bypass or failure switch is committed. Node syntax, PowerShell 5 parser and
diff checks pass. The working source/manifest/capabilities/lockfiles are unchanged.
Full unit/mock E2E suites are exercised by final-head CI, not repeated locally
for this harness-only change. No native discovery/service/campaign claim is made.

## CI startup investigation (2026-10-05)

[PR #157](https://github.com/yoyoCadence/AlphaFactorForge/pull/157), initial head
`ef9b93f7`, [run 37244064242](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37244064242):
five jobs passed; native build linked successfully, but the live desktop did
not expose the CDP port within the existing 45-second readiness deadline.
This does not establish a root cause or bridge acceptance on the runner.

The scoped CI fix retains the assertions/deadline, uses explicit child test
environment values when Start-Process supports them (PowerShell 7.4+, with the
existing inherited path for PS5), avoids the automatic PROFILE variable name,
and captures app/DB/WAL/WebView profile/debug-port flags before teardown on
failure. No process environment or unrelated browser command line is dumped.
User authorization to autonomously complete/verify PRs covers this CI fix;
the working GitHub connector follows AGENTS.md over the gh-fix-ci CLI preference.

## Elevated runner browser flags (2026-10-05)

The second run, [37244841303](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37244841303),
again passed five jobs but timed out before CDP: the desktop stayed alive, DB/WAL
existed and six WebView processes matched the isolated profile, while none had
the requested debug-port argument. [Microsoft's elevated-host rules](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/security)
say environment and HKCU browser overrides are ignored for elevated hosts;
HKLM overrides are honored. This is a plausible explanation, pending runner
elevation diagnostics and a passing native run.

The launcher now detects an enabled administrator token. Only when both
GITHUB_ACTIONS=true and RUNNER_ENVIRONMENT=github-hosted does an elevated launch
set the documented AdditionalBrowserArguments policy for the exact executable
name. It saves any existing value/type and restores them in finally, or deletes
only the added value; no wildcard policy or registry-tree deletion. Policy
restoration runs before process/temp cleanup and errors still fail the smoke.
An elevated local/self-hosted launch is refused before allocating temp paths;
the local non-elevated environment path is unchanged. This changes no user's
registry, product code, capability or dependency, and preserves the 45-second
deadline and all bridge assertions. Native CI remains required before merge.

Local non-elevated verification passed in 7.0 s (`elevated=False`,
`hostedPolicy=False`), including real commands/event and complete cleanup; no
local policy write was performed. PowerShell 5 parsing, Node syntax and diff
checks pass. The elevated policy path is exercised only on the hosted CI runner.

## Startup readiness ordering (2026-10-05)

[Run 37245718679](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37245718679)
confirmed `elevated=True` and `hostedPolicy=True`: the profile-owned WebView
received the debug-port flag and exposed loopback CDP. The exact policy value
was restored during failure cleanup. The smoke then checked DB/WAL immediately,
1.9 s after launch, before those files existed; earlier 45-second failures had
both files. CDP availability alone is therefore insufficient startup readiness.

The launcher now waits for CDP and both non-empty SQLite/WAL files within the
same existing 45-second deadline before inspecting ownership and invoking the
bridge. No assertion or timeout is weakened. CI must still prove the complete
bridge/cleanup path; the newly observed readiness ordering is not acceptance.

## Resolution (2026-10-05)

[PR #157](https://github.com/yoyoCadence/AlphaFactorForge/pull/157) merged as
`e86270652848e5a9f12e85fbe174dd95356de258` after all six jobs passed on final
head `b6093ab8ea726ebb1a445ee2ffe269c0de2dc6f4`,
[run 37246153488](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37246153488).
The native runner confirmed elevated=True/hostedPolicy=True, then CDP plus
SQLite/WAL readiness, real invoke/event assertions, exact policy restoration
and verified process/temp cleanup. The hosted smoke completed in about 4.3 s
after launch. Earlier failed runs and their diagnostics remain above.
This closes CI-TAURI-SMOKE-001b only; native campaign/discovery/service operator
acceptance remains separate.
