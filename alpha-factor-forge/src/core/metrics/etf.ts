// Opt-in daily ETF metrics; metrics-v2 and all legacy callers remain unchanged.
import { computeMetrics, type MetricsInput } from './index';
import { validateEtfMarket, type EtfMarket } from '../market-data/etf';
import { utcDateToMs } from '../market-data/foundation';

export const ETF_METRICS_VERSION = 'etf-metrics-v1';
export const ELAPSED_YEAR_MS = 365.25 * 86_400_000;

export function computeEtfMetrics(input: Omit<MetricsInput, 'barsPerYear' | 'startEquity'> & {
  startEquity: number; periodStartMs: number; periodEndMs: number; market: EtfMarket;
}) {
  validateEtfMarket(input.market);
  const { periodStartMs: start, periodEndMs: end, equity } = input;
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || end <= start
    || start < utcDateToMs(input.market.calendarFrom)! || end > utcDateToMs(input.market.calendarToExclusive)!
    || !Number.isFinite(input.startEquity) || input.startEquity <= 0
    || !Number.isFinite(input.riskFreePerBar ?? 0) || !equity.length
    || input.totalBars !== equity.length) throw new RangeError('invalid_metrics_input');
  const returns: number[] = [];
  let previous = input.startEquity, previousTime = start;
  for (const [index, point] of equity.entries()) {
    if (!Number.isSafeInteger(point.time) || point.time <= previousTime || point.time > end
      || !Number.isFinite(point.equity) || point.equity < 0 || (point.equity === 0 && index < equity.length - 1)) {
      throw new RangeError('invalid_equity');
    }
    returns.push(point.equity / previous - 1);
    previous = point.equity;
    previousTime = point.time;
  }
  // A stale final valuation must not silently dilute annualized results.
  if (previousTime !== end) throw new RangeError('missing_final_valuation');
  for (const trade of input.trades) {
    if (!Number.isSafeInteger(trade.entryTime) || !Number.isSafeInteger(trade.exitTime)
      || trade.entryTime < start || trade.exitTime > end || trade.exitTime < trade.entryTime
      || trade.side !== 'LONG' || !Number.isInteger(trade.bars) || trade.bars < 0 || trade.bars > input.totalBars
      || ![trade.entryPrice, trade.exitPrice].every(value => Number.isFinite(value) && value > 0)
      || ![trade.pnl, trade.pnlPct].every(Number.isFinite)) throw new RangeError('invalid_trade');
  }
  const metrics = computeMetrics({ ...input, barsPerYear: input.market.sessionsPerYear });
  const cagr = Math.pow(previous / input.startEquity, ELAPSED_YEAR_MS / (end - start)) - 1;
  if (!Number.isFinite(cagr) || returns.some(value => !Number.isFinite(value))) throw new RangeError('numeric_overflow');
  const mean = returns.reduce((a, b) => a + b, 0) / returns.length;
  const variance = returns.reduce((sum, value) => sum + (value - mean) ** 2, 0) / returns.length;
  const annualizedVolatility = Math.sqrt(variance * input.market.sessionsPerYear);
  if (!Number.isFinite(annualizedVolatility)) throw new RangeError('numeric_overflow');
  return { version: ETF_METRICS_VERSION, currency: input.market.currency,
    calendarId: input.market.calendar.calendarId, sessionsPerYear: input.market.sessionsPerYear,
    elapsedYears: (end - start) / ELAPSED_YEAR_MS, annualizedVolatility,
    metrics: { ...metrics, cagr, calmar: metrics.maxDrawdown > 0 ? cagr / metrics.maxDrawdown : cagr > 0 ? Infinity : 0 } };
}
