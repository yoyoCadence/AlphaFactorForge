// CAMPAIGN-LISTING-001: live Binance data + the real native UI/SQLite bridge.
// The PowerShell launcher owns the app, loopback endpoint and disposable data.
// One frozen default strategy exercises admission/exploration, never research PASS.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium, expect } from '@playwright/test';

const [endpoint, artifacts, inputFile, phase] = process.argv.slice(2);
assert(endpoint && artifacts && inputFile && ['initial', 'reopen'].includes(phase),
  'usage: node native-campaign-smoke.mjs <CDP endpoint> <artifacts> <input JSON> <initial|reopen>');
const address = new URL(endpoint);
assert(address.protocol === 'http:' && address.hostname === '127.0.0.1' && address.port,
  'only the separately owned loopback endpoint is allowed');
const parseFile = async file => JSON.parse((await readFile(file, 'utf8')).replace(/^\uFEFF/, ''));
const input = await parseFile(inputFile);
const instrumentId = 'crypto:binance:BTCUSDT';
const { oldFetch, newFetch, receipt } = input;
assert.equal(receipt.instrumentId, instrumentId);
const browser = await chromium.connectOverCDP(endpoint, { timeout: 20_000 });
let page;
const errors = [];

async function invoke(command, args = {}) {
  return page.evaluate(({ command, args }) => {
    const bridge = window.__TAURI_INTERNALS__;
    if (!bridge || typeof bridge.invoke !== 'function') throw new Error('native bridge is missing');
    return bridge.invoke(command, args);
  }, { command, args });
}

async function checkPinnedData() {
  const instruments = await invoke('list_market_instruments');
  const snapshots = await invoke('list_market_snapshots');
  const newInstrument = instruments.find(row => row.id === receipt.rowId);
  assert(newInstrument, 'latest instrument revision must remain readable');
  // This API deliberately lists latest revisions only. The old snapshot's
  // immutable binding and backend preview below prove its retained refusal.
  assert.equal(instruments.some(row => row.id === oldFetch.instrumentRowId), false);
  assert.equal(newInstrument.listedFrom, input.expectedListing);
  assert.equal(newInstrument.contentHash, receipt.contentHash);
  assert.equal(newInstrument.revision, receipt.revision);
  assert.equal(newInstrument.sourceCapabilities.listingRegistration.rawSha256, receipt.rawSha256);
  const sourceBytes = await readFile(input.sourceFile);
  assert.equal(createHash('sha256').update(sourceBytes).digest('hex'), receipt.rawSha256);
  assert.equal(newInstrument.sourceCapabilities.listingRegistration.rawByteLen, sourceBytes.length);
  assert.equal(newInstrument.sourceCapabilities.listingRegistration.rawContent, sourceBytes.toString('utf8'),
    'stored source evidence must preserve the supplied original bytes');
  for (const report of [oldFetch, newFetch]) {
    const option = snapshots.find(item => item.snapshot.snapshotId === report.snapshotId);
    assert(option, `pinned snapshot ${report.snapshotId} must remain readable`);
    for (const key of ['id', 'snapshotId', 'instrumentRowId', 'instrumentId', 'datasetId',
      'datasetHash', 'version', 'interval', 'calendarId', 'kind', 'status', 'asOf', 'createdAt',
      'coverage', 'priceBasis', 'costProfileVersion', 'corporateActionVersion']) {
      assert.deepEqual(option.snapshot[key], report[key], `old/new snapshot ${key} was rewritten`);
    }
    assert.equal(option.dataset.candleCount, 744);
    assert.equal(option.qualificationEligible, true);
  }
  assert.notEqual(oldFetch.snapshotId, newFetch.snapshotId);
  assert.notEqual(oldFetch.instrumentRowId, newFetch.instrumentRowId);
  assert.equal(oldFetch.datasetHash, newFetch.datasetHash);
  return { newInstrument, snapshots };
}

async function readRun(campaignId, runId) {
  const campaigns = await invoke('list_research_campaigns');
  const campaign = campaigns.find(row => row.campaignId === campaignId);
  assert(campaign && campaign.status === 'valid', 'saved campaign must validate on read');
  assert(campaign.runs.some(run => run.runId === runId && run.instrumentId === instrumentId),
    'stored campaign must retain its run');
  const admission = await invoke('get_campaign_admission', { runId });
  assert.equal(admission.campaignId, campaignId);
  assert.equal(admission.instrumentId, instrumentId);
  assert.equal(admission.status, 'ELIGIBLE', JSON.stringify(admission.report.reasons));
  assert.equal(admission.report.snapshot.snapshotId, newFetch.snapshotId);
  assert.equal(admission.report.snapshot.instrumentRowId, receipt.rowId);
  assert.equal(admission.report.snapshot.datasetHash, newFetch.datasetHash);
  assert.equal(admission.feePct, 0.05);
  assert.equal(admission.slipPct, 0.02);
  const progress = await invoke('get_discovery_progress', { runId });
  assert.equal(progress.status, 'completed', progress.errorMessage ?? 'run did not complete');
  assert.equal(progress.counts.totalCandidates, 1);
  assert.equal(progress.counts.completedCandidates, 1);
  assert.equal(progress.counts.failedCandidates, 0);
  const attempts = await invoke('list_research_attempts', { filter: { discoveryRunId: runId, limit: 200 } });
  assert.equal(attempts.length, 1, 'one frozen candidate must leave one immutable attempt');
  assert.equal(attempts[0].status, 'completed');
  assert.equal(attempts[0].inputFingerprint.datasetHash, newFetch.datasetHash);
  assert.equal(attempts[0].inputFingerprint.rootSeed, 20261007);
  assert(attempts[0].resultArtifact, 'completed research attempt must have an immutable artifact');
  const detail = await invoke('get_research_attempt', { id: attempts[0].id });
  assert.equal(detail.resultError, null, 'artifact checksum/read must succeed');
  assert(detail.result && detail.hypothesis, 'hypothesis and result must remain readable');
  assert.deepEqual(detail.attempt, attempts[0]);
  return { campaign, admission, progress, attempts, detail };
}

async function showStoredRun(campaignId, runId, attemptId) {
  await page.getByTestId('campaign-refresh').click();
  const saved = page.getByTestId(`campaign-saved-${campaignId}`);
  await expect(saved.getByTestId('campaign-row-status')).toContainText('宣告有效');
  await saved.getByTestId(`campaign-decision-${runId}`).click();
  await expect(page.getByTestId('campaign-decision-view')).toContainText('Admission：ELIGIBLE');
  await expect(page.getByTestId('campaign-decision-view')).not.toContainText('PASS');
  await page.getByTestId('research-history-toggle').click();
  const attempt = page.getByTestId(`research-history-attempt-${attemptId}`);
  await expect(attempt).toHaveAttribute('data-status', 'completed');
  await attempt.click();
  await expect(page.getByTestId('research-history-detail')).toBeVisible();
  await expect(page.getByTestId('research-history-result')).toBeVisible();
  const hashBody = newFetch.datasetHash.split(':')[1];
  await expect(page.getByTestId('research-history-fingerprints')).toContainText(`${hashBody.slice(0, 8)}…${hashBody.slice(-4)}`);
  assert.equal(await page.getByTestId('research-history-detail-error').count(), 0);
  assert.equal(await page.getByTestId('research-history-result-error').count(), 0);
  assert.equal(await page.getByTestId('campaign-error').count(), 0);
}

try {
  const context = browser.contexts()[0];
  assert(context, 'WebView2 existing context is required');
  page = context.pages()[0] ?? await context.waitForEvent('page', { timeout: 20_000 });
  page.setDefaultTimeout(20_000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  await page.waitForURL(/^(https?:\/\/tauri\.localhost|tauri:\/\/localhost)(?:[/?#]|$)/,
    { timeout: 20_000 });
  assert.equal(new URL(page.url()).searchParams.has('mock'), false);
  await page.locator('.app-header-status').filter({ hasText: 'database already initialized at startup' })
    .waitFor({ state: 'visible', timeout: 20_000 });
  assert.equal(await page.locator('vite-error-overlay').count(), 0);
  const workspace = await invoke('get_workspace_info');
  assert.equal(workspace.hostMode, 'desktop-embedded');
  assert.equal(workspace.commandProtocolVersion, 'research-command-v1');
  const pinnedData = await checkPinnedData();
  await page.getByTestId('campaign-toggle').click();
  await expect(page.getByTestId('campaign-snapshot-choice').locator(`option[value="${newFetch.snapshotId}"]`))
    .toHaveCount(1);
  let result;
  if (phase === 'initial') {
    assert.deepEqual(await invoke('list_research_campaigns'), [], 'fresh workspace must have no campaigns');
    await page.getByTestId('campaign-snapshot-choice').selectOption(oldFetch.snapshotId);
    await page.getByTestId('campaign-add').click();
    await expect(page.getByTestId('campaign-error')).toContainText(/缺少.*(?:instrument 修訂|上市起點)/);
    assert.equal(await page.getByTestId(`campaign-draft-${instrumentId}`).count(), 0,
      'the unchanged old snapshot must remain rejected');
    await page.getByTestId('campaign-snapshot-choice').selectOption(newFetch.snapshotId);
    await page.getByTestId('campaign-add').click();
    await expect(page.getByTestId(`campaign-draft-${instrumentId}`)).toBeVisible();
    for (const [field, value] of Object.entries({
      minimumTotalBars: '744', minimumTrainBars: '100', foldValidationBars: '50', foldCount: '2',
    })) {
      await page.getByTestId(`campaign-${field}-${instrumentId}`).fill(value);
    }
    await page.getByTestId(`campaign-rationale-${instrumentId}`).fill(
      'One official January 2025 hourly BTC snapshot, one default candidate and two Train-only folds; native admission/storage smoke, no tuning or research PASS.');
    for (const [field, value] of Object.entries({ alphaPpm: '50000', maxRelativeStandardErrorPpm: '200000',
      bootstrapSamples: '100000', maxBootstrapSamples: '200000' })) {
      await page.getByTestId(`campaign-${field}`).fill(value);
    }
    await page.getByTestId('campaign-seed').fill('20261007');
    await page.getByTestId('campaign-holding').fill('0');
    await expect(page.getByTestId('campaign-freeze')).toBeDisabled();
    await page.getByTestId('campaign-preview-button').click();
    await expect(page.getByTestId('campaign-preview')).toContainText('已解析，744 bars');
    const previewId = (await page.getByTestId('campaign-preview').innerText()).match(/Campaign ID：([0-9a-f]{64})/)?.[1];
    assert(previewId, 'backend preview must expose its derived campaign ID');
    await expect(page.getByTestId('campaign-freeze')).toBeEnabled();
    await page.getByTestId('campaign-freeze').click();
    await expect(page.getByTestId('campaign-frozen')).toBeVisible();
    const campaigns = await invoke('list_research_campaigns');
    assert.equal(campaigns.length, 1);
    const campaignId = campaigns[0].campaignId;
    assert.equal(campaignId, previewId, 'the saved campaign ID must equal the exact successful preview');
    assert.equal(campaigns[0].status, 'valid');
    await expect(page.getByTestId('campaign-frozen')).toContainText(campaignId);
    const declaration = JSON.parse(campaigns[0].rawDocumentJson);
    assert.equal(declaration.instruments.length, 1);
    assert.equal(declaration.instruments[0].snapshotId, newFetch.snapshotId);
    assert.equal(declaration.instruments[0].listedAtMs, input.expectedListing);
    const oldDeclaration = structuredClone(declaration);
    oldDeclaration.instruments[0].snapshotId = oldFetch.snapshotId;
    const oldPreview = await invoke('preview_research_campaign', { declaration: oldDeclaration });
    assert.equal(oldPreview.instruments[0].resolved, false, 'backend must still reject the old unknown listing');
    assert.match(oldPreview.instruments[0].error, /listing/i);
    await page.getByTestId(`campaign-start-${instrumentId}`).click();
    await expect(page.getByTestId('campaign-message')).toContainText('探索任務 #');
    const started = await invoke('list_research_campaigns');
    const startedCampaign = started.find(row => row.campaignId === campaignId);
    assert.equal(startedCampaign.runs.length, 1);
    const runId = startedCampaign.runs[0].runId;
    await expect.poll(async () => (await invoke('get_discovery_progress', { runId })).status,
      { timeout: 90_000, intervals: [100, 250, 500, 1000], message: 'one frozen native candidate must finish' })
      .toBe('completed');
    const run = await readRun(campaignId, runId);
    await showStoredRun(campaignId, runId, run.attempts[0].id);
    result = { status: 'passed', phase, scope: input.scope, workspace, pinnedData, campaignId, runId, oldPreview, ...run, errors };
  } else {
    const previous = await parseFile(path.join(artifacts, 'initial-result.json'));
    assert.equal(workspace.workspaceId, previous.workspace.workspaceId);
    assert(workspace.epoch > previous.workspace.epoch, 'process reopen must acquire a new ownership epoch');
    const run = await readRun(previous.campaignId, previous.runId);
    for (const key of ['campaign', 'admission', 'progress', 'attempts', 'detail']) {
      assert.deepEqual(run[key], previous[key], `reopen changed immutable saved ${key}`);
    }
    await showStoredRun(previous.campaignId, previous.runId, run.attempts[0].id);
    result = { status: 'passed', phase, scope: input.scope, workspace, pinnedData,
      campaignId: previous.campaignId, runId: previous.runId, ...run, errors };
  }
  assert.deepEqual(errors, [], 'native page must have no runtime/console errors');
  await page.screenshot({ path: path.join(artifacts, `${phase}-success.png`), fullPage: true });
  await writeFile(path.join(artifacts, `${phase}-result.json`), JSON.stringify(result, null, 2) + '\n');
  console.log(`Native campaign ${phase} passed: BTC admission, one candidate, immutable history; no research PASS claim.`);
} catch (error) {
  if (page) await page.screenshot({ path: path.join(artifacts, `${phase}-failure.png`), fullPage: true }).catch(() => {});
  await writeFile(path.join(artifacts, `${phase}-failure.json`), JSON.stringify({
    error: String(error), url: page?.url(), phase, errors,
  }, null, 2) + '\n');
  throw error;
} finally {
  await browser.close(); // disconnect CDP; the PowerShell launcher owns teardown
}
