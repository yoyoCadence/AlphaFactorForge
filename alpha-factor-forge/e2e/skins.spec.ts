import { test, expect } from '@playwright/test';

// Skins PR-D — the picker goes live. Covers the three things the switcher has
// to get right: every skin renders without breaking the layout, the choice
// survives a reload, and a pop-out window opens on the same skin as its parent.
//
// Runs against the Vite dev app with the mock data client (?mock=1). Purely a
// presentation concern — no backtest maths or persistence is involved. The skin
// preference is a non-sensitive UI setting kept in localStorage (afs.ui.skin);
// swapping that for an app_settings wrapper later must keep these passing.

const SKINS = [
  'forge-paper', 'midnight-tape', 'swiss-forge', 'atelier-warm', 'blueprint',
  'signal-orange', 'broadsheet', 'brutal-yellow', 'frost-grey', 'aurora-glass',
] as const;

for (const id of SKINS) {
  test(`skin ${id} renders`, async ({ page }) => {
    await page.goto('/?mock=1');
    await page.getByTestId('skin-picker').locator('select').selectOption(id);
    await expect(page.locator('[data-skin]').first()).toHaveAttribute('data-skin', id);

    // The skins vary padding, font size, radius and header height, so this also
    // guards against a token blowing the layout apart: the workspace has to
    // still be there and usable afterwards.
    await expect(page.getByTestId('load-sample')).toBeVisible();
    await page.getByTestId('load-sample').click();
    await expect(page.getByTestId('candle-canvas')).toBeVisible();
    await expect(page.getByTestId('run-backtest')).toBeEnabled();
  });
}

test('the chosen skin survives a reload', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('skin-picker').locator('select').selectOption('blueprint');
  await expect(page.locator('[data-skin]').first()).toHaveAttribute('data-skin', 'blueprint');

  await page.reload();
  await expect(page.locator('[data-skin]').first()).toHaveAttribute('data-skin', 'blueprint');
  await expect(page.getByTestId('skin-picker').locator('select')).toHaveValue('blueprint');
});

test('a pop-out window opens on the same skin as the main window', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('skin-picker').locator('select').selectOption('midnight-tape');
  await expect(page.locator('[data-skin]').first()).toHaveAttribute('data-skin', 'midnight-tape');

  // The child window mounts its own React tree with no provider above it, so it
  // has to fall back to the stored preference rather than the default skin.
  const child = await page.context().newPage();
  await child.goto('/?window=chart&mock=1');
  await expect(child.getByTestId('chart-window-loading')).toBeVisible();
  await expect(child.locator('[data-skin]').first()).toHaveAttribute('data-skin', 'midnight-tape');
  await child.close();
});

test('switching skin does not disturb loaded data or the strategy form', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('applied-fastMA').or(page.getByTestId('strategy-name')).first().waitFor();
  await page.getByTestId('strategy-name').fill('skin-switch-probe');

  await page.getByTestId('skin-picker').locator('select').selectOption('brutal-yellow');
  await expect(page.locator('[data-skin]').first()).toHaveAttribute('data-skin', 'brutal-yellow');

  // A skin is a presentation change: the dataset, the chart and the edited
  // strategy name all have to be exactly where they were.
  await expect(page.getByTestId('strategy-name')).toHaveValue('skin-switch-probe');
  await expect(page.getByTestId('candle-canvas')).toBeVisible();
  await expect(page.getByTestId('chart-zoom-status')).toContainText('顯示 500 根');
});
