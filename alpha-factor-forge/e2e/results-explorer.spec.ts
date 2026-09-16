import { test, expect, type Page } from '@playwright/test';

// P01 Results Explorer — browser/mock flow. The seeded history is produced by
// the real composer chain (see src/tauri-client/mockHistorySeed.ts), so what
// the explorer renders here is the shape the backend stores. Real SQLite reads
// stay covered by the Rust repository tests.

const recordRows = (page: Page) => page.locator('tr[data-testid^="results-explorer-record-"]');
const summaryRows = (page: Page) => page.locator('tr[data-testid^="results-explorer-summary-"]');

test('ranks seeded validation records, opens the snapshot, and loads trades', async ({ page }) => {
  await page.goto('/?mock=1&seedHistory=1');
  await page.getByTestId('results-explorer-toggle').click();

  const rows = recordRows(page);
  await expect(rows).toHaveCount(2);
  // Gate-passed first with its persisted score; gate-failed second with no score.
  await expect(rows.nth(0)).toHaveAttribute('data-gate', 'pass');
  await expect(rows.nth(0)).toContainText('seed MA 3/8');
  await expect(rows.nth(0)).toContainText('通過');
  await expect(rows.nth(0).locator('td').nth(5)).toHaveText(/^\d\.\d{4}$/);
  await expect(rows.nth(1)).toHaveAttribute('data-gate', 'fail');
  await expect(rows.nth(1)).toContainText('seed MA 9/21');
  await expect(rows.nth(1).locator('td').nth(5)).toHaveText('—');
  await expect(rows.nth(0)).toContainText('手動');
  await expect(page.getByTestId('results-explorer-loaded-at')).toContainText('紀錄 2 · 摘要 5');

  // The passing record's immutable snapshot.
  await rows.nth(0).click();
  const detail = page.getByTestId('results-explorer-record-detail');
  await expect(detail).toBeVisible();
  await expect(detail).toContainText('策略指紋');
  await expect(detail).toContainText('gate=gate-v1');
  await expect(detail).toContainText('score=score-v1');
  const gateRows = page.locator('tr[data-testid^="results-explorer-gate-"]');
  await expect(gateRows).toHaveCount(8);
  await expect(page.locator('tr[data-testid^="results-explorer-gate-"][data-pass="false"]')).toHaveCount(0);
  await expect(page.getByTestId('results-explorer-score')).toBeVisible();
  await expect(page.getByTestId('results-explorer-benchmarks')).toContainText('buyHold');
  await expect(page.getByTestId('results-explorer-benchmarks')).toContainText('Random Entry 50 次');
  await expect(page.getByTestId('results-explorer-missing')).toHaveCount(0);
  await expect(detail).toContainText('Test 段未執行、未揭露');
  await expect(detail).not.toContainText('Test [');

  // Latest Train + Validation summaries, then the Validation trades on demand.
  const segments = page.getByTestId('results-explorer-segment-summaries');
  await expect(segments).toContainText('Train #');
  await expect(segments).toContainText('Validation #');
  const loadValidation = detail.locator('button[data-testid^="results-explorer-load-trades-"]').nth(1);
  const label = await loadValidation.textContent();
  const expectedTrades = Number(/（(\d+) 筆）/.exec(label ?? '')?.[1]);
  expect(expectedTrades).toBeGreaterThan(0);
  await loadValidation.click();
  await expect(detail.locator('table[data-testid^="results-explorer-trade-rows-"]')).toBeVisible();
  await expect(detail.locator('table[data-testid^="results-explorer-trade-rows-"] tbody tr')).toHaveCount(expectedTrades);

  // A refresh re-reads the tables but keeps the selection.
  await page.getByTestId('results-explorer-refresh').click();
  await expect(page.getByTestId('results-explorer-refresh')).toBeEnabled();
  await expect(detail).toContainText('驗證紀錄 #');

  // The failed record: no score by contract, some criteria failed, and its
  // selection survives a filter that hides it — reported, not dropped.
  await rows.nth(1).click();
  await expect(detail).toContainText('seed MA 9/21');
  await expect(page.getByTestId('results-explorer-score')).toHaveCount(0);
  await expect(detail).toContainText('未通過 Gate，依契約沒有 Score');
  await expect(page.locator('tr[data-testid^="results-explorer-gate-"][data-pass="false"]').first()).toBeVisible();
  await page.getByTestId('results-explorer-filter-gate').check();
  await expect(rows).toHaveCount(1);
  await expect(page.getByTestId('results-explorer-stale-selection')).toBeVisible();
  await expect(detail).toContainText('seed MA 9/21');
  await page.getByTestId('results-explorer-filter-gate').uncheck();
  await expect(rows).toHaveCount(2);
  await expect(page.getByTestId('results-explorer-stale-selection')).toHaveCount(0);

  // Summaries view: Test segment hidden, every visible segment listed.
  await page.getByTestId('results-explorer-view-summaries').click();
  const summaries = summaryRows(page);
  await expect(summaries).toHaveCount(5);
  await expect(page.locator('tr[data-testid^="results-explorer-summary-"][data-segment="test"]')).toHaveCount(0);
  await expect(page.locator('tr[data-testid^="results-explorer-summary-"][data-segment="full"]')).toHaveCount(1);
  await expect(page.locator('tr[data-testid^="results-explorer-summary-"][data-segment="train"]')).toHaveCount(2);
  await expect(page.locator('tr[data-testid^="results-explorer-summary-"][data-segment="validation"]')).toHaveCount(2);

  // Filtering by strategy narrows both lists.
  const strategyFilter = page.getByTestId('results-explorer-filter-strategy');
  const secondStrategy = await strategyFilter.locator('option', { hasText: 'seed MA 3/8' }).getAttribute('value');
  await strategyFilter.selectOption(secondStrategy!);
  await expect(summaries).toHaveCount(2);
  await strategyFilter.selectOption('');
  await expect(summaries).toHaveCount(5);
});

test('shows a manually saved backtest with its trades, and says when nothing is saved', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('results-explorer-toggle').click();
  await expect(page.getByTestId('results-explorer-empty')).toContainText('還沒有驗證紀錄');
  await page.getByTestId('results-explorer-view-summaries').click();
  await expect(page.getByTestId('results-explorer-empty')).toContainText('還沒有回測摘要');

  await page.getByTestId('load-sample').click();
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
  await page.getByTestId('strategy-name').fill('explorer manual');
  await page.getByTestId('save-result').click();
  await expect(page.getByText(/已存檔：strategy #/)).toBeVisible();

  // Nothing appears until the user asks: the explorer read before the save.
  await expect(page.getByTestId('results-explorer-empty')).toContainText('還沒有回測摘要');
  await page.getByTestId('results-explorer-refresh').click();
  const summaries = summaryRows(page);
  await expect(summaries).toHaveCount(1);
  await expect(summaries.first()).toHaveAttribute('data-segment', 'full');
  await expect(summaries.first()).toContainText('explorer manual');

  await summaries.first().click();
  const detail = page.getByTestId('results-explorer-summary-detail');
  await expect(detail).toContainText('摘要 #');
  await expect(detail).toContainText('全期');
  const load = detail.locator('button[data-testid^="results-explorer-load-trades-"]');
  const expectedTrades = Number(/（(\d+) 筆）/.exec((await load.textContent()) ?? '')?.[1]);
  expect(expectedTrades).toBeGreaterThan(0);
  await load.click();
  await expect(detail.locator('table[data-testid^="results-explorer-trade-rows-"] tbody tr')).toHaveCount(expectedTrades);
});
