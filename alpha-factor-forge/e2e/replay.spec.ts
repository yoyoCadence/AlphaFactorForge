import { test, expect } from '@playwright/test';

// Slice 6-1 — bar-replay cursor. Enabling 回放模式 shows a scrubber + step/reset
// controls; the chart clips to bars [.., cursor]. Runs against the Vite dev app
// with the mock data client (?mock=1; 600 sample candles). Canvas pixels aren't
// E2E-assertable, so this asserts the cursor readout / control state; the window
// geometry is unit-tested (scale.test.ts replayWindow).

test('replay cursor steps, scrubs, and resets', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();

  // controls are hidden until 回放模式 is enabled
  await expect(page.getByTestId('replay-readout')).toHaveCount(0);

  await page.getByTestId('replay-toggle').check();
  const readout = page.getByTestId('replay-readout');
  await expect(readout).toContainText('第 600 / 600 根'); // starts at the latest bar

  // step back twice
  await page.getByTestId('replay-back').click();
  await expect(readout).toContainText('第 599 / 600 根');
  await page.getByTestId('replay-back').click();
  await expect(readout).toContainText('第 598 / 600 根');

  // jump via the scrubber (0-based value 300 -> "301 / 600")
  await page.getByTestId('replay-cursor').fill('300');
  await expect(readout).toContainText('第 301 / 600 根');

  // step forward
  await page.getByTestId('replay-fwd').click();
  await expect(readout).toContainText('第 302 / 600 根');

  // reset jumps back to the latest bar
  await page.getByTestId('replay-reset').click();
  await expect(readout).toContainText('第 600 / 600 根');

  // turning replay off hides the controls again
  await page.getByTestId('replay-toggle').uncheck();
  await expect(page.getByTestId('replay-readout')).toHaveCount(0);
});
