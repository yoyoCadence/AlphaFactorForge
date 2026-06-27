// FULL — performance metrics as pure functions over a closed-trade list and
// an equity curve. Deterministic. Returns the column set persisted in
// backtest_summary. (Scoring/Gate live in core/scoring — Phase B.)

export interface ClosedTrade {
  entryTime: number;
  exitTime: number;
  side: 'LONG' | 'SHORT';
  entryPrice: number;
  exitPrice: number;
  pnl: number; // absolute, after cost
  pnlPct: number; // return on the position, after cost
  bars: number; // holding period in bars
}

export interface EquityPoint {
  time: number;
  equity: number;
}

export interface Metrics {
  netReturn: number;
  cagr: number;
  maxDrawdown: number;
  sharpe: number;
  sortino: number;
  calmar: number;
  winRate: number;
  tradeCount: number;
  profitFactor: number;
  avgTradeReturn: number;
  medianTradeReturn: number;
  avgHoldingBars: number;
  exposure: number;
  turnover: number;
  largestWin: number;
  largestLoss: number;
  consecutiveLosses: number;
  monthlyReturns: Record<string, number>;
}

const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);

function median(xs: number[]): number {
  if (!xs.length) return 0;
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

function std(xs: number[]): number {
  if (xs.length < 2) return 0;
  const m = mean(xs);
  return Math.sqrt(mean(xs.map((x) => (x - m) ** 2)));
}

/** Max peak-to-trough drawdown of an equity curve, as a positive fraction. */
export function maxDrawdown(equity: EquityPoint[]): number {
  let peak = -Infinity;
  let mdd = 0;
  for (const p of equity) {
    if (p.equity > peak) peak = p.equity;
    if (peak > 0) mdd = Math.max(mdd, (peak - p.equity) / peak);
  }
  return mdd;
}

export interface MetricsInput {
  trades: ClosedTrade[];
  equity: EquityPoint[];
  /** total bars in the tested segment, for exposure/turnover. */
  totalBars: number;
  /** bars per year for the interval, for CAGR/annualization. */
  barsPerYear: number;
  /** risk-free per-bar return, default 0. */
  riskFreePerBar?: number;
}

/** Compute the full metric set. Pure. */
export function computeMetrics(input: MetricsInput): Metrics {
  const { trades, equity, totalBars, barsPerYear } = input;
  const rf = input.riskFreePerBar ?? 0;

  const start = equity.length ? equity[0].equity : 1;
  const end = equity.length ? equity[equity.length - 1].equity : 1;
  const netReturn = start > 0 ? end / start - 1 : 0;

  const years = barsPerYear > 0 ? totalBars / barsPerYear : 0;
  const cagr = years > 0 && start > 0 ? Math.pow(end / start, 1 / years) - 1 : 0;

  // per-bar equity returns for Sharpe/Sortino
  const rets: number[] = [];
  for (let i = 1; i < equity.length; i++) {
    const prev = equity[i - 1].equity;
    if (prev > 0) rets.push(equity[i].equity / prev - 1);
  }
  const excess = rets.map((r) => r - rf);
  const sd = std(excess);
  const downside = std(excess.filter((r) => r < 0));
  const ann = barsPerYear > 0 ? Math.sqrt(barsPerYear) : 1;
  const sharpe = sd > 0 ? (mean(excess) / sd) * ann : 0;
  const sortino = downside > 0 ? (mean(excess) / downside) * ann : 0;

  const mdd = maxDrawdown(equity);
  const calmar = mdd > 0 ? cagr / mdd : 0;

  const pnls = trades.map((t) => t.pnl);
  const retPcts = trades.map((t) => t.pnlPct);
  const wins = trades.filter((t) => t.pnl > 0);
  const losses = trades.filter((t) => t.pnl < 0);
  const grossWin = wins.reduce((a, t) => a + t.pnl, 0);
  const grossLoss = -losses.reduce((a, t) => a + t.pnl, 0);

  let curStreak = 0;
  let maxStreak = 0;
  for (const t of trades) {
    if (t.pnl < 0) {
      curStreak += 1;
      maxStreak = Math.max(maxStreak, curStreak);
    } else curStreak = 0;
  }

  const barsInMarket = trades.reduce((a, t) => a + t.bars, 0);

  return {
    netReturn,
    cagr,
    maxDrawdown: mdd,
    sharpe,
    sortino,
    calmar,
    winRate: trades.length ? wins.length / trades.length : 0,
    tradeCount: trades.length,
    profitFactor: grossLoss > 0 ? grossWin / grossLoss : grossWin > 0 ? Infinity : 0,
    avgTradeReturn: mean(retPcts),
    medianTradeReturn: median(retPcts),
    avgHoldingBars: trades.length ? barsInMarket / trades.length : 0,
    exposure: totalBars > 0 ? barsInMarket / totalBars : 0,
    turnover: pnls.length / Math.max(1, totalBars),
    largestWin: wins.length ? Math.max(...wins.map((t) => t.pnlPct)) : 0,
    largestLoss: losses.length ? Math.min(...losses.map((t) => t.pnlPct)) : 0,
    consecutiveLosses: maxStreak,
    monthlyReturns: monthlyReturns(equity),
  };
}

/** Equity grouped into per-calendar-month returns (YYYY-MM -> fraction). */
export function monthlyReturns(equity: EquityPoint[]): Record<string, number> {
  const out: Record<string, number> = {};
  const byMonth = new Map<string, { first: number; last: number }>();
  for (const p of equity) {
    const d = new Date(p.time);
    const key = `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, '0')}`;
    const cur = byMonth.get(key);
    if (!cur) byMonth.set(key, { first: p.equity, last: p.equity });
    else cur.last = p.equity;
  }
  for (const [k, v] of byMonth) out[k] = v.first > 0 ? v.last / v.first - 1 : 0;
  return out;
}
