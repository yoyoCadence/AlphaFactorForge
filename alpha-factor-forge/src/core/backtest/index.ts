// FULL — deterministic backtest engine (pure function).
// Input: candles + a boolean entry/exit signal series + execution & cost model.
// Output: closed trades + equity curve + metrics. No DOM/React/IO/randomness.
//
// Phase A scope: long/short, percent-equity or fixed sizing, fee+slippage,
// next-bar or close fill, optional SL/TP. The Discovery engine (Phase B) and
// the DSL compiler feed signals in; this engine never knows about indicators.

import { computeMetrics, type ClosedTrade, type EquityPoint, type Metrics } from '../metrics';

export interface Candle {
  t: number;
  o: number;
  h: number;
  l: number;
  c: number;
  v: number;
}

export type Direction = 'long' | 'short' | 'both';
export type FillMode = 'close' | 'nextOpen';

export interface ExecutionModel {
  direction: Direction;
  sizingPct: number; // % of equity per position (0..1); 1 = all-in
  fillMode: FillMode;
}

export interface CostModel {
  feePct: number; // per side, fraction (0.0005 = 0.05%)
  slippagePct: number; // per side, fraction
}

export interface RiskModel {
  stopLossPct?: number; // fraction; undefined = none
  takeProfitPct?: number;
}

export interface Signals {
  /** entry[i] true => open in the execution direction at bar i. */
  entry: boolean[];
  /** exit[i] true => close an open position at bar i. */
  exit: boolean[];
}

export interface BacktestConfig {
  exec: ExecutionModel;
  cost: CostModel;
  risk?: RiskModel;
  startEquity?: number;
  barsPerYear: number;
  /** restrict the test to [from, to] index range (inclusive). */
  from?: number;
  to?: number;
}

export interface BacktestResult {
  trades: ClosedTrade[];
  equity: EquityPoint[];
  metrics: Metrics;
}

interface OpenPos {
  side: 'LONG' | 'SHORT';
  entryIdx: number;
  entryTime: number;
  entryPrice: number;
  qty: number;
}

/** Apply per-side slippage to a fill price (buys pay up, sells receive less). */
function fill(price: number, buy: boolean, slip: number): number {
  return buy ? price * (1 + slip) : price * (1 - slip);
}

/**
 * Run the backtest. Deterministic: identical inputs -> identical output.
 */
export function runBacktest(candles: Candle[], signals: Signals, cfg: BacktestConfig): BacktestResult {
  const start = cfg.startEquity ?? 10_000;
  const from = Math.max(0, cfg.from ?? 0);
  const to = Math.min(candles.length - 1, cfg.to ?? candles.length - 1);
  const { feePct, slippagePct } = cfg.cost;
  const { direction, sizingPct, fillMode } = cfg.exec;
  const sl = cfg.risk?.stopLossPct;
  const tp = cfg.risk?.takeProfitPct;

  let cash = start;
  let pos: OpenPos | null = null;
  const trades: ClosedTrade[] = [];
  const equity: EquityPoint[] = [];

  const priceAt = (i: number, buy: boolean): { idx: number; px: number } => {
    if (fillMode === 'nextOpen' && i + 1 <= to) {
      return { idx: i + 1, px: fill(candles[i + 1].o, buy, slippagePct) };
    }
    return { idx: i, px: fill(candles[i].c, buy, slippagePct) };
  };

  const close = (i: number, price: number, _reason: string) => {
    if (!pos) return;
    const buyToClose = pos.side === 'SHORT';
    const raw = pos.side === 'LONG' ? price * pos.qty : pos.entryPrice * pos.qty + (pos.entryPrice - price) * pos.qty;
    const exitFee = price * pos.qty * feePct;
    if (pos.side === 'LONG') {
      cash += price * pos.qty - exitFee;
    } else {
      // short: profit = (entry - exit) * qty; settle borrowed value
      cash += (pos.entryPrice - price) * pos.qty - exitFee;
      cash += pos.entryPrice * pos.qty; // return collateral booked at open
    }
    void buyToClose;
    void raw;
    const pnlPct = pos.side === 'LONG' ? price / pos.entryPrice - 1 : pos.entryPrice / price - 1;
    const pnlAbs = pnlPct * pos.entryPrice * pos.qty;
    trades.push({
      entryTime: pos.entryTime,
      exitTime: candles[i].t,
      side: pos.side,
      entryPrice: pos.entryPrice,
      exitPrice: price,
      pnl: pnlAbs,
      pnlPct,
      bars: i - pos.entryIdx,
    });
    pos = null;
  };

  const open = (i: number, side: 'LONG' | 'SHORT') => {
    const buy = side === 'LONG';
    const { idx, px } = priceAt(i, buy);
    const budget = cash * Math.min(1, Math.max(0, sizingPct));
    const entryFee = budget * feePct;
    const qty = px > 0 ? (budget - entryFee) / px : 0;
    if (qty <= 0) return;
    if (side === 'LONG') cash -= px * qty + entryFee;
    else cash -= px * qty + entryFee; // reserve collateral; returned on close
    pos = { side, entryIdx: idx, entryTime: candles[idx].t, entryPrice: px, qty };
  };

  for (let i = from; i <= to; i++) {
    const c = candles[i];

    // intrabar SL/TP check (approximate: order unknown without sub-bars)
    const riskPos = pos as OpenPos | null;
    if (riskPos && (sl != null || tp != null)) {
      const isLong = riskPos.side === 'LONG';
      const slPrice = sl != null ? (isLong ? riskPos.entryPrice * (1 - sl) : riskPos.entryPrice * (1 + sl)) : null;
      const tpPrice = tp != null ? (isLong ? riskPos.entryPrice * (1 + tp) : riskPos.entryPrice * (1 - tp)) : null;
      if (slPrice != null && ((isLong && c.l <= slPrice) || (!isLong && c.h >= slPrice))) {
        close(i, slPrice, 'stop-loss');
      } else if (tpPrice != null && ((isLong && c.h >= tpPrice) || (!isLong && c.l <= tpPrice))) {
        close(i, tpPrice, 'take-profit');
      }
    }

    const exitPos = pos as OpenPos | null;
    if (exitPos && signals.exit[i]) {
      const { px } = priceAt(i, exitPos.side === 'SHORT');
      close(i, px, 'signal');
    }

    if (!(pos as OpenPos | null) && signals.entry[i]) {
      if (direction === 'long' || direction === 'both') open(i, 'LONG');
      else if (direction === 'short') open(i, 'SHORT');
    }

    // mark-to-market equity
    let eq = cash;
    const markPos = pos as OpenPos | null;
    if (markPos) {
      eq += markPos.side === 'LONG' ? c.c * markPos.qty : (markPos.entryPrice - c.c) * markPos.qty + markPos.entryPrice * markPos.qty;
    }
    equity.push({ time: c.t, equity: eq });
  }

  // force-close at the end
  if (pos) close(to, candles[to].c, 'eod');

  const metrics = computeMetrics({
    trades,
    equity,
    totalBars: to - from + 1,
    barsPerYear: cfg.barsPerYear,
  });

  return { trades, equity, metrics };
}
