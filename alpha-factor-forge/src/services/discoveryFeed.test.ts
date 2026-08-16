import { describe, expect, it } from 'vitest';
import {
  INITIAL_DISCOVERY_FEED,
  MAX_FEED_RESULTS,
  reduceDiscoveryFeed,
  type DiscoveryFeedAction,
  type DiscoveryFeedState,
} from './discoveryFeed';
import { DISCOVERY_EVENT_VERSION, type DiscoveryProgressCounts, type RunStatus } from '../tauri-client/events';
import type { DiscoveryProgressSnapshot } from '../tauri-client/commands';

function counts(completed: number, total = 4): DiscoveryProgressCounts {
  return {
    totalCandidates: total,
    queuedCandidates: Math.max(0, total - completed),
    runningCandidates: 0,
    completedCandidates: completed,
    failedCandidates: 0,
    skippedCandidates: 0,
  };
}

function snapshot(overrides: Partial<DiscoveryProgressSnapshot> = {}): DiscoveryProgressSnapshot {
  return {
    version: 'discovery-progress-v1',
    runId: 1,
    name: 'run #1',
    status: 'running',
    counts: counts(0),
    currentCandidateIndexes: [],
    bestStrategyId: null,
    errorMessage: null,
    lastEventSequence: 1,
    ...overrides,
  };
}

function progress(sequence: number, completed: number, status: RunStatus = 'running', runId = 1): DiscoveryFeedAction {
  return {
    type: 'progress',
    event: {
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence,
      runId,
      status,
      counts: counts(completed),
      candidate: null,
      bestStrategyId: null,
    },
  };
}

function result(sequence: number, candidateIndex: number, runId = 1): DiscoveryFeedAction {
  return {
    type: 'result',
    event: {
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence,
      runId,
      candidateIndex,
      jobIds: { train: 1, validation: 2 },
      strategyId: 1000 + candidateIndex,
      strategyHash: `strategy-v2:${String(candidateIndex).padStart(64, '0')}`,
      datasetId: 5,
      validationRecordId: 100 + candidateIndex,
      gatePassed: true,
      score: 0.5,
    },
  };
}

function done(sequence: number, status: RunStatus = 'completed', runId = 1): DiscoveryFeedAction {
  return {
    type: 'done',
    event: {
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence,
      runId,
      status,
      bestStrategyId: 1002,
      errorMessage: null,
    },
  };
}

/** Apply a list of actions in order. */
function run(actions: DiscoveryFeedAction[], from: DiscoveryFeedState = INITIAL_DISCOVERY_FEED): DiscoveryFeedState {
  return actions.reduce(reduceDiscoveryFeed, from);
}

const ADOPTED = reduceDiscoveryFeed(INITIAL_DISCOVERY_FEED, { type: 'snapshot', snapshot: snapshot(), adopt: true });

describe('adoption', () => {
  it('adopts a snapshot as the followed run and takes its sequence', () => {
    expect(ADOPTED.run).toMatchObject({ runId: 1, name: 'run #1', status: 'running' });
    expect(ADOPTED.statusSequence).toBe(1);
    expect(ADOPTED.results).toEqual([]);
  });

  it('ignores events for a run it does not follow', () => {
    const next = run([progress(9, 3, 'running', 99), result(10, 0, 99)], ADOPTED);
    expect(next).toBe(ADOPTED);
  });
});

// The defect PR #102 review found: the applied sequence lived in a ref that was
// rewritten from lagging React state on every render, so a result advanced it and
// the next render walked it back.
describe('the applied sequence never goes backwards', () => {
  it('keeps a replayed result out after a newer progress was applied', () => {
    const next = run([result(4, 0), progress(5, 1), result(4, 0)], ADOPTED);
    expect(next.results).toHaveLength(1);
    expect(next.resultSequence).toBe(4);
    expect(next.statusSequence).toBe(5);
  });

  it('ignores a replayed progress tick', () => {
    const applied = run([progress(5, 2)], ADOPTED);
    const replayed = reduceDiscoveryFeed(applied, progress(5, 2));
    expect(replayed).toBe(applied);
    expect(reduceDiscoveryFeed(applied, progress(4, 1))).toBe(applied);
  });

  // Results are not throttled and progress is, so the two channels arrive out of
  // order by design. A single counter for both would drop the coalesced progress.
  it('accepts a coalesced progress that arrives after a newer result', () => {
    const next = run([result(6, 0), result(8, 1), progress(7, 2)], ADOPTED);
    expect(next.run?.counts.completedCandidates).toBe(2);
    expect(next.statusSequence).toBe(7);
    expect(next.results.map((r) => r.sequence)).toEqual([8, 6]);
  });

  it('drops a delayed snapshot read that is older than what events established', () => {
    const advanced = run([progress(7, 3)], ADOPTED);
    const stale = reduceDiscoveryFeed(advanced, { type: 'snapshot', snapshot: snapshot({ lastEventSequence: 2, counts: counts(0) }), adopt: false });
    expect(stale).toBe(advanced);
    expect(advanced.run?.counts.completedCandidates).toBe(3);
  });

  it('applies a re-read whose sequence matches, because it carries authoritative counts', () => {
    const afterDone = run([done(9)], ADOPTED);
    const reread = reduceDiscoveryFeed(afterDone, {
      type: 'snapshot',
      snapshot: snapshot({ status: 'completed', counts: counts(4), lastEventSequence: 9 }),
      adopt: false,
    });
    expect(reread.run?.counts.completedCandidates).toBe(4);
    expect(reread.run?.status).toBe('completed');
  });
});

describe('a terminal status is never overwritten', () => {
  it('ignores an older progress tick delivered after done', () => {
    const next = run([done(9, 'completed'), progress(8, 2, 'running')], ADOPTED);
    expect(next.run?.status).toBe('completed');
    expect(next.statusSequence).toBe(9);
  });

  it('keeps a cancelled status against a late running tick', () => {
    const next = run([done(9, 'cancelled'), progress(7, 1, 'running')], ADOPTED);
    expect(next.run?.status).toBe('cancelled');
  });
});

describe('results', () => {
  it('lists newest first and never duplicates a sequence', () => {
    const next = run([result(4, 0), result(5, 1), result(5, 1), result(6, 2)], ADOPTED);
    expect(next.results.map((r) => r.candidateIndex)).toEqual([2, 1, 0]);
  });

  it('caps the rolling window', () => {
    const many: DiscoveryFeedAction[] = [];
    for (let i = 0; i < MAX_FEED_RESULTS + 5; i++) many.push(result(10 + i, i));
    const next = run(many, ADOPTED);
    expect(next.results).toHaveLength(MAX_FEED_RESULTS);
    // The newest survive, so the window drops history rather than fresh rows.
    expect(next.results[0].candidateIndex).toBe(MAX_FEED_RESULTS + 4);
  });
});

// The second blocking finding: `start_discovery` emits its first progress event
// and spawns the coordinator BEFORE it returns the run id, so a short run can
// emit results — even finish — before the panel knows which run to follow. A
// snapshot can restore counts, but no command returns result history.
describe('the start / adoption window', () => {
  it('buffers events that arrive before the run id is known and drains them on adoption', () => {
    const beforeId = run([
      { type: 'starting' },
      progress(1, 0),
      result(2, 0),
      result(3, 1),
    ]);
    expect(beforeId.run).toBeNull();
    expect(beforeId.buffer?.results).toHaveLength(2);

    const adopted = reduceDiscoveryFeed(beforeId, { type: 'snapshot', snapshot: snapshot({ lastEventSequence: 3 }), adopt: true });
    expect(adopted.run?.runId).toBe(1);
    // Both results survived, even though their sequences are not newer than the
    // snapshot's — the snapshot never contained them.
    expect(adopted.results.map((r) => r.candidateIndex)).toEqual([1, 0]);
    expect(adopted.resultSequence).toBe(3);
    expect(adopted.buffer).toBeNull();
  });

  it('keeps a run that finished before its id was known', () => {
    const beforeId = run([
      { type: 'starting' },
      result(2, 0),
      result(3, 1),
      progress(4, 2, 'completed'),
      done(5, 'completed'),
    ]);
    // The snapshot resolves later and reports the terminal state too.
    const adopted = reduceDiscoveryFeed(beforeId, {
      type: 'snapshot',
      snapshot: snapshot({ status: 'completed', counts: counts(2), lastEventSequence: 5 }),
      adopt: true,
    });
    expect(adopted.run?.status).toBe('completed');
    expect(adopted.results).toHaveLength(2);
  });

  // A snapshot read can resolve BEFORE a buffered event that is newer than it.
  it('re-applies a buffered status event that is newer than the adopted snapshot', () => {
    const beforeId = run([{ type: 'starting' }, progress(6, 3, 'running'), done(7, 'completed')]);
    const adopted = reduceDiscoveryFeed(beforeId, {
      type: 'snapshot',
      snapshot: snapshot({ counts: counts(1), lastEventSequence: 2 }),
      adopt: true,
    });
    expect(adopted.run?.status).toBe('completed');
    expect(adopted.run?.counts.completedCandidates).toBe(3);
    expect(adopted.statusSequence).toBe(7);
  });

  it('does not mix buffered events from a different run into the adopted one', () => {
    const beforeId = run([{ type: 'starting' }, result(2, 0, 77)]);
    const adopted = reduceDiscoveryFeed(beforeId, { type: 'snapshot', snapshot: snapshot(), adopt: true });
    expect(adopted.results).toEqual([]);
  });

  it('clears the previous run when a start is requested', () => {
    const finished = run([done(9, 'completed'), result(8, 0)], ADOPTED);
    const restarted = reduceDiscoveryFeed(finished, { type: 'starting' });
    expect(restarted).toEqual(INITIAL_DISCOVERY_FEED);
  });
});

describe('dropped payloads', () => {
  it('marks the view stale and clears it on a successful re-read', () => {
    const dropped = reduceDiscoveryFeed(ADOPTED, { type: 'dropped' });
    expect(dropped.stale).toBe(true);
    expect(reduceDiscoveryFeed(dropped, { type: 'dropped' })).toBe(dropped);

    const reread = reduceDiscoveryFeed(dropped, { type: 'snapshot', snapshot: snapshot({ lastEventSequence: 4 }), adopt: false });
    expect(reread.stale).toBe(false);
  });
});
