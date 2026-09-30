import { test, expect } from '@playwright/test';

const btc = 'crypto:binance:BTCUSDT';
const eth = 'crypto:binance:ETHUSDT';

async function add(page: import('@playwright/test').Page, id: string, rationale: string) {
  await page.getByTestId('campaign-snapshot-choice').selectOption(id);
  await page.getByTestId('campaign-add').click();
  const instrument = id === 'a'.repeat(64) ? btc : eth;
  for (const [field, value] of Object.entries({
    minimumTotalBars: '8760', minimumTrainBars: '1000', foldValidationBars: '500', foldCount: '3',
  })) {
    await page.getByTestId(`campaign-${field}-${instrument}`).fill(value);
  }
  await page.getByTestId(`campaign-rationale-${instrument}`).fill(rationale);
}

test('authors two instruments, previews, freezes, starts them separately and reads admission', async ({ page }) => {
  await page.goto('/?mock=1&discoveryStep=30', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('campaign-toggle').click();
  await expect(page.getByTestId('campaign-snapshot-choice').locator('option')).toHaveCount(4);
  await add(page, 'a'.repeat(64), 'One year of hourly BTC history and three Train folds.');
  await add(page, 'd'.repeat(64), 'One year of hourly ETH history and three Train folds.');
  await expect(page.getByTestId('campaign-freeze')).toBeDisabled();
  await page.getByTestId('campaign-preview-button').click();
  await expect(page.getByTestId('campaign-preview')).toContainText('已解析，8760 bars');
  await expect(page.getByTestId('campaign-freeze')).toBeEnabled();
  await page.getByTestId('campaign-freeze').click();
  await expect(page.getByTestId('campaign-frozen')).toBeVisible();
  await expect(page.getByTestId('campaign-saved-list')).toContainText(btc);
  await expect(page.getByTestId('campaign-saved-list')).toContainText(eth);

  await page.getByTestId(`campaign-start-${btc}`).click();
  await expect(page.getByTestId('campaign-decision-view')).toContainText('Admission：ELIGIBLE');
  await expect(page.getByTestId('campaign-decision-view')).toContainText('fee 0.05% · slip 0.02%');
  await expect(page.getByTestId('discovery-status')).toContainText('已完成');

  await page.getByTestId(`campaign-start-${eth}`).click();
  await expect(page.getByTestId('campaign-decision-view')).toContainText('Admission：NOT_ELIGIBLE');
  await expect(page.getByTestId('campaign-decision-view')).toContainText('precision_not_eligible');
  await expect(page.getByTestId('campaign-saved-list')).toContainText('NOT_ELIGIBLE');
  await page.getByTestId('campaign-decision-1').click();
  await expect(page.getByTestId('campaign-decision-view')).toContainText('Admission：ELIGIBLE');
  await expect(page.getByTestId('campaign-decision-view')).not.toContainText('PASS');
});

test('changed draft and unresolved preview cannot be frozen', async ({ page }) => {
  await page.goto('/?mock=1&campaignPreviewFail=eth', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('campaign-toggle').click();
  await add(page, 'a'.repeat(64), 'BTC predeclared policy.');
  await page.getByTestId('campaign-preview-button').click();
  await expect(page.getByTestId('campaign-freeze')).toBeEnabled();
  await page.getByTestId(`campaign-rationale-${btc}`).fill('Changed before freezing');
  await expect(page.getByTestId('campaign-freeze')).toBeDisabled();
  await add(page, 'd'.repeat(64), 'ETH predeclared policy.');
  await page.getByTestId('campaign-preview-button').click();
  await expect(page.getByTestId('campaign-preview')).toContainText('未通過');
  await expect(page.getByTestId('campaign-freeze')).toBeDisabled();
  await expect(page.getByTestId('campaign-saved-list')).toContainText('尚無已保存');
});

test('explains when no service-created snapshot exists', async ({ page }) => {
  await page.goto('/?mock=1&campaignEmptySnapshots=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('campaign-toggle').click();
  await expect(page.getByTestId('campaign-no-snapshots')).toBeVisible();
  await expect(page.getByTestId('campaign-add')).toBeDisabled();
});

test('starts a saved campaign after its snapshot ages out of the authoring list', async ({ page }) => {
  await page.goto('/?mock=1&campaignHideSnapshotsAfterFreeze=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('campaign-toggle').click();
  await add(page, 'a'.repeat(64), 'Policy authored before the result.');
  await page.getByTestId('campaign-preview-button').click();
  await expect(page.getByTestId('campaign-freeze')).toBeEnabled();
  await page.getByTestId('campaign-freeze').click();
  await expect(page.getByTestId('campaign-no-snapshots')).toBeVisible();
  await page.getByTestId(`campaign-start-${btc}`).click();
  await expect(page.getByTestId('campaign-decision-view')).toContainText('Admission：ELIGIBLE');
});
