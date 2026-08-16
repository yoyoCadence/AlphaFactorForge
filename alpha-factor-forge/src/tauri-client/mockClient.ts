// DEV/TEST-ONLY in-memory mock of the Tauri data boundary.
//
// Activated only through the `dataClient` seam (`?mock=1` in Vite dev) so that
// browser E2E (Playwright) can drive the real React UI without a Tauri backend.
// This is NOT a product path: state lives in memory only — no localStorage, no
// real SQLite — and it is never used in a Tauri/production build. It does NOT
// replace real Tauri/Rust/SQLite verification (Rust integration tests +
// `cargo tauri dev` smoke still own that).

import type {
  Candle,
  Dataset,
  StrategyDef,
  BacktestSummary,
  TradeRow,
  ValidationRecordRow,
  DiscoveryProgressSnapshot,
} from './commands';
import {
  DISCOVERY_EVENTS,
  DISCOVERY_EVENT_VERSION,
  TERMINAL_RUN_STATUSES,
  parseDiscoveryDoneEvent,
  parseDiscoveryProgressEvent,
  parseDiscoveryResultEvent,
  type DiscoveryDoneEvent,
  type DiscoveryProgressCounts,
  type DiscoveryProgressEvent,
  type DiscoveryResultEvent,
  type InvalidEventHandler,
  type RunStatus,
} from './events';
import { axisValues, type DiscoveryAxis } from '../services/discoveryConfig';
import { prepareDatasetImport, type ImportCandlesInput } from './dbClient';
import { assertValidBundle } from '../services/validationRecord';
import { strategyHashFromDefinitionJson } from '../core/hashing';

/**
 * E2E-only candle-load controls, read from `?mock=1&candleDelay=<ms>` and
 * `candleFailId=<dataset-id>`.
 *
 * BUG-RESULT-CONTEXT-001 needs a deterministic in-flight dataset load to prove
 * that result actions are disabled while loading and that a late response is
 * discarded, plus a deterministic rejection to prove a failed load cannot
 * reuse the prior dataset's candles/result. Real SQLite reads are fast and
 * reliable enough that these paths are otherwise untestable. The controls live
 * inside the mock (already `import.meta.env.DEV`-gated and absent from
 * production builds), so no product path can observe them.
 */
function mockCandleDelayMs(): number {
  const search = (typeof globalThis !== 'undefined' && globalThis.location?.search) || '';
  const raw = new URLSearchParams(search).get('candleDelay');
  const ms = raw == null ? 0 : Number(raw);
  return Number.isFinite(ms) && ms > 0 ? Math.min(ms, 10_000) : 0;
}

function mockCandleFailureDatasetId(): number | null {
  const search = (typeof globalThis !== 'undefined' && globalThis.location?.search) || '';
  const raw = new URLSearchParams(search).get('candleFailId');
  const id = raw == null ? NaN : Number(raw);
  return Number.isSafeInteger(id) && id > 0 ? id : null;
}

function mockSearchParam(name: string): string | null {
  const search = (typeof globalThis !== 'undefined' && globalThis.location?.search) || '';
  return new URLSearchParams(search).get(name);
}

/** RUNNER-UI-001b-2 pacing between simulated candidates. */
function mockDiscoveryStepMs(): number {
  const raw = mockSearchParam('discoveryStep');
  const ms = raw == null ? 40 : Number(raw);
  return Number.isFinite(ms) && ms >= 0 ? Math.min(ms, 5_000) : 40;
}

/**
 * `?mock=1&discoveryRun=paused` pre-creates a paused run BEFORE the panel
 * mounts, which is the only way to observe recovered-run adoption: startup
 * recovery turns an orphaned run into `paused`, and no product path can create a
 * run and then reload the window inside one E2E.
 */
function mockPreexistingDiscoveryRun(): 'paused' | null {
  return mockSearchParam('discoveryRun') === 'paused' ? 'paused' : null;
}

export function makeMockClient() {
  const candleDelayMs = mockCandleDelayMs();
  const candleFailureDatasetId = mockCandleFailureDatasetId();
  const datasets: Dataset[] = [];
  const candlesByDs = new Map<number, Candle[]>();
  const strategies: StrategyDef[] = [];
  const summaries: BacktestSummary[] = [];
  // There is no trades reader yet, but the E2E seam still mirrors SQLite's
  // replace-on-summary-key persistence instead of silently dropping the rows.
  const tradesBySummaryId = new Map<number, TradeRow[]>();
  const validationRecords: ValidationRecordRow[] = [];
  let nextId = 1;

  const db = {
    init: async () => 'mock database ready',
    runMigrations: async () => 'mock: migrations up to date',
    getDatasets: async () => datasets.slice(),
    getCandles: async (datasetId: number, from: number, to: number) => {
      if (candleDelayMs > 0) await new Promise((resolve) => globalThis.setTimeout(resolve, candleDelayMs));
      if (datasetId === candleFailureDatasetId) {
        throw new Error(`mock candle load failed for dataset #${datasetId}`);
      }
      return (candlesByDs.get(datasetId) ?? []).filter((c) => c.timestamp >= from && c.timestamp <= to);
    },
    importCandles: async (dataset: Dataset, rows: Candle[]) => {
      const prepared = await prepareDatasetImport({
        exchange: dataset.exchange,
        symbol: dataset.symbol,
        interval: dataset.interval,
        source: dataset.source,
        candles: rows,
      });
      if (
        dataset.dataset_hash !== prepared.dataset.dataset_hash
        || dataset.start_time !== prepared.dataset.start_time
        || dataset.end_time !== prepared.dataset.end_time
        || dataset.candle_count !== prepared.dataset.candle_count
      ) {
        throw new Error('dataset identity or derived metadata mismatch');
      }
      const existing = datasets.find((row) => row.dataset_hash === dataset.dataset_hash);
      if (existing) {
        const existingRows = candlesByDs.get(existing.id!) ?? [];
        const sameDataset = existing.exchange === dataset.exchange
          && existing.symbol === dataset.symbol
          && existing.interval === dataset.interval
          && existing.start_time === dataset.start_time
          && existing.end_time === dataset.end_time
          && existing.candle_count === dataset.candle_count
          && existing.source === dataset.source;
        const sameRows = JSON.stringify(existingRows) === JSON.stringify(prepared.candles);
        if (!sameDataset || !sameRows) throw new Error('dataset hash conflicts with stored payload');
        return existing.id!;
      }
      const id = nextId++;
      datasets.push({ ...dataset, id });
      candlesByDs.set(id, prepared.candles.map((row) => ({ ...row })));
      return id;
    },
    saveStrategy: async (def: StrategyDef) => {
      const expectedHash = await strategyHashFromDefinitionJson(def.original_definition_json);
      const parsed = JSON.parse(def.original_definition_json) as Record<string, unknown>;
      if (def.strategy_hash !== expectedHash || def.type !== parsed.mode) {
        throw new Error('strategy identity mismatch');
      }
      const id = nextId++;
      strategies.push({ ...def, id });
      return id;
    },
    getStrategies: async () => strategies.slice(),
    saveBacktestResult: async (summary: BacktestSummary, trades: TradeRow[]) => {
      const existingIndex = summaries.findIndex(
        (row) => row.strategy_id === summary.strategy_id
          && row.dataset_id === summary.dataset_id
          && row.segment === summary.segment,
      );
      const existingId = existingIndex >= 0 ? summaries[existingIndex].id : undefined;
      const id = existingId ?? nextId++;
      const stored = { ...summary, id };
      if (existingIndex >= 0) summaries[existingIndex] = stored;
      else summaries.push(stored);
      tradesBySummaryId.set(id, trades.map((trade) => ({ ...trade })));
      return id;
    },
    getBacktestResults: async (strategyId?: number) =>
      summaries.filter((s) => strategyId == null || s.strategy_id === strategyId),
    // PERSIST-001 parity: runs the SAME shared bundle validator the composer
    // targets (the TS mirror of Rust's validate_validation_bundle), so
    // `?mock=1` rejects exactly the bundles native Tauri rejects, then
    // applies every write all-or-nothing.
    saveValidationRecord: async (
      trainSummary: BacktestSummary,
      trainTrades: TradeRow[],
      validationSummary: BacktestSummary,
      validationTrades: TradeRow[],
      record: ValidationRecordRow,
    ) => {
      assertValidBundle({ trainSummary, trainTrades, validationSummary, validationTrades, record });
      await db.saveBacktestResult(trainSummary, trainTrades);
      await db.saveBacktestResult(validationSummary, validationTrades);
      const id = nextId++;
      validationRecords.push({ ...record, id, created_at: new Date().toISOString() });
      return id;
    },
    // Reads return DETACHED copies so callers can never mutate the mock's
    // append-only records in place (PR #65 review).
    listValidationRecords: async (strategyId?: number) =>
      validationRecords
        .filter((r) => strategyId == null || r.strategy_id === strategyId)
        .map((r) => ({ ...r }))
        .reverse(),
    getValidationRecord: async (id: number) => {
      const row = validationRecords.find((r) => r.id === id);
      if (!row) throw new Error(`no validation record ${id}`);
      return { ...row };
    },
  };

  const files = {
    saveReport: async (suggestedFilename: string, contents: string) => {
      if (typeof document === 'undefined' || typeof Blob === 'undefined') {
        return `mock-download:${suggestedFilename}`;
      }
      const blob = new Blob([contents], { type: 'text/plain;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = suggestedFilename;
      a.style.display = 'none';
      document.body.appendChild(a);
      a.click();
      a.remove();
      globalThis.setTimeout(() => URL.revokeObjectURL(url), 0);
      return `browser-download:${suggestedFilename}`;
    },
  };

  const importDataset = async (input: ImportCandlesInput): Promise<number> => {
    const prepared = await prepareDatasetImport(input);
    return db.importCandles(prepared.dataset, prepared.candles);
  };

  // ---------- RUNNER-UI-001b-2: fake discovery runner ----------
  //
  // Emits REAL `discovery-event-v1` wire payloads (camelCase, omitted optionals,
  // explicit null score on gate failure) through the same subscription functions
  // the production panel uses, so the E2E exercises the actual parsers rather
  // than a convenient shape. It simulates the runner's observable contract only:
  // a monotonic sequence, progress counts, one result per candidate, and a
  // terminal done event whose sequence is the highest of the run.

  const discoveryStepMs = mockDiscoveryStepMs();
  type MockRun = {
    runId: number;
    name: string;
    status: RunStatus;
    total: number;
    completed: number;
    bestStrategyId: number | null;
    errorMessage: string | null;
    nextCandidate: number;
    timer: ReturnType<typeof globalThis.setTimeout> | null;
  };
  const progressHandlers = new Set<(event: DiscoveryProgressEvent) => void>();
  const resultHandlers = new Set<(event: DiscoveryResultEvent) => void>();
  const doneHandlers = new Set<(event: DiscoveryDoneEvent) => void>();
  let sequence = 0;
  let mockRun: MockRun | null = null;

  const counts = (run: MockRun): DiscoveryProgressCounts => ({
    totalCandidates: run.total,
    queuedCandidates: Math.max(0, run.total - run.completed - (run.status === 'running' ? 1 : 0)),
    runningCandidates: run.status === 'running' && run.completed < run.total ? 1 : 0,
    completedCandidates: run.completed,
    failedCandidates: 0,
    skippedCandidates: 0,
  });

  const snapshot = (run: MockRun): DiscoveryProgressSnapshot => ({
    version: 'discovery-progress-v1',
    runId: run.runId,
    name: run.name,
    status: run.status,
    counts: counts(run),
    currentCandidateIndexes: run.status === 'running' && run.completed < run.total ? [run.completed] : [],
    bestStrategyId: run.bestStrategyId,
    errorMessage: run.errorMessage,
    lastEventSequence: sequence,
  });

  /** Payloads are built as plain records so an omitted optional is genuinely
   *  absent, exactly as `skip_serializing_if` produces on the Rust side. */
  function emitProgress(run: MockRun, candidateIndex: number | null): void {
    sequence += 1;
    const payload: Record<string, unknown> = {
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence,
      runId: run.runId,
      status: run.status,
      counts: counts(run),
    };
    if (candidateIndex != null) {
      payload.candidate = {
        candidateIndex,
        strategyId: 1000 + candidateIndex,
        datasetId: 1,
        jobIds: { train: 2 * candidateIndex + 1, validation: 2 * candidateIndex + 2 },
      };
    }
    if (run.bestStrategyId != null) payload.bestStrategyId = run.bestStrategyId;
    for (const handler of progressHandlers) handler(payload as unknown as DiscoveryProgressEvent);
  }

  function emitResult(run: MockRun, candidateIndex: number): void {
    sequence += 1;
    // Every other candidate fails the Gate, so the E2E sees both a scored row
    // and the explicit-null-score row.
    const gatePassed = candidateIndex % 2 === 0;
    const payload = {
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence,
      runId: run.runId,
      candidateIndex,
      jobIds: { train: 2 * candidateIndex + 1, validation: 2 * candidateIndex + 2 },
      strategyId: 1000 + candidateIndex,
      strategyHash: `strategy-v2:${String(candidateIndex).padStart(64, '0')}`,
      datasetId: 1,
      validationRecordId: 500 + candidateIndex,
      gatePassed,
      score: gatePassed ? Number((0.5 + candidateIndex / 100).toFixed(4)) : null,
    };
    if (gatePassed) run.bestStrategyId = payload.strategyId;
    for (const handler of resultHandlers) handler(payload as unknown as DiscoveryResultEvent);
  }

  function emitDone(run: MockRun): void {
    sequence += 1;
    const payload: Record<string, unknown> = {
      eventVersion: DISCOVERY_EVENT_VERSION,
      sequence,
      runId: run.runId,
      status: run.status,
    };
    if (run.bestStrategyId != null) payload.bestStrategyId = run.bestStrategyId;
    if (run.errorMessage != null) payload.errorMessage = run.errorMessage;
    for (const handler of doneHandlers) handler(payload as unknown as DiscoveryDoneEvent);
  }

  function stopTimer(run: MockRun): void {
    if (run.timer != null) {
      globalThis.clearTimeout(run.timer);
      run.timer = null;
    }
  }

  function step(): void {
    const run = mockRun;
    if (!run || run.status !== 'running') return;
    run.timer = null;
    if (run.nextCandidate >= run.total) {
      run.status = 'completed';
      emitProgress(run, null);
      emitDone(run);
      return;
    }
    const candidateIndex = run.nextCandidate;
    run.nextCandidate += 1;
    emitResult(run, candidateIndex);
    run.completed += 1;
    emitProgress(run, candidateIndex);
    schedule(run);
  }

  function schedule(run: MockRun): void {
    stopTimer(run);
    run.timer = globalThis.setTimeout(step, discoveryStepMs);
  }

  /** Candidate count of the submitted envelope's single axis, using the same
   *  `axisValues` the real enumerator uses; no axes means one candidate. */
  function plannedCandidates(config: unknown): number {
    const bases = (config as { bases?: { axes?: DiscoveryAxis[] }[] })?.bases ?? [];
    const axes = bases[0]?.axes ?? [];
    return axes.length === 0 ? 1 : axisValues(axes[0]).length;
  }

  const discovery = {
    start: async (config: unknown) => {
      if (mockRun != null && !isTerminalMockStatus(mockRun.status)) {
        throw new Error('mock: a discovery run is already active');
      }
      const runId = nextId++;
      mockRun = {
        runId,
        name: `mock run #${runId}`,
        status: 'running',
        total: plannedCandidates(config),
        completed: 0,
        bestStrategyId: null,
        errorMessage: null,
        nextCandidate: 0,
        timer: null,
      };
      emitProgress(mockRun, null);
      schedule(mockRun);
      return runId;
    },
    pause: async (runId: number) => {
      const run = requireMockRun(runId);
      stopTimer(run);
      run.status = 'paused';
      emitProgress(run, null);
    },
    resume: async (runId: number) => {
      const run = requireMockRun(runId);
      run.status = 'running';
      emitProgress(run, null);
      schedule(run);
    },
    cancel: async (runId: number) => {
      const run = requireMockRun(runId);
      stopTimer(run);
      run.status = 'cancelled';
      emitProgress(run, null);
      emitDone(run);
    },
    progress: async (runId: number) => snapshot(requireMockRun(runId)),
    getActiveRun: async () =>
      mockRun != null && !isTerminalMockStatus(mockRun.status) ? snapshot(mockRun) : null,
  };

  function requireMockRun(runId: number): MockRun {
    if (mockRun == null || mockRun.runId !== runId) throw new Error(`mock: no discovery run ${runId}`);
    return mockRun;
  }

  const discoveryEvents = {
    onProgress: async (
      onEvent: (event: DiscoveryProgressEvent) => void,
      onInvalid?: InvalidEventHandler,
    ) => subscribeMock(progressHandlers, parseDiscoveryProgressEvent, DISCOVERY_EVENTS.progress, onEvent, onInvalid),
    onResult: async (
      onEvent: (event: DiscoveryResultEvent) => void,
      onInvalid?: InvalidEventHandler,
    ) => subscribeMock(resultHandlers, parseDiscoveryResultEvent, DISCOVERY_EVENTS.result, onEvent, onInvalid),
    onDone: async (
      onEvent: (event: DiscoveryDoneEvent) => void,
      onInvalid?: InvalidEventHandler,
    ) => subscribeMock(doneHandlers, parseDiscoveryDoneEvent, DISCOVERY_EVENTS.done, onEvent, onInvalid),
  };

  /** The mock parses its own payloads with the PRODUCTION parsers, so the E2E
   *  proves the contract end to end instead of trusting the mock's shape. */
  function subscribeMock<T>(
    handlers: Set<(event: T) => void>,
    parse: (payload: unknown) => T | null,
    channel: string,
    onEvent: (event: T) => void,
    onInvalid?: InvalidEventHandler,
  ): () => void {
    const handler = (payload: T): void => {
      const parsed = parse(payload);
      if (parsed == null) {
        onInvalid?.(channel, payload);
        return;
      }
      onEvent(parsed);
    };
    handlers.add(handler);
    return () => handlers.delete(handler);
  }

  if (mockPreexistingDiscoveryRun() === 'paused') {
    const runId = nextId++;
    mockRun = {
      runId,
      name: `recovered run #${runId}`,
      status: 'paused',
      total: 4,
      completed: 1,
      bestStrategyId: null,
      errorMessage: null,
      nextCandidate: 1,
      timer: null,
    };
    sequence = 3;
  }

  return { db, files, importDataset, isTauri: () => true, discovery, discoveryEvents };
}

function isTerminalMockStatus(status: RunStatus): boolean {
  return TERMINAL_RUN_STATUSES.includes(status);
}
