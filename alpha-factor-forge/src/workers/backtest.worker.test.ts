import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runParamSweep, type RunParamSweepArgs } from '../services/paramSweep';
import { defaultStrategy } from '../services/strategy';
import { makeSampleCandles } from '../services/sampleData';
import { toCoreCandles } from '../services/candleAdapter';
import type { BacktestWorkerRequest, BacktestWorkerResponse } from './backtest.worker';

const args: RunParamSweepArgs = {
  candles: toCoreCandles(makeSampleCandles({ count: 500, seed: 42, startTime: 1704067200000 })),
  strat: defaultStrategy(), interval: '1h',
  sweep: {
    x: { key: 'fastMA', min: 5, max: 20, step: 1 },
    y: { key: 'slowMA', min: 20, max: 35, step: 1 }, metric: 'net',
  },
};
let responses: BacktestWorkerResponse[];
let scope: {
  onmessage: ((event: MessageEvent<BacktestWorkerRequest>) => void) | null;
  postMessage: (response: BacktestWorkerResponse) => void;
};

beforeEach(async () => {
  responses = [];
  scope = { onmessage: null, postMessage: response => { responses.push(structuredClone(response)); } };
  vi.stubGlobal('self', scope);
  vi.resetModules();
  await import('./backtest.worker');
});
afterEach(() => { vi.unstubAllGlobals(); });

function dispatch(payload: RunParamSweepArgs, jobId: string): BacktestWorkerResponse {
  scope.onmessage!({ data: structuredClone({ type: 'runSweep', jobId, payload }) } as MessageEvent<BacktestWorkerRequest>);
  return responses[responses.length - 1];
}

describe('real sweep worker handler', () => {
  it('returns all 256 fixed-seed cells exactly like the unchanged synchronous engine', () => {
    const sync = runParamSweep(args);
    expect(sync.grid.flat()).toHaveLength(256);
    expect(sync.best).not.toBeNull();
    expect(dispatch(args, 'seed-42')).toStrictEqual({ type: 'sweepResult', jobId: 'seed-42', payload: sync });
  });

  it('retains the declared Holdout range and is independent of the unseen suffix', () => {
    const prefix = { ...args, to: 399 };
    const first = dispatch(prefix, 'prefix');
    expect(first).toStrictEqual({ type: 'sweepResult', jobId: 'prefix', payload: runParamSweep(prefix) });
    const changed = {
      ...prefix,
      candles: args.candles.map((c, i) => i <= 399 ? c : { ...c, o: c.o * 10, h: c.h * 10, l: c.l * 10, c: c.c * 10 }),
    };
    expect(dispatch(changed, 'prefix')).toStrictEqual(first);
  });

  it('returns a correlated engine error for an invalid sweep without crashing the handler', () => {
    const invalid = { ...args, sweep: { ...args.sweep, y: { ...args.sweep.x } } };
    const response = dispatch(invalid, 'invalid');
    expect(response).toMatchObject({ type: 'error', jobId: 'invalid' });
    expect(response.payload).toMatch(/X \/ Y/);
  });
});
