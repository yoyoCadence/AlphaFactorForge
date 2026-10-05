// Own one module worker per sweep. Terminate on every terminal outcome; after
// cancellation the next run creates a fresh worker, with no shared callbacks.
import type { RunParamSweepArgs, SweepResult } from './paramSweep';
import type { BacktestWorkerResponse } from '../workers/backtest.worker';

export class SweepCancelledError extends Error {
  constructor() {
    super('掃描已取消');
    this.name = 'SweepCancelledError';
  }
}

export interface SweepWorkerJob {
  promise: Promise<SweepResult>;
  cancel: () => void;
}

export function runSweepInWorker(args: RunParamSweepArgs, jobId: string): SweepWorkerJob {
  const worker = new Worker(new URL('../workers/backtest.worker.ts', import.meta.url), { type: 'module' });
  let cancel = (): void => {};
  const promise = new Promise<SweepResult>((resolve, reject) => {
    let settled = false;
    const cleanup = (): void => {
      settled = true;
      worker.onmessage = null;
      worker.onerror = null;
      worker.onmessageerror = null;
      worker.terminate();
    };
    const fail = (error: unknown): void => {
      if (settled) return;
      cleanup();
      reject(error);
    };
    cancel = () => fail(new SweepCancelledError());
    worker.onmessage = (event: MessageEvent<BacktestWorkerResponse>): void => {
      if (settled || event.data.jobId !== jobId) return;
      if (event.data.type === 'sweepResult') {
        cleanup();
        resolve(event.data.payload);
      } else if (event.data.type === 'error') {
        fail(new Error(event.data.payload));
      }
    };
    worker.onerror = (event): void => {
      event.preventDefault();
      fail(new Error(event.message || '掃描 worker 執行失敗'));
    };
    worker.onmessageerror = () => fail(new Error('無法讀取掃描 worker 的回應'));
    try {
      worker.postMessage({ type: 'runSweep', jobId, payload: args });
    } catch (error) {
      fail(error);
    }
  });
  return { promise, cancel: () => cancel() };
}
