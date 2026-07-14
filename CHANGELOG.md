# Changelog

## Unreleased

- Restored legacy `direction: both` reversal semantics so entry requests long and exit requests short in both close and `nextOpen` modes, and made the pure backtest core reject non-finite or out-of-range normalized sizing/cost/risk fractions instead of silently clamping them. This intentionally changes all affected `both` trades and metrics while leaving UI/service percentage conversion in `backtestRunner`.
- Corrected backtest fill semantics so `nextOpen` orders execute on the following candle without leaking that open into the signal-bar equity, SL/TP exits use gap-aware prices plus closing-side slippage with conservative SL-first ambiguity handling, and EOD settlement applies normal exit slippage. This intentionally changes affected trade timestamps, risk-exit prices, and derived golden metrics.
- Corrected backtest accounting so long/short trades report fee-inclusive PnL, 100% sizing budgets entry fees without negative free cash, 1× short collateral reconciles on wins and losses, and EOD metrics use settled final equity from the configured starting balance. This intentionally updates golden metric values without changing trade count, fill time, or fill price.
- Added backtest-engine golden behaviour tests and a legacy parity report covering execution timing, risk fills, short accounting, end-of-data settlement, and `both` direction semantics without changing product behaviour.
- Fixed parameter sweep leaking out-of-sample data when Holdout is on: the sweep now optimises on the in-sample segment only (same split as the backtest), and the sweep panel states its in-sample scope so the heatmap is not misread as full-period.
- Fixed loading legacy params and blocks strategies saved before rule-builder or manual-code fields were introduced, while retaining strict validation for active and malformed fields.
- Added Slice 8b-1 native chart pop-out: open or focus a resizable Tauri OS window and keep its chart/replay state synchronized through typed targeted events with least-privilege Tauri event permissions.
- Added Slice 10-2 chart drag-pan: drag zoomed charts with pointer capture, preserve click/hover behavior through a movement threshold, and clamp historical views to dataset/replay boundaries.
- Added Slice 10-1 chart wheel zoom: zoom around the candle under the cursor without scrolling the page, display the visible-bar count, reset to fit, and preserve replay no-future-data bounds.
- Added Slice 7-3 strategy library: list SQLite-saved strategies, safely load params/blocks/code definitions back into the editor, and refresh the library after saving.
- Added an optional `E2E_PORT` override with strict port binding so Playwright does not reuse an unrelated local dev server.
- Added Slice 7-2 report export UI: Backtest results now expose JSON report and CSV trades export buttons, with Tauri filesystem writes and dev/mock browser downloads.
- Improved button interaction feedback and export download status messaging.
