import { test, expect } from '@playwright/test';

// Browser/mock regression for the three strategy editors. Each editor owns a
// separate slice of the same strategy object, which must survive tab unmounts.
test('params, blocks, and code state survives strategy-mode switches', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  const run = page.getByTestId('run-backtest');
  const paramsTab = page.getByTestId('strategy-mode-params');
  const blocksTab = page.getByTestId('strategy-mode-blocks');
  const codeTab = page.getByTestId('strategy-mode-code');
  const entrySignal = page.getByLabel('進場訊號');
  const blockRightOperands = page.getByPlaceholder('series 或數字');
  const entryCode = page.getByLabel('進場條件 (entry)');

  const expectActiveMode = async (active: 'params' | 'blocks' | 'code'): Promise<void> => {
    await expect(paramsTab).toHaveAttribute('aria-pressed', String(active === 'params'));
    await expect(blocksTab).toHaveAttribute('aria-pressed', String(active === 'blocks'));
    await expect(codeTab).toHaveAttribute('aria-pressed', String(active === 'code'));
  };

  await expect(run).toBeDisabled();
  await page.getByTestId('load-sample').click();
  await expect(run).toBeEnabled();

  await expectActiveMode('params');
  await expect(entrySignal).toBeVisible();
  await expect(blockRightOperands).toHaveCount(0);
  await expect(entryCode).toHaveCount(0);
  await entrySignal.selectOption('rsiOversold');

  await blocksTab.click();
  await expectActiveMode('blocks');
  await expect(entrySignal).toHaveCount(0);
  await expect(blockRightOperands).toHaveCount(2);
  await expect(entryCode).toHaveCount(0);
  await blockRightOperands.first().fill('rsi');
  await expect(run).toBeEnabled();

  await codeTab.click();
  await expectActiveMode('code');
  await expect(entrySignal).toHaveCount(0);
  await expect(blockRightOperands).toHaveCount(0);
  await expect(entryCode).toBeVisible();
  await entryCode.fill('price > maSlow');
  await expect(run).toBeEnabled();

  await paramsTab.click();
  await expectActiveMode('params');
  await expect(entrySignal).toHaveValue('rsiOversold');
  await expect(run).toBeEnabled();

  await blocksTab.click();
  await expectActiveMode('blocks');
  await expect(blockRightOperands.first()).toHaveValue('rsi');
  await expect(run).toBeEnabled();

  await codeTab.click();
  await expectActiveMode('code');
  await expect(entryCode).toHaveValue('price > maSlow');
  await expect(run).toBeEnabled();

  await blocksTab.click();
  await expectActiveMode('blocks');
  await run.click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
});
