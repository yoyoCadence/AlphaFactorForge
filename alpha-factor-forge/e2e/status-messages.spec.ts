import { test, expect } from '@playwright/test';

// UX-FEEDBACK-001 (2026-10-13 project review). The panel's status and error
// messages sat at the very top of a page several screens tall and never went
// away: an action started further down (discovery, a sweep cell) reported into
// a place the user could not see, and an old message stayed forever.
//
// They now stay in view while the page scrolls, can be dismissed, and a status
// message clears itself; an error stays until it is dismissed or replaced.
// Message texts are unchanged — other specs locate them with getByText.

test('a message from an action far down the page is shown where the user is', async ({ page }) => {
  await page.goto('/?mock=1&discoveryStep=20', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('load-sample').click();
  await expect(page.getByTestId('run-backtest')).toBeEnabled({ timeout: 20_000 });

  // The discovery panel is about two screens below the top of the page.
  await page.getByTestId('discovery-toggle').click();
  await page.getByTestId('discovery-start').click();

  const started = page.getByText(/已啟動探索任務 #\d+/);
  await expect(started).toBeVisible();
  await expect(started).toBeInViewport();
  await expect(page.getByTestId('discovery-start')).toBeInViewport();
  await expect(page.getByTestId('status-message')).toHaveAttribute('role', 'status');

  // dismissed by hand
  await page.getByTestId('status-message-close').click();
  await expect(started).toHaveCount(0);
});

test('a status message clears itself; an error stays until dismissed', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('load-sample').click();
  await expect(page.getByTestId('run-backtest')).toBeEnabled({ timeout: 20_000 });

  // An error: importing text that is not JSON.
  await page.locator('textarea').first().fill('not json');
  await page.getByRole('button', { name: '匯入 JSON' }).click();
  const error = page.getByTestId('status-error');
  await expect(error).toBeVisible();
  await expect(error).toHaveAttribute('role', 'alert');

  // A status message that does not clear the error: applying a sweep cell.
  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-best-marker')).toBeVisible();
  await page.getByTestId('sweep-cell-7').click();
  const applied = page.getByText('已套用：快線MA=7');
  await expect(applied).toBeVisible();
  await expect(applied).toBeInViewport();
  await expect(error).toBeVisible();

  // The status message goes away by itself; the error does not.
  await expect(applied).toHaveCount(0, { timeout: 15_000 });
  await expect(error).toBeVisible();

  await page.getByTestId('status-error-close').click();
  await expect(error).toHaveCount(0);
});

// PR #173 acceptance R3: the same action is a new notice even if its text matches.
test('repeating an identical notice restarts its full eight-second lifetime', async ({ page }) => {
  await page.clock.install();
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('load-sample').click();
  await page.getByTestId('sweep-toggle').click();
  await page.getByTestId('run-sweep').click();
  await expect(page.getByTestId('sweep-cell-7')).toBeVisible();
  await page.getByTestId('sweep-cell-7').click();
  const applied = page.getByText('已套用：快線MA=7');
  await expect(applied).toBeVisible();
  await page.clock.fastForward(7000);
  await page.getByTestId('sweep-cell-7').click();
  await page.clock.fastForward(2000);
  await expect(applied).toBeVisible();
  await page.clock.fastForward(5000);
  await expect(applied).toBeVisible();
  await page.clock.fastForward(1100);
  await expect(applied).toHaveCount(0);
});
