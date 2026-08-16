import { afterEach, describe, expect, it, vi } from 'vitest';
import contract from '../../fixtures/rs-core/discovery-event-v1.json';
import {
  DISCOVERY_EVENTS,
  DISCOVERY_EVENT_VERSION,
  RUN_STATUSES,
  TERMINAL_RUN_STATUSES,
  createThrottle,
  parseDiscoveryDoneEvent,
  parseDiscoveryProgressEvent,
  parseDiscoveryResultEvent,
} from './events';

/** The authored samples Rust asserts its structs serialize to. */
const samples = contract.samples;

/** Structural clone so a mutation case cannot leak into the next test. */
function sample<T>(value: T): T {
  return structuredClone(value);
}

describe('discovery-event-v1 contract', () => {
  // The other half of this assertion lives in
  // src-tauri/src/discovery_runner/event_contract_tests.rs. Both sides read this
  // one authored file, so a field added or renamed on either side fails the
  // other side's test — the guard that was missing while the frontend DTOs sat
  // stale for a whole phase.
  it('pins the same version and channel names as the Rust runner', () => {
    expect(contract.eventVersion).toBe(DISCOVERY_EVENT_VERSION);
    expect(contract.channels.progress).toBe(DISCOVERY_EVENTS.progress);
    expect(contract.channels.result).toBe(DISCOVERY_EVENTS.result);
    expect(contract.channels.done).toBe(DISCOVERY_EVENTS.done);
    expect(contract.authored).toBe(true);
  });

  it('parses a progress event with every field present', () => {
    expect(parseDiscoveryProgressEvent(sample(samples.progressWithCandidate))).toEqual({
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence: 7,
      runId: 12,
      status: 'running',
      counts: {
        totalCandidates: 4,
        queuedCandidates: 1,
        runningCandidates: 1,
        completedCandidates: 2,
        failedCandidates: 0,
        skippedCandidates: 0,
      },
      candidate: {
        candidateIndex: 2,
        strategyId: 31,
        datasetId: 5,
        jobIds: { train: 60, validation: 61 },
      },
      bestStrategyId: 31,
    });
  });

  // `candidate` / `bestStrategyId` are skip_serializing_if on the Rust side, so
  // the KEYS ARE ABSENT. The old DTO would have read undefined here.
  it('treats absent progress optionals as null', () => {
    const raw = sample(samples.progressMinimal) as Record<string, unknown>;
    expect('candidate' in raw).toBe(false);
    expect('bestStrategyId' in raw).toBe(false);
    const parsed = parseDiscoveryProgressEvent(raw);
    expect(parsed?.candidate).toBeNull();
    expect(parsed?.bestStrategyId).toBeNull();
    expect(parsed?.status).toBe('paused');
    expect(parsed?.counts.queuedCandidates).toBe(4);
  });

  it('parses a gate-passed result event', () => {
    expect(parseDiscoveryResultEvent(sample(samples.resultGatePassed))).toEqual({
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence: 8,
      runId: 12,
      candidateIndex: 2,
      jobIds: { train: 60, validation: 61 },
      strategyId: 31,
      strategyHash: samples.resultGatePassed.strategyHash,
      datasetId: 5,
      validationRecordId: 44,
      gatePassed: true,
      score: 0.7351,
    });
  });

  // The opposite convention from the progress optionals: `score` has no
  // skip_serializing_if, so the key is present and explicitly null.
  it('accepts the explicit null score of a gate-failed result', () => {
    const raw = sample(samples.resultGateFailed) as Record<string, unknown>;
    expect('score' in raw).toBe(true);
    expect(raw.score).toBeNull();
    const parsed = parseDiscoveryResultEvent(raw);
    expect(parsed?.gatePassed).toBe(false);
    expect(parsed?.score).toBeNull();
    expect(parsed?.validationRecordId).toBe(45);
  });

  it('parses both terminal done shapes', () => {
    expect(parseDiscoveryDoneEvent(sample(samples.doneCompleted))).toEqual({
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence: 10,
      runId: 12,
      status: 'completed',
      bestStrategyId: 31,
      errorMessage: null,
    });
    const failed = parseDiscoveryDoneEvent(sample(samples.doneFailed));
    expect(failed?.status).toBe('failed');
    expect(failed?.bestStrategyId).toBeNull();
    expect(failed?.errorMessage).toContain('price_not_positive');
  });

  it('declares the run-status vocabulary the runner serializes', () => {
    expect([...RUN_STATUSES]).toEqual(['idle', 'running', 'paused', 'completed', 'failed', 'cancelled']);
    expect([...TERMINAL_RUN_STATUSES]).toEqual(['completed', 'failed', 'cancelled']);
  });
});

describe('payload rejection', () => {
  it('rejects a payload that is not this contract version', () => {
    const raw = sample(samples.progressWithCandidate) as Record<string, unknown>;
    raw.eventVersion = 'discovery-event-v2';
    expect(parseDiscoveryProgressEvent(raw)).toBeNull();
    delete raw.eventVersion;
    expect(parseDiscoveryProgressEvent(raw)).toBeNull();
  });

  it.each(['sequence', 'runId', 'status', 'counts'])('rejects a progress event missing %s', (key) => {
    const raw = sample(samples.progressWithCandidate) as Record<string, unknown>;
    delete raw[key];
    expect(parseDiscoveryProgressEvent(raw)).toBeNull();
  });

  it.each([
    'candidateIndex', 'jobIds', 'strategyId', 'strategyHash', 'datasetId',
    'validationRecordId', 'gatePassed',
  ])('rejects a result event missing %s', (key) => {
    const raw = sample(samples.resultGatePassed) as Record<string, unknown>;
    delete raw[key];
    expect(parseDiscoveryResultEvent(raw)).toBeNull();
  });

  it('rejects an unknown run status', () => {
    const raw = sample(samples.doneCompleted) as Record<string, unknown>;
    raw.status = 'finished';
    expect(parseDiscoveryDoneEvent(raw)).toBeNull();
  });

  it('rejects wrong runtime types even when the key is present', () => {
    const progress = sample(samples.progressWithCandidate) as Record<string, unknown>;
    progress.runId = '12';
    expect(parseDiscoveryProgressEvent(progress)).toBeNull();

    const result = sample(samples.resultGatePassed) as Record<string, unknown>;
    result.gatePassed = 'true';
    expect(parseDiscoveryResultEvent(result)).toBeNull();

    const done = sample(samples.doneFailed) as Record<string, unknown>;
    done.errorMessage = { message: 'boom' };
    expect(parseDiscoveryDoneEvent(done)).toBeNull();
  });

  // i64/u64 ids cannot be represented past 2^53, so a value that far out is a
  // transport error, not a large id — the same fail-closed boundary the runner
  // applies to its own counts.
  it('rejects integers outside the JavaScript safe range and non-finite numbers', () => {
    const progress = sample(samples.progressWithCandidate) as Record<string, unknown>;
    progress.sequence = Number.MAX_SAFE_INTEGER + 1;
    expect(parseDiscoveryProgressEvent(progress)).toBeNull();

    const fractional = sample(samples.progressWithCandidate) as Record<string, unknown>;
    (fractional.counts as Record<string, unknown>).totalCandidates = 4.5;
    expect(parseDiscoveryProgressEvent(fractional)).toBeNull();

    const result = sample(samples.resultGatePassed) as Record<string, unknown>;
    result.score = Number.POSITIVE_INFINITY;
    expect(parseDiscoveryResultEvent(result)).toBeNull();
  });

  it('rejects a present-but-malformed optional instead of reading it as empty', () => {
    const raw = sample(samples.progressWithCandidate) as Record<string, unknown>;
    (raw.candidate as Record<string, unknown>).jobIds = { train: 60 };
    // "the candidate is broken" must not be silently reported as "no candidate".
    expect(parseDiscoveryProgressEvent(raw)).toBeNull();
  });

  it.each([null, undefined, 42, 'progress', [samples.progressWithCandidate]])(
    'rejects the non-object payload %s',
    (payload) => {
      expect(parseDiscoveryProgressEvent(payload)).toBeNull();
      expect(parseDiscoveryResultEvent(payload)).toBeNull();
      expect(parseDiscoveryDoneEvent(payload)).toBeNull();
    },
  );
});

describe('createThrottle', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /** Injected clock: the throttle's window must not depend on timer internals.
   *  `jump` moves the clock WITHOUT running timers, which is what makes the
   *  window-reopens-mid-timer race reproducible. */
  function clock(start = 1_000) {
    let current = start;
    return {
      now: () => current,
      jump: (ms: number) => {
        current += ms;
      },
      advance: (ms: number) => {
        current += ms;
        vi.advanceTimersByTime(ms);
      },
    };
  }

  it('delivers the first call immediately and coalesces the rest into one trailing call', () => {
    vi.useFakeTimers();
    const time = clock();
    const seen: number[] = [];
    const throttled = createThrottle((n: number) => seen.push(n), 300, { now: time.now });

    throttled.call(1);
    expect(seen).toEqual([1]);

    throttled.call(2);
    throttled.call(3);
    throttled.call(4);
    expect(seen).toEqual([1]);

    time.advance(300);
    // Only the NEWEST coalesced payload is delivered, exactly once.
    expect(seen).toEqual([1, 4]);
  });

  // The defect this replaces: a leading call taken while a trailing timer was
  // still queued delivered the same pending payload TWICE. Reproduced by letting
  // the window reopen (clock jump) before the queued timer has run — which a
  // long synchronous task, a busy sweep, or a delayed timer all cause in
  // practice. With the previous implementation this produced [1, 2, 2].
  it('never delivers the same payload twice when the window reopens mid-timer', () => {
    vi.useFakeTimers();
    const time = clock();
    const seen: number[] = [];
    const throttled = createThrottle((n: number) => seen.push(n), 300, { now: time.now });

    throttled.call(1);
    throttled.call(2); // queues a trailing call for the newest payload
    time.jump(400); // window is open again, but the timer has NOT run yet
    throttled.call(2);
    time.advance(300); // now let the queued timer run

    expect(seen).toEqual([1, 2]);
    time.advance(1_000);
    expect(seen).toEqual([1, 2]);
  });

  it('drops a queued trailing call on cancel, so an unmounted consumer is never called', () => {
    vi.useFakeTimers();
    const time = clock();
    const seen: number[] = [];
    const throttled = createThrottle((n: number) => seen.push(n), 300, { now: time.now });

    throttled.call(1);
    throttled.call(2);
    throttled.cancel();
    time.advance(1_000);

    expect(seen).toEqual([1]);
  });

  it('is idempotent to cancel and still usable afterwards', () => {
    vi.useFakeTimers();
    const time = clock();
    const seen: number[] = [];
    const throttled = createThrottle((n: number) => seen.push(n), 300, { now: time.now });

    throttled.cancel();
    throttled.cancel();
    throttled.call(1);
    expect(seen).toEqual([1]);

    throttled.call(2);
    throttled.cancel();
    time.advance(300);
    throttled.call(3);
    expect(seen).toEqual([1, 3]);
  });

  it('delivers again after a quiet period without waiting for a timer', () => {
    vi.useFakeTimers();
    const time = clock();
    const seen: number[] = [];
    const throttled = createThrottle((n: number) => seen.push(n), 300, { now: time.now });

    throttled.call(1);
    time.advance(5_000);
    throttled.call(2);
    expect(seen).toEqual([1, 2]);
  });
});
