import { test, expect } from '@playwright/test';

// Slice 8b-1 browser-testable boundary: the child route mounts only the chart
// window shell. Rust WebviewWindowBuilder creation/focus and live Tauri event
// transport remain native smoke-test responsibilities.
test('chart child route mounts without the main backtest workspace', async ({ page }) => {
  await page.goto('/?window=chart&mock=1');
  await expect(page.getByText('ALPHAFACTORFORGE /chart')).toBeVisible();
  await expect(page.getByTestId('chart-window-loading')).toContainText('正在同步圖表資料');
  await expect(page.getByTestId('load-sample')).toHaveCount(0);
});
