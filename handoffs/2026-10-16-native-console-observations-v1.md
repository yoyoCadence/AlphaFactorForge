# Handoff: deterministic native startup console observations

Date: 2026-10-16
Branch: test/native-startup-console-observations
Base: merged #184 (`13649f4`)
Status: Verified locally; separate prerequisite PR for #185.

## Failure and scope

PR #185 head `ec966db` passed five checks, but native-smoke failed in
[run 38901884460](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38901884460),
job `116975870758`. Main-window CDP attached early enough to capture existing
CSP denials for startup IPC methods and the Google Fonts stylesheet. All
actual invokes and event/metrics assertions had completed before the final
console assertion failed. The metrics child already classified and retained
these sources under the accepted #178 test; main did not. A slower attachment
could miss them, so re-running the job alone would leave the timing defect.

The fix shares a narrow observer across main and child. Known startup methods
are an explicit per-surface allowlist, matched only on the observed CSP
message forms and exact IPC origin. Fonts retain `FONT-OFFLINE-001`; locked
Tauri's fallback retains its reason. Successful and failed main artifacts
now retain classified text/type/reason, as the child already did. All page
errors, unexpected console errors and functional assertions stay fatal.
No product/CSP/network/font-policy change or timeout relaxation.

## Verification

Four Node regressions run before every native smoke. They replay the CI
message forms, retain all observations, cover late/metrics attachment and
reject other commands/origins, non-CSP failures and adjacent unexpected errors.
`node --test scripts/native-console-observations.test.mjs`: 4 passed.

Built the real debug desktop with `npm run tauri -- build --debug --no-bundle`
(includes typecheck/frontend build), then ran `scripts/native-smoke.ps1`
against it. Real SQLite invokes, wrong argument refusal, manual numeric
strategy cases, Rust event payload, metrics Holdout/infinities/clear/update,
listener reload and window reuse all passed. Main attached late in this run
(0 known observations); child retained 8; unexpected errors 0. Both timing
paths are covered by the replay regressions. Owned processes and all three
temporary paths were removed by the existing launcher.

Evidence: OS temp `aff-native-console-20261016/`, including screenshots and
`native/native-result.json`. Existing UI/backend suites are unchanged; six
exact-head CI checks remain required before merging the repair. Rebase #185
onto the merged repair and recheck its final head before merging it. Then
continue P13c design from the merged baseline.
