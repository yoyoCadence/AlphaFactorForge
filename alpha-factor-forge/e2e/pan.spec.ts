import { test, expect, type Locator, type Page } from '@playwright/test';

async function windowBounds(status: Locator): Promise<{ start: number; end: number }> {
  return {
    start: Number(await status.getAttribute('data-window-start')),
    end: Number(await status.getAttribute('data-window-end')),
  };
}

async function zoomInAtCentre(page: Page, canvas: Locator): Promise<{ x: number; y: number }> {
  const box = await canvas.boundingBox();
  if (!box) throw new Error('chart canvas has no bounding box');
  const point = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  await page.mouse.move(point.x, point.y);
  await page.mouse.wheel(0, -100);
  return point;
}

test('drag pans a zoomed chart while a short click preserves hover/window', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();

  const canvas = page.getByTestId('candle-canvas');
  const status = page.getByTestId('chart-zoom-status');
  const point = await zoomInAtCentre(page, canvas);
  await expect(status).toContainText('顯示 400 根');
  const before = await windowBounds(status);

  // A press/release without crossing the 4px threshold is still hover/click,
  // not pan: the shared bar-info remains available and bounds do not change.
  await page.mouse.down();
  await page.mouse.up();
  await expect(page.getByTestId('bar-info')).toBeVisible();
  expect(await windowBounds(status)).toEqual(before);

  // Drag content right -> reveal older bars (both indices decrease).
  await page.mouse.move(point.x, point.y);
  await page.mouse.down();
  await page.mouse.move(point.x + 120, point.y, { steps: 5 });
  await expect(status).toHaveAttribute('data-dragging', 'true');
  await page.mouse.up();
  await expect(status).toHaveAttribute('data-dragging', 'false');
  const after = await windowBounds(status);
  expect(after.start).toBeLessThan(before.start);
  expect(after.end).toBeLessThan(before.end);
  expect(after.end - after.start).toBe(before.end - before.start);

  await page.getByTestId('chart-zoom-reset').click();
  await expect(status).toContainText('顯示 500 根');
});

test('drag pan stays behind the replay cursor', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('replay-toggle').check();
  await page.getByTestId('replay-cursor').fill('300');

  const canvas = page.getByTestId('candle-canvas');
  const status = page.getByTestId('chart-zoom-status');
  const point = await zoomInAtCentre(page, canvas);
  await expect(status).toContainText('顯示 241 根');

  await page.mouse.down();
  await page.mouse.move(point.x + 100, point.y, { steps: 5 });
  await page.mouse.up();
  const panned = await windowBounds(status);
  expect(panned.end).toBeLessThanOrEqual(300);

  // Moving replay backwards clamps the historical window to the new boundary.
  await page.getByTestId('replay-cursor').fill('250');
  await expect(page.getByTestId('replay-readout')).toContainText('第 251 / 600 根');
  const clamped = await windowBounds(status);
  expect(clamped.end).toBeLessThanOrEqual(250);
});
