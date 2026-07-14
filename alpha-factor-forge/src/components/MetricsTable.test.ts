import { describe, expect, it } from 'vitest';
import type { Metrics } from '../core/metrics';
import { metricColumns } from './MetricsTable';

function metrics(tradeCount: number): Metrics {
  return {
    netReturn: 0,
    cagr: 0,
    maxDrawdown: 0,
    sharpe: 0,
    sortino: 0,
    calmar: 0,
    winRate: 0,
    tradeCount,
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
  };
}

describe('metricColumns', () => {
  it('uses a single unlabeled full-period column without a complete holdout split', () => {
    const full = metrics(10);
    expect(metricColumns({ full })).toEqual([{ label: '', metrics: full }]);
    expect(metricColumns({ full, inSample: metrics(6) })).toEqual([{ label: '', metrics: full }]);
  });

  it('orders full, in-sample, and out-of-sample columns for a complete split', () => {
    const full = metrics(10);
    const inSample = metrics(6);
    const outSample = metrics(4);
    expect(metricColumns({ full, inSample, outSample })).toEqual([
      { label: '全期', metrics: full },
      { label: '樣本內', metrics: inSample },
      { label: '樣本外', metrics: outSample },
    ]);
  });
});
