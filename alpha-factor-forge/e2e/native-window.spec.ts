import { test, expect } from '@playwright/test';

// Slice 8b browser-testable boundary: each child route mounts only its pop-out
// shell. Rust WebviewWindowBuilder creation/focus and live Tauri event transport
// remain native smoke-test responsibilities.
test('chart child route mounts without the main backtest workspace', async ({ page }) => {
  await page.goto('/?window=chart&mock=1');
  await expect(page.getByText('ALPHAFACTORFORGE /chart')).toBeVisible();
  await expect(page.getByTestId('chart-window-loading')).toContainText('正在同步圖表資料');
  await expect(page.getByTestId('load-sample')).toHaveCount(0);
});

test('metrics child route mounts without the main backtest workspace', async ({ page }) => {
  await page.goto('/?window=metrics&mock=1');
  await expect(page.getByText('ALPHAFACTORFORGE /metrics')).toBeVisible();
  await expect(page.getByTestId('metrics-window-loading')).toContainText('正在等待主視窗的績效資料');
  await expect(page.getByTestId('load-sample')).toHaveCount(0);
});
