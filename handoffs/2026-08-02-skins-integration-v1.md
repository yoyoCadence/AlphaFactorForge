# Handoff: 介面皮膚（Skins）整合 — SKIN-001

Date: 2026-08-02
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/skins-pr-a-theme-base` → `feat/skins-pr-b-panelstyles` → `feat/skins-pr-c-chartpaint` → `feat/skins-chart-tokens-volcolor` → `feat/skins-pr-d-picker`
PR: #80, #81, #82, #83, #84
Status: resolved — all five merged; open items below are decisions, not blockers

## Summary

Ported the externally designed ten-skin system into the app. Every colour, font
stack, radius, border width, and density value the UI used to hard-code now
comes from one `Theme` object; ten themes implement it; a header picker switches
between them. Source of truth for the design is the `AlphaFactorForge 皮膚系統.zip`
drop (`handoff/AGENT_TASK.md` = 施工單, `handoff/README.md` = 設計規格 + token 對照).

Shipped in the施工單's prescribed order, one PR at a time, each passing the full
§5 command set before review.

## What landed

| PR | Scope |
|---|---|
| #80 PR-A | `src/theme/{theme,themeCss,ThemeProvider}.ts(x)`; `ThemeProvider` wraps App + both pop-out trees; `styles.css` replaced by the token-reading version |
| #81 PR-B | `panelStyles` → `makeStyles(theme)`; colour literals removed from `src/components/**` and the `main.tsx` app shell; `HelpTip` popover logic fixed |
| #82 PR-C | `src/charts/chartPaint.ts`; `CandleChart` paints from `theme.chart`; sweep heatmap on the token ramp |
| #83 | `ema` / `bb` chart tokens for all ten skins + a volume-colour toggle (both requested at #82 review) |
| #84 PR-D | `SkinPicker` mounted in the header; `e2e/skins.spec.ts`; `themeCss` key-set unit test |

Tests went 409 → 460 vitest and 29 → 42 Playwright.

## Decisions that future work depends on

**1. PR-A was not visually equivalent, and that was accepted knowingly.**
The施工單 required the default `forge-paper` skin to look identical to the
pre-merge app. It does not, for reasons that were measured rather than guessed
(Playwright computed-style/geometry probes, main vs branch, same warm dev
server — full numbers in PR #80):

- The repo named `'IBM Plex Sans'` / `'IBM Plex Mono'` in CSS but shipped no
  `@font-face` and no CDN link, so it had always rendered in system fallback.
  `ThemeProvider`'s font link makes them actually load, changing the whole UI's
  typeface and raising control heights 1–3px.
- The handoff stylesheet adds `body { margin: 0 }` (removes an 8px page gutter),
  `* { box-sizing: border-box }` (every `width:100%` field loses 16px and stops
  overflowing its grid cell), and a token-derived focus ring (`#2f6df0` blue →
  `color.accent`, which is near-black in `forge-paper`).
- `:root { color-scheme: light dark }` has no `light` counterpart for light
  skins, so on a dark-mode OS the native form controls under `forge-paper` may
  render dark. **Not reproduced here (this machine is light-mode) — left as the
  handoff wrote it, per the "don't invent" rule. Worth checking on a dark-mode
  machine.**

Owner accepted all of the above on 2026-08-02, and separately confirmed keeping
the spec's focus-ring mapping and the `color-scheme` line as-is.

**2. Colours the spec's §4 table does not map were left alone, on instruction.**
Still literals, each with a comment saying why: `#eef4ff` (sweep-applied field
fill), `#3c3a30` used as body text (§4 maps it only to `line2`, a border token
that is far too light for text), `#cfccc4`, `#f4f2ec`. Because of these,
`grep -nE "#[0-9a-fA-F]{6}" src/components/` does **not** come back clean — the
施工單's PR-B acceptance line expected it to. Reporting them rather than
inventing token values was the explicit instruction. Assigning them tokens is a
reasonable follow-up.

**3. `chartPaint.paintGrid` gained one parameter beyond the handoff file.**
An optional `format` callback (default unchanged) so the price axis can keep two
decimals. The handoff's hard-coded `toFixed(0)` would render every gridline as
the same number for any instrument priced under ~10. Everything else in
`chartPaint.ts` is verbatim.

**4. Three UI strings were corrected because the token swap made them false.**
「顏色越綠越佳」 (the heatmap ramp is no longer red→yellow→green) and two
「藍框」 references (the accent is near-black in the default skin). Changed only
far enough to be true; the wording is open to a copy pass.

**5. The volume strip lost its up/down colouring, then got a switch.**
`chart.vol` is a single neutral per skin, so PR-C's volume strip no longer
encoded direction. A `量色` button (`data-testid="vol-color-toggle"`) toggles
back to the classic tint. The flag lives in `OverlayToggles` rather than a new
prop, because `ChartWindowSnapshot.show` *is* `OverlayToggles` — it therefore
syncs to the native chart window with no change to the window-bridge contract
(which carries no version field; changing it would ripple into e2e).

**6. `ema` / `bb` colours were chosen by the agent, on instruction, and are
test-locked.** `src/theme/theme.test.ts` requires the three overlay lines to
stay ≥60 RGB apart from each other, the EMA to stay ≥60 from the candle
up/down colours, and the Bollinger envelope ≥40 from the grid. It caught two of
the first picks (atelier-warm 40.8, frost-grey 49.9 against `ma1` — both are
deliberately low-contrast skins), which were deepened. **An eleventh skin must
satisfy this suite.**

## Known open items (carried verbatim from the施工單 §6)

- `forge-paper` 的 `muted` #8a8678 與 `frost-grey` 的 #7b8794 對比為 3.65:1 /
  3.66:1，未達 AA；前者沿用既有 `panelStyles.ts` 的值。要補到 AA 就改成
  #6f6b5e / #61707e（只影響 10px 欄位標籤），是否更動由 owner 決定。
- 字型走 Google Fonts CDN；桌面版離線需求出現時改為打包 woff2 + `@font-face`，
  token 不用動。

## Additional open items found during the port

- **`TEST-E2E-LAYOUT-001`** (already in `tasks.md` Backlog). The CDN font swap
  reflows the page after first paint — measured ~4px down on CI's Ubuntu
  fallback, ~6px up on Windows. It broke `zoom.spec.ts`, which had been proving
  "the wheel did not scroll the page" by asserting the canvas had not moved;
  that now reads the scroll offsets directly instead (verified it still fails
  when the wheel really does scroll: `main.scrollTop` 0 → 294). `pan.spec.ts:11`
  and the replay case in `zoom.spec.ts` still take a `boundingBox()` right after
  load to aim the mouse — latent, not failing.
- The pop-out window shells keep a hard-coded 46px header while the main window
  now reads `header.height` (46–68px per skin). Not changed because
  `ChartPopoutWindow` derives its canvas height from `innerHeight - 54`, which
  assumes that constant — logic, not style, so out of this task's scope.
- `AlphaFactorForge Skins.dc.html`, the browsable design mock the two handoff
  documents reference, was missing from the first zip; the designer supplied it
  (with `support.js`) on 2026-08-04 and the implementation was verified against
  it — see "Verification against the design mock" below.

## Verification against the design mock (2026-08-04)

The mock arrived after PR-D was opened. Two checks were run against it.

**Token values: exact match.** The mock's `SKINS` array was extracted and
diffed against `theme.ts` + `themeToCssVars()` — 10 skins × (CSS vars + chart +
tab/chip state pairs) produced **zero value mismatches**. The only key-level
differences are representational (the mock also carries `ma1`/`ma2` in its flat
CSS-var bag; the app keeps them in `theme.chart`, same values) plus the
`ema`/`bb` chart tokens the app added, which the mock genuinely does not define
— its chart draws `ma1`/`ma2` only, so the app needed colours for overlays the
mock never had.

**Canvas painting: five gaps, all closed in PR-D.** Reading the mock's own
draw code turned up differences no token diff could catch:

1. The last-price tag (dashed accent rule at the newest close plus a filled tag
   in the right gutter) was missing entirely — even though `chart.accent`'s own
   doc comment names it. Added, reading the newest *visible* close so the replay
   cursor still bounds it and no future bar leaks.
2. Volume opacity 0.85 (was 1.0) for the neutral fill.
3. `ma2` dashed `[5,3]` in the OHLC-`bar` skin.
4. RSI 30/70 guides always dashed `[2,3]`, unlike the price grid, which follows
   the skin's own `dash`.
5. Axis type unified on `9px 'IBM Plex Mono', monospace` (the handoff's
   `paintGrid` had a no-op ternary that always yielded plain `monospace`).

**The two handoff artifacts disagreed on heat-cell ink, and the mock was
right.** `chartPaint.heatTextColor` thresholded the *ramp position*
(`t > 0.55`); the mock measures the luminance of the fill actually produced.
Position is a poor proxy because `accent` is near-black in some skins and a
bright green or cyan in others. Now measured — and measured better than the
mock: rather than one luminance cut-off, both candidate inks are scored by
contrast ratio and the stronger wins, because a single cut-off picks the wrong
side for mid-tone fills.

**Residual, needs an owner call.** The mock fixes the two inks at `#111111` /
`#f2f2f2`. For a mid-tone fill sitting between them the best either can achieve
is ~4.09:1 — a property of that pair, not a tuning miss. Measured worst case
across all ten skins at 1% ramp steps is **4.112 (broadsheet, t=0.77)**, just
under WCAG AA. Swapping the pair for pure `#000000` / `#ffffff` raises the
floor to **4.587** and clears AA, at an imperceptible cost on a coloured fill.
Left at the design's values; `src/charts/chartPaint.test.ts` asserts ≥4.0 and
documents both numbers.

**Design decisions deliberately NOT adopted** (outside this task, which the
施工單 scoped to the style layer with "behaviour diff 應為零"):

- The mock's screen layout differs — chart and strategy editor side by side on
  top, then dataset / metrics / sweep in a row. The app keeps its current
  arrangement.
- The mock renders the overlay toggles (MA / EMA / BB / RSI / VOL) as chips,
  which is what `panelStyles`' still-unused `chip()` helper is for; the app uses
  checkboxes.
- The mock shows screens and controls the app does not have (symbol + interval
  bar with live price, 策略庫 / 探索執行 tabs, exchange-fallback select,
  engine-parity footer). Those belong with the PR-E screens.

## Not in scope (per the施工單)

PR-E — the Strategy Library and Discovery Runner screens are token-ised in the
design but those screens do not exist yet. When they are built, use
`makeStyles(theme)` with the existing `card` / `tableRow` / `tableHead` /
`chip()`, and do not define new colours. Status badges map to
`color.ok` / `color.warn` / `color.danger`.

## Verification

Per PR, the施工單 §5 set: `npm test`, `npm run typecheck`, `npm run build`,
`npx playwright test`, `cd src-tauri && cargo check`. All green on every PR
locally and in CI (typecheck / test / build / cargo-check / e2e).

Final state: 460 vitest, 42 Playwright e2e, `cargo check` clean. `data-testid`
inventories were diffed mechanically at each step and stayed identical (49
occurrences); `skin-picker` and `vol-color-toggle` are the only additions.
