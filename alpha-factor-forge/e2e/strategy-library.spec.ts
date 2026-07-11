import { test, expect } from '@playwright/test';

// Slice 7-3 browser/mock flow. Real persistence remains covered by the Rust
// repository tests; this verifies the React list/save/load interaction.
test('lists a saved strategy and loads it back into the editor', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('save-result')).toBeEnabled();

  await page.getByTestId('strategy-name').fill('My saved MA');
  await page.getByTestId('save-result').click();

  const library = page.getByTestId('strategy-library-select');
  await expect(library.locator('option')).toContainText(['選擇已存策略', 'My saved MA · params']);
  await expect(library).not.toHaveValue('');

  await page.getByTestId('strategy-mode-code').click();
  await expect(page.getByTestId('strategy-mode-code')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('load-strategy').click();

  await expect(page.getByTestId('strategy-mode-params')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByTestId('strategy-name')).toHaveValue('My saved MA');
  await expect(page.getByText(/已載入策略：My saved MA/)).toBeVisible();
});
