import { test, expect } from '@playwright/test';

// P04b — the host-mode badge and the two switches in the discovery panel,
// driven by the DEV-only mock host. The mock goes through 'switching' with a
// delay, announces the change on `runtime://host` with the real payload shape
// (parsed by the production parser), and the panel re-reads the active run
// snapshot on every announcement — the reconnect the runtime contract fixes.
//
// What this cannot prove: the real hand-over (lock release, service launch,
// proxying). That is Rust-tested (`discovery_runner/tests/host.rs`) and smoked
// natively; this spec pins the window's side of the contract.

test('starts embedded, hands over to the background service, and takes it back', async ({ page }) => {
  // First navigation of a run: the dev server may still be pre-bundling.
  test.setTimeout(90_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  const badge = page.getByTestId('host-mode');
  const toggle = page.getByTestId('host-mode-toggle');
  await expect(badge).toContainText('桌面內建');
  await expect(badge).toHaveAttribute('data-host-mode', 'desktop-embedded');
  await expect(toggle).toHaveText('在背景繼續');

  await toggle.click();
  await expect(badge).toContainText('背景 service');
  await expect(badge).toHaveAttribute('data-host-mode', 'desktop-connect');
  await expect(toggle).toHaveText('收回桌面');
  await expect(page.getByText('關閉視窗不會中斷進行中的探索')).toBeVisible();

  await toggle.click();
  await expect(badge).toHaveAttribute('data-host-mode', 'desktop-embedded');
  await expect(toggle).toHaveText('在背景繼續');
  await expect(page.getByText('研究回到桌面內建模式')).toBeVisible();
});

test('a desktop that opens against a running service starts connected and adopts its run', async ({ page }) => {
  // The mock service already has a paused run, exactly as a service that
  // drained to a checkpoint would; the panel must adopt it, not start blank.
  await page.goto('/?mock=1&hostMode=desktop-connect&discoveryRun=paused', { waitUntil: 'domcontentloaded' });
  await expect(page.getByTestId('host-mode')).toHaveAttribute('data-host-mode', 'desktop-connect');
  await expect(page.getByTestId('host-mode-toggle')).toHaveText('收回桌面');
  await expect(page.getByTestId('discovery-status')).toContainText('已暫停');

  // Proxied lifecycle: resume the service's run from the connected desktop.
  await page.getByTestId('discovery-toggle').click();
  await page.getByTestId('discovery-resume').click();
  await expect(page.getByTestId('discovery-status')).toContainText('已完成', { timeout: 30_000 });
});

test('a failed switch reports the mode the desktop fell back to and keeps the controls usable', async ({ page }) => {
  await page.goto('/?mock=1&hostSwitchFail=1', { waitUntil: 'domcontentloaded' });
  const badge = page.getByTestId('host-mode');
  const toggle = page.getByTestId('host-mode-toggle');
  // The panel's error line lives in its expanded body.
  await page.getByTestId('discovery-toggle').click();
  await toggle.click();
  await expect(page.getByTestId('discovery-error')).toContainText('the desktop is now desktop-embedded');
  await expect(badge).toHaveAttribute('data-host-mode', 'desktop-embedded');
  await expect(toggle).toBeEnabled();
  // The failure was one-shot; the next attempt succeeds.
  await toggle.click();
  await expect(badge).toHaveAttribute('data-host-mode', 'desktop-connect');
});

test('a missing terminal event is recovered by re-reading the run, not by waiting for it', async ({ page }) => {
  // The mock withholds the Done event and posts runtime://resnapshot instead,
  // as the connect-mode bridge does for a ledger gap. The panel must land on
  // the terminal status through the re-read.
  await page.goto('/?mock=1&hostMode=desktop-connect&discoveryStep=30&discoveryDropDone=1', { waitUntil: 'domcontentloaded' });
  await page.getByTestId('discovery-toggle').click();
  await page.getByTestId('load-sample').click();
  const start = page.getByTestId('discovery-start');
  await expect(start).toBeEnabled({ timeout: 20_000 });
  await start.click();
  await expect(page.getByTestId('discovery-progress')).toContainText('完成 3/3', { timeout: 30_000 });
  await expect(page.getByTestId('discovery-status')).toContainText('已完成', { timeout: 30_000 });
  await expect(page.getByText('已重新讀取進度')).toBeVisible();
  // Terminal: the controls agree with the database, not with the last event.
  await expect(page.getByTestId('discovery-cancel')).toBeDisabled();
  await expect(start).toBeEnabled();
});
