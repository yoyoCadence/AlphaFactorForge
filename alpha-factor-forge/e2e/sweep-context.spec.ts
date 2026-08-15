import { test, expect } from '@playwright/test';

// BUG-SWEEP-CONTEXT-001 — a completed parameter sweep may be shown, and its
// cells may be applied, only while the inputs it optimised over are still the
// live inputs. The audit's reproductions:
//   A. sweep with Holdout OFF -> switch Holdout ON -> 套用最佳 without re-scanning
//      (the winning combo was chosen using the out-of-sample tail)
//   B. sweep -> edit a field the sweep held constant (手續費) -> the grid still
//      shows metrics computed with the old cost
//   C. sweep on dataset A -> select dataset B -> A's heatmap must be unreachable
// Plus the intentional case that must NOT invalidate: applying a cell of a
// swept axis, which by definition only writes the parameters being swept.
//
// Runs against the Vite dev app with the in-memory mock client (?mock=1). The
// context contract itself is unit-tested in src/services/sweepArtifact.test.ts.

/** A second dataset that is unmistakably different from the 600-bar sample. */
const IMPORTED_DATASET = JSON.stringify({
  symbol: 'IMPB',
  interval: '1h',
  candles: Array.from({ length: 30 }, (_, i) => ({
    t: 1_700_000_000_000 + i * 3_600_000,
    o: 100 + i,
    h: 102 + i,
    l: 99 + i,
    c: 101 + i,
    v: 10 + i,
  })),
});

test('toggling Holdout invalidates a full-period sweep instead of letting it tune on the tail', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  await page.getByTestId('load-sample').click();
  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await expect(page.getByTestId('apply-best')).toBeVisible();
  await expect(page.getByTestId('sweep-stale')).toHaveCount(0);

  // Repro A. The grid was optimised over every bar, so reserving the last 30%
  // as out-of-sample makes it invalid — heatmap, 套用最佳 and every clickable
  // cell must go, not just the label.
  await page.getByTestId('holdout-toggle').check();
  await expect(page.getByTestId('sweep-best-marker')).toHaveCount(0);
  await expect(page.getByTestId('apply-best')).toHaveCount(0);
  await expect(page.getByTestId('sweep-cell-7')).toHaveCount(0);
  await expect(page.getByTestId('sweep-stale')).toBeVisible();

  // Re-scanning against the in-sample segment restores every action.
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await expect(page.getByTestId('apply-best')).toBeVisible();
  await expect(page.getByTestId('sweep-stale')).toHaveCount(0);
  await expect(page.getByTestId('sweep-scope')).toContainText('僅樣本內');

  // The percentage is part of the recorded range too: moving the boundary
  // invalidates the grid that was optimised on the previous one.
  const holdoutPct = page.locator('label').filter({ has: page.getByTestId('holdout-toggle') }).getByRole('spinbutton');
  await holdoutPct.fill('40');
  await expect(page.getByTestId('sweep-best-marker')).toHaveCount(0);
  await expect(page.getByTestId('apply-best')).toHaveCount(0);
  await expect(page.getByTestId('sweep-stale')).toBeVisible();
});

test('applying a swept cell keeps the grid valid, but a non-axis edit invalidates it', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  await page.getByTestId('load-sample').click();
  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();

  // The intentional case: 快線MA is the swept axis, so applying one of its cells
  // is exactly what the heatmap is for and must not invalidate the grid it came
  // from — the ✓ marker, ★ best cell and 套用最佳 all survive.
  await page.getByTestId('sweep-cell-7').click();
  await expect(page.getByTestId('sweep-applied-marker')).toBeVisible();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await expect(page.getByTestId('apply-best')).toBeVisible();
  await expect(page.getByTestId('sweep-stale')).toHaveCount(0);
  await expect(page.getByTestId('applied-fastMA').getByRole('spinbutton')).toHaveValue('7');

  // Repro B: 手續費 is held constant by the sweep, so changing it changes every
  // cell's metric. The whole grid, including the applied highlight, must go.
  await page.getByLabel('手續費 %').fill('0.2');
  await expect(page.getByTestId('sweep-best-marker')).toHaveCount(0);
  await expect(page.getByTestId('sweep-applied-marker')).toHaveCount(0);
  await expect(page.getByTestId('apply-best')).toHaveCount(0);
  await expect(page.getByTestId('sweep-stale')).toBeVisible();

  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await expect(page.getByTestId('sweep-stale')).toHaveCount(0);
});

test('a dataset switch cannot leave the previous dataset heatmap reachable', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  await page.getByTestId('load-sample').click();
  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();

  // Repro C. Structural locators avoid coupling this regression to the legacy
  // UI's currently mojibaked Traditional-Chinese import copy.
  await page.locator('textarea').first().fill(IMPORTED_DATASET);
  await page.locator('textarea + button').click();
  await expect(page.getByText(/已匯入 30 根 K 線/)).toBeVisible();
  await expect(page.getByTestId('sweep-best-marker')).toHaveCount(0);
  await expect(page.getByTestId('apply-best')).toHaveCount(0);

  // Re-opening the sweep for the NEW dataset must start empty: no grid, and no
  // stale notice either, because nothing from dataset A survived the switch.
  await page.getByTestId('sweep-toggle').click();
  await expect(page.getByTestId('run-sweep')).toBeVisible();
  await expect(page.getByTestId('sweep-best-marker')).toHaveCount(0);
  await expect(page.getByTestId('sweep-stale')).toHaveCount(0);
});
