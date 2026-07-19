# Handoff: RUNNER-001 design proposal — discovery job runner program (reviewer decisions needed)

Date: 2026-07-19
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/handoff-runner-001-proposal
PR: (this handoff PR)
Status: resolved — implementation follows the Resolution below; IDENTITY-001 is first

## Summary

With PERSIST-001 merged, the Phase B pipeline (split → segmented backtests → embargo → benchmarks → Gate → Score → immutable record) is complete but only callable piece-by-piece from TypeScript. The remaining backlog item is the **Tauri backend discovery job runner**: start/pause/resume/cancel, checkpointed progress in SQLite, event protocol, and — per the SCORE-001 and PERSIST-001 Resolutions — ownership of the promotion policy and of the `discovery_runs`/`discovery_jobs` state machine.

This is the largest remaining Phase B effort and it contains one hard architectural fork (D1), so per the established governance pattern this is a Mode A proposal. **Nothing below is implemented.** The proposal decomposes the effort into a multi-slice program; RUNNER-001 is the program's first slice, not the whole program.

## Architectural ground truth (already recorded — not up for re-decision)

- `STRATEGY_DISCOVERY.md` §4 (v3 定案): long discovery tasks run in the **Tauri backend job runner**; 「backend 依 CPU 核心數開 worker thread pool 跑回測」; event protocol `discovery://progress|result|done` with jobId, frontend throttled at ~300ms/10 items; pause/resume/cancel + SQLite checkpoint so a restarted app can continue.
- Roadmap line: 「core backtest / indicator 抽離成純函數模組（前端 Worker 與 backend 可共用**邏輯規格**）」 — i.e. the backend gets its own implementation of the SAME contracts, held together by parity tests, not by sharing code.
- PERSIST-001 Resolution: `discovery_runs.config_json` stores INPUT config only; the runner owns the run → jobs state machine; if the runner needs `validation_records.discovery_run_id` linkage it adds a nullable column via its own migration.
- SCORE-001 Resolution: the runner only calls Score for `GateVerdict.pass === true`, uses lineage-final unique-hypothesis N, and defines the promotion policy.
- AGENTS.md: heavy discovery must be off the UI thread and out of the interactive Web Worker; no `eval` anywhere; Test segment never executes.

## Required Action / Decision

Answer D1–D7, then append a `## Resolution`. Implementation follows the Resolution.

### D1 — Where does discovery computation run? (the fork everything hangs on)

- **Option A (recommended — it is what v3 already mandates): phased Rust engine port with cross-language parity gates.** The backend thread pool runs a Rust implementation of the pure pipeline (indicators → params signals → backtest engine + metrics → split/embargo → benchmarks incl. seeded Random Entry → Gate → Score), each piece landed as its own slice and locked to the TS implementation by shared parity fixtures (D3) before the runner may use it. Cost: the largest engineering effort in the program; drift risk is real and is exactly what the parity harness exists to kill.
- **Option B: Rust state machine + a dedicated hidden WebView executing the existing TS pipeline.** One engine implementation, fastest to correct results; but it contradicts the v3 定案 (backtests in backend worker threads), couples long runs to a WebView lifetime, and makes checkpoint/parallelism awkward. Listed for completeness.
- **Option C: sidecar Node process running the TS pipeline.** Violates the local-first/no-extra-runtime posture and adds a supply-chain surface; listed to be rejected explicitly.

### D2 — v1 candidate space (what discovery actually enumerates)

- v1 candidates = **params-mode strategies only**: a base preset × numeric parameter grids (the `SWEEP_PARAM_KEYS` families), enumerated deterministically from the run config. `N = lineage-final unique combinations` feeds the Score data-mining penalty, computed when enumeration completes — never a running count.
- blocks-mode and AI DSL candidates are later phases; **code mode is never a discovery candidate** (manual-only contract), which also means the expression interpreter needs no Rust port in this program.

### D3 — Cross-language parity harness (the drift killer)

- TS remains the reference implementation. A generator script exports **committed JSON fixture files** (inputs + expected outputs + contract versions: seeded sample candles, indicator series, backtest trades/equity/metrics incl. METRIC-001 non-finite semantics, benchmark suite results, Random Entry distributions from the shared mulberry32, gate verdicts, score breakdowns).
- Rust tests consume the SAME fixtures; a Rust engine slice is DONE only when its fixture parity is exact (float tolerance policy to be fixed in the fixture format — propose exact for integers/flags and 1e-12 relative for floats).
- Fixtures regenerate only via the script; regeneration diffs are reviewable contract changes.

### D4 — Run config schema (`discovery_runs.config_json`, input-only)

Versioned envelope `discovery-config-v1`: dataset id + hash; base strategy (params mode); parameter grid axes; embargo holding allowance; gate config; score config + testedCombinations basis; Random Entry runs/seed policy (seed derived deterministically from run id + candidate index for reproducibility); benchmark costs source; concurrency cap; candidate cap (fail closed above it).

### D5 — State machine, jobs, checkpoint, events

- Run statuses follow the existing schema CHECK (`idle→running→paused/completed/failed/cancelled`). One `discovery_jobs` row per candidate × segment (`train`/`validation`, matching the schema); a candidate's two job rows transition together; `result_id` links each to its upserted summary.
- Checkpoint = job granularity: on resume/restart, `queued` jobs re-run; `done` jobs are skipped via the DUP-skip rule (`strategy_hash × dataset_hash × segment` — this also implements the "duplicate skip and result reuse" backlog item at runner level).
- Events per the doc: `discovery://progress` (counts + current candidate), `discovery://result` (per-candidate verdict/score digest — never full records), `discovery://done`; least-privilege capability like the pop-out windows.
- SQLite writes stay on ONE writer path (the runner thread serializes DB access; compute threads only compute) to respect the existing `Mutex<Connection>` model.

### D6 — Promotion policy (lifecycle)

- Runner-owned, automatic, per the §8 lifecycle table: Gate pass → `lifecycle = validated`; Gate fail → `rejected`; both only for runner-produced assessments (manual UI saves keep `candidate`). Score is recorded for RANKING only — no min-score/top-K cut in v1 (Results Explorer sorts by score; promotion beyond `validated` stays Phase D).
- Every runner assessment persists through the PERSIST-001 atomic bundle; migration `0003` adds nullable `validation_records.discovery_run_id` (runner-owned, per the PERSIST Resolution).

### D7 — Slice plan (each its own PR with the usual verification)

1. **RS-CORE-001** indicators + sample-candle generator parity (fixtures from D3).
2. **RS-CORE-002** backtest engine + metrics parity (incl. METRIC-001 semantics + execution contract).
3. **RS-CORE-003** split/embargo/params-signals parity.
4. **RS-CORE-004** benchmarks + Random Entry (mulberry32) parity.
5. **RS-CORE-005** Gate + Score parity (score breakdown JSON must match the TS shape byte-for-structure).
6. **RUNNER-001** state machine + queue + checkpoint + events + config schema + migration 0003 + promotion policy, computing via RS-CORE (thread pool).
7. **RUNNER-002** frontend subscription UI (progress panel) — thin, after the backend works.

RUNNER-001 (slice 6) must not start before RS-CORE-005 is merged; slices 1–5 are pure + test-only against fixtures and carry no runner risk.

## Review Notes

- The program is intentionally long; every RS-CORE slice is independently valuable (a Rust engine usable for future features) and independently verifiable.
- The Test segment remains unexecuted everywhere, including inside the runner.
- The interactive Web Worker keeps its current light-duty role; nothing in this program touches it.

## Verification

Proposal only — no code. Baseline on `main` (post-PERSIST-001, PR #65): 300 vitest + 21 Rust + 25 Playwright e2e green.

## Resolution (added when acted on)

Date: 2026-07-19. Decider: Codex (reviewer), delivered as PR #66 review guidance.
**Implementation authority: this Resolution > the original proposal wherever they differ.**

### Mandated execution order

1. Merge this resolved handoff before implementation.
2. Execute one independently reviewable slice per PR, in the D7 order below.
3. Move only the active slice to In Progress; later slices remain blocked on their predecessor's verification gate.
4. Do not start any runner execution/thread/event work before RS-CORE-005, RUNNER-CONFIG-001, and RUNNER-STORE-001 have merged.

### D0 — Durable identity prerequisite (added by review)

The existing durable identity is not safe enough for discovery reuse:

- `datasetHash()` hashes exchange/symbol/interval/bounds/version but not candle values.
- Re-importing a conflicting payload can reuse the dataset row while `INSERT OR IGNORE` retains old candles.
- `sha256Hex()` falls back to FNV when Web Crypto is unavailable, so one durable identity can use different algorithms in different runtimes.

Before any Rust parity or discovery-cache work, deliver **IDENTITY-001**:

- Durable hashes are versioned SHA-256 identities (for example `strategy-v2:` and `dataset-content-v2:`). A persisted identity must never change algorithm by runtime; absence of SHA-256 fails closed. FNV may only be an explicitly ephemeral fingerprint.
- Dataset content identity covers field-mapping/schema version, metadata, and the complete timestamp-sorted OHLCV byte representation. The backend recomputes and verifies it at import rather than trusting the frontend.
- Dataset row and candles import in one transaction. The same content hash may only reuse an identical payload; a contradictory payload is rejected, and import failure cannot leave an empty or mixed dataset.
- Legacy unversioned hashes are ineligible for discovery cross-run identity/reuse until rehashed or re-imported.
- TS and Rust v2 hash fixtures compare exactly.

If IDENTITY-001 needs a migration, the runner linkage migration uses the next available number; implementation must not permanently assume it will be `0003`.

### D1 — Computation location (final)

Adopt **Option A: phased pure-Rust engine port with committed TS-reference parity fixtures**. Reject the hidden-WebView and Node-sidecar options.

- Rust discovery-core modules remain independent of Tauri and rusqlite. Threading, DB access, and events arrive only after all parity gates pass.
- Do not port the sample-candle generator into runtime Rust. TS may use it only to produce committed fixture candle inputs; generated sample data is not discovery evaluation data.
- Preserve Rust 1.77 compatibility, the no-dynamic-code rule, and the prohibition on Test-segment execution.

### D2 — v1 candidate space (final)

- Params mode only. Blocks and AI DSL are deferred; code mode is permanently excluded.
- Config explicitly records versioned base preset definitions and whitelisted finite numeric axes. Unknown fields/keys and invalid ranges/steps fail closed.
- Preflight the raw Cartesian product, then apply cross-field validity rules and canonical `strategy-v2` hash deduplication. Record `raw`, `prunedInvalid`, `duplicates`, and `finalUnique` counts.
- Stable candidate index is assigned after sorting by canonical strategy hash, so input object/preset order and thread completion order cannot affect identity.
- Score N is the enumeration-complete `finalUnique` count, computed by the runner and shared by the whole lineage. Callers cannot supply N.
- v1 UI/default candidate cap is 256; the engine hard cap is 4096. Exceeding the raw-product cap fails before jobs are created. Changing the hard cap requires a config-contract bump and performance evidence.

### D3 — Cross-language parity harness (final)

- TypeScript remains the reference. Fixture envelopes include fixture schema version, all relevant contract versions, inputs, expected outputs, tolerance policy, and generator commit/hash.
- Integers/times/indexes/booleans/enums/array lengths and the raw PRNG u32 sequence compare exactly. JSON objects compare structurally (key order irrelevant; array order significant). METRIC-001 statuses compare exactly.
- Floats use a declared per-field policy, with a default of `abs <= 1e-12 OR rel <= 1e-10`; trade/equity/metric classes may declare reviewed overrides. Tolerance changes are contract changes.
- Compare Random Entry distributions, trades, equity, metrics, and ScoreBreakdown at numeric leaves; never require byte-string JSON equality.
- Fixtures cover the cases locked in `docs/engine-parity-report.md`: long/short/both, close/nextOpen, SL/TP, same-bar conflicts, costs, last-bar settlement, empty/range boundaries, and non-finite semantics.
- Fixtures regenerate only through an explicit script; the generated diff is reviewed like source.

### D4 — `discovery-config-v1` (final)

The strict input-only envelope records contract versions, dataset id plus content hash v2, resolved presets/axes, split contract, embargo allowance, gate and score configs, start equity, resolved benchmark costs, Random Entry runs, root seed, candidate caps, and max concurrency.

- Reject seed derivation from DB run id or candidate index. `rootSeed` is an explicit stored u32.
- Derive candidate-purpose sub-seeds with a versioned deterministic SHA-256 input such as `seed-v1 + rootSeed + datasetContentHash + strategyHash + purpose`, taking a fixed-endian u32. Row ids, thread ids, enumeration order, and completion order never participate.
- N is derived from enumeration, not accepted from config. Benchmark costs store resolved numeric values rather than only a mutable source pointer.
- Start revalidates dataset id/content hash. Unknown fields, version mismatch, non-finite values, and caps fail closed.
- Concurrency affects performance only. Default is `max(1, logicalCores - 1)` and an override must be within `1..=logicalCores`.

### D5 — State machine, jobs, checkpoint, events (final)

- The scheduling unit is one candidate assessment. Its Train and Validation job rows are paired child/checkpoint rows and transition together. Test rows never exist.
- v1 permits only one non-terminal discovery run globally (`running` or `paused`). A paused run must resume or cancel before a new run starts.
- The runner migration adds candidate index and uniqueness sufficient to prevent duplicate `(run, candidate, segment)` jobs. `validation_records` receives nullable run linkage and a uniqueness rule allowing at most one assessment for the same run/strategy/dataset.
- One candidate's final commit is one SQLite transaction: Train/Validation summaries and trades, append-only validation record, both job rows, run progress, and strategy lifecycle all commit or all roll back. Refactor the PERSIST writer to accept a caller-owned transaction; never append a record and patch job state later.
- Crash recovery changes orphan `running` runs to `paused` and paired `running` jobs to `queued`. The app never resumes CPU work automatically; the user explicitly resumes. `done` means the atomic assessment exists.
- Pause stops dequeueing and lets an in-flight candidate finish/commit. Cancel is cooperative at candidate boundaries; a result observing cancel before commit is discarded, unfinished jobs become skipped, and no partial record persists. Engine/system failure fails the run with evidence; it is not silently retried.
- Emit versioned events only after DB commit, with monotonic sequence, run/job id, and candidate index. Progress/result contain digests, and done is terminal. Compute workers share immutable candle data and never touch SQLite; one coordinator/writer owns DB writes.
- v1 duplicate handling is enumeration deduplication plus same-run completed-checkpoint skipping. **Cross-run result reuse is deferred**: current summaries are mutable UPSERT views whose key omits split/engine contract, and trades are not an immutable execution cache. Cross-run reuse requires a separate versioned immutable-cache slice.

State transitions are fixed: `idle -> running`; `running -> paused|completed|failed|cancelled`; `paused -> running|cancelled`; terminal states never resume.

### D6 — Promotion policy (final)

- Gate pass precedes Score. Score ranks only; v1 has no minimum score or top-K cutoff. Test never executes.
- `strategy_def.lifecycle` is a coarse global state: pass moves candidate/rejected to validated; fail moves only candidate to rejected. A validated strategy is not automatically demoted by a later dataset/run failure; that evidence remains in its immutable validation record. This avoids completion-order-dependent lifecycle changes.
- Manual UI saves remain candidate. Runner lifecycle updates are part of the D5 candidate transaction.
- On run completion, `best_strategy_id` is the highest finite-score Gate passer, with ties resolved by candidate index then strategy hash. It remains null when no candidate passes.

### D7 — Final one-PR slice order

0. **IDENTITY-001** — durable strategy/data hash v2, backend verification, atomic immutable dataset import.
1. **RS-CORE-001** — parity harness foundation, Rust candle/types, indicators; sample candles are fixture input only.
2. **RS-CORE-002** — backtest engine and metrics parity.
3. **RS-CORE-003** — params signals plus split/embargo parity.
4. **RS-CORE-004** — deterministic benchmarks plus mulberry32/Random Entry parity.
5. **RS-CORE-005** — Gate and Score structural parity.
6. **RUNNER-CONFIG-001** — strict config parsing, enumeration/deduplication, v2 hashes, seed derivation, and caps; pure, no DB/threads.
7. **RUNNER-STORE-001** — next migration, run/job repositories, atomic candidate commit, recovery/idempotency tests; no worker pool/events.
8. **RUNNER-EXEC-001** — fixed CPU worker pool, commands, pause/resume/cancel, single writer, and versioned events; no frontend UI.
9. **RUNNER-UI-001** — typed frontend wrappers and throttled progress/results UI.

The first implementation slice after this handoff merges is IDENTITY-001. Do not start a monolithic RUNNER-001.

### RS-CORE-001 implementation record (append-only update)

Date: 2026-07-19. Implementer: Codex. Branch:
`agent/rs-core-001-indicator-parity`. Implementation commit: `8b8b8c6`.

- Added the explicit TypeScript reference command
  `npm run fixtures:indicators` and committed
  `fixtures/rs-core/indicators-v1.json`. The envelope records source SHA-256
  hashes, contract versions, exact warm-up positions, and the resolved default
  float policy (`abs <= 1e-12 OR rel <= 1e-10`).
- Added the pure Rust `discovery_core` candle/types and indicator modules. Rust
  consumes the same fixture and covers SMA, EMA, WMA, RSI, MACD, true range,
  ATR, Bollinger Bands, population standard deviation, rolling high/low, and
  ROC. The module imports no Tauri, rusqlite, runner threads, events, or UI.
- Preserved the D1 constraint: TypeScript sample candles are committed fixture
  input only; there is no Rust runtime sample generator and no discovery
  evaluation path in this slice.
- Verification: the fixture SHA-256 was unchanged across consecutive explicit
  regenerations; 310 Vitest tests, TypeScript typecheck, production build,
  rustfmt check, cargo check, 32 Rust tests, and 25 Playwright tests pass.

RS-CORE-001 is Done. The only newly unblocked implementation slice is
RS-CORE-002 (backtest engine, trades/equity, and METRIC-001 parity); runner
orchestration remains blocked.

### RS-CORE-001 CI portability correction (append-only update)

Date: 2026-07-19. Fix commit: `0ba1631`.

The first PR #68 Linux test run exposed a metadata-only portability defect:
`sampleData.ts` was hashed from CRLF bytes by the Windows generator but from LF
bytes by the Linux Vitest checkout. Indicator inputs and expected numeric series
were unchanged; only `generator.sourceHashes.sampleData` differed. The other CI
jobs (typecheck, build, cargo-check, and E2E) passed.

Source hashing is now explicitly versioned as `utf8-lf-v1`. The generator and
freshness test share one CRLF/CR-to-LF canonicalization function before SHA-256,
the fixture records that encoding, and a direct regression test locks the
conversion. The regenerated fixture SHA-256 is
`35eef00a1494c130a12236664dba54d7704b3a568274971c928c984484cd267e`.

Local re-verification: deterministic regeneration, 311 Vitest tests,
typecheck, production build, and 32 Rust tests pass. No indicator semantics,
tolerance, runner boundary, DB, event, thread, or UI behavior changed.
