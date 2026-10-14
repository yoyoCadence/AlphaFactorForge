import { test, expect, type Page } from '@playwright/test';

async function importBars(page: Page, length: number, interval: '1d' | '1h') {
  const step = interval === '1d' ? 86_400_000 : 3_600_000;
  const candles = Array.from({ length }, (_, i) => {
    const c = 100 + i * 0.05 + Math.sin(i / 7) * 2;
    return { t: Date.UTC(2024, 0, 1) + i * step, o: c, h: c + 1, l: c - 1, c, v: 100 };
  });
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });
  await page.locator('textarea').first().fill(JSON.stringify({ symbol: 'ANNUALIZATION', interval, candles }));
  await page.getByRole('button', { name: '匯入 JSON', exact: true }).click();
  await expect(page.getByTestId('run-backtest')).toBeEnabled();
}

// PR #172 acceptance R1: the full range is long; its displayed holdout is not.
test('a short holdout warns in the main and expanded metrics views', async ({ page }) => {
  await importBars(page, 600, '1d');
  await page.getByTestId('holdout-toggle').check();
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('col-樣本外')).toBeVisible();
  const note = page.getByTestId('annualization-note');
  await expect(note).toContainText('樣本外約 180 天');
  await expect(note).not.toContainText('全期約');
  await expect(note).not.toContainText('樣本內約');
  const text = await note.textContent();
  await page.getByTestId('popout-metrics').click();
  await expect(page.getByTestId('metrics-popout-annualization-note')).toHaveText(text!);
});

// PR #172 acceptance R2: an inclusive tested-bar count is the engine's year.
for (const [length, interval] of [[365, '1d'], [8760, '1h']] as const) {
  test(`exactly ${length} ${interval} bars do not warn below one year`, async ({ page }) => {
    await importBars(page, length, interval);
    await page.getByTestId('run-backtest').click();
    await expect(page.getByTestId('save-result')).toBeVisible();
    await expect(page.getByTestId('annualization-note')).toHaveCount(0);
    await page.getByTestId('popout-metrics').click();
    await expect(page.getByTestId('metrics-popout-annualization-note')).toHaveCount(0);
  });
}

test('a saved full-year summary keeps its recorded dataset bar-count basis', async ({ page }) => {
  await importBars(page, 365, '1d');
  await page.getByTestId('run-backtest').click();
  await page.getByTestId('save-result').click();
  await page.getByTestId('results-explorer-toggle').click();
  await page.getByTestId('results-explorer-view-summaries').click();
  await page.locator('tr[data-testid^="results-explorer-summary-"]').first().click();
  await expect(page.getByTestId('results-explorer-annualization-note')).toHaveCount(0);
});
