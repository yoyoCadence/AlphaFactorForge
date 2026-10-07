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

test('long-decimal save and load preserves values; edits create an explicit new version', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  const fee = page.getByLabel('手續費 %');
  await fee.fill('0.0036944444444444438');
  await page.getByTestId('strategy-name').fill('Exact original');
  await page.getByTestId('run-backtest').click();
  const save = page.getByTestId('save-result');
  await expect(save).toHaveText('儲存結果');
  await save.click();
  const library = page.getByTestId('strategy-library-select');
  await expect(library.locator('option')).toHaveCount(2);
  const originalId = await library.inputValue();
  await fee.fill('0.2');
  await page.getByTestId('load-strategy').click();
  await expect(fee).toHaveValue('0.0036944444444444438');
  await page.getByTestId('run-backtest').click();
  await expect(save).toHaveText('儲存結果');
  await page.getByTestId('strategy-name').fill('Exact renamed');
  await save.click();
  await expect(library.locator('option')).toHaveCount(2);
  await fee.fill('0.1');
  await page.getByTestId('strategy-name').fill('Exact child');
  await page.getByTestId('run-backtest').click();
  await expect(save).toHaveText('另存新版本');
  await expect(page.getByTestId('save-new-version-note')).toContainText('保留原策略');
  await save.click();
  await expect(library.locator('option')).toHaveCount(3);
  await expect(save).toHaveText('儲存結果');
  await library.selectOption(originalId);
  await page.getByTestId('load-strategy').click();
  await expect(fee).toHaveValue('0.0036944444444444438');
  await expect(page.getByTestId('strategy-name')).toHaveValue('Exact renamed');
});

test('a delayed strategy preparation cannot replace newer editor changes', async ({ page }) => {
  await page.goto('/?mock=1&strategyPrepareDelay=600');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('strategy-name').fill('Saved source');
  await page.getByTestId('run-backtest').click();
  await page.getByTestId('save-result').click();
  await expect(page.getByTestId('strategy-library-select')).not.toHaveValue('');
  await page.getByLabel('手續費 %').fill('0.2');
  await page.getByTestId('load-strategy').click();
  await expect(page.getByTestId('load-strategy')).toBeDisabled();
  await page.getByTestId('strategy-name').fill('Newer edit');
  await expect(page.getByTestId('load-strategy')).toBeEnabled();
  // Observe beyond the delayed response, then assert both edits survived.
  await page.waitForTimeout(700);
  await expect(page.getByTestId('strategy-name')).toHaveValue('Newer edit');
  await expect(page.getByLabel('手續費 %')).toHaveValue('0.2');
  await expect(page.getByText(/已載入策略：Saved source/)).toHaveCount(0);
});
