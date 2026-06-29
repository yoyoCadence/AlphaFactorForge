import { test, expect } from '@playwright/test';

// Slice 5a Holdout stale-UI regression — the bug that previously needed manual
// catching: after disabling Holdout, the metrics table could keep showing the
// three-column (full / in-sample / out-of-sample) view.
//
// Runs against the Vite dev app with the in-memory mock data client (?mock=1).
// Does NOT exercise real Tauri/Rust/SQLite — that stays in cargo tests +
// `cargo tauri dev` smoke.
test('holdout columns appear on run and clear when disabled', async ({ page }) => {
  await page.goto('/?mock=1');

  // seed a dataset through the mock client
  await page.getByTestId('load-sample').click();

  // single-column view before holdout is run
  await expect(page.getByTestId('col-樣本外')).toHaveCount(0);

  // enable Holdout + run -> 全期 / 樣本內 / 樣本外 columns
  await page.getByTestId('holdout-toggle').check();
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('col-全期')).toBeVisible();
  await expect(page.getByTestId('col-樣本內')).toBeVisible();
  await expect(page.getByTestId('col-樣本外')).toBeVisible();

  // disable Holdout -> table returns to the single full-period column
  await page.getByTestId('holdout-toggle').uncheck();
  await expect(page.getByTestId('col-樣本外')).toHaveCount(0);
  await expect(page.getByTestId('col-全期')).toHaveCount(0);
});
