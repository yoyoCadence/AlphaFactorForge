import { test, expect } from '@playwright/test';

// STRATEGY-VALIDATION-001 — an indicator parameter that cannot produce a series
// must fail visibly instead of yielding a confident zero-trade result.
//
// The audit's reproduction: 快線 MA = 2.5 reached sma(), where values[i - 2.5] is
// undefined, so every indicator value became NaN, every signal false, and the
// backtest "succeeded" with 0 trades — saveable and exportable.
//
// These specs also pin the two-layer design: min/step are input HINTS (they can
// repair 0 on blur) and the runtime validator is the enforcement (it rejects 2.5,
// which no min can repair). Rule-level coverage is in
// src/services/strategyValidation.test.ts.

test('a fractional indicator period blocks the run and names the field', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  const run = page.getByTestId('run-backtest');
  await page.getByTestId('load-sample').click();
  await expect(run).toBeEnabled({ timeout: 20_000 });
  await expect(page.getByTestId('strategy-issues')).toHaveCount(0);

  // Scoped: the chart's quick row carries the same field labels.
  const strategy = page.getByTestId('strategy-section');
  const fastMA = strategy.getByLabel('快線 MA');
  await fastMA.fill('2.5');
  // Blur: 2.5 satisfies min=1, so the input hint cannot silently repair it —
  // which is exactly why the runtime validator has to exist.
  await strategy.getByLabel('慢線 MA').click();
  await expect(fastMA).toHaveValue('2.5');

  await expect(page.getByTestId('strategy-issues')).toContainText('fastMA');
  await expect(page.getByTestId('strategy-issues')).toContainText('2.5');
  await expect(run).toBeDisabled();

  // Repairing the field restores the run, which then completes normally.
  await fastMA.fill('9');
  await expect(page.getByTestId('strategy-issues')).toHaveCount(0);
  await expect(run).toBeEnabled();
  await run.click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
});

test('a zero period is blocked until the min hint repairs it on blur', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  const run = page.getByTestId('run-backtest');
  await page.getByTestId('load-sample').click();
  await expect(run).toBeEnabled({ timeout: 20_000 });

  const strategy = page.getByTestId('strategy-section');
  const rsiPeriod = strategy.getByLabel('RSI 週期');
  await rsiPeriod.fill('0');
  await expect(page.getByTestId('strategy-issues')).toContainText('rsiPeriod');
  await expect(run).toBeDisabled();

  // Secondary protection: NumberInput clamps to min on blur.
  await strategy.getByLabel('RSI 買').click();
  await expect(rsiPeriod).toHaveValue('1');
  await expect(page.getByTestId('strategy-issues')).toHaveCount(0);
  await expect(run).toBeEnabled();
});

test('a dubious but computable combination warns without blocking the run', async ({ page }) => {
  test.setTimeout(60_000);
  await page.goto('/?mock=1', { waitUntil: 'domcontentloaded' });

  const run = page.getByTestId('run-backtest');
  await page.getByTestId('load-sample').click();
  await expect(run).toBeEnabled({ timeout: 20_000 });
  await expect(page.getByTestId('strategy-warnings')).toHaveCount(0);

  // fastMA 30 vs slowMA 21: the same combination discovery prunes from a grid.
  // It computes fine, so it stays runnable — it is a hypothesis-quality note,
  // not a malformed parameter.
  await page.getByTestId('strategy-section').getByLabel('快線 MA').fill('30');
  await expect(page.getByTestId('strategy-warnings')).toContainText('fastMA');
  await expect(page.getByTestId('strategy-issues')).toHaveCount(0);
  await expect(run).toBeEnabled();

  await run.click();
  await expect(page.getByTestId('save-result')).toBeEnabled();
});
