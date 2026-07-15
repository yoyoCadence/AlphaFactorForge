import { test, expect } from '@playwright/test';

// Browser/mock coverage for the code-mode editor. The restricted interpreter
// itself is unit-tested; this flow owns the React validation-to-Run wiring.
test('invalid code conditions disable Run until both expressions are valid', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('load-sample').click();

  const run = page.getByTestId('run-backtest');
  await page.getByTestId('strategy-mode-code').click();

  const entry = page.getByLabel('進場條件 (entry)');
  const exit = page.getByLabel('出場條件 (exit)');
  await expect(run).toBeEnabled();

  await entry.fill('unknownSignal()');
  await expect(entry).toHaveAttribute('aria-invalid', 'true');
  await expect(entry).toHaveAccessibleDescription(/invalid expression/i);
  await expect(run).toBeDisabled();

  // An invalid dormant code expression must not block the other strategy modes.
  await page.getByTestId('strategy-mode-params').click();
  await expect(run).toBeEnabled();
  await page.getByTestId('strategy-mode-code').click();
  await expect(run).toBeDisabled();

  await entry.fill('price > maSlow');
  await expect(entry).toHaveAttribute('aria-invalid', 'false');
  await expect(run).toBeEnabled();

  await exit.fill('rsi = 70');
  await expect(exit).toHaveAttribute('aria-invalid', 'true');
  await expect(exit).toHaveAccessibleDescription(/invalid expression/i);
  await expect(run).toBeDisabled();

  await exit.fill('rsi > 70');
  await expect(exit).toHaveAttribute('aria-invalid', 'false');
  await expect(run).toBeEnabled();

  await run.click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
});
