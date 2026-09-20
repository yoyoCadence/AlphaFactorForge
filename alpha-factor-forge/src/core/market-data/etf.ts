// P08 pure, opt-in ETF semantics. Rust mirror: discovery_core/etf.rs.
// Does not route legacy backtests through a new execution model. P18 owns fills.
import { expectedBarStarts, parseInstrumentId, utcDateToMs, validateCalendar,
  type SessionCalendar, type Suspension } from './foundation';
import { MIN_MARKET_TIMESTAMP_MS as MIN, MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE as MAX,
  type MarketDataCandle as Candle } from './quality';

export const ETF_SEMANTICS_VERSION = 'etf-semantics-v1';
export type NativeCurrency = 'USD' | 'TWD' | 'USDT';
const DAY = 86_400_000;

function requireThat(ok: boolean, code: string): asserts ok {
  if (!ok) throw new RangeError(code);
}
function nonnegative(value: number): boolean { return Number.isFinite(value) && value >= 0; }
function positive(value: number): boolean { return Number.isFinite(value) && value > 0; }
function dateMs(date: string): number {
  const ms = utcDateToMs(date);
  requireThat(ms !== null && ms >= MIN && ms < MAX, 'invalid_date');
  return ms;
}
function currencyFor(instrumentId: string): NativeCurrency {
  const parsed = parseInstrumentId(instrumentId).id;
  requireThat(parsed?.market === 'us-etf' || parsed?.market === 'tw-etf', 'invalid_etf_instrument');
  return parsed.market === 'us-etf' ? 'USD' : 'TWD';
}

export interface EtfMarket {
  instrumentId: string;
  currency: NativeCurrency;
  calendar: SessionCalendar;
  /** Inclusive/exclusive calendar evidence bounds; never extrapolate holidays. */
  calendarFrom: string;
  calendarToExclusive: string;
  /** Explicit frozen daily annualization setting, never inferred from missing bars. */
  sessionsPerYear: number;
}
export function validateEtfMarket(market: EtfMarket): void {
  requireThat(currencyFor(market.instrumentId) === market.currency, 'currency_mismatch');
  requireThat(validateCalendar(market.calendar) === null && market.calendar.kind === 'trading-days', 'invalid_calendar');
  requireThat(market.calendar.timezone === (market.currency === 'USD' ? 'America/New_York' : 'Asia/Taipei'), 'timezone_mismatch');
  requireThat(dateMs(market.calendarToExclusive) > dateMs(market.calendarFrom), 'invalid_calendar_range');
  requireThat(Number.isInteger(market.sessionsPerYear) && market.sessionsPerYear > 0 && market.sessionsPerYear <= 366, 'invalid_annualization');
}
/** Daily timestamps remain UTC-midnight session DATE labels, not opening instants. */
export function etfSessionDates(market: EtfMarket, from: string, toExclusive: string,
  suspensions: readonly Suspension[] = []): number[] {
  validateEtfMarket(market);
  const start = dateMs(from), end = dateMs(toExclusive);
  requireThat(start >= dateMs(market.calendarFrom) && end <= dateMs(market.calendarToExclusive), 'calendar_out_of_range');
  const result = expectedBarStarts({ calendar: market.calendar, interval: '1d', fromMs: start,
    toMsExclusive: end, suspensions });
  requireThat(result.issue === null, result.issue ?? 'invalid_range');
  return result.timestamps;
}

export interface EtfCostProfile {
  version: string;
  currency: NativeCurrency;
  confirmed: boolean;
  commissionRate: number;
  minimumCommission: number;
  slippageRate: number;
  buyTaxRate: number;
  sellTaxRate: number;
}
export function validateEtfCosts(costs: EtfCostProfile, currency: NativeCurrency): void {
  requireThat(['USD', 'TWD'].includes(currency) && costs.currency === currency, 'currency_mismatch');
  requireThat(costs.version.trim().length > 0, 'missing_cost_version');
  requireThat([costs.commissionRate, costs.slippageRate, costs.buyTaxRate, costs.sellTaxRate]
    .every(value => nonnegative(value) && value < 1) && nonnegative(costs.minimumCommission), 'invalid_cost');
}
/** Explicit estimate only; no order execution, automatic rounding or tax defaults. */
export function estimateEtfCosts(costs: EtfCostProfile, currency: NativeCurrency,
  side: 'buy' | 'sell', quantity: number, rawPrice: number) {
  validateEtfCosts(costs, currency);
  requireThat(side === 'buy' || side === 'sell', 'invalid_side');
  requireThat(positive(quantity) && positive(rawPrice), 'invalid_order');
  const executionPrice = rawPrice * (1 + (side === 'buy' ? 1 : -1) * costs.slippageRate);
  const notional = quantity * executionPrice;
  const commission = Math.max(costs.minimumCommission, notional * costs.commissionRate);
  const transactionTax = notional * (side === 'buy' ? costs.buyTaxRate : costs.sellTaxRate);
  const cashDelta = side === 'buy' ? -(notional + commission + transactionTax) : notional - commission - transactionTax;
  requireThat([executionPrice, notional, commission, transactionTax, cashDelta].every(Number.isFinite), 'numeric_overflow');
  return { version: ETF_SEMANTICS_VERSION, currency, executionPrice, commission, transactionTax,
    slippage: quantity * Math.abs(executionPrice - rawPrice), cashDelta, personalIncomeTaxIncluded: false };
}

export interface SplitAction {
  id: string;
  instrumentId: string;
  currency: NativeCurrency;
  effectiveDate: string;
  /** New shares per old share, including reverse splits. */
  ratio: number;
  availableAtMs: number;
}
export interface DividendAction {
  id: string;
  instrumentId: string;
  currency: NativeCurrency;
  exDate: string;
  paymentDate: string | null;
  amountPerShare: number;
}
export interface DividendReceivable {
  actionId: string;
  exDate: string;
  paymentDate: string | null;
  amount: number;
}
export interface EtfPosition {
  instrumentId: string;
  currency: NativeCurrency;
  cash: number;
  quantity: number;
  /** Total acquisition basis, unchanged by a split. */
  costBasis: number;
  orders: { id: string; quantity: number; limitPrice: number | null; stopPrice: number | null }[];
  receivables: DividendReceivable[];
  appliedActionIds: string[];
  /** Effective accounting date, not wall-clock processing time. */
  lastEventDate: string;
}
function validatePosition(state: EtfPosition): void {
  requireThat(currencyFor(state.instrumentId) === state.currency, 'currency_mismatch');
  dateMs(state.lastEventDate);
  requireThat([state.cash, state.quantity, state.costBasis].every(nonnegative)
    && (state.quantity > 0 || state.costBasis === 0), 'invalid_position');
  requireThat(state.appliedActionIds.every(id => id.trim().length > 0)
    && new Set(state.appliedActionIds).size === state.appliedActionIds.length, 'duplicate_action');
  requireThat(new Set(state.orders.map(o => o.id)).size === state.orders.length, 'invalid_order');
  for (const order of state.orders) requireThat(order.id.trim().length > 0 && positive(order.quantity)
    && (order.limitPrice === null || positive(order.limitPrice))
    && (order.stopPrice === null || positive(order.stopPrice)), 'invalid_order');
  requireThat(new Set(state.receivables.map(r => r.actionId)).size === state.receivables.length, 'invalid_receivable');
  for (const r of state.receivables) requireThat(state.appliedActionIds.includes(r.actionId)
    && nonnegative(r.amount) && dateMs(r.exDate) <= dateMs(state.lastEventDate)
    && (r.paymentDate === null || dateMs(r.paymentDate) >= dateMs(r.exDate)), 'invalid_receivable');
}
function actionBoundary(state: EtfPosition, id: string, instrument: string, currency: NativeCurrency, date: string): void {
  validatePosition(state);
  requireThat(instrument === state.instrumentId && currency === state.currency, 'currency_or_instrument_mismatch');
  requireThat(id.trim().length > 0, 'invalid_action_id');
  requireThat(!state.appliedActionIds.includes(id), 'duplicate_action');
  requireThat(dateMs(date) >= dateMs(state.lastEventDate), 'out_of_order_action');
}
/** Native-currency equity includes unpaid entitlements; spending still uses cash only. */
export function valueEtfPosition(state: EtfPosition, rawPrice: number) {
  validatePosition(state);
  requireThat(positive(rawPrice), 'invalid_raw_price');
  const receivables = state.receivables.reduce((sum, r) => sum + r.amount, 0);
  const marketValue = state.quantity * rawPrice;
  const equity = state.cash + marketValue + receivables;
  requireThat(Number.isFinite(equity), 'numeric_overflow');
  return { currency: state.currency, cash: state.cash, marketValue, receivables, equity };
}
/** Input is the holding immediately BEFORE ex-date events/trading. */
export function accrueDividend(state: EtfPosition, action: DividendAction): EtfPosition {
  actionBoundary(state, action.id, action.instrumentId, action.currency, action.exDate);
  requireThat(nonnegative(action.amountPerShare), 'invalid_dividend');
  requireThat(action.paymentDate === null || dateMs(action.paymentDate) >= dateMs(action.exDate), 'invalid_payment_date');
  const amount = state.quantity * action.amountPerShare;
  requireThat(Number.isFinite(amount), 'numeric_overflow');
  return { ...state, lastEventDate: action.exDate, appliedActionIds: [...state.appliedActionIds, action.id],
    receivables: [...state.receivables, { actionId: action.id, exDate: action.exDate, paymentDate: action.paymentDate, amount }] };
}
/** A payment cannot infer an unknown date, or recalculate entitlement from today's shares. */
export function payDividend(state: EtfPosition, actionId: string, paymentDate: string): EtfPosition {
  validatePosition(state);
  const receivable = state.receivables.find(r => r.actionId === actionId);
  requireThat(receivable !== undefined, 'missing_receivable');
  requireThat(receivable.paymentDate !== null, 'unknown_payment_date');
  requireThat(paymentDate === receivable.paymentDate, 'invalid_payment_date');
  requireThat(dateMs(paymentDate) >= dateMs(state.lastEventDate), 'out_of_order_action');
  requireThat(Number.isFinite(state.cash + receivable.amount), 'numeric_overflow');
  return { ...state, cash: state.cash + receivable.amount, lastEventDate: paymentDate,
    receivables: state.receivables.filter(r => r.actionId !== actionId) };
}
function validateSplit(action: SplitAction): void {
  requireThat(action.id.trim().length > 0 && currencyFor(action.instrumentId) === action.currency, 'invalid_split');
  dateMs(action.effectiveDate);
  requireThat(positive(action.ratio) && Number.isSafeInteger(action.availableAtMs)
    && action.availableAtMs >= MIN && action.availableAtMs < MAX, 'invalid_split');
}
export function applySplit(state: EtfPosition, action: SplitAction): EtfPosition {
  validateSplit(action);
  actionBoundary(state, action.id, action.instrumentId, action.currency, action.effectiveDate);
  const next = { ...state, quantity: state.quantity * action.ratio, lastEventDate: action.effectiveDate,
    appliedActionIds: [...state.appliedActionIds, action.id], orders: state.orders.map(order => ({ ...order,
      quantity: order.quantity * action.ratio,
      limitPrice: order.limitPrice === null ? null : order.limitPrice / action.ratio,
      stopPrice: order.stopPrice === null ? null : order.stopPrice / action.ratio })) };
  validatePosition(next); // Overflow/underflow must not create a successful state.
  requireThat(state.quantity === 0 || next.quantity > 0, 'numeric_overflow');
  return next;
}
/** Raw prices stay untouched. Only splits both effective AND observable at the cut apply.
 * Total-return adjustment is deliberately unsupported without point-in-time payment evidence. */
export function splitAdjustedSignals(raw: readonly Candle[], instrumentId: string,
  splits: readonly SplitAction[], asOfMs: number): Candle[] {
  currencyFor(instrumentId);
  requireThat(Number.isSafeInteger(asOfMs) && asOfMs >= MIN && asOfMs < MAX, 'invalid_as_of');
  requireThat(new Set(splits.map(s => s.id)).size === splits.length, 'duplicate_action');
  for (const s of splits) {
    validateSplit(s);
    requireThat(s.instrumentId === instrumentId, 'currency_or_instrument_mismatch');
  }
  const visible = splits.filter(s => dateMs(s.effectiveDate) <= asOfMs && s.availableAtMs <= asOfMs)
    .slice().sort((a, b) => dateMs(a.effectiveDate) - dateMs(b.effectiveDate) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  return raw.map((bar, index) => {
    requireThat(Number.isSafeInteger(bar.timestamp) && bar.timestamp % DAY === 0
      && bar.timestamp >= MIN && bar.timestamp <= asOfMs
      && (index === 0 || raw[index - 1].timestamp < bar.timestamp)
      && [bar.open, bar.high, bar.low, bar.close].every(positive) && nonnegative(bar.volume)
      && bar.low <= Math.min(bar.open, bar.close) && bar.high >= Math.max(bar.open, bar.close), 'invalid_raw_bar');
    const factor = visible.filter(s => bar.timestamp < dateMs(s.effectiveDate)).reduce((v, s) => v * s.ratio, 1);
    requireThat(positive(factor), 'numeric_overflow');
    const adjusted = { ...bar, open: bar.open / factor, high: bar.high / factor,
      low: bar.low / factor, close: bar.close / factor, volume: bar.volume * factor };
    requireThat([adjusted.open, adjusted.high, adjusted.low, adjusted.close].every(positive)
      && nonnegative(adjusted.volume), 'numeric_overflow');
    return adjusted;
  });
}

/** Necessary market-semantic checks only, never a strategy promotion verdict. */
export function assessEtfSemantics(market: EtfMarket, costs: EtfCostProfile,
  evidence: { version: string; complete: boolean; from: string; toExclusive: string; dividends: DividendAction[] },
  from: string, toExclusive: string) {
  etfSessionDates(market, from, toExclusive);
  validateEtfCosts(costs, market.currency);
  const reasons: string[] = [];
  const evidenceFrom = dateMs(evidence.from), evidenceTo = dateMs(evidence.toExclusive);
  requireThat(evidenceTo > evidenceFrom, 'invalid_action_range');
  if (!evidence.version.trim() || !evidence.complete || evidenceFrom > dateMs(from)
    || evidenceTo < dateMs(toExclusive)) reasons.push('corporate_actions_unconfirmed');
  requireThat(new Set(evidence.dividends.map(d => d.id)).size === evidence.dividends.length, 'duplicate_action');
  for (const d of evidence.dividends) {
    requireThat(d.id.trim().length > 0 && d.instrumentId === market.instrumentId
      && d.currency === market.currency && nonnegative(d.amountPerShare), 'invalid_dividend');
    const ex = dateMs(d.exDate);
    requireThat(d.paymentDate === null || dateMs(d.paymentDate) >= ex, 'invalid_payment_date');
    if (ex >= dateMs(from) && ex < dateMs(toExclusive) && d.paymentDate === null
      && !reasons.includes('unknown_payment_date')) reasons.push('unknown_payment_date');
  }
  if (!costs.confirmed) reasons.push('costs_unconfirmed');
  return { version: ETF_SEMANTICS_VERSION, status: reasons.length ? 'DEGRADED' : 'OK',
    semanticsEligible: reasons.length === 0, reasons };
}
