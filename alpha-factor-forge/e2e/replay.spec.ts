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

// Slice 6-2 — autoplay: ⏵ advances the cursor on a timer and auto-stops at the
// last bar. Start a few bars from the end at 4× so it finishes fast/deterministic.
test('autoplay advances the cursor and stops at the end', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('replay-toggle').check();

  await page.getByTestId('replay-cursor').fill('595');
  await expect(page.getByTestId('replay-readout')).toContainText('第 596 / 600 根');
  await page.getByTestId('replay-speed').selectOption('4');

  // play -> button shows the pause glyph while running
  await page.getByTestId('replay-play').click();
  await expect(page.getByTestId('replay-play')).toContainText('⏸');

  // the timer advances to the last bar and autoplay stops (button back to play)
  await expect(page.getByTestId('replay-readout')).toContainText('第 600 / 600 根', { timeout: 5000 });
  await expect(page.getByTestId('replay-play')).toContainText('⏵');
});

// Slice 6-3 — live signal readout: at the current replay bar, show whether the
// entry/exit condition is TRUE plus the position (from the last backtest).
test('replay shows the live entry/exit signal + position at the cursor', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('replay-toggle').check();

  const signal = page.getByTestId('replay-signal');
  await expect(signal).toBeVisible();
  await expect(signal).toContainText('進場');
  await expect(signal).toContainText('出場');
  await expect(signal).toContainText('持倉');

  // position is unknown until a backtest has run...
  await expect(page.getByTestId('replay-position')).toContainText('回測後顯示');

  // ...and resolves to a concrete state once it has
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('replay-position')).not.toContainText('回測後顯示');
});
