import { test, expect } from '@playwright/test';

// BUG-RESULT-CONTEXT-001 — a completed backtest may only be rendered, saved, or
// exported together with the exact inputs that produced it. These are the audit's
// shortest reproductions, which previously all ended in a stale result being
// persisted/exported under NEW inputs:
//   A. run -> edit the strategy -> save/export without re-running
//   B. run on dataset A -> switch to B (still loading / failed) -> save/run
//   C. run -> apply a sweep combo -> save/export without re-running
//
// Runs against the Vite dev app with the in-memory mock client (?mock=1). Real
// SQLite persistence stays with the Rust tests and the native Tauri smoke check.

/** A second dataset that is unmistakably different from the 600-bar sample. */
const IMPORTED_DATASET = JSON.stringify({
  symbol: 'IMPB',
  interval: '1h',
  candles: Array.from({ length: 30 }, (_, i) => ({
    t: 1_700_000_000_000 + i * 3_600_000,
    o: 100 + i,
    h: 102 + i,
    l: 99 + i,
    c: 101 + i,
    v: 10 + i,
  })),
});

test('editing the strategy invalidates save/export until the backtest is re-run', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  await page.getByTestId('load-sample').click();
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
  await expect(page.getByTestId('export-json')).toBeVisible();
  await expect(page.getByTestId('result-stale')).toHaveCount(0);

  // Repro A: an execution-model edit changes what a run would produce, so the
  // finished run stops being a valid representation of the current inputs.
  await page.getByLabel('手續費 %').fill('0.2');

  await expect(page.getByTestId('save-result')).toHaveCount(0);
  await expect(page.getByTestId('export-json')).toHaveCount(0);
  await expect(page.getByTestId('export-csv')).toHaveCount(0);
  await expect(page.getByTestId('popout-metrics')).toHaveCount(0);
  await expect(page.getByTestId('result-stale')).toBeVisible();

  // Re-running against the new inputs brings every result action back.
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
  await expect(page.getByTestId('export-json')).toBeVisible();
  await expect(page.getByTestId('result-stale')).toHaveCount(0);

  // The same rule covers the Holdout split, which is part of the run range.
  await page.getByTestId('holdout-toggle').check();
  await expect(page.getByTestId('save-result')).toHaveCount(0);
  await expect(page.getByTestId('result-stale')).toBeVisible();
});

test('applying a sweep combo invalidates the previous run', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  await page.getByTestId('load-sample').click();
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('save-result')).toBeEnabled();

  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();

  // Repro C. A specific cell (fastMA=7, never the default 9) keeps this
  // deterministic; 套用最佳 reaches the panel through the same applySweepCombo
  // -> onApplyCombo path, so it is covered by the same guard.
  await page.getByTestId('sweep-cell-7').click();
  await expect(page.getByTestId('applied-fastMA')).toBeVisible();

  await expect(page.getByTestId('save-result')).toHaveCount(0);
  await expect(page.getByTestId('export-json')).toHaveCount(0);
  await expect(page.getByTestId('result-stale')).toBeVisible();

  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
});

test('a dataset switch drops the old candles/result and a late load cannot repopulate them', async ({ page }) => {
  test.setTimeout(90_000);
  // The DEV-only mock latency knob keeps a candle load open long enough for the
  // race to be deterministic; production reads have no such switch.
  await page.goto('/?mock=1&candleDelay=1000', { waitUntil: 'domcontentloaded' });

  const run = page.getByTestId('run-backtest');
  const datasetSelect = page.locator('select').filter({ hasText: 'SAMPLE · 1h · 600 根' });

  await page.getByTestId('load-sample').click();
  // Run is disabled while the selected dataset is still loading.
  await expect(run).toBeDisabled();
  await expect(run).toBeEnabled({ timeout: 20_000 });

  await run.click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
  await expect(page.getByText('SAMPLE · 600 根')).toBeVisible();

  // Repro B: import + select a different dataset. Its candles are not loaded
  // yet, so nothing about the previous run may remain reachable.
  await page.getByPlaceholder('貼上 K 線 JSON').fill(IMPORTED_DATASET);
  await page.getByRole('button', { name: '匯入 JSON' }).click();
  await expect(page.getByText(/已匯入 30 根 K 線（IMPB · 1h）/)).toBeVisible();
  await expect(run).toBeDisabled();
  await expect(page.getByTestId('save-result')).toHaveCount(0);
  await expect(page.getByTestId('export-json')).toHaveCount(0);
  await expect(page.getByText('SAMPLE · 600 根')).toHaveCount(0);

  // Switch back while the second dataset's load is still in flight; that
  // response is now orphaned. Readiness is keyed by dataset identity, so the
  // first dataset's candles are usable again immediately.
  await datasetSelect.selectOption({ label: 'SAMPLE · 1h · 600 根' });
  await expect(page.getByText('SAMPLE · 600 根')).toBeVisible();
  await expect(run).toBeEnabled();

  // Deliberate wait past the orphaned load's scheduled completion: a late
  // response must be dropped instead of painted into the current selection.
  await page.waitForTimeout(1800);
  await expect(page.getByText('SAMPLE · 600 根')).toBeVisible();
  await expect(page.getByText('IMPB · 30 根')).toHaveCount(0);
  // The completed run was dropped by the switch and is not resurrected.
  await expect(page.getByTestId('save-result')).toHaveCount(0);
  await expect(page.getByTestId('result-stale')).toHaveCount(0);
});
