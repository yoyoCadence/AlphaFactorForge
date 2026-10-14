# Handoff: native metrics window acceptance

Date: 2026-10-14
Repo: yoyoCadence/AlphaFactorForge
Branch: `test/native-metrics-window-sync`
Base: merged PR #176 (`a8e73de`); rebase after #177 before publication.
Status: Implemented and verified locally; exact-head CI pending.

## Scope

Extend the existing isolated native smoke to drive actual results through the
installed Rust event plugin into a separate Tauri WebView2. Import 600 monotone
daily bars, choose price-above/below-slow signals and zero costs, then run a
30% Holdout. Compare every rendered metrics row and the sample note between
main and child; Sortino, Calmar and Profit Factor must be ∞ in all three columns.

Then invalidate the result, require the child to clear, rerun without Holdout,
import/run exactly 365 daily bars, reload the child listener/ready handshake
and focus the existing window. No mock, injected metrics, production hook,
new permission, CSP change, dataset/schema change or dependency.

## Verification

Built the #176 desktop with `npm run tauri -- build --debug --no-bundle`,
using an independent temporary Cargo target. Ran the existing
`scripts/native-smoke.ps1` against that binary: all actual IPC, SQLite,
numeric-policy, event and metrics-window assertions passed. The launcher
verified its loopback/profile owner and removed all owned processes and
workspace/registry/profile paths after each attempt.

Local artifacts: `native-metrics-evidence-3/native-result.json`,
`native-metrics-holdout.png`, `native-metrics-full-year.png` in the isolated
worktree. CI uploads these same assertions and screenshots through the existing
native-smoke artifact; local absolute paths are not needed for reproduction.

Two initial attempts exposed a test locator issue (the wrapping select label
includes option text) and pre-existing CSP messages. The repaired test retains
eight startup observations rather than discarding them: blocked Google Fonts
are the existing `FONT-OFFLINE-001`; locked Tauri 2.11.3
`scripts/ipc-protocol.js` explicitly falls back to postMessage after its IPC
fetch is blocked. Only those exact message sources are classified; unexpected
console errors and all page errors still fail. The test does not claim an
error-free console or widen CSP.

## Limits

Native OS close/reopen and #174's release-dialog OK → exit 1 remain unexecuted:
the Windows computer-use connection reports its native pipe unavailable.
Listener reload and existing-window focus are explicitly distinct from OS
close/reopen. The captured real release dialog remains valid earlier evidence.

## Resolution

The previously untested #172 non-finite transport, per-column note, clear,
update and listener recovery now have a real native regression test. Publication
and six exact-head CI checks follow; the two OS interactions stay open.
