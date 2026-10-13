import { test, expect } from '@playwright/test';

// METRIC-DISPLAY-001 (2026-10-13 project review) — display only; no metric
// changes. Runs against the Vite dev app with the mock data client.

test('a run shorter than a year says its annualized figures are an extrapolation', async ({ page }) => {
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('load-sample').click();
  await expect(page.getByTestId('annualization-note')).toHaveCount(0);

  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('save-result')).toBeVisible();

  // The seeded sample is 600 hourly bars: first to last bar is just under 25 days.
  const note = page.getByTestId('annualization-note');
  await expect(note).toBeVisible();
  await expect(note).toContainText('約 25 天');
  await expect(note).toContainText('CAGR');
});

test('the explorer labels times as UTC and shows Gate values in their own units', async ({ page }) => {
  await page.goto('/?mock=1&seedHistory=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('results-explorer-toggle').click();

  const rows = page.locator('tr[data-testid^="results-explorer-record-"]');
  await expect(rows).toHaveCount(2);
  await expect(page.getByTestId('results-explorer-loaded-at')).toContainText('UTC');
  await expect(page.getByTestId('results-explorer-records')).toContainText('建立時間（UTC）');
  // created_at as YYYY-MM-DD HH:MM, not the stored text with seconds and zone
  await expect(rows.nth(0).locator('td').nth(7)).toHaveText(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);

  await rows.nth(0).click();
  const cellsOf = (criterion: string) => page.getByTestId(`results-explorer-gate-${criterion}`).locator('td');

  // observed value is the third cell, threshold the fourth
  for (const count of ['minTrades', 'benchmarkWins']) {
    await expect(cellsOf(count).nth(2)).toHaveText(/^\d+$/);
    await expect(cellsOf(count).nth(3)).toHaveText(/^\d+$/);
  }
  for (const fraction of ['avgTradeReturn', 'rollingConsistency', 'maxDrawdown', 'monthlyConcentration', 'tradeConcentration']) {
    await expect(cellsOf(fraction).nth(2)).toHaveText(/^-?\d+\.\d{2}%$/);
    await expect(cellsOf(fraction).nth(3)).toHaveText(/^-?\d+\.\d{2}%$/);
  }
  await expect(cellsOf('randomEntryPercentile').nth(2)).toHaveText(/^\d+\.\d$/);
  await expect(cellsOf('randomEntryPercentile').nth(3)).toHaveText(/^\d+\.\d$/);

  // the seeded Train / Validation summaries cover well under a year
  await expect(page.getByTestId('results-explorer-annualization-note')).toContainText('不足一年');
});
