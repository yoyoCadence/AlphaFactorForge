// FULL — deterministic backtest engine (pure function).
// Input: candles + a boolean entry/exit signal series + execution & cost model.
// Output: closed trades + equity curve + metrics. No DOM/React/IO/randomness.
//
// Phase A scope: long/short, percent-equity or fixed sizing, fee+slippage,
// next-bar or close fill, optional SL/TP. The Discovery engine (Phase B) and
// the DSL compiler feed signals in; this engine never knows about indicators.
// Adopted semantics and phased correction status: docs/backtest-execution-contract.md.

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
  sizingPct: number; // normalized fraction [0, 1]; 1 = all-in
  fillMode: FillMode;
}

export interface CostModel {
  feePct: number; // per side, normalized fraction [0, 1] (0.0005 = 0.05%)
  slippagePct: number; // per side, normalized fraction [0, 1]
}

export interface RiskModel {
  stopLossPct?: number; // active normalized fraction (0, 1]; undefined = none
  takeProfitPct?: number; // active normalized fraction (0, 1]; undefined = none
}

export interface Signals {
  /** entry[i] true => open in the execution direction; `both` requests LONG. */
  entry: boolean[];
  /** exit[i] true => close long/short mode; `both` requests SHORT. */
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
  entryNotional: number;
  entryFee: number;
  qty: number;
}

interface PendingNextOpen {
  exit: boolean;
  entrySide: 'LONG' | 'SHORT' | null;
}

/** Apply per-side slippage to a fill price (buys pay up, sells receive less). */
function fill(price: number, buy: boolean, slip: number): number {
  return buy ? price * (1 + slip) : price * (1 - slip);
}

function assertNormalizedFraction(path: string, value: number, allowZero: boolean): void {
  const belowMinimum = allowZero ? value < 0 : value <= 0;
  if (!Number.isFinite(value) || belowMinimum || value > 1) {
    const range = allowZero ? '[0, 1]' : '(0, 1]';
    throw new RangeError(`BacktestConfig.${path} must be a finite normalized fraction in ${range}`);
  }
}

function validateNormalizedFractions(cfg: BacktestConfig): void {
  assertNormalizedFraction('exec.sizingPct', cfg.exec.sizingPct, true);
  assertNormalizedFraction('cost.feePct', cfg.cost.feePct, true);
  assertNormalizedFraction('cost.slippagePct', cfg.cost.slippagePct, true);
  if (cfg.risk?.stopLossPct !== undefined) {
    assertNormalizedFraction('risk.stopLossPct', cfg.risk.stopLossPct, false);
  }
  if (cfg.risk?.takeProfitPct !== undefined) {
    assertNormalizedFraction('risk.takeProfitPct', cfg.risk.takeProfitPct, false);
  }
}

/**
 * Run the backtest. Deterministic: identical inputs -> identical output.
 */
export function runBacktest(candles: Candle[], signals: Signals, cfg: BacktestConfig): BacktestResult {
  validateNormalizedFractions(cfg);
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
  let pendingNextOpen: PendingNextOpen | null = null;

  const close = (i: number, price: number, _reason: string) => {
    if (!pos) return;
    const exitNotional = price * pos.qty;
    const exitFee = exitNotional * feePct;
    const grossPnl = pos.side === 'LONG'
      ? (price - pos.entryPrice) * pos.qty
      : (pos.entryPrice - price) * pos.qty;
    const pnl = grossPnl - pos.entryFee - exitFee;
    if (pos.side === 'LONG') {
      cash += exitNotional - exitFee;
    } else {
      // Phase A short = unleveraged 1x collateral + realised price PnL.
      cash += pos.entryNotional + grossPnl - exitFee;
    }
    trades.push({
      entryTime: pos.entryTime,
      exitTime: candles[i].t,
      side: pos.side,
      entryPrice: pos.entryPrice,
      exitPrice: price,
      pnl,
      pnlPct: pos.entryNotional > 0 ? pnl / pos.entryNotional : 0,
      bars: i - pos.entryIdx,
    });
    pos = null;
  };

  const open = (i: number, side: 'LONG' | 'SHORT', px: number) => {
    const budget = cash * sizingPct;
    // sizingPct budgets entry notional + its fee, so 100% sizing never makes
    // free cash negative merely to pay the entry commission.
    const entryNotional = budget / (1 + feePct);
    const entryFee = entryNotional * feePct;
    const qty = px > 0 ? entryNotional / px : 0;
    if (qty <= 0) return;
    cash -= entryNotional + entryFee;
    pos = { side, entryIdx: i, entryTime: candles[i].t, entryPrice: px, entryNotional, entryFee, qty };
  };

  const requestedEntrySide = (): 'LONG' | 'SHORT' => direction === 'short' ? 'SHORT' : 'LONG';
  const requestedBothSide = (i: number): 'LONG' | 'SHORT' | null => (
    signals.entry[i] ? 'LONG' : signals.exit[i] ? 'SHORT' : null
  );

  for (let i = from; i <= to; i++) {
    const c = candles[i];

    // Signals from the previous bar become fills only now, at this bar's open.
    // This keeps nextOpen prices out of the prior signal bar's equity point.
    if (fillMode === 'nextOpen' && pendingNextOpen) {
      const pendingExitPos = pos as OpenPos | null;
      if (pendingNextOpen.exit && pendingExitPos) {
        close(i, fill(c.o, pendingExitPos.side === 'SHORT', slippagePct), 'signal');
      }
      if (pendingNextOpen.entrySide && !pos) {
        const side = pendingNextOpen.entrySide;
        open(i, side, fill(c.o, side === 'LONG', slippagePct));
      }
      pendingNextOpen = null;
    }

    // intrabar SL/TP check (approximate: order unknown without sub-bars)
    const riskPos = pos as OpenPos | null;
    if (riskPos && (sl != null || tp != null)) {
      const isLong = riskPos.side === 'LONG';
      const slPrice = sl != null ? (isLong ? riskPos.entryPrice * (1 - sl) : riskPos.entryPrice * (1 + sl)) : null;
      const tpPrice = tp != null ? (isLong ? riskPos.entryPrice * (1 + tp) : riskPos.entryPrice * (1 - tp)) : null;
      if (slPrice != null && ((isLong && c.l <= slPrice) || (!isLong && c.h >= slPrice))) {
        const base = isLong ? Math.min(c.o, slPrice) : Math.max(c.o, slPrice);
        close(i, fill(base, !isLong, slippagePct), 'stop-loss');
      } else if (tpPrice != null && ((isLong && c.h >= tpPrice) || (!isLong && c.l <= tpPrice))) {
        const base = isLong ? Math.max(c.o, tpPrice) : Math.min(c.o, tpPrice);
        close(i, fill(base, !isLong, slippagePct), 'take-profit');
      }
    }

    if (fillMode === 'close') {
      if (direction === 'both') {
        const desiredSide = requestedBothSide(i);
        const currentPos = pos as OpenPos | null;
        if (desiredSide && currentPos?.side !== desiredSide) {
          if (currentPos) {
            close(i, fill(c.c, currentPos.side === 'SHORT', slippagePct), 'reverse');
          }
          open(i, desiredSide, fill(c.c, desiredSide === 'LONG', slippagePct));
        }
      } else {
        const exitPos = pos as OpenPos | null;
        if (exitPos && signals.exit[i]) {
          close(i, fill(c.c, exitPos.side === 'SHORT', slippagePct), 'signal');
        }
        if (!(pos as OpenPos | null) && signals.entry[i]) {
          const side = requestedEntrySide();
          open(i, side, fill(c.c, side === 'LONG', slippagePct));
        }
      }
    } else if (i < to) {
      const currentPos = pos as OpenPos | null;
      if (direction === 'both') {
        const desiredSide = requestedBothSide(i);
        if (desiredSide && currentPos?.side !== desiredSide) {
          pendingNextOpen = { exit: currentPos != null, entrySide: desiredSide };
        }
      } else {
        const hasPosition = currentPos != null;
        const exit = hasPosition && signals.exit[i];
        const entrySide = signals.entry[i] && (!hasPosition || exit) ? requestedEntrySide() : null;
        if (exit || entrySide) pendingNextOpen = { exit, entrySide };
      }
    }

    // mark-to-market equity
    let eq = cash;
    const markPos = pos as OpenPos | null;
    if (markPos) {
      eq += markPos.side === 'LONG'
        ? c.c * markPos.qty
        : markPos.entryNotional + (markPos.entryPrice - c.c) * markPos.qty;
    }
    equity.push({ time: c.t, equity: eq });
  }

  // Force-close at the end using the normal closing side of slippage. A final
  // nextOpen signal has no execution candle and therefore cannot create a fill.
  const endPos = pos as OpenPos | null;
  if (endPos) close(to, fill(candles[to].c, endPos.side === 'SHORT', slippagePct), 'eod');
  // The curve keeps one point per tested candle, but its endpoint must be the
  // settled account value (including EOD exit fee) used by headline metrics.
  if (equity.length) equity[equity.length - 1] = { time: equity[equity.length - 1].time, equity: cash };

  const metrics = computeMetrics({
    trades,
    equity,
    totalBars: to - from + 1,
    barsPerYear: cfg.barsPerYear,
    startEquity: start,
  });

  return { trades, equity, metrics };
}
