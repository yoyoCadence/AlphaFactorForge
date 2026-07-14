# Backtest Engine / Legacy Parity Report

> Status: parity record plus maintainer decisions. TEST-002 created the observation-only baseline; the maintainer adopted `backtest-execution-contract.md` on 2026-07-14. BUG-002 and BUG-003 now implement the accounting and fill-policy decisions with intentional golden updates; BUG-004 remains pending.

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

- **Current engine (BUG-003):** A signal creates a pending order only when another tested candle exists (`index.ts:184-188`). The order fills at the start of the next candle using that candle's open, index, and timestamp before risk checks or mark-to-market equity (`:143-158`). A final-bar signal therefore cannot create an out-of-range fill.
- **Legacy:** `baseAt()` returns the next candle's price, index, and timestamp (`AlphaFactorForge.dc.html:1080`). Signal exits pass all three values into `recordClose()` (`:1115-1124`), which records the supplied execution timestamp and index (`:1093-1100`).
- **Decision:** Record actual execution time/index, not signal time. BUG-003 implements this scheduling model and adds focused tests for entry/exit timestamps, holding bars, final-bar no-fill, and absence of next-open leakage into the signal-bar equity point.
- **Impact:** Trade-marker placement, CSV timestamps, holding bars, exposure, and any time-based trade analysis. Prices and trade count need not change, but several persisted/report fields would.

### 2. SL/TP exit slippage and gap policy

- **Current engine (BUG-003):** Risk exits choose a gap-aware base from the candle open and threshold, then apply normal closing-side slippage (`index.ts:160-172`). Long exits use sell-side slippage and short exits use buy-side slippage.
- **Legacy:** `approxRes()` uses a gap-aware open/trigger choice (`AlphaFactorForge.dc.html:1082`), then applies closing-side slippage with `fillPx()` before `recordClose()` (`:1111-1112`). Bar Magnifier exits also pass through that slippage step.
- **Decision:** Use the legacy-compatible gap-aware base policy—long SL `min(open, stop)`, long TP `max(open, target)`, short SL `max(open, stop)`, and short TP `min(open, target)`—then apply normal closing-side slippage. BUG-003 locks all four cases with hand-calculated fixtures.
- **Impact:** Every SL/TP trade price and PnL may change; downstream metrics, sweep rankings, saved summaries, and exported reports can all move.

### 3. SL and TP both touched within one bar

- **Current engine (BUG-003):** The `if ... else if` ordering chooses SL before TP when both thresholds are touched (`index.ts:166-172`).
- **Legacy:** Without Bar Magnifier, `approxRes()` also checks SL first (`AlphaFactorForge.dc.html:1082`). With sub-bars available, a sub-bar touching both selects whichever threshold is closer to that sub-bar's open (`:1085-1091`).
- **Decision:** Keep SL-first as the documented conservative fallback while no sub-bar data exists. BUG-003 adds an ambiguous-bar regression test. A future Bar Magnifier remains a separate feature with its own ordering tests.
- **Impact:** Only ambiguous bars that touch both thresholds, but these can materially affect PnL and parameter rankings.

### 4. Short cash and collateral accounting

- **Current engine (BUG-002):** Entry budget explicitly includes notional plus its fee (`index.ts:129-139`). A short reserves 1× entry notional as collateral; mark-to-market equity is free cash + collateral + unrealized PnL (`:191-199`). Closing releases collateral and emits fee-inclusive PnL/PnL% (`:102-125`).
- **Legacy:** A short entry credits sale proceeds minus entry fee and holds a negative position (`AlphaFactorForge.dc.html:1103`); equity is always `cash + pos * close` (`:1127`). Closing debits buy-to-cover price plus fee (`:1093-1101`). Quantity uses the full notional before fee, and the round-trip PnL explicitly subtracts both fees (`:1094-1100`).
- **Decision:** Adopt the explicit unleveraged 1× collateral model with fee-inclusive entry budgeting and require `finalEquity = startEquity + sum(trade.pnl)`. BUG-002 implements this with hand-calculated long/short/partial-size/multi-trade tests. Borrow interest, funding, leverage, and liquidation remain out of Phase A scope.
- **Impact:** Short quantity, per-trade PnL, equity curve, net return, drawdown, and all PnL-derived statistics. This is a high-blast-radius behaviour change.

### 5. End-of-data forced close

- **Current engine (BUG-002 + BUG-003):** A remaining position is force-closed at the final candle close using normal closing-side slippage (`index.ts:202-205`). The final equity point is replaced with settled cash including fees (`:206-208`), and metrics use configured `startEquity` plus that settled endpoint (`:210-216`; `metrics/index.ts:86-110`).
- **Legacy:** The final candle close is passed through closing-side slippage before settlement (`AlphaFactorForge.dc.html:1129`). Its headline net return is calculated from post-settlement cash (`:1130-1133`), although its previously collected equity series is also pre-settlement.
- **Decision:** Metrics and the final equity point use settled equity (BUG-002), and EOD settlement uses normal closing-side slippage (BUG-003). Focused long and short tests cover the final fill price.
- **Impact:** Any run ending with an open position; trade exit price, net return, final equity, fees, and consistency between the trade list and metrics.

### 6. `direction: both` semantics

- **Current engine:** `requestedEntrySide()` maps every non-`short` direction, including `both`, to long (`index.ts:141`). Close and pending-order paths therefore open only long positions and treat exit signals as closes (`:175-188`); there is no reversal into short.
- **Legacy:** `both` is explicitly a reverse system: entry signal requests long, exit signal requests short, and an opposing position is closed before the new one opens (`AlphaFactorForge.dc.html:1121-1125`).
- **Recommendation for maintainer decision:** Decide whether `both` means legacy reversal or merely permits either side when a future signal model specifies one. Candidate actions are either to restore legacy reversal semantics or rename/split the mode so its current limitation is explicit.
- **Impact:** Very high for every `both` strategy: trade side, count, timestamps, equity, metrics, sweep outcomes, and saved strategy meaning.

### 7. UI input normalization boundary

- **Current engine:** `runBacktest()` clamps fractional `sizingPct` to `[0, 1]` while opening (`index.ts:130`) but does not implement the legacy `sizePct = 0` fallback and uses normalized costs without rejecting negative values (`:91-92`). The product runner converts percentage units, maps zero size to 100%, floors small size at 1%, and clamps negative fee/slippage to zero (`backtestRunner.ts:38-54`).
- **Legacy:** The same percentage-unit fallback and clamps live inside `runBacktestCore()` (`AlphaFactorForge.dc.html:1065-1067`).
- **Recommendation for maintainer decision:** Keep one documented normalization boundary and prevent UI/DSL callers from bypassing it. If raw core validation is desired, add it as a separate contract change rather than duplicating percentage-unit rules in the pure engine.
- **Impact:** Direct core/worker/DSL callers only when they bypass `runParamsBacktest()`; malformed inputs could otherwise create no trades or unintended rebates.

## Decision summary

The maintainer adopted the target semantics in `backtest-execution-contract.md` on 2026-07-14. BUG-002 resolves the accounting and settled-equity portions in rows 4–5; BUG-003 resolves rows 1–3 plus EOD slippage; BUG-004 remains responsible for rows 6–7. Golden values preserve each implemented contract and are updated only when an approved correction intentionally changes output.

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

