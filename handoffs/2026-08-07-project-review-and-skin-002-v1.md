# Handoff: Deep Project Review And SKIN-002

Date: 2026-08-07
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/theme-backgrounds-quality`
PR: not opened
Status: SKIN-002 implemented; review findings recorded for ordered follow-up

## Summary

This review covered the pure TypeScript backtest/validation core, the Rust/Tauri
runner and SQLite persistence, and the React theme/UI layer in parallel. The
current branch implements the bounded SKIN-002 slice and records the remaining
findings as focused tasks rather than mixing correctness-contract changes into
a visual PR.

A separate correctness fix was completed first on branch
`fix/monthly-return-baseline` as commit `ae4fefc` (`METRIC-002`). It is not
included in this branch so the metric-contract/fixture change and the theme
change remain independently reviewable.

## SKIN-002 Resolution

- Added `Theme.workspaceBackground`, keeping `color.bg` opaque and testable.
- Added three generated local WebP assets, each 1440x900:
  - `paper-fiber-v1.webp` — 40,962 bytes; SHA-256
    `EB2306C49545823AFA4A4E8F0E88DBE2E37A5DEE28C7088CCD02F224DE48AB6A`
  - `aurora-glass-v1.webp` — 12,530 bytes; SHA-256
    `D0126CB347712B98D0D87ED0ECE471DF0DA6817AE0518018F0639311E459E6C6`
  - `signal-orange-v1.webp` — 13,580 bytes; SHA-256
    `F84B1291EC2D0BE7BB378289E46FFBA8DFAEA17C07C7A3ED4EC7E6D48D790E54`
- Paper is shared, with different scrims, by forge-paper, atelier-warm, and
  broadsheet. Aurora and signal-orange use their dedicated generated assets.
  Midnight-tape and blueprint use lightweight CSS scanline/grid patterns.
  Swiss-forge, brutal-yellow, and frost-grey intentionally remain flat.
- Fixed invalid persisted-skin fallback/removal and live sibling-window skin
  updates through the standard `storage` event.
- Restored the global `:focus-visible` ring by removing the inline
  `outline:none`; fixed duplicate SkinPicker keys.
- Added a responsive single-column workspace below 860px, `minmax(0,1fr)`
  containment above it, light/dark native `color-scheme`, reduced-motion button
  handling, theme-sized pop-out headers, and a usable 260px floating-chart
  minimum height.
- PR review follow-up gave the mobile skin selector an explicit accessible
  name, made CSS variables the sole workspace-background renderer, required
  every registered skin to belong to exactly one background class, and changed
  Aurora cards to dark translucent glass. A unit regression pins the generated
  WebP SHA and composites its measured brightest pixel through the weakest
  scrim stop and card alpha before requiring AA contrast for ink and muted text.
  The second follow-up added a dedicated 520px accessible-name E2E and changed
  text-bearing `surface2` to deep violet glass, with assertions for both its
  nested-card and free-standing workspace composites. The final follow-up
  raises those assertions to AA + 0.5 and removes the unused `color.surface`
  token and its equally unused `--afs-surface` CSS variable.

## Image Generation Record

Mode: built-in image generation (`image2`/gpt-image-2 path), followed only by a
browser-canvas resize/quality conversion to WebP. No external image search or
remote runtime asset is used.

Final prompts:

1. Paper fibre: “Create a seamless-feeling, low-contrast warm ivory handmade
   paper texture for a professional quantitative-trading desktop workstation.
   Fine natural fibres, faint deckle variation and extremely subtle graphite
   specks; evenly lit, no folds, objects, borders, text, logos, charts or focal
   point. Calm neutral material background with generous visual quiet so dense
   white analysis cards remain readable. Landscape 16:10.”
2. Aurora glass: “Create a premium abstract desktop-app background for a dark
   quantitative trading workstation: near-black indigo centre with soft cyan,
   violet and restrained teal aurora ribbons flowing mainly around the outer
   edges. Low-frequency shapes, deep negative space behind the central data
   area, subtle atmospheric depth, no stars, text, logos, charts, UI or hard
   focal point. Dark, calm, glass-compatible, landscape 16:10.”
3. Signal orange: “Create a restrained industrial desktop-app background for
   a professional trading workstation: dark graphite machine panels with
   sparse signal-orange edge lighting, very subtle seams and diagonal technical
   structure concentrated near the perimeter. Keep the central workspace
   nearly black and visually quiet; no text, logos, charts, controls, hazard
   stripes or bright focal object. Matte, precise, low contrast, landscape
   16:10.”

Original generated PNGs remain under the Codex generated-images directory; the
workspace contains only the optimized final assets.

## Review Findings

### P1 — correctness and audit integrity

1. `PERSIST-AUDIT-001`, `RUNNER-OWNERSHIP-001`, and `DATA-QUALITY-001` remain
   required gates already recorded in `tasks.md`. In particular,
   `validation-record-v1` cannot identify the metric formula, the live runner
   is not protected from a second desktop process, and imported OHLCV lacks the
   full semantic admission checks.
2. `validationRun.ts` constructs signals from the complete candle array before
   applying the Train/Validation ranges. The backtest range prevents Test
   execution, but the stronger project rule says hidden Test values must not be
   read or precomputed. Slice the signal input at the Validation boundary and
   add an access-tracing regression (`TEST-HIDDEN-TEST-001`).
3. A signal derived from candle close can currently fill on that same close.
   This may be an accepted execution convention, but it is optimistic unless
   the information set/order timing is explicit. Treat it as a versioned
   product decision, not a local bug fix (`EXECUTION-TIMING-DECISION-001`).
4. Random Entry can clip a requested matched trade/holding plan near segment
   boundaries and shift a forced exit by one bar; an existing case effectively
   locks “3 requested -> 2 executed.” Version the planner/evidence before
   changing Gate inputs (`BENCH-RANDOM-CONTRACT-001`).
5. CAGR/Sharpe/Sortino annualization is driven by bar count/interval rather than
   elapsed timestamps. Sparse data can therefore report implausible annualized
   values. Define and version a missing-candle policy
   (`METRIC-ANNUALIZATION-001`).
6. A one-bar Holdout can resolve to an empty out-of-sample run. Fold this
   boundary into the split/holdout contract work rather than allowing a silent
   “validation” with no evaluated bar.

### P2 — persistence and runner robustness

1. `backtest_summary` is replaced by strategy/dataset/segment key while old
   `discovery_jobs.result_id` values can continue pointing at that row. This
   rewrites the meaning of historical jobs (`PERSIST-RESULT-HISTORY-001`).
2. Run admission and in-memory control registration are not one fail-safe
   boundary. A coordinator panic/startup failure can leave a DB run marked
   `running` without a usable control until recovery (`RUNNER-FAILSAFE-001`).
3. `PERSIST-INVARIANT-001` is still needed for summary/trade bundle invariants:
   finite values, direction, time/range ordering, positive prices, and
   `trade_count == trades.len()`.
4. Direct report serialization can silently turn nested non-finite numbers into
   `null` if a caller bypasses the shared codec. Keep the codec as the only
   boundary and add the metric formula identity to the next report/record
   schema.
5. SQLite still needs a documented `busy_timeout`, deterministic read ordering,
   propagated migration lookup errors (already `DB-MIGRATION-DIAGNOSTIC-001`),
   and explicit source/hash provenance alignment.

### UI, performance, and accessibility

1. Native Tauri CSP permits self-hosted resources while `ThemeProvider` injects
   Google Fonts. Native/offline builds therefore fall back, and browser cold
   loads can reflow controls. Bundle a reviewed local subset without widening
   CSP (`FONT-OFFLINE-001`).
2. Canvas indicators/trade maps are recomputed more often than pointer painting
   requires; the existing `PERF-CHART-COMPUTE-001` remains the right task.
3. Table semantics, icon-only button names/targets, canvas alternatives, and
   screen-reader coverage need a focused pass (`A11Y-SEMANTICS-001`). SKIN-002
   already adds reduced-motion handling and restores keyboard focus visibility.
4. The browser requests a missing `/favicon.ico`; it is harmless in Tauri but
   is the sole console error in browser visual QA and can be closed in a tiny
   asset/document-head task.

## Verification

- `npm run test -- src/theme/theme.test.ts src/theme/themeCss.test.ts src/theme/contrast.test.ts`
  — 184/184 passed.
- `npm run typecheck` — passed.
- `npm run build` — passed; all three WebPs emitted by Vite at 12.53/13.58/40.96
  kB and no new runtime dependency was added.
- `cargo check` — passed.
- The preceding review head completed `E2E_PORT=4184 npm run e2e --
  e2e/skins.spec.ts` at 20/20. The second follow-up's dedicated 520px
  compact-picker accessible-name guard passed 1/1 on port 4185. A post-change
  local full-suite attempt continued making progress without an assertion
  failure but exceeded the 15-minute outer command limit; the PR's complete
  Playwright job remains the final 50-test gate.
- Playwright CLI visual QA — aurora-glass and signal-orange inspected with the
  complete 600-candle chart; background placement and chart/control contrast
  remained usable.
- Post-review in-app Browser QA confirmed the Aurora root still paints the
  generated background while cards resolve to rgba(11,10,24,0.55); the pinned
  worst-case composite measures approximately 11.06:1 for ink and 5.04:1 for
  muted text. The final review changes `surface2` to rgba(30,26,63,0.78): muted
  text measures approximately 6.10:1 over a card and 5.38:1 directly over the
  brightest workspace composite, clearing the enforced 5.0 floor.
- The first full `npm test` rerun hit the Windows sandbox's `spawn EPERM`.
  Escalated local retries subsequently completed, including the PR review
  follow-up at 683/683 Vitest tests. GitHub Actions run
  https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/31252107776 also
  completed build, e2e, typecheck, cargo-check, and test successfully for the
  original PR head; the review commit receives the same five required checks.
- After #88 merged and this branch rebased onto `c60cea6`, final local checks
  completed at 690/690 Vitest tests plus typecheck, production build, and
  `git diff --check`; the rebased PR CI remains the final Rust/50-E2E gate.

## Risks / Next Recommended Step

The generated backgrounds are deliberately subtle and only paint the workspace
layer; chart canvases and ordinary cards keep their established surfaces. The
remaining material risk is not visual — it is the ordered P1 correctness/audit
queue. Merge `METRIC-002` independently, then execute its required successor
`PERSIST-AUDIT-001` before runner UI work, followed by ownership/data-quality
gates and the new hidden-Test isolation task.
