# Handoff: chart E2E waits for font/layout readiness

Date: 2026-10-05
Repo: yoyoCadence/AlphaFactorForge
Branch: `test/chart-layout-ready`, from merged PR #156 (`863adaf7`)
Status: Complete locally; rebased onto checked #157/#158 merges, ready for PR/CI.

## Scope / agreed plan

The maintainer authorized ongoing task-board implementation and checked PR
merges. TEST-E2E-LAYOUT-001 identifies cold-CDN font swaps between the canvas
bounding-box read and manual mouse input in pan/zoom E2E. Mock UPSERT parity is
complete locally; publication remains ordered after PR #157 and that mock fix.

1. Install a test-only resource event observer before navigation so the existing
   ThemeProvider link (afs-fonts) records either stylesheet load or failure.
   This avoids treating a late stylesheet as ready or waiting forever after
   an already-fired error. Font-CDN failure can use the product's fallback.
2. Before every coordinate calculation in pan/zoom, await that resource result,
   document.fonts.ready, and two animation frames; then read the canvas box.
   Share this small helper, retaining the existing missing-box error.
3. Preserve wheel/no-scroll, click/drag threshold, window arithmetic and replay
   no-future-data assertions. Run the four existing pan/zoom flows and final-head
   full E2E CI; no new mirror test or product/UI/dependency change is needed.

This is authored project E2E validation, not a browser-control workaround for
the registered Chrome Browser session. It makes no styling/performance claim.

## Implementation / verification

The shared chartLayout helper observes stylesheet load/error from an init
script, waits for the marked afs-fonts link and used fonts, then two animation
frames before boundingBox. All three manual coordinate reads in pan/zoom use
it; every existing wheel/no-scroll, click/drag, window and replay assertion is
unchanged. No product source or fixed readiness sleep was added.

Four existing Chromium flows pass at localhost:5201 (21.1 s). The two pan
flows also pass with a temporary routed font stylesheet and a real system
font delayed 600 ms (3.6 s), then with the stylesheet request aborted (2.3 s).
Both probes were immediately restored; no platform font or interception is
committed. The hidden Vite server's listener PID/command line was verified and
only that owned PID (38372) was stopped. The three changed E2E files pass an
explicit strict TypeScript check; diff checks pass.

Full E2E CI remains the merge gate after rebase onto preceding checked merges.
Unit/native/product builds are unchanged and not repeated locally for this
test-only slice. No browser UI redesign or native campaign acceptance claim.

## Rebase verification (2026-10-05)

Rebased onto merged #157 (`e8627065`), preserving both task-board insertions.
Four existing Chromium flows pass again in 16.8 s and the strict E2E typecheck
passes. Verified hidden Vite PID 35132 was stopped and port 5201 is clear.
Publication/rebase onto the subsequent checked mock merge remains pending.

## Final publish verification (2026-10-05)

Rebased onto #158's checked merge `2c2362bb`. Replayed only this task's three
board entries onto latest main, preserving #157/#158 completion evidence and
avoiding obsolete duplicate native status. The four Chromium flows pass in
9.4 s; strict E2E typecheck passes. Verified Vite PID 35836 was stopped and
port 5201 is clear. The product code and original assertions remain unchanged;
six final-head CI jobs still gate merge.
