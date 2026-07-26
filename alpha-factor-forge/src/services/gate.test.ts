import { describe, it, expect } from 'vitest';
import type { BacktestResult } from '../core/backtest';
import type { ClosedTrade, EquityPoint, Metrics } from '../core/metrics';
import { DETERMINISTIC_BENCHMARK_IDS, type BenchmarkRun } from './benchmarks';
import type { RandomEntryBenchmark } from './randomEntry';
import {
  DEFAULT_GATE_CONFIG,
  GATE_CONTRACT_VERSION,
  evaluateGate,
  rollingPositiveRatio,
  type GateCriterionId,
} from './gate';
import { GATE_CONTRACT_VERSION as VALIDATION_RECORD_GATE_CONTRACT_VERSION } from './validationRecord';

const zeroMetrics = (): Metrics => ({
  netReturn: 0,
  cagr: 0,
  maxDrawdown: 0,
  sharpe: 0,
  sortino: 0,
  calmar: 0,
  winRate: 0,
  tradeCount: 0,
  profitFactor: 0,
  avgTradeReturn: 0,
  medianTradeReturn: 0,
  avgHoldingBars: 0,
  exposure: 0,
  turnover: 0,
  largestWin: 0,
  largestLoss: 0,
  consecutiveLosses: 0,
  monthlyReturns: {},
});

const trade = (pnl: number, exitTime: number, pnlPct = 0.01): ClosedTrade => ({
  entryTime: exitTime - 1,
  exitTime,
  side: 'LONG',
  entryPrice: 100,
  exitPrice: 101,
  pnl,
  pnlPct,
  bars: 1,
});

const risingEquity = (n: number): EquityPoint[] =>
  Array.from({ length: n }, (_, i) => ({ time: i, equity: 1000 + i }));

/** 36 winning trades of pnl 10 spread over six UTC months: every month
 *  contributes 1/6 and every trade 1/36 of the total profit. */
const spreadTrades = (): ClosedTrade[] => {
  const months = Array.from({ length: 6 }, (_, m) => Date.UTC(2024, m, 10));
  return Array.from({ length: 36 }, (_, i) => trade(10, months[i % 6] + i));
};

const passingCandidate = (): BacktestResult => ({
  trades: spreadTrades(),
  equity: risingEquity(100),
  metrics: {
    ...zeroMetrics(),
    netReturn: 0.5,
    tradeCount: 36,
    avgTradeReturn: 0.01,
    maxDrawdown: 0.1,
  },
});

const benches = (netReturn = 0.1): BenchmarkRun[] =>
  DETERMINISTIC_BENCHMARK_IDS.map((id) => ({
    id,
    strat: null,
    result: { trades: [], equity: [], metrics: { ...zeroMetrics(), netReturn } },
  }));

const randomEntry = (candidatePercentile = 97): RandomEntryBenchmark => ({
  runs: 200,
  seed: 1,
  netReturns: [],
  candidateNetReturn: 0.5,
  candidatePercentile,
});

const ORDER: GateCriterionId[] = [
  'minTrades',
  'avgTradeReturn',
  'rollingConsistency',
  'maxDrawdown',
  'monthlyConcentration',
  'tradeConcentration',
  'benchmarkWins',
  'randomEntryPercentile',
];

const failed = (verdict: ReturnType<typeof evaluateGate>): GateCriterionId[] =>
  verdict.criteria.filter((c) => !c.pass).map((c) => c.id);

describe('rollingPositiveRatio', () => {
  it('counts positive step-1 windows and is null when the curve is too short', () => {
    const eq = (xs: number[]): EquityPoint[] => xs.map((equity, time) => ({ time, equity }));
    expect(rollingPositiveRatio(eq([1, 2, 3, 4, 5]), 2)).toBe(1);
    expect(rollingPositiveRatio(eq([5, 4, 3, 2, 1]), 2)).toBe(0);
    expect(rollingPositiveRatio(eq([1, 2, 1, 2, 1]), 2)).toBe(0); // ties are not positive
    expect(rollingPositiveRatio(eq([1, 2, 3]), 3)).toBeNull();
  });

  it('fails closed when any equity value is non-finite', () => {
    const eq = (xs: number[]): EquityPoint[] => xs.map((equity, time) => ({ time, equity }));
    expect(rollingPositiveRatio(eq([1, 2, Number.NaN, 4, 5]), 2)).toBeNull();
    expect(rollingPositiveRatio(eq([1, 2, 3, Number.POSITIVE_INFINITY]), 3)).toBeNull();
  });
});

describe('evaluateGate', () => {
  const passArgs = () => ({
    candidateResult: passingCandidate(),
    benchmarks: benches(),
    randomEntry: randomEntry(),
  });

  it('passes a clean candidate and reports the fixed §5.1 criteria order', () => {
    const verdict = evaluateGate(passArgs());
    expect(verdict.pass).toBe(true);
    expect(verdict.criteria.map((c) => c.id)).toEqual(ORDER);
    expect(verdict.config).toEqual(DEFAULT_GATE_CONFIG);
  });

  it('owns and re-exports one gate-v1 contract constant', () => {
    expect(GATE_CONTRACT_VERSION).toBe('gate-v1');
    expect(VALIDATION_RECORD_GATE_CONTRACT_VERSION).toBe(GATE_CONTRACT_VERSION);
  });

  it('fails each §5.1 criterion independently', () => {
    const flip = (mutate: (args: ReturnType<typeof passArgs>) => void): GateCriterionId[] => {
      const args = passArgs();
      mutate(args);
      const verdict = evaluateGate(args);
      expect(verdict.pass).toBe(false);
      return failed(verdict);
    };

    expect(flip((a) => { a.candidateResult.metrics.tradeCount = 29; })).toEqual(['minTrades']);
    expect(flip((a) => { a.candidateResult.metrics.avgTradeReturn = 0; })).toEqual(['avgTradeReturn']); // strict >
    expect(flip((a) => { a.candidateResult.equity = risingEquity(100).reverse(); })).toEqual(['rollingConsistency']);
    expect(flip((a) => { a.candidateResult.metrics.maxDrawdown = 0.4; })).toEqual(['maxDrawdown']);
    expect(
      flip((a) => { a.candidateResult.trades = spreadTrades().map((t) => ({ ...t, exitTime: Date.UTC(2024, 0, 10) })); }),
    ).toEqual(['monthlyConcentration']);
    // 150 of 510 total = 29.4%: over the 25% single-trade cap but, in its own
    // month, still under the 40% monthly cap — isolates tradeConcentration.
    expect(
      flip((a) => { a.candidateResult.trades = [...spreadTrades(), trade(150, Date.UTC(2024, 6, 20))]; }),
    ).toEqual(['tradeConcentration']);
    expect(flip((a) => { a.randomEntry = randomEntry(90); })).toEqual(['randomEntryPercentile']);
  });

  it('requires strictly beating every deterministic benchmark', () => {
    const args = passArgs();
    args.benchmarks = benches(0.5); // ties with the candidate's 0.5 — not a win
    const verdict = evaluateGate(args);
    const bench = verdict.criteria.find((c) => c.id === 'benchmarkWins')!;
    expect(bench.pass).toBe(false);
    expect(bench.value).toBe(0);
    expect(bench.detail).toContain('buyHold');

    args.benchmarks = benches(0.1);
    args.benchmarks[1] = { ...args.benchmarks[1], result: { trades: [], equity: [], metrics: { ...zeroMetrics(), netReturn: 0.6 } } };
    const one = evaluateGate(args);
    const benchOne = one.criteria.find((c) => c.id === 'benchmarkWins')!;
    expect(benchOne.pass).toBe(false);
    expect(benchOne.value).toBe(3);
    expect(benchOne.detail).toBe('not beaten: smaCross');
  });

  it('rejects duplicate deterministic benchmark ids in fixed suite order', () => {
    const args = passArgs();
    args.benchmarks = [
      ...args.benchmarks,
      args.benchmarks[3],
      args.benchmarks[1],
    ];
    let thrown: unknown;
    try {
      evaluateGate(args);
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBeInstanceOf(RangeError);
    expect((thrown as Error).message).toBe(
      'duplicate deterministic benchmark(s): smaCross, bollingerReversion',
    );
  });

  it('fails closed for every non-finite scalar criterion input', () => {
    const onlyFailure = (
      id: GateCriterionId,
      mutate: (args: ReturnType<typeof passArgs>) => void,
    ): void => {
      const args = passArgs();
      mutate(args);
      const verdict = evaluateGate(args);
      expect(verdict.pass, id).toBe(false);
      expect(failed(verdict), id).toEqual([id]);
    };

    for (const tradeCount of [
      Number.NaN,
      Number.POSITIVE_INFINITY,
      -1,
      36.5,
      Number.MAX_SAFE_INTEGER + 1,
    ]) {
      onlyFailure('minTrades', (args) => {
        args.candidateResult.metrics.tradeCount = tradeCount;
      });
    }
    for (const avgTradeReturn of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      onlyFailure('avgTradeReturn', (args) => {
        args.candidateResult.metrics.avgTradeReturn = avgTradeReturn;
      });
    }
    for (const maxDrawdown of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      onlyFailure('maxDrawdown', (args) => { args.candidateResult.metrics.maxDrawdown = maxDrawdown; });
    }
    onlyFailure('rollingConsistency', (args) => {
      args.candidateResult.equity[50].equity = Number.NaN;
    });
    for (const netReturn of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      onlyFailure('benchmarkWins', (args) => { args.candidateResult.metrics.netReturn = netReturn; });
      onlyFailure('benchmarkWins', (args) => {
        args.benchmarks[0].result.metrics.netReturn = netReturn;
      });
    }
    for (const percentile of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      onlyFailure('randomEntryPercentile', (args) => {
        args.randomEntry.candidatePercentile = percentile;
      });
    }
  });

  it('returns null concentration values with precise details for non-finite profit evidence', () => {
    for (const pnl of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
      const args = passArgs();
      args.candidateResult.trades[0].pnl = pnl;
      const verdict = evaluateGate(args);
      expect(failed(verdict)).toEqual(['monthlyConcentration', 'tradeConcentration']);
      for (const id of ['monthlyConcentration', 'tradeConcentration'] as const) {
        expect(verdict.criteria.find((criterion) => criterion.id === id)).toMatchObject({
          value: null,
          detail: 'non-finite profit evidence',
        });
      }
    }

    const overflow = passArgs();
    overflow.candidateResult.trades = [
      trade(Number.MAX_VALUE, Date.UTC(2024, 0, 1)),
      trade(Number.MAX_VALUE, Date.UTC(2024, 1, 1)),
    ];
    const overflowVerdict = evaluateGate(overflow);
    expect(failed(overflowVerdict)).toEqual(['monthlyConcentration', 'tradeConcentration']);
    for (const id of ['monthlyConcentration', 'tradeConcentration'] as const) {
      expect(overflowVerdict.criteria.find((criterion) => criterion.id === id)).toMatchObject({
        value: null,
        detail: 'non-finite profit evidence',
      });
    }

    const ratioOverflow = passArgs();
    ratioOverflow.candidateResult.trades = [
      trade(Number.MAX_VALUE, Date.UTC(2024, 0, 1)),
      trade(-Number.MAX_VALUE, Date.UTC(2024, 1, 1)),
      trade(Number.MIN_VALUE, Date.UTC(2024, 2, 1)),
    ];
    const ratioOverflowVerdict = evaluateGate(ratioOverflow);
    expect(failed(ratioOverflowVerdict)).toEqual([
      'monthlyConcentration',
      'tradeConcentration',
    ]);
    for (const id of ['monthlyConcentration', 'tradeConcentration'] as const) {
      expect(ratioOverflowVerdict.criteria.find((criterion) => criterion.id === id))
        .toMatchObject({
          value: null,
          detail: 'non-finite profit evidence',
        });
    }
  });

  it('fails monthly concentration for non-finite or TimeClip-invalid exit times', () => {
    for (const exitTime of [Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER]) {
      const args = passArgs();
      args.candidateResult.trades[0].exitTime = exitTime;
      const verdict = evaluateGate(args);
      expect(failed(verdict)).toEqual(['monthlyConcentration']);
      expect(verdict.criteria.find((criterion) => criterion.id === 'monthlyConcentration'))
        .toMatchObject({
          value: null,
          detail: 'invalid trade exit-time evidence',
        });
    }
  });

  it('records the exact rolling failure reason', () => {
    const nonFinite = passArgs();
    nonFinite.candidateResult.equity[50].equity = Number.NaN;
    expect(evaluateGate(nonFinite).criteria.find((criterion) => criterion.id === 'rollingConsistency'))
      .toMatchObject({ value: null, detail: 'non-finite equity evidence' });

    const short = passArgs();
    short.candidateResult.equity = risingEquity(10);
    expect(evaluateGate(short).criteria.find((criterion) => criterion.id === 'rollingConsistency'))
      .toMatchObject({
        value: null,
        detail: 'equity curve shorter than one 30-bar window',
      });
  });

  it('fails closed when the evidence is insufficient', () => {
    // Equity too short for one rolling window.
    const short = passArgs();
    short.candidateResult.equity = risingEquity(10);
    const rolled = evaluateGate(short).criteria.find((c) => c.id === 'rollingConsistency')!;
    expect(rolled.pass).toBe(false);
    expect(rolled.value).toBeNull();
    expect(rolled.detail).toBe('equity curve shorter than one 30-bar window');

    // No positive total profit: both concentration criteria unverifiable.
    const losing = passArgs();
    losing.candidateResult.trades = spreadTrades().map((t) => ({ ...t, pnl: -10 }));
    const verdict = evaluateGate(losing);
    for (const id of ['monthlyConcentration', 'tradeConcentration'] as const) {
      const c = verdict.criteria.find((x) => x.id === id)!;
      expect(c.pass).toBe(false);
      expect(c.value).toBeNull();
      expect(c.detail).toBe('no positive total profit to attribute');
    }
  });

  it('honors explicit threshold overrides', () => {
    const args = passArgs();
    args.candidateResult.metrics.tradeCount = 6;
    expect(evaluateGate(args).pass).toBe(false);
    expect(evaluateGate({ ...args, config: { minTrades: 5 } }).pass).toBe(true);
  });

  it('throws on a missing benchmark or invalid config', () => {
    const args = passArgs();
    expect(() =>
      evaluateGate({ ...args, benchmarks: args.benchmarks.slice(0, 3) }),
    ).toThrow(/missing deterministic benchmark/);
    expect(() => evaluateGate({ ...args, config: { minTrades: 0 } })).toThrow(RangeError);
    expect(() => evaluateGate({ ...args, config: { maxDrawdown: 1.5 } })).toThrow(RangeError);
    expect(() => evaluateGate({ ...args, config: { minRandomEntryPercentile: 101 } })).toThrow(RangeError);
    expect(() => evaluateGate({ ...args, config: { rollingWindowBars: 0 } })).toThrow(RangeError);
  });
});
