import { test, expect } from '@playwright/test';

// Browser/mock coverage for the save success banner. Real SQLite persistence
// remains owned by Rust tests and a native Tauri smoke check.
test('saving a backtest reports the persisted strategy and dataset', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  const save = page.getByTestId('save-result');
  await expect(save).toHaveCount(0);

  await page.getByTestId('load-sample').click();
  await page.getByTestId('run-backtest').click();
  await expect(save).toBeEnabled();
  await expect(save).toHaveText('儲存結果');

  await save.click();

  await expect(page.getByText(/^已存檔：strategy #\d+（type=params）· dataset #\d+ · \d+ trades$/)).toBeVisible();
  await expect(save).toBeEnabled();
  await expect(save).toHaveText('儲存結果');
});
