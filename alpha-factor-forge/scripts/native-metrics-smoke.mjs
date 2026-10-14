// PR #172 native acceptance: real UI results cross the installed Rust event
// plugin into a separately created Tauri WebView2. No mock or injected metrics.
import assert from 'node:assert/strict';
import path from 'node:path';
import { expect } from '@playwright/test';

async function importDailyBars(page, length) {
  const candles = Array.from({ length }, (_, i) => {
    const c = 100 + i;
    return { t: Date.UTC(2024, 0, 1) + i * 86_400_000, o: c, h: c + 1, l: c - 1, c, v: 100 };
  });
  await page.locator('textarea').first().fill(JSON.stringify({
    symbol: 'NATIVE-METRICS', interval: '1d', candles,
  }));
  await page.getByRole('button', { name: '匯入 JSON', exact: true }).click();
  await expect(page.getByTestId('run-backtest')).toBeEnabled();
}

async function metricRows(page) {
  return page.getByRole('table').first().locator('tbody tr').evaluateAll(rows =>
    rows.map(row => Array.from(row.querySelectorAll('td'), cell => cell.textContent.trim())));
}

async function expectInfiniteRatios(page, columns) {
  await expect.poll(async () => {
    const rows = await metricRows(page);
    return ['Sortino', 'Calmar', 'Profit Factor'].map(label => rows.find(row => row[0] === label)?.slice(1));
  }).toEqual(Array.from({ length: 3 }, () => Array(columns).fill('∞')));
}

export async function verifyMetricsWindows(page, context, artifactDirectory, errors) {
  await importDailyBars(page, 600);
  await page.getByLabel('進場訊號').selectOption('priceAboveSlow');
  await page.getByLabel('出場訊號').selectOption('priceBelowSlow');
  await page.getByLabel('手續費 %', { exact: true }).fill('0');
  await page.getByLabel('滑價 %', { exact: true }).fill('0');
  await page.getByTestId('holdout-toggle').check();
  await page.getByTestId('run-backtest').click();
  await expect(page.getByTestId('col-樣本外')).toBeVisible();
  await expectInfiniteRatios(page, 3);
  const note = page.getByTestId('annualization-note');
  await expect(note).toContainText('樣本外約 180 天');
  await expect(note).not.toContainText('全期約');
  await expect(note).not.toContainText('樣本內約');
  const holdoutRows = await metricRows(page);
  const holdoutNote = await note.textContent();

  const childPromise = context.waitForEvent('page', { timeout: 20_000 });
  await page.getByTestId('native-popout-metrics').click();
  const child = await childPromise;
  const consoleObservations = [];
  child.on('pageerror', error => errors.push('metrics: ' + error.message));
  child.on('console', message => {
    const text = message.text();
    // Keep these pre-existing startup observations in the evidence. The locked
    // Tauri 2.11.3 ipc-protocol.js explicitly falls back to postMessage on CSP
    // rejection; the UI/event assertions below must still pass. Remote fonts
    // are the already-tracked FONT-OFFLINE-001, not a reason to widen CSP here.
    const blockedIpc = text.includes("http://ipc.localhost/plugin%3Aevent%7Clisten")
      && text.includes('Content Security Policy');
    const blockedFont = text.startsWith("Loading the stylesheet 'https://fonts.googleapis.com/css2?")
      && text.includes('Content Security Policy') && text.includes('style-src');
    const ipcFallback = text.startsWith('IPC custom protocol failed, Tauri will now use the postMessage interface instead');
    if (blockedIpc || blockedFont || ipcFallback) {
      consoleObservations.push({ type: message.type(), text, reason: blockedFont ? 'FONT-OFFLINE-001' : 'locked Tauri IPC fallback' });
    } else if (message.type() === 'error') errors.push('metrics: ' + text);
  });
  await child.waitForURL(/window=metrics/, { timeout: 20_000 });
  assert.equal(new URL(child.url()).searchParams.has('mock'), false);
  await expect(child.getByTestId('col-樣本外')).toBeVisible();
  await expectInfiniteRatios(child, 3);
  await expect.poll(() => metricRows(child)).toEqual(holdoutRows);
  await expect(child.getByTestId('annualization-note')).toHaveText(holdoutNote);
  await child.screenshot({ path: path.join(artifactDirectory, 'native-metrics-holdout.png') });

  // Invalidating the artifact must clear the child, then the next real run
  // must replace all columns and sample metadata through the event plugin.
  await page.getByTestId('holdout-toggle').uncheck();
  await expect(child.getByTestId('metrics-window-loading')).toBeVisible();
  await expect(child.getByRole('table')).toHaveCount(0);
  await page.getByTestId('run-backtest').click();
  await expectInfiniteRatios(child, 1);
  await expect(child.getByTestId('annualization-note')).toHaveCount(0);
  await expect.poll(() => metricRows(child)).toEqual(await metricRows(page));

  // A changed dataset carries the inclusive bar count, not timestamp span.
  await importDailyBars(page, 365);
  await expect(child.getByTestId('metrics-window-loading')).toBeVisible();
  await page.getByTestId('run-backtest').click();
  await expectInfiniteRatios(child, 1);
  await expect(page.getByTestId('annualization-note')).toHaveCount(0);
  await expect(child.getByTestId('annualization-note')).toHaveCount(0);
  const fullYearRows = await metricRows(page);
  await expect.poll(() => metricRows(child)).toEqual(fullYearRows);

  // Reinitialize the child listener/ready handshake, then focus the existing
  // window again. This covers listener recovery; OS close/reopen is separate.
  await child.reload();
  await expect.poll(() => metricRows(child)).toEqual(fullYearRows);
  await expectInfiniteRatios(child, 1);
  await expect(child.getByTestId('annualization-note')).toHaveCount(0);
  await page.getByTestId('native-popout-metrics').click();
  await expect(page.getByTestId('status-message')).toContainText('已開啟原生績效視窗');
  assert.equal(context.pages().filter(p => /window=metrics/.test(p.url())).length, 1);
  await expect.poll(() => metricRows(child)).toEqual(fullYearRows);
  await child.screenshot({ path: path.join(artifactDirectory, 'native-metrics-full-year.png') });
  await expect(page.getByTestId('status-error')).toHaveCount(0);
  return {
    status: 'passed', childUrl: child.url(), holdoutRows, holdoutNote, fullYearRows,
    nonFiniteColumns: 3, invalidationCleared: true, updated: true,
    listenerReloaded: true, existingWindowReused: true, osCloseReopen: 'not exercised',
    consoleObservations,
  };
}
