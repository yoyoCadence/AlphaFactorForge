import type { Locator, Page } from '@playwright/test';

/** Observe before navigation: the skin stylesheet can finish (or fail) before
 *  the chart is queried. Resource errors still allow the normal fallback font. */
export async function observeThemeFonts(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const settled = (event: Event): void => {
      const link = event.target;
      if (link instanceof HTMLLinkElement && link.id === 'afs-fonts') {
        link.dataset.e2eFonts = 'settled';
      }
    };
    document.addEventListener('load', settled, true);
    document.addEventListener('error', settled, true);
  });
}

/** Font swaps reflow rows above the canvas. Complete the stylesheet, used-font
 *  loading and layout before deriving coordinates for manual mouse input. */
export async function chartBounds(canvas: Locator): Promise<{
  x: number; y: number; width: number; height: number;
}> {
  const page = canvas.page();
  await page.locator('link#afs-fonts[data-e2e-fonts="settled"]').waitFor({ state: 'attached' });
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    });
  });
  const box = await canvas.boundingBox();
  if (!box) throw new Error('chart canvas has no bounding box');
  return box;
}
