import { test, expect } from '@playwright/test';

// Slice 5b-2 parameter-sweep UI flow: load sample -> expand sweep -> run ->
// heatmap with a highlighted best cell -> apply best (updates the strategy).
//
// Runs against the Vite dev app with the in-memory mock data client (?mock=1).
// Does NOT exercise real Tauri/Rust/SQLite — that stays in cargo tests +
// `cargo tauri dev` smoke. The sweep engine itself is unit-tested in 5b-1.
test('parameter sweep runs, renders a heatmap, and applies the best combo', async ({ page }) => {
  await page.goto('/?mock=1');

  // seed a dataset (600 sample candles) through the mock client
  await page.getByTestId('load-sample').click();

  // the sweep section only appears once candles are loaded
  await page.getByTestId('sweep-toggle').click();

  // default 1-D config = fastMA 5..20 step 1 = 16 combos (pre-flight count)
  await expect(page.getByTestId('sweep-combos')).toContainText('組合數 16');

  // run the sweep -> best cell (★) + apply-best button appear
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await expect(page.getByTestId('apply-best')).toBeVisible();

  // applying the best combo surfaces the confirmation message + applied mark
  // ('已套用：' with the colon = the message card, not the '✓已套用' cell badge)
  await page.getByTestId('apply-best').click();
  await expect(page.getByText(/已套用：/)).toBeVisible();
  await expect(page.getByTestId('sweep-applied-marker')).toBeVisible();
});

// Slice 5b-3: any heatmap cell can be clicked to apply that combo, and the
// applied cell gets a ✓ highlight so the user sees which combo is on the strategy.
test('clicking a heatmap cell applies that combo and highlights it', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();

  // click a specific (fastMA=7) cell -> applied highlight + confirmation
  await page.getByTestId('sweep-cell-7').click();
  await expect(page.getByTestId('sweep-applied-marker')).toBeVisible();
  await expect(page.getByText('已套用：快線MA=7')).toBeVisible();

  // the swept param is also highlighted in the strategy form (and reads the
  // applied value), so it's clear which variable the heatmap changed
  await expect(page.getByTestId('applied-fastMA')).toBeVisible();
  await expect(page.getByTestId('applied-fastMA').getByRole('spinbutton')).toHaveValue('7');
});

// Stale-result regression (PR #15 review): changing any sweep config after a
// completed run must clear the old heatmap / 套用最佳, forcing a rerun — so the
// visible controls can never describe a different sweep than the action acts on.
test('changing sweep config clears the previous result', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('sweep-toggle').click();

  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await expect(page.getByTestId('apply-best')).toBeVisible();

  // change the optimisation metric -> stale heatmap + apply-best must disappear
  await page.getByTestId('sweep-metric').selectOption('sharpe');
  await expect(page.getByTestId('sweep-best-marker')).toHaveCount(0);
  await expect(page.getByTestId('apply-best')).toHaveCount(0);
});
