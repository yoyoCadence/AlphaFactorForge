import { test, expect, type Page } from '@playwright/test';

// REACT-STYLE-WARN-001 (2026-10-13 project review). React warns in development
// when a re-render changes or removes a longhand style (`borderColor`,
// `borderBottom`) on an element that also sets the matching shorthand
// (`border`): the browser may keep the stale value, so the control can stay in
// its previous colour. The state styles below each used that mix.
//
// The flow walks every control state that swaps a border: the applied-from-sweep
// field, an invalid period, the strategy mode buttons, the code editor's error
// state, and the Results Explorer tabs. HELPTIP-SHADOW-001 rides along: the
// small inline controls must not take the skin's button shadow.

function collectStyleWarnings(page: Page): string[] {
  const seen: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error' && /style property during rerender/.test(message.text())) {
      seen.push(message.text());
    }
  });
  return seen;
}

test('switching control states raises no shorthand/longhand style warning', async ({ page }) => {
  test.setTimeout(60_000);
  const warnings = collectStyleWarnings(page);
  await page.goto('/?mock=1&seedHistory=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('load-sample').click();
  await expect(page.getByTestId('run-backtest')).toBeEnabled({ timeout: 20_000 });

  // applied-from-sweep highlight on, then off by a hand edit
  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await page.getByTestId('sweep-cell-7').click();
  const applied = page.getByTestId('applied-fastMA');
  await expect(applied).toBeVisible();
  await applied.getByRole('spinbutton').fill('8');
  await expect(page.getByTestId('applied-fastMA')).toHaveCount(0);

  // an invalid period switches the field to the error border and back
  const strategy = page.getByTestId('strategy-section');
  const fastMA = strategy.getByLabel('快線 MA');
  await fastMA.fill('2.5');
  await expect(page.getByTestId('strategy-issues')).toContainText('fastMA');
  await fastMA.fill('9');
  await expect(page.getByTestId('strategy-issues')).toHaveCount(0);

  // the mode buttons swap their active border; the code editor shows and
  // clears its error border
  await page.getByTestId('strategy-mode-code').click();
  const entry = page.getByLabel('進場條件 (entry)');
  await entry.fill('rsi >');
  await expect(entry).toHaveAttribute('aria-invalid', 'true');
  await entry.fill('rsi > 70');
  await expect(entry).toHaveAttribute('aria-invalid', 'false');
  await page.getByTestId('strategy-mode-blocks').click();
  await page.getByTestId('strategy-mode-params').click();

  // the Results Explorer tabs swap their on/off state
  await page.getByTestId('results-explorer-toggle').click();
  await expect(page.locator('[data-testid^="results-explorer-record-"]').first()).toBeVisible();
  await page.getByTestId('results-explorer-view-summaries').click();
  await page.getByTestId('results-explorer-view-records').click();

  expect(warnings).toEqual([]);
});

test('small inline controls do not take the skin button shadow', async ({ page }) => {
  await page.addInitScript(() => {
    try {
      // The skin with the heaviest button shadow, where the defect was visible.
      localStorage.setItem('afs.ui.skin', 'brutal-yellow');
    } catch {
      /* preference only */
    }
  });
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('load-sample').click();
  await expect(page.getByTestId('candle-canvas')).toBeVisible({ timeout: 20_000 });

  const shadowOf = (testId: string) =>
    page.getByTestId(testId).evaluate((element) => getComputedStyle(element).boxShadow);

  // A regular button on this skin does carry the shadow — the control is real.
  expect(await shadowOf('load-sample')).not.toBe('none');

  expect(await shadowOf('help-dataset')).toBe('none');
  expect(await shadowOf('chart-zoom-reset')).toBe('none');

  await page.getByTestId('popout-chart').click();
  expect(await shadowOf('chart-popout-close')).toBe('none');
});
