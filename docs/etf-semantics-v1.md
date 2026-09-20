# P08 ETF daily market semantics

Status: implemented as opt-in pure TypeScript and Rust contracts (2026-09-20).
Plan: [`plans/active-plan.md`](plans/active-plan.md) §4.1 and P08.
Dependencies: P06 calendar/market types and the existing metrics implementation.

## Boundary and compatibility

`etf-semantics-v1` lives in `src/core/market-data/etf.ts` and
`src-tauri/src/discovery_core/etf.rs`. `etf-metrics-v1` lives in
`src/core/metrics/etf.ts` and `discovery_core/etf_metrics.rs`.
All are pure: no IO, database, React, Tauri, network, clock, or random state.

These are reusable primitives for the ETF adapters (P09/P10) and common
execution kernel (P18). Existing backtests, discovery, UI, snapshots, identities,
`metrics-v2`, and `barsPerYear('1d') = 365` retain their existing behavior.
No migration, dependency, provider download, production calendar, tax default,
or new ETF command is introduced. Existing ETF snapshots do not acquire new
qualification simply because this code exists.

The active plan's first ETF release is daily-only. Earlier P06 notes assigning
intraday sessions to P08 were broader than that scope. This phase neither needs
a timezone database nor claims to turn a daily label into an exchange opening
instant. P18/P20 still own actual execution and observation timing.

## Calendar and currency

`EtfMarket` binds instrument, native currency, versioned P06 `SessionCalendar`,
the calendar's inclusive/exclusive evidence dates and explicit integer
`sessionsPerYear` (1–366). There is no default annualization factor. US ETFs
require USD / America/New_York; TW ETFs require TWD / Asia/Taipei. USDT is a
distinct currency and cannot be used in an ETF account or converted implicitly.

`etfSessionDates` reuses P06 `expectedBarStarts`: weekends, listed holidays and
explicit suspension ranges are not missing bars; an early-close day still has
one daily bar. Dates remain UTC-midnight **labels of the venue's trading date**.
They are not midnight/open/close instants in that timezone. Requests outside
the calendar evidence interval fail instead of extrapolating future holidays.
Adapters must supply listing bounds and verified calendars before requesting
their research range. Fixture calendars are synthetic, never registered by
runtime initialization.

## Corporate actions and account values

The small `EtfPosition` value holds one instrument/currency, cash, quantity,
total acquisition cost basis, pending-order quantities/limit/stop prices,
dividend receivables, applied action IDs and last effective event date.
Functions return a new value; originals are not mutated. They reject negative,
non-finite, overflowing, mismatched, duplicate and out-of-order inputs.

- `accrueDividend`: use holdings immediately **before** ex-date events/trading.
  Freeze `quantity × amountPerShare` as an entitlement. Cash does not change.
- `payDividend`: move that exact entitlement to cash on its declared payment
  date, even if shares have since been sold or split. Unknown payment dates
  cannot be guessed. Duplicate payment fails after the receivable is removed.
- `applySplit`: multiply shares and pending-order quantities by new/old ratio;
  divide limit and stop prices by it. Cash, total cost basis and existing
  receivables remain unchanged. Per-share basis is total basis / new quantity.
  Reverse splits preserve fractional entitlement; this primitive does not
  invent rounding or cash-in-lieu. Provider and execution rules must settle
  those before placing executable orders.
- `valueEtfPosition`: equity = cash + quantity × raw price + receivables.
  Receivables contribute to equity but never to spending power before payment.

Caller responsibilities: process events in effective-date order, freeze the
pre-ex-date holding, adjudicate same-date action ordering using source evidence,
and persist event application atomically with the position. P08 rejects
duplicates; durable replay/idempotent order processing belongs to P18/P19.
Dates here describe economic events, not a claim that a late observation was
known earlier. Unknown payment evidence leaves the position unpaid; source
revision/replay must resolve it before any payment can be booked.

## Causal signal prices

`splitAdjustedSignals(raw, instrumentId, splits, asOfMs)` returns a separate
split-adjusted OHLCV view. It never changes the raw fill-price source.
An adjustment requires **both** effective date ≤ cut and `availableAtMs` ≤ cut;
each eligible ratio applies only to bars strictly before its effective date.
OHLC divide by ratio and volume multiplies by ratio. Future announcements,
late-observed events and duplicate events cannot silently rewrite the signal's
past. Bars must be ordered, unique, valid daily labels no later than the cut.
The caller must use completed bars and raw input, and call at each signal's
own cut; one end-of-history adjustment is not a causal full-history backtest.
Total-return-adjusted signal prices are not offered by this contract.

## Costs and eligibility

`EtfCostProfile` explicitly records version, native currency, user confirmation,
commission rate and minimum, slippage, and separate buy/sell transaction tax
rates. Rates are fractions in [0,1); the minimum is non-negative. There are no
assumed current fee schedules or tax laws.

`estimateEtfCosts` uses raw price × (1 ± slippage) for the estimated execution
price. Commission is max(minimum, execution notional × rate); transaction tax
uses that same notional and the side's rate. Returned cash delta includes both,
and slippage is disclosed separately rather than deducted twice. The result
explicitly states `personalIncomeTaxIncluded: false`. It is an estimate, not
an order acceptance or fill. Lot/tick rounding and available-cash checks are P18.

`assessEtfSemantics` returns `DEGRADED` with stable reasons for unconfirmed or
out-of-range corporate-action evidence, missing in-range dividend payment dates,
or unconfirmed costs. Invalid data is rejected. `semanticsEligible` means only
these necessary semantic checks passed, **not** data completeness, provider
authenticity, statistical evidence, Test eligibility, or strategy promotion.
Evidence version/completeness must come from audited adapters, never AI output.
P06's snapshot gate and future P12/P13 qualification remain separate requirements.

## Versioned metrics

`computeEtfMetrics` reuses the existing trade/drawdown/risk metric computation
with `sessionsPerYear` as its volatility/Sharpe/Sortino factor. It overrides:

```text
elapsedYears = (periodEndMs - periodStartMs) / (365.25 × 86400000)
CAGR = (finalEquity / initialEquity) ^ (1 / elapsedYears) - 1
Calmar = CAGR / maximumDrawdown
annualizedVolatility = populationStd(per-session equity returns) × sqrt(sessionsPerYear)
```

The year convention is frozen in this new version, independent of trading-day
count. Caller supplies actual valuation instants: initial equity before the
first observation and a strictly increasing, complete daily equity series ending
at `periodEndMs`. Endpoints must lie within calendar evidence. Empty series,
zero duration, stale final valuations, invalid trades, short trades, invalid
equity, non-finite input and overflow fail. A final total loss yields CAGR −1;
zero equity followed by recovery is refused. Legitimate infinite ratios retain
the existing explicit non-finite codec requirement; they do not grant eligibility.

## Verification

`fixtures/rs-core/etf-semantics-v1.json` is an independently authored matrix of
55 synthetic scenarios. Both suites consume the same expected values. Session
dates, identities, states and rejection reasons are exact; calculated amounts
use absolute 1e-12 / relative 1e-10 tolerance. The TS suite also verifies input
immutability; both sides retain legacy CAGR and reject non-finite input.
No fixture claims real venue history, current costs, or investment performance.

Coverage: holidays/early close/halt; calendar bounds/timezone/currency; buy/sell
costs; ex-date and payment (including after sale); unknown/early/repeated payment;
splits/reverse splits and pending orders; equity conservation; future/late
split observations; duplicate/future bars; incomplete actions/costs; actual
elapsed years, total loss, stale/unsorted equity and invalid/short trades.

Run `npm.cmd test`, `npm.cmd run typecheck`, `npm.cmd run build`,
`npm.cmd run e2e`, `cargo check --locked --all-targets`, `cargo test --locked`.
Actual session results and limitations are recorded in `tasks.md` and the P08
handoff. The next phase is P09, which still requires an actual Tiingo account;
this phase does not claim its data permissions or company-event completeness.
