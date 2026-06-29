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

  // run the sweep -> best cell + apply-best button appear
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-cell')).toBeVisible();
  await expect(page.getByTestId('apply-best')).toBeVisible();

  // applying the best combo surfaces the confirmation message
  await page.getByTestId('apply-best').click();
  await expect(page.getByText('已套用最佳參數')).toBeVisible();
});
