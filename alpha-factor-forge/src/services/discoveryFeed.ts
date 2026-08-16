// RUNNER-UI-001b-2 review follow-up — the discovery panel's ordering rules as a
// pure reducer.
//
// The first implementation kept the run id and the applied sequence in React
// refs and re-derived them from state on every render. The PR #102 review found
// that this cannot be monotonic: a result event advances the mutable sequence but
// does NOT advance the state's, so the very next render writes the ref BACK to
// the older state value and a replayed event is accepted twice. Ordering that
// depends on render timing is not ordering, so it moved here, where it is a
// function of (state, event) and nothing else.
//
// Three properties this module owns, each with a named test:
//   1. Applied sequence never goes backwards — including across a throttled
//      progress channel, a delayed snapshot read, and a replayed event.
//   2. A result is never applied twice, and never lost.
//   3. A terminal status is never overwritten by an older progress tick.
//
// The guard is per STATE SLICE, not one counter for all three channels: progress
// is throttled and results are not, so a single counter would make a coalesced
// progress tick that arrives after a newer result look stale and drop its counts.
// `statusSequence` covers status/counts (progress, done, snapshots) and
// `resultSequence` covers the result list.
//
// It also owns the start/adoption window. The backend emits its first progress
// event and spawns the coordinator BEFORE `start_discovery` returns the run id
// (`discovery_runner/mod.rs`), so a short run can emit results — even finish —
// before the WebView learns which run to follow. Progress can be re-read from the
// database, but result rows cannot: no command returns result history. Events
// arriving while no run is followed are therefore buffered and drained on
// adoption instead of being dropped.
//
// Pure: no React, DOM, IO, or Tauri.

import type { DiscoveryProgressSnapshot } from '../tauri-client/commands';
import {
  TERMINAL_RUN_STATUSES,
  type DiscoveryDoneEvent,
  type DiscoveryProgressCounts,
  type DiscoveryProgressEvent,
  type DiscoveryResultEvent,
  type RunStatus,
} from '../tauri-client/events';

/** Rolling window of retained results. Full history is the Results Explorer's
 *  job (a separate Phase B task). */
export const MAX_FEED_RESULTS = 20;

export interface FollowedRun {
  runId: number;
  name: string;
  status: RunStatus;
  counts: DiscoveryProgressCounts;
  bestStrategyId: number | null;
  errorMessage: string | null;
}

/** Events seen before the followed run is known (the start/adoption window). */
export interface FeedBuffer {
  runId: number;
  /** Newest first, capped like the visible list. */
  results: DiscoveryResultEvent[];
  /** Only the newest matters: counts are a replacement value. */
  progress: DiscoveryProgressEvent | null;
  done: DiscoveryDoneEvent | null;
}

export interface DiscoveryFeedState {
  run: FollowedRun | null;
  /** Newest first. */
  results: DiscoveryResultEvent[];
  /** Highest sequence applied to status/counts. */
  statusSequence: number;
  /** Highest result sequence applied. */
  resultSequence: number;
  buffer: FeedBuffer | null;
  /** A payload was dropped, so the view may be behind the database. */
  stale: boolean;
}

export type DiscoveryFeedAction =
  /** A start is about to be requested: stop following the previous run so the
   *  buffer window is open for the new one's early events. */
  | { type: 'starting' }
  | { type: 'snapshot'; snapshot: DiscoveryProgressSnapshot; adopt: boolean }
  | { type: 'progress'; event: DiscoveryProgressEvent }
  | { type: 'result'; event: DiscoveryResultEvent }
  | { type: 'done'; event: DiscoveryDoneEvent }
  /** A payload could not be parsed and was dropped. */
  | { type: 'dropped' };

export const INITIAL_DISCOVERY_FEED: DiscoveryFeedState = {
  run: null,
  results: [],
  statusSequence: 0,
  resultSequence: 0,
  buffer: null,
  stale: false,
};

export function isTerminalStatus(status: RunStatus): boolean {
  return TERMINAL_RUN_STATUSES.includes(status);
}

function runFromSnapshot(snapshot: DiscoveryProgressSnapshot): FollowedRun {
  return {
    runId: snapshot.runId,
    name: snapshot.name,
    status: snapshot.status,
    counts: snapshot.counts,
    bestStrategyId: snapshot.bestStrategyId,
    errorMessage: snapshot.errorMessage,
  };
}

function cap(results: DiscoveryResultEvent[]): DiscoveryResultEvent[] {
  return results.length > MAX_FEED_RESULTS ? results.slice(0, MAX_FEED_RESULTS) : results;
}

/** Buffer an event for a run we do not follow yet. */
function buffered(state: DiscoveryFeedState, runId: number): FeedBuffer {
  return state.buffer != null && state.buffer.runId === runId
    ? state.buffer
    : { runId, results: [], progress: null, done: null };
}

/**
 * Apply one feed action. Returns the SAME state object when the action is
 * ignored, so a caller can treat identity as "nothing changed".
 */
export function reduceDiscoveryFeed(
  state: DiscoveryFeedState,
  action: DiscoveryFeedAction,
): DiscoveryFeedState {
  switch (action.type) {
    case 'starting':
      return { ...INITIAL_DISCOVERY_FEED };

    case 'dropped':
      return state.stale ? state : { ...state, stale: true };

    case 'snapshot': {
      const { snapshot, adopt } = action;
      const following = state.run;
      const isNewRun = following == null || following.runId !== snapshot.runId;

      if (!adopt) {
        // A re-read only refreshes the run we already follow, and only when it
        // is not older than what events have already established — an in-flight
        // read that resolves late must not walk the view backwards.
        if (isNewRun) return state;
        if (snapshot.lastEventSequence < state.statusSequence) return state;
        return {
          ...state,
          run: runFromSnapshot(snapshot),
          statusSequence: snapshot.lastEventSequence,
          stale: false,
        };
      }

      if (!isNewRun) {
        // Adopting the run we already follow: same freshness rule.
        if (snapshot.lastEventSequence < state.statusSequence) return state;
        return {
          ...state,
          run: runFromSnapshot(snapshot),
          statusSequence: snapshot.lastEventSequence,
          stale: false,
        };
      }

      // A different run: adopt it and drain anything buffered for it. The
      // snapshot carries authoritative counts but never result rows, so buffered
      // results are kept regardless of their sequence relative to the snapshot.
      const pending = state.buffer != null && state.buffer.runId === snapshot.runId ? state.buffer : null;
      let next: DiscoveryFeedState = {
        run: runFromSnapshot(snapshot),
        results: pending == null ? [] : cap(pending.results),
        statusSequence: snapshot.lastEventSequence,
        resultSequence: pending == null || pending.results.length === 0
          ? 0
          : Math.max(...pending.results.map((result) => result.sequence)),
        buffer: null,
        stale: state.stale,
      };
      // Buffered status events still have to be re-applied through the normal
      // rules: one of them can be NEWER than the snapshot (the run may have
      // progressed, or even finished, between the emit and the read).
      if (pending?.progress != null) {
        next = reduceDiscoveryFeed(next, { type: 'progress', event: pending.progress });
      }
      if (pending?.done != null) {
        next = reduceDiscoveryFeed(next, { type: 'done', event: pending.done });
      }
      return next;
    }

    case 'progress': {
      const { event } = action;
      if (state.run == null) {
        const slot = buffered(state, event.runId);
        // Keep only the newest buffered progress: counts replace, they do not
        // accumulate.
        if (slot.progress != null && slot.progress.sequence >= event.sequence) return state;
        return { ...state, buffer: { ...slot, progress: event } };
      }
      if (state.run.runId !== event.runId) return state;
      if (event.sequence <= state.statusSequence) return state;
      return {
        ...state,
        run: {
          ...state.run,
          status: event.status,
          counts: event.counts,
          bestStrategyId: event.bestStrategyId ?? state.run.bestStrategyId,
        },
        statusSequence: event.sequence,
      };
    }

    case 'result': {
      const { event } = action;
      if (state.run == null) {
        const slot = buffered(state, event.runId);
        if (slot.results.some((result) => result.sequence === event.sequence)) return state;
        return { ...state, buffer: { ...slot, results: cap([event, ...slot.results]) } };
      }
      if (state.run.runId !== event.runId) return state;
      if (event.sequence <= state.resultSequence) return state;
      return {
        ...state,
        results: cap([event, ...state.results]),
        resultSequence: event.sequence,
      };
    }

    case 'done': {
      const { event } = action;
      if (state.run == null) {
        const slot = buffered(state, event.runId);
        if (slot.done != null && slot.done.sequence >= event.sequence) return state;
        return { ...state, buffer: { ...slot, done: event } };
      }
      if (state.run.runId !== event.runId) return state;
      if (event.sequence <= state.statusSequence) return state;
      return {
        ...state,
        run: {
          ...state.run,
          status: event.status,
          bestStrategyId: event.bestStrategyId ?? state.run.bestStrategyId,
          errorMessage: event.errorMessage,
        },
        statusSequence: event.sequence,
      };
    }
  }
}
