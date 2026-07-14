# Backtest Engine / Legacy Parity Report

> Status: parity record plus maintainer decisions. TEST-002 created the observation-only baseline; the maintainer adopted `backtest-execution-contract.md` on 2026-07-14. Result-changing corrections are implemented separately in BUG-002 → BUG-004 with intentional golden updates.

## Scope and baseline

The current engine is `alpha-factor-forge/src/core/backtest/index.ts`. The comparison target is the legacy `runBacktestCore()` in `AlphaFactorForge.dc.html:1059-1155`.

`backtest.golden.test.ts` locks the current engine with `makeSampleCandles({ seed: 42, count: 300 })` and fixed entry/exit signals (entry every 20 bars from index 5; exit every 20 bars from index 12). It covers:

1. long, close fill, no SL/TP;
2. long, next-open fill;
3. both, close fill, SL 2% / TP 4%;
4. short, close fill, no SL/TP.

Each configuration fixes the trade count, first/last trade timestamps and prices, net return, maximum drawdown, and Sharpe ratio. Five boundary cases cover same-bar entry/exit signals, a one-candle dataset, `from === to`, UI `sizePct = 0`, and negative UI fee/slippage. The final two inputs are normalized through the product-path `toExecCostFractions()` before entering the core engine (`backtestRunner.ts:38-54`); raw `runBacktest()` does not accept legacy percentage units.

## Behaviour comparison

### 1. `nextOpen` signal exit timestamp

- **Current engine:** `priceAt()` selects candle `i + 1` and its opening price (`index.ts:93-97`), but `close()` records `exitTime: candles[i].t` and holding bars from `i` (`index.ts:100-124`). Entry fills do use the returned execution index/time (`index.ts:129-138`). A next-open exit therefore has a next-bar price but a signal-bar timestamp.
- **Legacy:** `baseAt()` returns the next candle's price, index, and timestamp (`AlphaFactorForge.dc.html:1080`). Signal exits pass all three values into `recordClose()` (`:1115-1124`), which records the supplied execution timestamp and index (`:1093-1100`).
- **Recommendation for maintainer decision:** Candidate fix: carry the execution index/time returned by `priceAt()` into `close()`. If the current timestamp convention is intentional, document it as “signal time” and add a separate execution-time field before persistence.
- **Impact:** Trade-marker placement, CSV timestamps, holding bars, exposure, and any time-based trade analysis. Prices and trade count need not change, but several persisted/report fields would.

### 2. SL/TP exit slippage and gap policy

- **Current engine:** Risk exits call `close()` with the exact stop or target level (`index.ts:144-154`); they do not pass that level through the slippage function. The fill also ignores a candle opening beyond the trigger.
- **Legacy:** `approxRes()` uses a gap-aware open/trigger choice (`AlphaFactorForge.dc.html:1082`), then applies closing-side slippage with `fillPx()` before `recordClose()` (`:1111-1112`). Bar Magnifier exits also pass through that slippage step.
- **Recommendation for maintainer decision:** Decide gap-fill policy and risk-exit slippage as two explicit assumptions. Candidate parity fix: reproduce the legacy gap-aware base price, then apply normal closing-side slippage. Do not combine this with unrelated engine cleanup.
- **Impact:** Every SL/TP trade price and PnL may change; downstream metrics, sweep rankings, saved summaries, and exported reports can all move.

### 3. SL and TP both touched within one bar

- **Current engine:** The `if ... else if` ordering always chooses SL before TP (`index.ts:150-154`).
- **Legacy:** Without Bar Magnifier, `approxRes()` also checks SL first (`AlphaFactorForge.dc.html:1082`). With sub-bars available, a sub-bar touching both selects whichever threshold is closer to that sub-bar's open (`:1085-1091`).
- **Recommendation for maintainer decision:** Keeping SL-first as the documented conservative fallback is a reasonable candidate while no sub-bar data exists. A future Bar Magnifier should be a separate feature with its own ordering tests rather than silently changing this fallback.
- **Impact:** Only ambiguous bars that touch both thresholds, but these can materially affect PnL and parameter rankings.

### 4. Short cash and collateral accounting

- **Current engine (BUG-002):** Entry budget explicitly includes notional plus its fee (`index.ts:129-140`). A short reserves 1× entry notional as collateral; mark-to-market equity is free cash + collateral + unrealized PnL (`:170-178`). Closing releases collateral and emits fee-inclusive PnL/PnL% (`:102-125`).
- **Legacy:** A short entry credits sale proceeds minus entry fee and holds a negative position (`AlphaFactorForge.dc.html:1103`); equity is always `cash + pos * close` (`:1127`). Closing debits buy-to-cover price plus fee (`:1093-1101`). Quantity uses the full notional before fee, and the round-trip PnL explicitly subtracts both fees (`:1094-1100`).
- **Decision:** Adopt the explicit unleveraged 1× collateral model with fee-inclusive entry budgeting and require `finalEquity = startEquity + sum(trade.pnl)`. BUG-002 implements this with hand-calculated long/short/partial-size/multi-trade tests. Borrow interest, funding, leverage, and liquidation remain out of Phase A scope.
- **Impact:** Short quantity, per-trade PnL, equity curve, net return, drawdown, and all PnL-derived statistics. This is a high-blast-radius behaviour change.

### 5. End-of-data forced close

- **Current engine (BUG-002):** The position is still force-closed at the final candle's raw close pending BUG-003 (`index.ts:181-182`), but the final equity point is now replaced with settled cash including the exit fee (`:183-185`). Metrics receive configured `startEquity` and use the settled endpoint (`:187-193`; `metrics/index.ts:86-110`).
- **Legacy:** The final candle close is passed through closing-side slippage before settlement (`AlphaFactorForge.dc.html:1129`). Its headline net return is calculated from post-settlement cash (`:1130-1133`), although its previously collected equity series is also pre-settlement.
- **Decision:** Metrics and the final equity point must use settled equity; BUG-002 implements that accounting decision. EOD closing-side slippage remains an approved BUG-003 fill-policy correction so it can be reviewed with all other exit fills.
- **Impact:** Any run ending with an open position; trade exit price, net return, final equity, fees, and consistency between the trade list and metrics.

### 6. `direction: both` semantics

- **Current engine:** When flat, `both` follows the same branch as `long` and opens only a long position; an exit signal closes that position (`index.ts:157-165`). There is no reversal into short.
- **Legacy:** `both` is explicitly a reverse system: entry signal requests long, exit signal requests short, and an opposing position is closed before the new one opens (`AlphaFactorForge.dc.html:1121-1125`).
- **Recommendation for maintainer decision:** Decide whether `both` means legacy reversal or merely permits either side when a future signal model specifies one. Candidate actions are either to restore legacy reversal semantics or rename/split the mode so its current limitation is explicit.
- **Impact:** Very high for every `both` strategy: trade side, count, timestamps, equity, metrics, sweep outcomes, and saved strategy meaning.

### 7. UI input normalization boundary

- **Current engine:** `runBacktest()` clamps fractional `sizingPct` to `[0, 1]` while opening (`index.ts:132`) but does not implement the legacy `sizePct = 0` fallback and does not clamp negative fractional costs (`:83-84`). The product runner converts percentage units, maps zero size to 100%, floors small size at 1%, and clamps negative fee/slippage to zero (`backtestRunner.ts:38-54`).
- **Legacy:** The same percentage-unit fallback and clamps live inside `runBacktestCore()` (`AlphaFactorForge.dc.html:1065-1067`).
- **Recommendation for maintainer decision:** Keep one documented normalization boundary and prevent UI/DSL callers from bypassing it. If raw core validation is desired, add it as a separate contract change rather than duplicating percentage-unit rules in the pure engine.
- **Impact:** Direct core/worker/DSL callers only when they bypass `runParamsBacktest()`; malformed inputs could otherwise create no trades or unintended rebates.

## Decision summary

The maintainer adopted the target semantics in `backtest-execution-contract.md` on 2026-07-14. BUG-002 resolves the accounting and settled-equity portions in rows 4–5; BUG-003 will resolve rows 1–3 plus EOD slippage; BUG-004 will resolve rows 6–7. Until each focused correction lands, its golden values continue to preserve the corresponding current output.

## Follow-up task template if a behaviour is approved for correction

```markdown
## BUG-00x — <one behaviour only>

- Decision reference: <maintainer, date, parity-report section>
- Approved semantics: <precise fill/time/accounting rule>
- Current golden expectations affected: <case names + fields>
- Files in scope: <exact product/test/doc files>
- Non-goals: <explicit adjacent behaviours not changed>
- Migration/compatibility: <effect on saved results and hashes, or none>

### Implementation plan
1. Add a failing focused correctness test with a hand-calculated fixture.
2. Change only the approved behaviour.
3. Update affected golden values with a before/after explanation.
4. Update this report row from “undecided” to the recorded decision.

### Acceptance criteria
- [ ] Focused correctness test passes.
- [ ] Unrelated golden configurations remain byte-for-byte unchanged.
- [ ] Full typecheck/test/build verification passes.
- [ ] Changelog identifies the result-changing assumption.
```

