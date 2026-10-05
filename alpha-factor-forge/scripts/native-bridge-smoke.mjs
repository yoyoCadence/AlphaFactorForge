// CI-TAURI-SMOKE-001b: attach only to the separately launched, isolated WebView2.
// No browser is downloaded/launched, no mock is installed, and product code is unchanged.
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from '@playwright/test';

const [endpoint, artifactDirectory] = process.argv.slice(2);
assert(endpoint && artifactDirectory, 'usage: node native-bridge-smoke.mjs <CDP endpoint> <artifacts>');
const address = new URL(endpoint);
assert(address.protocol === 'http:' && address.hostname === '127.0.0.1' && address.port,
  'the harness only accepts its loopback CDP endpoint');
await mkdir(artifactDirectory, { recursive: true });

const browser = await chromium.connectOverCDP(endpoint, { timeout: 20_000 });
let page;
const errors = [];
try {
  const context = browser.contexts()[0];
  assert(context, 'WebView2 must expose its existing context');
  page = context.pages()[0] ?? await context.waitForEvent('page', { timeout: 20_000 });
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') errors.push(message.text());
  });
  await page.waitForURL(/^(https?:\/\/tauri\.localhost|tauri:\/\/localhost)(?:[/?#]|$)/,
    { timeout: 20_000 });
  assert.equal(new URL(page.url()).searchParams.has('mock'), false, 'native smoke must not use ?mock');
  await page.locator('.app-header-status').filter({ hasText: 'database already initialized at startup' })
    .waitFor({ state: 'visible', timeout: 20_000 });
  assert.match(await page.locator('.app-header').innerText(), /ALPHAFACTORFORGE/);
  assert.equal(await page.locator('vite-error-overlay').count(), 0);

  // These calls exercise the installed native bridge, registered Rust handlers
  // and actual SQLite connection. A missing/wrong command or argument fails.
  const commands = await page.evaluate(async () => {
    const bridge = window.__TAURI_INTERNALS__;
    if (!bridge || typeof bridge.invoke !== 'function') throw new Error('native invoke is missing');
    let rejectedArguments;
    try {
      await bridge.invoke('get_candles', { dataset_id: 987654321, from: 0, to: 1 });
    } catch (error) {
      rejectedArguments = String(error);
    }
    return {
      workspace: await bridge.invoke('get_workspace_info'),
      datasets: await bridge.invoke('get_datasets'),
      candles: await bridge.invoke('get_candles', { datasetId: 987654321, from: 0, to: 1 }),
      rejectedArguments,
    };
  });
  assert.equal(commands.workspace.hostMode, 'desktop-embedded');
  assert.equal(commands.workspace.commandProtocolVersion, 'research-command-v1');
  assert.equal(commands.workspace.eventProtocolVersion, 'research-event-v1');
  assert.equal(typeof commands.workspace.workspaceId, 'string');
  assert(commands.workspace.workspaceId.length > 0 && commands.workspace.epoch > 0);
  assert.deepEqual(commands.datasets, [], 'a new isolated workspace must have no datasets');
  assert.deepEqual(commands.candles, [], 'a nonexistent dataset must return no candles');
  assert.match(commands.rejectedArguments, /datasetId/, 'Rust must reject the wrong invoke argument key');

  const nonce = randomUUID();
  const event = await page.evaluate(async ({ nonce }) => {
    // Match the installed @tauri-apps/api/event wire protocol. withGlobalTauri
    // is deliberately not enabled just for testing; no production test hook.
    const bridge = window.__TAURI_INTERNALS__;
    const name = `aff:native-smoke-${nonce}`;
    let receive;
    const received = new Promise(resolve => { receive = resolve; });
    const callbackId = bridge.transformCallback(receive);
    let eventId;
    let timer;
    try {
      eventId = await bridge.invoke('plugin:event|listen', {
        event: name, target: { kind: 'Any' }, handler: callbackId,
      });
      await bridge.invoke('plugin:event|emit_to', {
        target: { kind: 'AnyLabel', label: 'main' }, event: name,
        payload: { version: 'native-bridge-smoke-v1', nonce, nested: { value: 42 } },
      });
      return await Promise.race([received, new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('native event callback did not arrive')), 5_000);
      })]);
    } finally {
      clearTimeout(timer);
      try {
        if (eventId !== undefined) {
          window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(name, eventId);
          await bridge.invoke('plugin:event|unlisten', { event: name, eventId });
        }
      } finally {
        bridge.unregisterCallback(callbackId);
      }
    }
  }, { nonce });
  assert.equal(event.event, `aff:native-smoke-${nonce}`);
  assert.deepEqual(event.payload, { version: 'native-bridge-smoke-v1', nonce, nested: { value: 42 } });
  assert.equal(Number.isSafeInteger(event.id), true);
  assert.deepEqual(errors, [], 'native page must have no runtime/console errors during the smoke');

  await page.screenshot({ path: path.join(artifactDirectory, 'native-success.png') });
  await writeFile(path.join(artifactDirectory, 'native-result.json'), JSON.stringify({
    status: 'passed', url: page.url(), commands, event, errors,
  }, null, 2) + '\n');
  console.log('Native smoke passed: rendered app, SQLite invokes and Rust event-plugin round trip.');
} catch (error) {
  if (page) await page.screenshot({ path: path.join(artifactDirectory, 'native-failure.png') }).catch(() => {});
  await writeFile(path.join(artifactDirectory, 'native-failure.json'), JSON.stringify({
    error: String(error), url: page?.url(), errors,
  }, null, 2) + '\n');
  throw error;
} finally {
  // For a CDP connection this disconnects Playwright; the launcher owns teardown.
  await browser.close();
}
