import { test, expect, type Page } from '@playwright/test';

// Slice 10-1 — cursor-anchored wheel zoom + reset. Canvas pixels are not
// asserted; anchor/window arithmetic is unit-tested in scale.test.ts. These
// flows verify that real wheel input changes the visible count and that replay
// keeps its no-future-data boundary while preserving the zoom level.

/** How far the page is scrolled, across both scrollers (the app shell scrolls
 *  <main>, not the document).
 *
 *  Wheeling over the chart must zoom it and scroll NOTHING. This used to be
 *  checked by asserting the canvas had not moved, but that proxy also trips on
 *  any unrelated reflow: the skin layer loads its web fonts from a CDN, and on
 *  a cold runner they swap in after first paint and resize the rows above the
 *  chart by a few px (down on CI's Ubuntu fallback, up on Windows). Reading the
 *  scroll offsets measures the actual claim and is immune to that. */
function scrollOffsets(page: Page) {
  return page.evaluate(() => ({
    main: document.querySelector('main')?.scrollTop ?? 0,
    doc: document.documentElement.scrollTop,
  }));
}

test('mouse wheel zooms the chart and reset returns to fit', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();

  const canvas = page.getByTestId('candle-canvas');
  const status = page.getByTestId('chart-zoom-status');
  const reset = page.getByTestId('chart-zoom-reset');
  await expect(status).toContainText('顯示 500 根');
  await expect(reset).toBeDisabled();

  const box = await canvas.boundingBox();
  if (!box) throw new Error('chart canvas has no bounding box');
  const restingScroll = await scrollOffsets(page);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, 100); // already at max fit: remains a true reset state
  await expect(status).toContainText('顯示 500 根');
  await expect(reset).toBeDisabled();
  await page.waitForTimeout(100); // allow any accidental scroll to settle
  expect(await scrollOffsets(page)).toEqual(restingScroll);
  await page.mouse.wheel(0, -100);

  await expect(status).toContainText('顯示 400 根');
  await expect(reset).toBeEnabled();
  expect(await scrollOffsets(page)).toEqual(restingScroll);
  await reset.click();
  await expect(status).toContainText('顯示 500 根');
  await expect(reset).toBeDisabled();
});

test('zoom follows the replay cursor without exposing future bars', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('replay-toggle').check();
  await page.getByTestId('replay-cursor').fill('300');

  const canvas = page.getByTestId('candle-canvas');
  const status = page.getByTestId('chart-zoom-status');
  await expect(status).toContainText('顯示 301 根');

  const box = await canvas.boundingBox();
  if (!box) throw new Error('chart canvas has no bounding box');
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, -100);
  await expect(status).toContainText('顯示 241 根');

  // Moving replay backwards keeps 241 bars but shifts the window to end at the
  // new cursor; the pure reconcileBarWindow tests assert the exact indices.
  await page.getByTestId('replay-cursor').fill('250');
  await expect(page.getByTestId('replay-readout')).toContainText('第 251 / 600 根');
  await expect(status).toContainText('顯示 241 根');

  await page.getByTestId('chart-zoom-reset').click();
  await expect(status).toContainText('顯示 251 根');
});
