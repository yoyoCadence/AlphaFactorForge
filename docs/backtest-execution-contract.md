# Backtest Execution Contract

Status: adopted and implemented target semantics (maintainer direction, 2026-07-14). BUG-002 through BUG-004 cover accounting, fill policy, direction, and normalized-fraction validation.

This contract is normative for `alpha-factor-forge/src/core/backtest`. The golden suite records actual output; when an approved correction changes that output, the affected golden values must be updated with a changelog explanation.

## Core invariants

For every deterministic run:

1. Trade timestamps and prices describe the actual simulated fill, not merely the signal bar.
2. `ClosedTrade.pnl` and `pnlPct` are net of entry and exit fees; slippage is represented in fill prices.
3. After all positions are closed, `finalEquity = startEquity + sum(closedTrade.pnl)` within floating-point tolerance.
4. The final equity point is settled equity. Metrics use `startEquity` as their baseline and the settled final point as their endpoint.
5. Long and 1× short positions cannot spend more than their configured entry budget.
6. No UI percentage units, invalid negative costs, or non-finite values may silently cross the core boundary.

## Units and validation boundary

- UI/service strategy fields retain legacy percent units (`feePct: 0.05` means 0.05%, `sizePct: 100` means 100%).
- `backtestRunner` is the single conversion boundary from legacy percent units to normalized fractions.
- Core `BacktestConfig` uses normalized fractions (`feePct: 0.0005`, `sizingPct: 1`).
- Core accepts `sizingPct`, `feePct`, and `slippagePct` in `[0, 1]`. Active `stopLossPct` and `takeProfitPct` must be in `(0, 1]`; `undefined` disables that risk rule.
- Non-finite or out-of-range normalized fractions throw a `RangeError`. Core does not clamp them or duplicate UI/service legacy fallbacks.

## Entry budget and fees (BUG-002)

`sizingPct` is the fraction of current cash/equity assigned as the total entry budget, including entry fee.

For entry fill price `P`, fee rate `f`, and budget `B`:

```text
entryNotional = B / (1 + f)
entryFee      = entryNotional * f
quantity      = entryNotional / P
```

Therefore `entryNotional + entryFee = B`; a 100% position does not create negative free cash merely to pay its entry fee.

## Long accounting (BUG-002)

- Opening deducts entry notional plus entry fee from free cash.
- Mark-to-market equity is `freeCash + currentPrice * quantity`.
- Closing credits exit notional minus exit fee.
- Net trade PnL is:

```text
(exitPrice - entryPrice) * quantity - entryFee - exitFee
```

## Short accounting (BUG-002)

Phase A shorting uses an unleveraged 1× collateral model; it does not model borrow interest, funding, liquidation, or cross-margin.

- Opening reserves entry notional as collateral and charges entry fee from the same total entry budget.
- Mark-to-market equity is `freeCash + collateral + (entryPrice - currentPrice) * quantity`.
- Closing releases collateral, realizes the price difference, and charges exit fee.
- Net trade PnL is:

```text
(entryPrice - exitPrice) * quantity - entryFee - exitFee
```

For both sides, `pnlPct = netTradePnl / entryNotional`.

## Equity, metrics, and EOD settlement (BUG-002)

- The equity curve continues to contain one point per tested candle.
- If a position remains after the final candle, it is force-closed and the final equity point is replaced with settled cash/equity.
- `netReturn` and CAGR use configured `startEquity`, not the first post-action equity point.
- Per-bar return statistics and maximum drawdown include the transition from `startEquity` to the first equity point.
- An EOD force-close uses the final candle close plus normal closing-side slippage before the settled final equity point is written.

## Fill time and risk exits (BUG-003)

- `nextOpen` signals create a pending order only when another tested candle exists. That order fills at the start of the next candle, before its risk checks and equity point; trade time and holding bars use the execution candle. A final-candle signal cannot fill beyond the tested range.
- SL/TP uses a gap-aware base price and normal closing-side slippage: long SL `min(open, stop)`, long TP `max(open, target)`, short SL `max(open, stop)`, and short TP `min(open, target)` before sell-side (long) or buy-side (short) slippage.
- Without sub-bars, a candle touching both SL and TP resolves conservatively as SL first.
- A future Bar Magnifier may use actual sub-bar order but must not silently change the no-sub-bar fallback.

## Direction semantics (BUG-004)

- `long`: entry opens long; exit closes long.
- `short`: entry opens short; exit closes short.
- `both`: retain legacy reversal semantics for the current two-signal model—entry requests long and exit requests short. If the requested side differs from the current side, the engine closes the current position and opens the requested side at the same execution base with the appropriate closing/opening slippage.
- If entry and exit are both true on one bar in `both` mode, entry takes precedence and requests long. A signal requesting the already-held side does nothing.
- Close fills reverse on the signal candle close; `nextOpen` fills reverse on the following tested candle open. A final-candle signal cannot create a reversal beyond the tested range.
- A future four-signal model may replace this with explicit long-entry/long-exit/short-entry/short-exit semantics through a separately versioned change.

## Deliberate non-goals

- Leverage, liquidation, borrow interest, funding, partial fills, order-book impact, and exchange-specific fee tiers.
- Intrabar path reconstruction without sub-bar data.
- Reinterpreting historical saved results. Existing results remain historical artifacts of the engine version that produced them; explicit engine-version persistence is a separate schema task.
