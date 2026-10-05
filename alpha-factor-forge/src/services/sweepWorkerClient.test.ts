import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { runSweepInWorker, SweepCancelledError } from './sweepWorkerClient';
import { runParamSweep, type RunParamSweepArgs } from './paramSweep';
import { defaultStrategy } from './strategy';
import type { BacktestWorkerResponse } from '../workers/backtest.worker';

const args: RunParamSweepArgs = {
  candles: [], strat: defaultStrategy(), interval: '1h',
  sweep: { x: { key: 'fastMA', min: 5, max: 5, step: 1 }, metric: 'net' },
};
const result = runParamSweep(args);

class MockWorker {
  static instances: MockWorker[] = [];
  onmessage: ((event: MessageEvent<BacktestWorkerResponse>) => void) | null = null;
  onerror: ((event: Pick<ErrorEvent, 'message' | 'preventDefault'>) => void) | null = null;
  onmessageerror: (() => void) | null = null;
  postMessage = vi.fn((request: unknown) => { structuredClone(request); });
  terminate = vi.fn();
  constructor(public url: URL, public options: WorkerOptions) { MockWorker.instances.push(this); }
  emit(response: BacktestWorkerResponse): void {
    this.onmessage?.({ data: response } as MessageEvent<BacktestWorkerResponse>);
  }
}

beforeEach(() => { MockWorker.instances = []; vi.stubGlobal('Worker', MockWorker); });
afterEach(() => { vi.unstubAllGlobals(); });

describe('owned sweep worker jobs', () => {
  it('uses the module worker protocol and resolves only the matching sweep result', async () => {
    const job = runSweepInWorker(args, 'a');
    const worker = MockWorker.instances[0];
    expect(worker.url.pathname).toMatch(/workers\/backtest\.worker\.ts$/);
    expect(worker.options).toEqual({ type: 'module' });
    expect(worker.postMessage).toHaveBeenCalledWith({ type: 'runSweep', jobId: 'a', payload: args });
    worker.emit({ type: 'sweepResult', jobId: 'other', payload: result });
    worker.emit({ type: 'error', jobId: 'other', payload: 'obsolete error' });
    expect(worker.terminate).not.toHaveBeenCalled();
    worker.emit({ type: 'sweepResult', jobId: 'a', payload: result });
    await expect(job.promise).resolves.toStrictEqual(result);
    job.cancel();
    expect(worker.terminate).toHaveBeenCalledTimes(1);
  });

  it('cancels once, ignores a queued late callback and starts a fresh independent worker', async () => {
    const old = runSweepInWorker(args, 'old');
    const worker = MockWorker.instances[0];
    const queuedCallback = worker.onmessage!;
    const rejected = expect(old.promise).rejects.toBeInstanceOf(SweepCancelledError);
    old.cancel();
    old.cancel();
    queuedCallback({ data: { type: 'sweepResult', jobId: 'old', payload: result } } as MessageEvent<BacktestWorkerResponse>);
    await rejected;
    expect(worker.terminate).toHaveBeenCalledTimes(1);
    expect(worker.onmessage).toBeNull();
    const replacement = runSweepInWorker(args, 'new');
    const fresh = MockWorker.instances[1];
    expect(fresh).not.toBe(worker);
    queuedCallback({ data: { type: 'error', jobId: 'old', payload: 'late' } } as MessageEvent<BacktestWorkerResponse>);
    fresh.emit({ type: 'sweepResult', jobId: 'new', payload: result });
    await expect(replacement.promise).resolves.toStrictEqual(result);
    expect(fresh.terminate).toHaveBeenCalledTimes(1);
  });

  it('keeps overlapping jobs independent and cannot resolve one with the other ID', async () => {
    const first = runSweepInWorker(args, 'first');
    const second = runSweepInWorker(args, 'second');
    const [a, b] = MockWorker.instances;
    a.emit({ type: 'sweepResult', jobId: 'second', payload: result });
    expect(a.terminate).not.toHaveBeenCalled();
    b.emit({ type: 'sweepResult', jobId: 'second', payload: result });
    await expect(second.promise).resolves.toStrictEqual(result);
    a.emit({ type: 'sweepResult', jobId: 'first', payload: result });
    await expect(first.promise).resolves.toStrictEqual(result);
  });

  it('propagates a matching engine error and releases its worker', async () => {
    const job = runSweepInWorker(args, 'bad');
    const worker = MockWorker.instances[0];
    const rejected = expect(job.promise).rejects.toThrow('invalid axis');
    worker.emit({ type: 'error', jobId: 'bad', payload: 'invalid axis' });
    await rejected;
    expect(worker.terminate).toHaveBeenCalledTimes(1);
  });

  it('propagates startup/runtime errors without leaving an unresolved promise', async () => {
    const job = runSweepInWorker(args, 'runtime');
    const worker = MockWorker.instances[0];
    const rejected = expect(job.promise).rejects.toThrow('module failed');
    const preventDefault = vi.fn();
    worker.onerror!({ message: 'module failed', preventDefault });
    await rejected;
    expect(preventDefault).toHaveBeenCalledOnce();
    expect(worker.terminate).toHaveBeenCalledOnce();
  });

  it('rejects response deserialization failure and terminates', async () => {
    const job = runSweepInWorker(args, 'decode');
    const worker = MockWorker.instances[0];
    const rejected = expect(job.promise).rejects.toThrow('回應');
    worker.onmessageerror!();
    await rejected;
    expect(worker.terminate).toHaveBeenCalledOnce();
  });

  it('rejects non-cloneable input and releases the already-created worker', async () => {
    const nonCloneable = { ...args, callback: () => {} };
    const job = runSweepInWorker(nonCloneable, 'clone');
    await expect(job.promise).rejects.toThrow();
    expect(MockWorker.instances[0].terminate).toHaveBeenCalledOnce();
  });

  it('surfaces constructor failure before taking ownership of a job', () => {
    vi.stubGlobal('Worker', class { constructor() { throw new Error('worker unavailable'); } });
    expect(() => runSweepInWorker(args, 'startup')).toThrow('worker unavailable');
  });
});
