import { test, expect } from '@playwright/test';

// Slice 5c — clickable "?" HelpTip markers. UI-only: clicking a marker toggles a
// short explanation popover; clicking it again, pressing Escape, or clicking
// outside closes it. Runs against the Vite dev app with the mock data client
// (?mock=1); no backtest/logic behaviour is involved. The section/action markers
// used here (資料集 / 策略 / 回測績效) render without loading any data.

test('a help marker toggles its explanation popover', async ({ page }) => {
  await page.goto('/?mock=1');

  const marker = page.getByTestId('help-dataset');
  await expect(marker).toBeVisible();

  // closed initially
  await expect(page.getByRole('tooltip')).toHaveCount(0);

  // click -> popover shows with the dataset explanation
  await marker.click();
  const tip = page.getByRole('tooltip');
  await expect(tip).toBeVisible();
  await expect(tip).toContainText('K 線資料集');

  // click again -> toggles closed
  await marker.click();
  await expect(page.getByRole('tooltip')).toHaveCount(0);
});

test('Escape and an outside click close the help popover', async ({ page }) => {
  await page.goto('/?mock=1');

  const marker = page.getByTestId('help-strategy');

  // Escape closes
  await marker.click();
  await expect(page.getByRole('tooltip')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('tooltip')).toHaveCount(0);

  // reopen, then a click elsewhere closes
  await marker.click();
  await expect(page.getByRole('tooltip')).toBeVisible();
  await page.getByRole('heading', { name: '回測績效' }).click();
  await expect(page.getByRole('tooltip')).toHaveCount(0);
});

test('opening a second marker leaves only one popover open', async ({ page }) => {
  await page.goto('/?mock=1');

  await page.getByTestId('help-dataset').click();
  await expect(page.getByRole('tooltip')).toContainText('K 線資料集');

  // opening another marker's popover replaces the first (outside-click closes it)
  await page.getByTestId('help-metrics').click();
  await expect(page.getByRole('tooltip')).toHaveCount(1);
  await expect(page.getByRole('tooltip')).toContainText('最大回撤');
});
