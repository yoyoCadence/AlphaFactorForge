import { test, expect } from '@playwright/test';

// Slice 9 — chart hover. Pointing at any bar shows that bar's info (OHLC +
// entry/exit condition + position) in the shared 「此根資訊」 row, in ANY mode
// (no replay needed), plus a crosshair on the chart. Canvas pixels aren't
// E2E-assertable, so this checks the row appears/updates on hover and hides on
// leave; the x->bar geometry (barAtX) is unit-tested in scale.test.ts.

test('hovering the chart shows that bar info without replay mode', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();

  // not in replay mode and not hovering -> the info row is hidden
  await expect(page.getByTestId('bar-info')).toHaveCount(0);

  // hover over the chart canvas -> the info row appears with OHLC + bar number
  await page.getByTestId('candle-canvas').hover({ position: { x: 300, y: 150 } });
  const info = page.getByTestId('bar-info');
  await expect(info).toBeVisible();
  await expect(info).toContainText('第');
  await expect(info).toContainText('收'); // OHLC present

  // moving the mouse off the chart hides the row again
  await page.getByTestId('load-sample').hover();
  await expect(page.getByTestId('bar-info')).toHaveCount(0);
});
