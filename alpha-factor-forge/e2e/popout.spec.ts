import { test, expect } from '@playwright/test';

// Slice 8a — pop-out 圖表 / 回測績效 into an in-app floating panel. The panel is
// NON-modal (no backdrop), so the left-column controls stay usable while it is
// open. This is the browser-testable variant; the real-OS-window pop-out is the
// future Slice 8b (not exercised here). Runs against the Vite dev app (?mock=1).

test('chart pop-out opens, leaves the left column usable, and closes', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();

  await expect(page.getByTestId('chart-popout')).toHaveCount(0);

  // enlarge the chart into a floating panel (positioned over the results area,
  // so the left-column strategy controls stay clear)
  await page.getByTestId('popout-chart').click();
  await expect(page.getByTestId('chart-popout')).toBeVisible();

  // non-modal: the left-column 執行回測 is still clickable while the panel is
  // open, and running a backtest reveals the 回測績效 放大 button
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('popout-metrics')).toBeVisible();

  // close via the panel ✕ -> the inline chart returns
  await page.getByTestId('chart-popout-close').click();
  await expect(page.getByTestId('chart-popout')).toHaveCount(0);
  await expect(page.getByTestId('popout-chart')).toContainText('放大');
});

test('metrics pop-out shows the table and closes on Escape', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('run-backtest').click();

  await page.getByTestId('popout-metrics').click();
  const panel = page.getByTestId('metrics-popout');
  await expect(panel).toBeVisible();
  await expect(panel).toContainText('淨報酬'); // a metrics row moved into the panel

  await page.keyboard.press('Escape');
  await expect(page.getByTestId('metrics-popout')).toHaveCount(0);
});
