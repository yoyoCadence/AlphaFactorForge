import { test, expect } from '@playwright/test';

// RUNNER-UI-001b-2 — the discovery runner panel, driven by the DEV-only mock
// runner. The mock emits REAL `discovery-event-v1` wire payloads (camelCase,
// omitted optionals, explicit null score on gate failure) through the same
// subscription functions production uses, and parses them with the production
// parsers — so these specs exercise the whole chain, not a convenient shape.
//
// Rule-level coverage lives in src/tauri-client/events.test.ts (parsers,
// throttle) and src/services/discoveryRunConfig.test.ts (envelope admission).

test('starts a run, streams progress and results, and reaches a terminal status', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto('/?mock=1&discoveryStep=30', { waitUntil: 'domcontentloaded' });

  const start = page.getByTestId('discovery-start');
  await page.getByTestId('discovery-toggle').click();
  // No dataset yet: there is no dataset identity to put in the envelope.
  await expect(start).toBeDisabled();

  await page.getByTestId('load-sample').click();
  await expect(start).toBeEnabled({ timeout: 20_000 });

  // Default axis is fastMA 5..11 step 3 -> 5, 8, 11.
  await expect(page.getByTestId('discovery-combos')).toContainText('候選 3 組');

  await start.click();
  await expect(page.getByTestId('discovery-progress')).toContainText('完成 3/3', { timeout: 30_000 });
  await expect(page.getByTestId('discovery-status')).toContainText('已完成');

  // One row per candidate, newest first, and the gate-failed candidate shows no
  // score rather than a zero.
  await expect(page.getByTestId('discovery-result-0')).toBeVisible();
  await expect(page.getByTestId('discovery-result-1')).toBeVisible();
  await expect(page.getByTestId('discovery-result-2')).toBeVisible();
  await expect(page.getByTestId('discovery-result-1')).toContainText('未通過');
  await expect(page.getByTestId('discovery-result-1')).toContainText('—');
  await expect(page.getByTestId('discovery-result-0')).toContainText('通過');
  await expect(page.getByTestId('discovery-best')).toBeVisible();

  // A terminal run releases the controls: no pause/resume/cancel, and Start is
  // available again.
  await expect(page.getByTestId('discovery-pause')).toBeDisabled();
  await expect(page.getByTestId('discovery-cancel')).toBeDisabled();
  await expect(start).toBeEnabled();
  await expect(page.getByTestId('discovery-stale')).toHaveCount(0);
  await expect(page.getByTestId('discovery-error')).toHaveCount(0);
});

test('pause and resume follow the run status, and cancel ends it', async ({ page }) => {
  test.setTimeout(90_000);
  // A slow step keeps the run observably in flight between transitions.
  await page.goto('/?mock=1&discoveryStep=400', { waitUntil: 'domcontentloaded' });

  await page.getByTestId('discovery-toggle').click();
  await page.getByTestId('load-sample').click();
  await expect(page.getByTestId('discovery-start')).toBeEnabled({ timeout: 20_000 });
  await page.getByTestId('discovery-start').click();
  await expect(page.getByTestId('discovery-status')).toContainText('執行中');

  await page.getByTestId('discovery-pause').click();
  await expect(page.getByTestId('discovery-status')).toContainText('已暫停');
  // Controls mirror the status: pause is gone, resume is offered.
  await expect(page.getByTestId('discovery-pause')).toBeDisabled();
  await expect(page.getByTestId('discovery-resume')).toBeEnabled();
  await expect(page.getByTestId('discovery-start')).toBeDisabled();

  await page.getByTestId('discovery-resume').click();
  await expect(page.getByTestId('discovery-status')).toContainText('執行中');
  await expect(page.getByTestId('discovery-resume')).toBeDisabled();

  await page.getByTestId('discovery-cancel').click();
  await expect(page.getByTestId('discovery-status')).toContainText('已取消');
  await expect(page.getByTestId('discovery-cancel')).toBeDisabled();
  await expect(page.getByTestId('discovery-start')).toBeEnabled();
});

test('adopts a run that already exists instead of relying on remembered state', async ({ page }) => {
  test.setTimeout(60_000);
  // Startup recovery turns an orphaned run into `paused`; the mock pre-creates
  // exactly that state before the panel mounts.
  await page.goto('/?mock=1&discoveryRun=paused', { waitUntil: 'domcontentloaded' });

  // Adopted from getActiveRun() — the database read, not an event.
  await expect(page.getByTestId('discovery-status')).toContainText('已暫停');
  await page.getByTestId('discovery-toggle').click();
  await expect(page.getByTestId('discovery-progress')).toContainText('完成 1/4');
  // Start must stay unavailable while a non-terminal run exists, even before any
  // dataset is loaded in this window.
  await expect(page.getByTestId('discovery-start')).toBeDisabled();
  await expect(page.getByTestId('discovery-resume')).toBeEnabled();

  // Re-querying the database is always available and does not disturb the state.
  await page.getByTestId('discovery-refresh').click();
  await expect(page.getByTestId('discovery-progress')).toContainText('完成 1/4');
  await expect(page.getByTestId('discovery-status')).toContainText('已暫停');
});

test('an invalid axis is rejected in the panel before any run is created', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  await page.getByTestId('discovery-toggle').click();
  await page.getByTestId('load-sample').click();
  await expect(page.getByTestId('discovery-start')).toBeEnabled({ timeout: 20_000 });

  // A fractional bound on an integer axis: the shared admission parser rejects
  // it, so no command is called and no run appears.
  const scope = page.getByTestId('discovery-axis-key').locator('xpath=ancestor::section[1]');
  await scope.getByLabel('迄').fill('11.5');
  await page.getByTestId('discovery-start').click();

  await expect(page.getByTestId('discovery-error')).toContainText('integer axis');
  await expect(page.getByTestId('discovery-progress')).toHaveCount(0);
  await expect(page.getByTestId('discovery-status')).toHaveCount(0);
});
