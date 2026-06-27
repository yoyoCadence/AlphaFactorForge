// FULL — typed wrappers around Tauri events for the Discovery job runner.
// The backend job runner (Phase B) emits these; the UI subscribes and throttles
// updates (every 300ms or every 10 jobs — see throttle helper).

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface DiscoveryProgress {
  runId: number;
  tested: number;
  total: number;
  skipped: number;
  current?: { symbol: string; interval: string; segment: 'train' | 'validation' };
  bestStrategyId?: number;
}

export interface DiscoveryResultEvent {
  runId: number;
  strategyId: number;
  segment: 'train' | 'validation';
  score: number | null;
  gatePassed: boolean | null;
}

export const DISCOVERY_EVENTS = {
  progress: 'discovery://progress',
  result: 'discovery://result',
  done: 'discovery://done',
} as const;

export function onDiscoveryProgress(cb: (p: DiscoveryProgress) => void): Promise<UnlistenFn> {
  return listen<DiscoveryProgress>(DISCOVERY_EVENTS.progress, (e) => cb(e.payload));
}

export function onDiscoveryResult(cb: (r: DiscoveryResultEvent) => void): Promise<UnlistenFn> {
  return listen<DiscoveryResultEvent>(DISCOVERY_EVENTS.result, (e) => cb(e.payload));
}

export function onDiscoveryDone(cb: (runId: number) => void): Promise<UnlistenFn> {
  return listen<{ runId: number }>(DISCOVERY_EVENTS.done, (e) => cb(e.payload.runId));
}

/** Throttle a callback to fire at most once per `ms`, with a trailing call. */
export function throttle<A extends unknown[]>(fn: (...a: A) => void, ms: number) {
  let last = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending: A | null = null;
  return (...args: A) => {
    const now = Date.now();
    pending = args;
    if (now - last >= ms) {
      last = now;
      fn(...args);
    } else if (!timer) {
      timer = setTimeout(() => {
        last = Date.now();
        timer = null;
        if (pending) fn(...pending);
      }, ms - (now - last));
    }
  };
}
