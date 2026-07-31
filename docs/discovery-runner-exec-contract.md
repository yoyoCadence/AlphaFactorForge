# Discovery Runner Execution Contract

Status: authoritative implementation contract for `RUNNER-EXEC-001`.

This contract turns the resolved discovery-runner decisions into the smallest
backend execution slice. Existing persistence and discovery-domain contracts
remain authoritative unless this document narrows them.

## 1. Scope

`RUNNER-EXEC-001` owns backend orchestration for one discovery run:

- create and execute one paired Train/Validation assessment per candidate;
- expose lifecycle commands and recoverable progress;
- checkpoint candidate results through the existing atomic persistence path;
- publish committed progress, candidate-result, and terminal events.

It does not own frontend wrappers or UI. Those belong to `RUNNER-UI`.

## 2. Canonical commands

The Tauri command names are:

```text
start_discovery({ config }) -> runId
pause_discovery({ runId }) -> void
resume_discovery({ runId }) -> void
cancel_discovery({ runId }) -> void
get_discovery_progress({ runId }) -> DiscoveryProgressV1
get_active_discovery_run() -> DiscoveryProgressV1 | null
```

Frontend invoke arguments use the camel-case field names shown above. Rust may
use snake case internally, but command serialization must preserve this
external contract.

`get_active_discovery_run` returns the only `running` or `paused` run, if one
exists. Startup recovery must make a previously interrupted run observable here
as `paused`.

The backend, not the caller, assigns the run name:

```text
discovery-{datasetId}-{hashPrefix}
```

`hashPrefix` is the first 12 lowercase hexadecimal characters following the
`dataset-content-v2:` prefix in the validated dataset content hash. The name is
deterministic and is presentation metadata, not identity.

## 3. Runtime ownership

Each started or resumed run gets a new, fixed-size standard-library worker pool.
The resolved concurrency is fixed for that pool's lifetime. There is one
coordinator and exactly that many compute workers; there is no global pool and
no worker reuse across runs.

Workers receive immutable run inputs, candle data, and candidate inputs. A
worker:

- performs only deterministic compute;
- returns one in-memory assessment outcome for one candidate;
- never receives a SQLite connection, repository, `AppHandle`, or mutable
  scheduler state;
- never writes the database and never emits an event.

The coordinator is the single writer. It alone:

- claims paired jobs;
- dispatches candidate assessments;
- serializes database mutations;
- assigns event sequence numbers;
- commits candidate outcomes;
- publishes post-commit events.

One scheduling unit is one candidate assessment containing its paired Train and
Validation jobs. A pair is claimed, resumed, completed, failed, or skipped
together. Candidate execution uses dataset length to derive the split and then
restricts evaluation to the Train-through-Validation candle slice; hidden Test
candle values are not scanned by timestamp validation, signals, backtests,
benchmarks, Gate, or Score.

Exactly one `discovery://result` event may be emitted for a candidate, and only
after the candidate's paired assessment has been committed atomically.

## 4. Lifecycle ordering

### Start

`start_discovery` must strictly parse the stored input, revalidate dataset ID
and content-hash identity, deterministically enumerate candidates, create the
run and paired jobs, transition the run to `running`, and then start its pool.
Execution-boundary validation failures after the run exists must use the
persisted failure path; they must not leave a nonterminal orphan.

### Pause

Pause ordering is mandatory:

1. record `pauseRequested` in coordinator memory;
2. stop claiming new candidate pairs;
3. keep the persisted run status `running` while all in-flight candidates
   finish and commit;
4. when no candidate is in flight, persist `paused`, or persist `completed` if
   the final candidate completed during the drain;
5. return from `pause_discovery` only after that state commit.

This ordering prevents valid in-flight results from being rejected by a
premature `paused` transition.

### Resume

Only a `paused` run may resume. Resume reparses the run's stored input,
revalidates its dataset identity, re-enumerates deterministically, and starts a
new fixed-size pool. It dispatches only queued paired jobs and never recomputes
already committed candidate assessments.

### Cancel and fail

Cancellation is cooperative. The coordinator first commits the terminal
`cancelled` state and skips unfinished pairs. Any worker result arriving after
that commit is discarded: it produces no database result, no progress success,
and no `discovery://result` event.

Failure follows the same late-result rule. A terminal `failed` state must retain
the actionable failure message and mark unfinished pairs consistently before a
terminal event is emitted.

The existing `random-entry-v1` contract fails closed when a candidate has zero
closed Validation trades. EXEC preserves that contract: it fails the run with
evidence rather than inventing a holding-period distribution. Treating this
case as an ordinary Gate rejection would require an explicit versioned change
to the benchmark and validation-record contracts, not a runner-only shortcut.

### Startup recovery

Startup recovery performs persistence repair only:

- `running` runs become `paused`;
- their `running` jobs return to `queued`;
- no pool is created and no CPU work starts automatically.

The user must explicitly call `resume_discovery`.

## 5. Persisted progress

`discovery_runs.progress_json` is a compact durable checkpoint, not a
denormalized copy of job state. It stores:

```json
{
  "version": "discovery-progress-v1",
  "enumeration": {
    "raw": 24,
    "prunedInvalid": 2,
    "duplicates": 2,
    "finalUnique": 20
  },
  "totalCandidates": 20,
  "completedCandidates": 3,
  "lastEventSequence": 7
}
```

Each successful candidate updates this checkpoint in the same transaction as
its assessment. Start, resume, and drained pause also checkpoint it before
publishing their progress event. `lastEventSequence` is therefore the last
durably checkpointed progress sequence; a terminal done event can have a later
sequence because terminal state itself is authoritative in the run row and
cannot resume.

`get_discovery_progress` and `get_active_discovery_run` never trust duplicated
status counts from JSON. They derive the live snapshot from the run and paired
job rows:

```json
{
  "version": "discovery-progress-v1",
  "runId": 42,
  "name": "discovery-9-a1b2c3d4e5f6",
  "status": "running",
  "counts": {
    "totalCandidates": 20,
    "queuedCandidates": 15,
    "runningCandidates": 2,
    "completedCandidates": 3,
    "failedCandidates": 0,
    "skippedCandidates": 0
  },
  "currentCandidateIndexes": [3, 4],
  "bestStrategyId": null,
  "errorMessage": null,
  "lastEventSequence": 7
}
```

The five state counts must sum to `totalCandidates`. SQLite remains the only
progress source of truth; the WebView owns no shadow checkpoint.

## 6. Event contract

All discovery event payloads use `eventVersion: "discovery-event-v1"`.
Channels and exact payloads are:

### `discovery://progress`

```json
{
  "eventVersion": "discovery-event-v1",
  "sequence": 9,
  "runId": 42,
  "status": "running",
  "counts": {
    "totalCandidates": 20,
    "queuedCandidates": 14,
    "runningCandidates": 2,
    "completedCandidates": 4,
    "failedCandidates": 0,
    "skippedCandidates": 0
  },
  "candidate": {
    "candidateIndex": 3,
    "strategyId": 55,
    "datasetId": 9,
    "jobIds": {
      "train": 103,
      "validation": 104
    }
  }
}
```

`candidate` is omitted when progress is caused only by a lifecycle transition.
`bestStrategyId` is optional and is omitted while no stored Gate passer exists.

### `discovery://result`

```json
{
  "eventVersion": "discovery-event-v1",
  "sequence": 8,
  "runId": 42,
  "candidateIndex": 3,
  "jobIds": {
    "train": 103,
    "validation": 104
  },
  "strategyId": 55,
  "strategyHash": "strategy-v2:...",
  "datasetId": 9,
  "validationRecordId": 301,
  "gatePassed": true,
  "score": 0.731
}
```

`score` is `null` when Gate rejects the candidate. The event is a compact digest
of the committed candidate assessment, not a replacement for persisted audit
records.

### `discovery://done`

```json
{
  "eventVersion": "discovery-event-v1",
  "sequence": 10,
  "runId": 42,
  "status": "completed",
  "bestStrategyId": 55
}
```

`status` is exactly one of `completed`, `failed`, or `cancelled`.
`bestStrategyId` is optional. `errorMessage` is present only for `failed`.

For every channel:

- reserve a strictly increasing per-run `sequence`; candidate and progress
  checkpoints persist their reserved sequence before emission;
- commit and release the database guard;
- call `emit_to("main", channel, payload)`;
- never roll back or fail the run solely because delivery failed.

Candidate completion publishes `discovery://result` and then
`discovery://progress`, each with its own increasing sequence. Terminal state
publishes exactly one `discovery://done`. Gaps are allowed after a failed commit
or delivery; reuse or regression is not. No success event is emitted for a late
result discarded after cancel or fail.

## 7. Explicit exclusions

`RUNNER-EXEC-001` does not include:

- TypeScript invoke wrappers, event listeners, state management, rendering, or
  throttling; those are `RUNNER-UI`;
- cross-run candidate-result reuse or a shared worker pool;
- Test-segment reads, tuning, ranking, prompts, or events;
- blocks mode, code mode, AI generation, regime analysis, paper trading, or
  live trading;
- Node sidecars, hidden WebViews, frontend Web Workers, or automatic recovery
  resume;
- schema work beyond what the existing discovery persistence contract requires.

Within-run checkpoint reuse after pause or restart is required. Cross-run reuse
is not.
