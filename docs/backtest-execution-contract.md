# Backtest Execution Contract

Status: adopted target semantics (maintainer direction, 2026-07-14). Implementation is intentionally split across BUG-002 → BUG-004 so each result-changing assumption has focused tests and review.

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
- BUG-004 will make core reject invalid normalized values. It will not duplicate legacy fallback rules inside the engine.

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
- EOD fill-price slippage is handled by BUG-003; BUG-002 first makes settlement and fee accounting internally consistent with the current raw-close policy.

## Fill time and risk exits (BUG-003)

- `nextOpen` signals fill on the next candle open; trade time and holding bars use that execution candle.
- SL/TP uses a gap-aware base price and normal closing-side slippage.
- Without sub-bars, a candle touching both SL and TP resolves conservatively as SL first.
- A future Bar Magnifier may use actual sub-bar order but must not silently change the no-sub-bar fallback.

## Direction semantics (BUG-004)

- `long`: entry opens long; exit closes long.
- `short`: entry opens short; exit closes short.
- `both`: retain legacy reversal semantics for the current two-signal model—entry requests long and exit requests short, closing the opposite side before reversal.
- A future four-signal model may replace this with explicit long-entry/long-exit/short-entry/short-exit semantics through a separately versioned change.

## Deliberate non-goals

- Leverage, liquidation, borrow interest, funding, partial fills, order-book impact, and exchange-specific fee tiers.
- Intrabar path reconstruction without sub-bar data.
- Reinterpreting historical saved results. Existing results remain historical artifacts of the engine version that produced them; explicit engine-version persistence is a separate schema task.
