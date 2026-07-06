import { readFileSync } from 'node:fs';
import { test, expect } from '@playwright/test';

// Slice 7-2 export UI flow in browser/mock mode. The mock data boundary turns
// saveReport into a Blob download; real filesystem writes stay in the Tauri
// command and Rust tests.
test('exports the latest backtest as JSON and trades CSV', async ({ page }) => {
  await page.goto('/?mock=1');
  await page.getByTestId('load-sample').click();
  await page.getByTestId('run-backtest').click();

  await expect(page.getByTestId('export-json')).toBeVisible();

  const [jsonDownload] = await Promise.all([
    page.waitForEvent('download'),
    page.getByTestId('export-json').click(),
  ]);
  expect(jsonDownload.suggestedFilename()).toMatch(/^AlphaFactorForge_SAMPLE_1h_\d{4}-\d{2}-\d{2}\.json$/);
  const jsonPath = await jsonDownload.path();
  expect(jsonPath).toBeTruthy();
  const report = JSON.parse(readFileSync(jsonPath!, 'utf8'));
  expect(report.app).toBe('AlphaFactorForge');
  expect(report.schema).toBe(1);
  expect(report.dataset.symbol).toBe('SAMPLE');
  expect(report.metrics.tradeCount).toBe(report.tradeCount);

  const [csvDownload] = await Promise.all([
    page.waitForEvent('download'),
    page.getByTestId('export-csv').click(),
  ]);
  expect(csvDownload.suggestedFilename()).toMatch(/^AlphaFactorForge_SAMPLE_1h_\d{4}-\d{2}-\d{2}\.csv$/);
  const csvPath = await csvDownload.path();
  expect(csvPath).toBeTruthy();
  const csv = readFileSync(csvPath!, 'utf8');
  expect(csv.split('\n')[0]).toBe('entry_time,entry_time_iso,exit_time,exit_time_iso,side,entry_price,exit_price,pnl,pnl_pct,bars');
});
