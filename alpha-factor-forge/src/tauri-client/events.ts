// RUNNER-UI-001a — the typed, version-checked frontend half of the
// `discovery-event-v1` contract emitted by the merged backend runner.
//
// The previous version of this file was written BEFORE the runner existed and
// matched none of the emitted fields (it expected `tested`/`total`/`skipped` and
// a `current: { symbol, interval, segment }`). Nothing failed, because nothing
// imported it and nothing compared it to Rust — so the first UI built on it
// would have rendered `undefined` for every value. The contract is now pinned
// from both sides against one authored fixture:
//   Rust:       src-tauri/src/discovery_runner/event_contract_tests.rs
//   TypeScript: events.test.ts
//   Fixture:    fixtures/rs-core/discovery-event-v1.json
//
// Two deliberate asymmetries in the payloads, both locked by that fixture:
//   - `candidate`, `bestStrategyId`, and `errorMessage` are `skip_serializing_if`
//     on the Rust side, so their keys are ABSENT rather than null;
//   - `score` is NOT, so a gate-failed candidate emits an explicit `null`.
// The parsers below accept either spelling and normalize both to `null`, so no
// consumer has to know which convention a given field uses.
//
// Unknown extra keys are ignored rather than rejected: a payload the backend
// extends must not break a running window. Real drift is caught at build time by
// the two fixture tests, and any observable change to these payloads is required
// to bump the version string, which IS rejected here.

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** Must equal `DISCOVERY_EVENT_VERSION` in `discovery_runner/mod.rs`. */
export const DISCOVERY_EVENT_VERSION = 'discovery-event-v1';

export const DISCOVERY_EVENTS = {
  progress: 'discovery://progress',
  result: 'discovery://result',
  done: 'discovery://done',
} as const;

/** `db::discovery::RunStatus`, which serializes lowercase. */
export const RUN_STATUSES = ['idle', 'running', 'paused', 'completed', 'failed', 'cancelled'] as const;
export type RunStatus = (typeof RUN_STATUSES)[number];

/** Terminal states: the runner emits no further events for the run. */
export const TERMINAL_RUN_STATUSES: readonly RunStatus[] = ['completed', 'failed', 'cancelled'];

export interface DiscoveryProgressCounts {
  totalCandidates: number;
  queuedCandidates: number;
  runningCandidates: number;
  completedCandidates: number;
  failedCandidates: number;
  skippedCandidates: number;
}

/** The paired Train/Validation job row ids of one candidate. */
export interface DiscoveryJobIds {
  train: number;
  validation: number;
}

export interface DiscoveryCandidateDigest {
  candidateIndex: number;
  strategyId: number;
  datasetId: number;
  jobIds: DiscoveryJobIds;
}

export interface DiscoveryProgressEvent {
  eventVersion: string;
  /** Monotonic per run: lets a consumer drop out-of-order or replayed events. */
  sequence: number;
  runId: number;
  status: RunStatus;
  counts: DiscoveryProgressCounts;
  candidate: DiscoveryCandidateDigest | null;
  bestStrategyId: number | null;
}

export interface DiscoveryResultEvent {
  eventVersion: string;
  sequence: number;
  runId: number;
  candidateIndex: number;
  jobIds: DiscoveryJobIds;
  strategyId: number;
  strategyHash: string;
  datasetId: number;
  validationRecordId: number;
  gatePassed: boolean;
  /** Null exactly when the Gate rejected the candidate (Score is not computed). */
  score: number | null;
}

export interface DiscoveryDoneEvent {
  eventVersion: string;
  sequence: number;
  runId: number;
  status: RunStatus;
  bestStrategyId: number | null;
  errorMessage: string | null;
}

// ---------- payload parsing ----------

function asRecord(value: unknown): Record<string, unknown> | null {
  return value != null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** Absent and explicit null are the same "no value"; anything else is a shape
 *  error rather than an empty field. */
function isAbsent(value: unknown): boolean {
  return value === undefined || value === null;
}

/**
 * Rust ids, sequences, and counts are `i64`/`u64`. A value outside the
 * JavaScript safe-integer range cannot be represented faithfully, so it is
 * rejected instead of silently rounded — the same fail-closed boundary
 * RUNNER-EXEC-001 applies to its own hypothesis counts.
 */
function asInteger(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) ? value : null;
}

function asFiniteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function asStatus(value: unknown): RunStatus | null {
  return typeof value === 'string' && (RUN_STATUSES as readonly string[]).includes(value)
    ? (value as RunStatus)
    : null;
}

/** Rejects a payload that is not this exact contract version. A backend that
 *  changes an observable field must bump the version, and an older window must
 *  then refuse the payload instead of misreading it. */
function hasCurrentVersion(record: Record<string, unknown>): boolean {
  return record.eventVersion === DISCOVERY_EVENT_VERSION;
}

function parseCounts(value: unknown): DiscoveryProgressCounts | null {
  const record = asRecord(value);
  if (!record) return null;
  const totalCandidates = asInteger(record.totalCandidates);
  const queuedCandidates = asInteger(record.queuedCandidates);
  const runningCandidates = asInteger(record.runningCandidates);
  const completedCandidates = asInteger(record.completedCandidates);
  const failedCandidates = asInteger(record.failedCandidates);
  const skippedCandidates = asInteger(record.skippedCandidates);
  if (
    totalCandidates == null || queuedCandidates == null || runningCandidates == null ||
    completedCandidates == null || failedCandidates == null || skippedCandidates == null
  ) {
    return null;
  }
  return {
    totalCandidates,
    queuedCandidates,
    runningCandidates,
    completedCandidates,
    failedCandidates,
    skippedCandidates,
  };
}

function parseJobIds(value: unknown): DiscoveryJobIds | null {
  const record = asRecord(value);
  if (!record) return null;
  const train = asInteger(record.train);
  const validation = asInteger(record.validation);
  return train == null || validation == null ? null : { train, validation };
}

function parseCandidate(value: unknown): DiscoveryCandidateDigest | null {
  const record = asRecord(value);
  if (!record) return null;
  const candidateIndex = asInteger(record.candidateIndex);
  const strategyId = asInteger(record.strategyId);
  const datasetId = asInteger(record.datasetId);
  const jobIds = parseJobIds(record.jobIds);
  return candidateIndex == null || strategyId == null || datasetId == null || jobIds == null
    ? null
    : { candidateIndex, strategyId, datasetId, jobIds };
}

/** Parsed event, or null when the payload is not a valid current-version event. */
export function parseDiscoveryProgressEvent(payload: unknown): DiscoveryProgressEvent | null {
  const record = asRecord(payload);
  if (!record || !hasCurrentVersion(record)) return null;
  const sequence = asInteger(record.sequence);
  const runId = asInteger(record.runId);
  const status = asStatus(record.status);
  const counts = parseCounts(record.counts);
  if (sequence == null || runId == null || status == null || counts == null) return null;

  let candidate: DiscoveryCandidateDigest | null = null;
  if (!isAbsent(record.candidate)) {
    candidate = parseCandidate(record.candidate);
    if (candidate == null) return null; // present but malformed: not "no candidate"
  }
  let bestStrategyId: number | null = null;
  if (!isAbsent(record.bestStrategyId)) {
    bestStrategyId = asInteger(record.bestStrategyId);
    if (bestStrategyId == null) return null;
  }

  return {
    eventVersion: DISCOVERY_EVENT_VERSION,
    sequence,
    runId,
    status,
    counts,
    candidate,
    bestStrategyId,
  };
}

export function parseDiscoveryResultEvent(payload: unknown): DiscoveryResultEvent | null {
  const record = asRecord(payload);
  if (!record || !hasCurrentVersion(record)) return null;
  const sequence = asInteger(record.sequence);
  const runId = asInteger(record.runId);
  const candidateIndex = asInteger(record.candidateIndex);
  const jobIds = parseJobIds(record.jobIds);
  const strategyId = asInteger(record.strategyId);
  const datasetId = asInteger(record.datasetId);
  const validationRecordId = asInteger(record.validationRecordId);
  if (
    sequence == null || runId == null || candidateIndex == null || jobIds == null ||
    strategyId == null || datasetId == null || validationRecordId == null
  ) {
    return null;
  }
  if (typeof record.strategyHash !== 'string' || record.strategyHash.length === 0) return null;
  if (typeof record.gatePassed !== 'boolean') return null;

  let score: number | null = null;
  if (!isAbsent(record.score)) {
    score = asFiniteNumber(record.score);
    if (score == null) return null;
  }

  return {
    eventVersion: DISCOVERY_EVENT_VERSION,
    sequence,
    runId,
    candidateIndex,
    jobIds,
    strategyId,
    strategyHash: record.strategyHash,
    datasetId,
    validationRecordId,
    gatePassed: record.gatePassed,
    score,
  };
}

export function parseDiscoveryDoneEvent(payload: unknown): DiscoveryDoneEvent | null {
  const record = asRecord(payload);
  if (!record || !hasCurrentVersion(record)) return null;
  const sequence = asInteger(record.sequence);
  const runId = asInteger(record.runId);
  const status = asStatus(record.status);
  if (sequence == null || runId == null || status == null) return null;

  let bestStrategyId: number | null = null;
  if (!isAbsent(record.bestStrategyId)) {
    bestStrategyId = asInteger(record.bestStrategyId);
    if (bestStrategyId == null) return null;
  }
  let errorMessage: string | null = null;
  if (!isAbsent(record.errorMessage)) {
    if (typeof record.errorMessage !== 'string') return null;
    errorMessage = record.errorMessage;
  }

  return { eventVersion: DISCOVERY_EVENT_VERSION, sequence, runId, status, bestStrategyId, errorMessage };
}

// ---------- subscriptions ----------

/** Called with the raw payload when one could not be parsed. A dropped event is
 *  reported, never swallowed: the UI can tell the user its view may be stale and
 *  re-query `get_discovery_progress`, which is the source of truth. */
export type InvalidEventHandler = (channel: string, payload: unknown) => void;

function subscribe<T>(
  channel: string,
  parse: (payload: unknown) => T | null,
  onEvent: (event: T) => void,
  onInvalid?: InvalidEventHandler,
): Promise<UnlistenFn> {
  return listen<unknown>(channel, (event) => {
    const parsed = parse(event.payload);
    if (parsed == null) {
      onInvalid?.(channel, event.payload);
      return;
    }
    onEvent(parsed);
  });
}

export function onDiscoveryProgress(
  onEvent: (event: DiscoveryProgressEvent) => void,
  onInvalid?: InvalidEventHandler,
): Promise<UnlistenFn> {
  return subscribe(DISCOVERY_EVENTS.progress, parseDiscoveryProgressEvent, onEvent, onInvalid);
}

export function onDiscoveryResult(
  onEvent: (event: DiscoveryResultEvent) => void,
  onInvalid?: InvalidEventHandler,
): Promise<UnlistenFn> {
  return subscribe(DISCOVERY_EVENTS.result, parseDiscoveryResultEvent, onEvent, onInvalid);
}

export function onDiscoveryDone(
  onEvent: (event: DiscoveryDoneEvent) => void,
  onInvalid?: InvalidEventHandler,
): Promise<UnlistenFn> {
  return subscribe(DISCOVERY_EVENTS.done, parseDiscoveryDoneEvent, onEvent, onInvalid);
}

// ---------- throttling ----------

export interface Throttled<A extends unknown[]> {
  /** Deliver immediately if the window is open, else coalesce into one trailing call. */
  call: (...args: A) => void;
  /** Drop any queued trailing call. Must be called on unmount. */
  cancel: () => void;
}

/**
 * At most one delivery per `ms`, with a trailing call carrying the NEWEST args.
 *
 * The previous implementation had two defects that only a long run would expose:
 * it could not be cancelled, so a queued trailing call fired into an unmounted
 * component; and once a trailing timer was queued, a later call whose window had
 * meanwhile reopened took the leading path too, delivering the SAME pending
 * payload twice. A duplicated progress tick is invisible; a duplicated result
 * event double-appends a row. Both are fixed here: a queued timer owns the
 * window, `pending` is cleared on every delivery, and `cancel()` clears both.
 *
 * `now` is injectable so tests are deterministic; it defaults to the monotonic
 * clock, which a wall-clock adjustment cannot move backwards.
 */
export function createThrottle<A extends unknown[]>(
  fn: (...args: A) => void,
  ms: number,
  options?: { now?: () => number },
): Throttled<A> {
  const now = options?.now ?? defaultNow;
  // -Infinity, not 0: the first call must always be a leading call, whatever the
  // clock's epoch happens to be.
  let last = Number.NEGATIVE_INFINITY;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending: A | null = null;

  const deliver = (args: A): void => {
    last = now();
    pending = null;
    fn(...args);
  };

  return {
    call: (...args: A): void => {
      if (timer != null) {
        pending = args; // a trailing delivery already owns this window
        return;
      }
      const elapsed = now() - last;
      if (elapsed >= ms) {
        deliver(args);
        return;
      }
      pending = args;
      timer = setTimeout(() => {
        timer = null;
        const queued = pending;
        pending = null;
        if (queued != null) deliver(queued);
      }, Math.max(0, ms - elapsed));
    },
    cancel: (): void => {
      if (timer != null) {
        clearTimeout(timer);
        timer = null;
      }
      pending = null;
    },
  };
}

function defaultNow(): number {
  return typeof performance !== 'undefined' && typeof performance.now === 'function'
    ? performance.now()
    : Date.now();
}
