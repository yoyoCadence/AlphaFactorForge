import { test, expect } from '@playwright/test';

// P05 — the research history panel over the DEV-only mock, whose rows follow
// the same rules the runner's do: one attempt per candidate frozen with the
// enqueue, moved forward with the candidate, and never rewritten by a later
// run. The Rust side (discovery_runner/tests/history.rs) proves the storage;
// this pins what the window shows and that a re-run adds rather than replaces.

test('a run leaves one attempt per candidate, a re-run adds new ones, and a detail shows the hypothesis and result', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto('/?mock=1&discoveryStep=20', { waitUntil: 'domcontentloaded' });

  const history = page.getByTestId('research-history');
  await page.getByTestId('research-history-toggle').click();
  await expect(page.getByTestId('research-history-empty')).toBeVisible();

  // A three-candidate run.
  await page.getByTestId('discovery-toggle').click();
  await page.getByTestId('load-sample').click();
  const start = page.getByTestId('discovery-start');
  await expect(start).toBeEnabled({ timeout: 20_000 });
  await start.click();
  await expect(page.getByTestId('discovery-status')).toContainText('已完成', { timeout: 30_000 });

  await page.getByTestId('research-history-refresh').click();
  await expect(page.getByTestId('research-history-count')).toHaveText('嘗試 3');
  const first = history.locator('[data-testid^="research-history-attempt-"]');
  await expect(first).toHaveCount(3);
  await expect(page.getByTestId('research-history-attempt-1')).toHaveAttribute('data-status', 'completed');
  await expect(page.getByTestId('research-history-attempt-1')).toContainText('Gate 通過');
  await expect(page.getByTestId('research-history-attempt-2')).toContainText('Gate 未通過');

  // Detail: the frozen hypothesis, the fingerprints, and the stored result.
  await page.getByTestId('research-history-attempt-1').click();
  const detail = page.getByTestId('research-history-detail');
  await expect(detail).toContainText(':candidate:0');
  await expect(page.getByTestId('research-history-hypothesis')).toContainText('param-sweep:fastMA');
  await expect(page.getByTestId('research-history-hypothesis')).toContainText('失效情境');
  await expect(page.getByTestId('research-history-fingerprints')).toContainText('結果檔');
  await expect(page.getByTestId('research-history-result')).toContainText('Train');

  // A re-run of the same configuration: three MORE attempts (#4–#6); the
  // first three keep their ids, statuses, and results.
  await start.click();
  await expect(page.getByTestId('discovery-status')).toContainText('已完成', { timeout: 30_000 });
  await page.getByTestId('research-history-refresh').click();
  await expect(page.getByTestId('research-history-count')).toHaveText('嘗試 6');
  await expect(first).toHaveCount(6);
  await expect(page.getByTestId('research-history-attempt-1')).toHaveAttribute('data-status', 'completed');
  await expect(page.getByTestId('research-history-attempt-1')).toContainText('Gate 通過');
  await expect(page.getByTestId('research-history-attempt-6')).toHaveAttribute('data-status', 'completed');
});

test('a cancelled run leaves its unfinished attempts skipped with the reason', async ({ page }) => {
  await page.goto('/?mock=1&discoveryStep=400', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('discovery-toggle').click();
  await page.getByTestId('load-sample').click();
  const start = page.getByTestId('discovery-start');
  await expect(start).toBeEnabled({ timeout: 20_000 });
  await start.click();
  await page.getByTestId('discovery-cancel').click();
  await expect(page.getByTestId('discovery-status')).toContainText('已取消', { timeout: 10_000 });

  await page.getByTestId('research-history-toggle').click();
  await expect(page.getByTestId('research-history-count')).toHaveText('嘗試 3');
  const skipped = page.locator('[data-testid^="research-history-attempt-"][data-status="skipped"]');
  await expect(skipped.first()).toContainText('run cancelled before this candidate ran');
});
