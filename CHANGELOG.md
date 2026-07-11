# Changelog

## Unreleased

- Added Slice 8b-1 native chart pop-out: open or focus a resizable Tauri OS window and keep its chart/replay state synchronized through typed targeted events.
- Added Slice 10-2 chart drag-pan: drag zoomed charts with pointer capture, preserve click/hover behavior through a movement threshold, and clamp historical views to dataset/replay boundaries.
- Added Slice 10-1 chart wheel zoom: zoom around the candle under the cursor without scrolling the page, display the visible-bar count, reset to fit, and preserve replay no-future-data bounds.
- Added Slice 7-3 strategy library: list SQLite-saved strategies, safely load params/blocks/code definitions back into the editor, and refresh the library after saving.
- Added an optional `E2E_PORT` override with strict port binding so Playwright does not reuse an unrelated local dev server.
- Added Slice 7-2 report export UI: Backtest results now expose JSON report and CSV trades export buttons, with Tauri filesystem writes and dev/mock browser downloads.
- Improved button interaction feedback and export download status messaging.
