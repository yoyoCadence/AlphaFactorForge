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

### RS-CORE-002 implementation record (append-only update)

Date: 2026-07-19. Implementer: Claude. Branch:
`feat/rs-core-002-backtest-parity`.

- Added `src/parity/backtestFixture.ts`, `npm run fixtures:backtest`, and the
  committed `backtest-parity-v1` envelope: 17 behaviour cases covering the
  `docs/engine-parity-report.md` semantics (long/short/both × close/nextOpen,
  same-bar conflicts, `both` entry-wins, gap-aware SL/TP with SL-first
  ambiguity, fee-budgeted full sizing, EOD settlement, from/to boundaries,
  zero trades, METRIC-001 `+Infinity` statuses, and two 180-day multi-month
  sample cases with risk exits) plus 3 fail-closed config error cases.
  Expected outputs come from the real TypeScript engine; generation-time
  sanity invariants reject degenerate scenarios.
- Added pure Rust `discovery_core/backtest.rs` (`backtest-execution-v1`) and
  `discovery_core/metrics.rs` (`metrics-v1`) with no Tauri, rusqlite, thread,
  event, or UI dependency. The parity tests lock trades (timestamps/side/bars
  exact, prices/PnL within the declared tolerance), full equity curves, every
  metric leaf including exact non-finite statuses and UTC monthly-return keys,
  and the exact error-message fragments. The Test segment is never executed.
- Verification: fixture regeneration is blob-identical across consecutive
  runs; 313 Vitest tests, TypeScript typecheck, production build, cargo check,
  and 34 Rust tests pass. Playwright is untouched (no UI/mock surface).

RS-CORE-002 is Done pending merge. The only newly unblocked implementation
slice is RS-CORE-003 (params signals plus split/embargo parity); runner
orchestration remains blocked.

### RS-CORE-002 review correction (append-only update)

Date: 2026-07-19. Fix for the PR #69 Codex review findings.

- The 3 fail-closed error cases are now HELD by the TypeScript reference:
  fixture generation and the vitest freshness test both execute them against
  the real TS engine and require a `RangeError` whose message contains the
  recorded fragment, before Rust consumes the same fixture.
- Added the Resolution-mandated empty boundaries as cross-language cases:
  `empty-candles-boundary` (no candles at all) and
  `inverted-range-empty-evaluation` (a from/to pair evaluating no bar,
  totalBars < 0 semantics included). The correct success-case count is **20**
  (the earlier record said 17 while the fixture held 18); TS and Rust now
  both assert the exact 20-case inventory by id, so an accidental deletion
  fails on either side.
- The new discovery_core Rust files now pass a targeted `rustfmt --check`
  (repo-wide legacy fmt debt untouched).
- Adopted: `sourceHashEncoding` now reuses RS-CORE-001's
  `FIXTURE_SOURCE_HASH_ENCODING` constant, asserted in the freshness test.
  Carried, not absorbed: the RS-CORE-001 indicator edge-fixture suggestion
  (constant RSI, period ≥ length, ROC zero base) remains open for a later
  indicator-fixture change; the `metrics.rs` UTC `.expect()` must become
  propagated fail-closed validation before RUNNER-EXEC wires execution paths
  (boundary comment added in code).
- Re-verification: fixture regeneration blob-identical; 315 Vitest tests,
  typecheck, production build, cargo check, 34 Rust tests, targeted rustfmt
  check all pass.

### RS-CORE-003 implementation record (append-only update)

Date: 2026-07-20. Implementer: Claude. Branch:
`feat/rs-core-003-signals-split-parity`.

- Added `src/parity/signalsSplitFixture.ts`, `npm run fixtures:signals-split`,
  and the committed `signals-split-parity-v1` envelope: 7 params-signal cases
  (hand-verified exact MA-cross index + one sample case per signal family),
  9 split cases (all five usable-bar residues, zero/non-zero embargo, the
  JS safe-integer extreme), 7 embargo-derivation cases, and 8 error cases
  HELD by the TypeScript reference (generation and the vitest freshness test
  execute the real TS functions and require the recorded fragment). Every
  expected leaf is exact — booleans and integers only; both languages assert
  the exact case inventories by id.
- Added pure Rust `discovery_core/signals.rs` (`params-signals-v1`),
  `split.rs` (`validation-split-v1`), and `embargo.rs`
  (`embargo-derivation-v1`), all free of Tauri/rusqlite/threads/events/UI.
  Per D2, only params mode is ported: blocks/code signal building and
  lookback derivation stay TypeScript-only and the expression interpreter is
  not ported; unsupported ids fail closed with the TS message.
- Verification: fixture regeneration blob-identical; 318 Vitest tests,
  typecheck, production build, cargo check, 38 Rust tests (4 new parity
  suites passing first run), and targeted rustfmt check on the new files all
  pass. Playwright untouched (no UI/mock surface).

RS-CORE-003 is Done pending merge. The only newly unblocked implementation
slice is RS-CORE-004 (deterministic benchmarks + mulberry32/Random Entry
parity); runner orchestration remains blocked.

### RS-CORE-003 review correction (append-only update)

Date: 2026-07-19. Fix for the PR #70 Codex review findings. Chronology
correction first: the RS-CORE-003 implementation record above misstated its
date as 2026-07-20; the correct date is 2026-07-19 (this correction is
append-only, the original text is retained as written).

- [P1] Safe-integer boundary parity: TypeScript `embargo.ts` now applies
  `safeLookback` checked arithmetic to every DERIVED lookback (+1/+2, MACD
  composite, blocks/code cross bonuses) and to the final
  `lookback + holdingAllowanceBars`, failing closed where IEEE-754 would
  silently round past `Number.MAX_SAFE_INTEGER`. Rust `embargo.rs` rejects
  raw periods above `JS_MAX_SAFE_INTEGER` BEFORE any `usize -> i64`
  conversion (the previous `as i64` could wrap) and uses checked adds with
  the same bound on every intermediate and the final sum. Both sides emit
  identical error fragments.
- [P1] Fixture boundary cases added, all HELD by the TS reference:
  `embargo-period-above-safe-range` (raw period MAX_SAFE+1),
  `embargo-derived-lookback-overflow` (legal MAX_SAFE period whose +1
  derivation leaves the safe range — the reviewer's RSI reproduction),
  `embargo-allowance-overflow` (final sum overflow), and the SUCCESS case
  `embargo-exact-safe-boundary` (embargoBars lands exactly on
  MAX_SAFE_INTEGER, asserted in the freshness test).
- [P2] Rust now asserts the exact ordered error-case ID inventories for all
  three groups (signals/split/embargo), not just counts.
- [P2] The `embargo.rs` module doc no longer claims non-params usage "fails
  closed here by construction": the recorded contract is that RUNNER-CONFIG
  must reject non-params candidate modes before this module, which accepts an
  already-validated params-only projection (unsupported signal ids still fail
  closed). Docs/PR wording corrected to say EXPECTED OUTPUT leaves are exact
  while inputs carry floats.
- Re-verification: fixture regenerated (blob-identical across consecutive
  runs); vitest, typecheck, build, cargo check, cargo tests, and targeted
  rustfmt all pass — exact counts recorded in the PR thread.

### RS-CORE-003 second review correction (append-only update)

Date: 2026-07-19. Fix for the PR #70 third-round finding: post-hoc
`isSafeInteger` cannot detect an intermediate IEEE-754 rounding that a later
subtraction cancels (`a + b - 1`). All derived lookback additions and the
final embargo sum now go through a PRE-checked `safeAdd` (overflow tested
before the add), and the MACD composite is reassociated as
`slowest + (signalPeriod - 1)`. The reviewer's blocks `macdHist > 0`
reproduction and its code-mode twin are locked as TypeScript unit
regressions (blocks/code stay TS-only, so they do not enter the Rust
fixture); the exact-`MAX_SAFE` composite remains accepted. Current fixture
ledger for one-glance reading: 7 signal + 9 split + 8 embargo success cases
and 11 error cases (1 signal + 4 split + 6 embargo); expected OUTPUT leaves
are exact booleans/integers while inputs still carry floats.

### RS-CORE-004 implementation record (append-only update)

Date: 2026-07-22. Implementer: Codex. Branch:
`feat/rs-core-004-benchmark-parity`.

- Added `src/parity/benchmarkFixture.ts`, `npm run fixtures:benchmarks`, and
  the committed `benchmark-parity-v1` envelope: 5 raw-u32 mulberry32 cases,
  4 deterministic-suite cases, 2 Random Entry planner cases, 6 seeded
  Random Entry integration cases, and 8 error cases HELD by the TypeScript
  reference. The case inventory positively exercises SMA 50/200, exact
  benchmark strategy structure, trades/equity/metrics, clipping/drop
  behavior, the unknown/prototype-key interval fallback, the zero-bar holding
  clamp, subranges, default runs, strict ties, accepted seed/run endpoints,
  and both negative and above-safe seeds.
- Added pure Rust `discovery_core/benchmarks.rs` (`benchmark-suite-v1`),
  `prng.rs` (`mulberry32-v1`), and `random_entry.rs` (`random-entry-v1`). Raw
  PRNG words and planner indexes compare exactly; strategy JSON objects,
  benchmark results, and distributions compare structurally with the D3
  numeric-leaf tolerance and exact METRIC-001 statuses. Shared TypeScript and
  Rust parity helpers remove the prior duplicate metrics codecs/comparators.
- Scope remains pure/test-only: no runner, SQLite, threads, events, UI, or
  hidden Test-segment execution. Fixture source provenance includes every
  direct computation dependency and the shared non-finite encoder.
- Verification: backtest and benchmark fixtures regenerate blob-identically;
  324 Vitest tests, typecheck, production build, cargo check, 42 Rust tests,
  targeted rustfmt check, and `git diff --check` pass. Playwright is untouched
  because this slice adds no UI or mock surface.

RS-CORE-004 is Done locally. The only newly unblocked implementation slice is
RS-CORE-005 (Gate + Score structural parity); runner orchestration remains
blocked by the mandated slice order.

### RS-CORE-005 implementation record (append-only update)

Date: 2026-07-22. Implementer: Codex. Branch:
`feat/rs-core-005-gate-score-parity`.

- Added `src/parity/gateScoreFixture.ts`, `npm run fixtures:gate-score`, and
  the committed `gate-score-parity-v1` envelope: 6 params-only complexity
  cases covering all 12 supported signal ids, 17 JSON-safe encoded Gate
  verdict cases, 4 complete `score-v1` breakdown cases, 7 Gate errors, and 11
  Score errors HELD by the TypeScript reference. Exact inventories and output
  structure lock criterion/component/penalty order, statuses, evidence,
  details, integer boundaries, and error fragments; only finite non-integer
  leaves use the declared tolerance. The fixture's SHA-256 is
  `0e064d14a7385f0137449b141eb2c79cdaf363f25da3acd6e52d1d1db865bf7f`
  before and after regeneration.
- Added pure Rust `discovery_core/gate.rs` (`gate-v1`) and `score.rs`
  (`score-v1`) plus a five-test fixture consumer. Gate exposes raw and
  JSON-safe encoded verdicts; Score accepts a params-only projection by
  construction. Coverage includes default/full/partial configuration,
  duplicate/missing benchmarks, strict Gate boundaries, UTC concentration,
  non-finite evidence, JavaScript MAX_SAFE limits, all params signal families,
  negative-zero normalization, extreme finite population sigma, and
  finite-weight aggregate overflow.
- Hardened the TypeScript reference before freezing parity: Gate scalar,
  rolling, concentration, benchmark, and percentile evidence now fails closed
  when malformed/non-finite, deterministic benchmark ids must be unique, Score
  uses scale-normalized population sigma, canonicalizes negative zero, and
  throws instead of emitting a non-finite aggregate score. The Gate contract
  version now has one owner in `gate.ts` and is re-exported by persistence.
- Scope remains pure/test-only: no runner, SQLite, threads, events, UI,
  blocks/code discovery candidates, or hidden Test-segment execution.
  Verification: fixture regeneration blob-identical; 337 Vitest tests,
  TypeScript typecheck, production build, cargo check, 47 Rust tests, targeted
  rustfmt checks, 25 Playwright tests, and `git diff --check` pass. Clippy was
  intentionally not run.

RS-CORE-005 is Done locally. The only newly unblocked slice is
RUNNER-CONFIG-001 (pure config parsing/enumeration/deduplication/hashes/seeds/
caps); DB/thread/event/UI runner work remains blocked by the mandated order.

### RS-CORE-005 review correction (append-only update)

Date: 2026-07-22. The original implementation record above is retained as
historical context; this correction supersedes its fixture counts, hash, and
Vitest total after final independent review.

- The final fixture inventory is 6 params-only complexity cases, 22 encoded
  Gate verdict cases, 4 complete Score cases, 16 Gate errors, and 11 Score
  errors. Added locks cover invalid JavaScript TimeClip dates, precise
  fail-closed audit details, fractional/negative/non-finite/unsafe trade
  counts, finite concentration-ratio overflow, and negative-zero Score
  weights/contributions/final score.
- The final committed fixture SHA-256 is
  `5151b9409f79424eec2974489d43328b2cca18ac0dcbf40ce7fa94074a3ffa2e`;
  consecutive regenerations remain blob-identical.
- Final verification totals are 339 Vitest tests, 47 Rust tests, and 25
  Playwright tests, alongside typecheck, production build, cargo check,
  targeted rustfmt checks, and `git diff --check`. Clippy remains
  intentionally unrun.


### RUNNER-CONFIG-001 implementation record (append-only update)

Date: 2026-07-26. Implementer: Claude. Branch:
`feat/runner-config-001-enumeration`.

- Added the pure TypeScript reference `src/services/discoveryConfig.ts`
  (`discovery-config-v1`), `src/services/candidateEnumeration.ts`
  (`discovery-enumeration-v1`), and `src/services/discoverySeed.ts`
  (`seed-v1`), plus the pure Rust ports `discovery_core/config.rs`,
  `enumerate.rs`, `seed.rs`, and `identity.rs`. No SQLite, thread, Tauri
  event, UI, or hidden Test-segment path was added on either side.
- D4 admission: exact key sets at every level (missing keys reported in
  declaration order, then unknown keys sorted, so both languages report the
  same problem first); contract-version pinning whose expected values the Rust
  port reads from the owning modules; durable `dataset-content-v2` identity
  required; resolved numeric `benchmarkCosts` that must equal every base
  preset's costs (a contradiction would make the benchmark fairness
  convention unauditable); full resolved Gate/Score configs validated with the
  owning modules' own messages (Rust delegates to `gate::resolve_gate_config`);
  explicit `rootSeed` u32; `caps.candidates` in `1..=4096` with 256 as the
  default budget; `maxConcurrency` null → `max(1, logicalCores - 1)` or an
  override bounded by `1..=logicalCores`, with `logicalCores` passed in so the
  module stays pure.
- D2 candidate space: params mode only. This module is where blocks/code
  candidates are rejected, which is the recorded contract that lets
  `embargo.rs` and `signals.rs` assume an already-validated params projection.
  The axis whitelist is the indicator and risk parameters; `feePct`,
  `slipPct`, and `sizePct` are execution model and are deliberately NOT
  axis-eligible. Axis values are inclusive `min + i*step` computed by
  multiplication (never `+= step`), capped at 64 per axis, and every generated
  value must satisfy its key's domain.
- Enumeration: the raw Cartesian product is preflighted against the cap BEFORE
  any candidate is built, so an over-budget run can never create jobs; the
  fixed-order cross-field rules `fastMA<slowMA`, `macdFast<macdSlow`,
  `rsiBuy<rsiSell` PRUNE (they are the expected outcome of a legal grid, not a
  malformed config); canonical `strategy-v2` hashing deduplicates across all
  bases with first-occurrence provenance; survivors sort by hash and only then
  receive `index = 0..n-1`; and `raw = prunedInvalid + duplicates +
  finalUnique` is asserted rather than trusted. `testedCombinations` is
  `{ n: finalUnique, basis: 'lineage-final-unique' }`, derived here and never
  accepted from config.
- Parity: `src/parity/runnerConfigFixture.ts` + `npm run fixtures:runner-config`
  own the committed `runner-config-parity-v1` envelope. Its declared
  `expectedNumericPolicy` is `exact-v1` — this slice admits NO tolerance,
  because every expected leaf is an identifier, counter, index, or an axis
  value both languages derive with identical IEEE-754 operations. One axis case
  deliberately locks accumulated drift (`0.30000000000000004`, not `0.3`).
  Inventory: 5 seed cases (exact preimage bytes + derived u32), 6 axis cases,
  5 concurrency cases, 2 complete resolved configs, 6 complete candidate plans
  (including a base-order-reversed twin proving declaration order changes
  neither identity, index, nor seed), and 54 TypeScript-held error cases
  (6 seed + 1 axis + 4 concurrency + 43 admission).
- Recorded follow-up, NOT taken in this slice: `discovery_core/identity.rs` and
  the binary crate's `identity` module now hold two copies of the canonical
  `strategy-v2` encoder. The split is forced by the crate boundary (discovery
  core must not reach `AppError`/rusqlite row types, and `lib.rs` exports only
  `discovery_core`). Both are locked to the SAME committed
  `src/core/hashing/identity-v2.fixture.json` in their own test binaries.
  Consolidating them touches the product write boundary and should be its own
  reviewed change.
- Verification: fixture regeneration blob-identical across consecutive runs
  (SHA-256 `c484523638c35e7bb7ef052c9cf83fbf04e5c4e35de0be3caba1304c9d76e2a2`);
  `npm run typecheck`; `npm test` (373); `npm run build`;
  `cargo check --locked`; `cargo test --locked` (63, all new parity tests
  passing on their first run); targeted `rustfmt --check` clean on every new
  Rust file; `npm run e2e` (25). Clippy remains intentionally unrun.

RUNNER-CONFIG-001 is Done pending merge. The only newly unblocked slice is
RUNNER-STORE-001 (next migration, run/job repositories, atomic candidate
commit, recovery/idempotency); worker pool, events, and UI remain blocked by
the mandated order.

### RUNNER-CONFIG-001 review correction (append-only update)

Date: 2026-07-26. Fix for the PR #73 review findings. The implementation
record above is retained as written; this correction supersedes its fixture
inventory, error total, and verification counts.

Placement correction first: the original record was appended after the
RS-CORE-005 *implementation* record but BEFORE the RS-CORE-005 *review
correction*, breaking this file's append-only chronology. The record has been
relocated to the end of the file; its text is unchanged.

- [Blocker] `slPct`, `tpPct`, `feePct`, and `slipPct` were admitted on a
  `>= 0` domain only. `backtestRunner` divides these legacy percent units by
  100 and `core/backtest`'s `assertNormalizedFraction` rejects anything above
  1, so a config carrying `feePct: 101` passed admission and was GUARANTEED to
  throw once a job executed — strictly worse than an unchecked field, because
  the failure lands after jobs exist. All four now use a `percent` domain
  bounded to `[0, 100]` (the inclusive endpoint stays legal: 100 maps to
  exactly the engine's 1.0 cap). `level` keeps the same numeric range for an
  unrelated reason (RSI is defined on 0..100) and stays a separate domain.
- [Blocker] Candidates shared one mutable `entryRules`/`exitRules` array with
  each other AND with the caller's input object, because the combination
  builder used a shallow spread. Mutating one candidate therefore changed the
  CONTENT of every other candidate while their already-computed hashes and
  seeds stayed put — an inconsistency the runner would persist as an immutable
  audit record. `parseDiscoveryConfig` now deep-clones the dormant rule arrays
  at admission and the enumerator deep-clones per combination, matching the
  decoupling standard PERSIST-001's review established. Rust was already
  correct (`serde_json::Value::clone` is deep).
- Durable identity validation checked only the version PREFIX, so
  `strategy-v2:` with an empty, truncated, uppercase, or non-hex digest was
  accepted and would have seeded a real random stream. Both languages now
  require `<version>:<64 lowercase hex>`, shared by the seed derivation and
  the config's dataset check.
- TypeScript sorted unknown keys with the default `Array.prototype.sort`
  (UTF-16 code units) while Rust used `String: Ord` (UTF-8 bytes). These
  disagree for non-ASCII keys — `"\u{1F600}"` sorts first in UTF-16 and last
  in UTF-8 — so the two languages could name a DIFFERENT unknown key first,
  violating this slice's own recorded rejection-order contract. TypeScript now
  sorts by UTF-8 bytes, matching Rust and the canonical encoder in
  `core/hashing`. A fixture case carrying both keys locks which one is named.
- The held-error total was misreported as 54 in `tasks.md`, this handoff, and
  the PR body: the 2 enumeration errors were omitted from the sum, though the
  itemized breakdown was correct. The real total was 56, and is now **68**
  after this correction's additions. Both languages now assert the total, so
  the number is derived from the artifact instead of being hand-carried.
- The 43 config error cases were locked by COUNT only, unlike the exact
  ordered ID inventories RS-CORE-003's review established. Both languages now
  assert the full ordered list (now 51 cases), so a deleted case cannot be
  masked by a new one that preserves the count.
- Fixture additions: 4 seed identity-digest errors, 2 dataset identity-digest
  errors, 4 percent-bound errors (including one generated by an axis), and the
  UTF-8 key-order case. New TypeScript regressions cover the percent
  endpoints, the digest shapes, the key ordering, caller-input decoupling, and
  candidate-to-candidate array isolation.
- Re-verification: fixture regenerated blob-identical across consecutive runs
  (SHA-256 `9557ae1fef5122a72192f11c734f9a8d272d20b7b01ee0ddb183d39e83802dd7`,
  superseding the record above); `npm run typecheck`; `npm test` (381);
  `npm run build`; `cargo check --locked`; `cargo test --locked` (64);
  targeted `rustfmt --check` clean; `npm run e2e` (25); `git diff --check`
  clean.
- Clippy WAS run this round (`cargo clippy --locked --all-targets`) and every
  new file is clean. Four warnings on my files were addressed: one
  `manual_range_contains` in `seed.rs` was adopted, and the four
  `neg_cmp_op_on_partial_ord` sites carry an explicit `#[allow]` plus a reason
  — `!(a < b)` and `!(value <= max)` are DELIBERATE, mirroring the TypeScript
  reference so a NaN prunes the candidate / terminates the axis loop. The
  clippy-suggested `a >= b` is false for NaN and would admit an invalid
  hypothesis or spin forever. Four pre-existing warnings remain in
  `backtest.rs` (2 `map_or`) and `score.rs` (`manual_range_contains`,
  `clamp`-like) from RS-CORE-002/005; per AGENTS.md §2 they are proposed
  rather than fixed inside this slice.

### RUNNER-CONFIG-001 second review correction (append-only update)

Date: 2026-07-26. Fix for the PR #73 second-round findings. Supersedes the
error totals recorded above: the held-rejection total is now **70** (10 seed +
1 axis + 4 concurrency + 53 admission + 2 enumeration).

- Rust exposed two public axis representations on `DiscoveryBase`: `axes`
  (serialized into the run's audit record and compared by the parity test) and
  `parsed_axes` (`#[serde(skip)]`, actually walked by the enumerator). They
  were built from one loop so they could not diverge today, but two public
  sources of truth mean a recorded config could describe a DIFFERENT grid from
  the one that produced the candidates — precisely the class of inconsistency
  the immutable audit record exists to prevent. `DiscoveryAxis` now stores a
  `&'static str` key borrowed from `DISCOVERY_AXIS_KEYS` (so an axis cannot
  name a non-whitelisted key by construction), `SerializedAxis` is gone, and
  `DiscoveryBase.axes` is the single field both serialization and enumeration
  read.
- The benchmark-cost percent regression could never reach its branch: bases
  parse BEFORE `benchmarkCosts`, so setting the base strategy's `feePct` to
  101 threw first and the mutation of `benchmarkCosts` in the same case was
  dead. The resolved costs carry their own domain check, so the base preset
  now stays valid and two fixture cases plus a unit case exercise
  `benchmarkCosts.feePct` / `.slipPct` directly.
- `exact-v1` did not state its sign-of-zero semantics. IEEE-754 defines
  `-0.0 == 0.0`, so the Rust comparator would have accepted a sign flip while
  the policy name implies it would not. Both languages now assert that no leaf
  is negative zero (and Rust additionally requires finiteness), making the
  declared policy match what is actually enforced.
- Documentation drift closed: `tasks.md` still quoted 54 held errors in the
  parity bullet while the fix bullet said 68, and the PR description still
  carried the pre-review figures (54 errors, the superseded fixture hash,
  373/63 test counts, prefix-only identity checking). Both are corrected, and
  the totals are asserted on both sides so prose cannot drift from the
  artifact again.
- The clippy note above said "Four warnings" while listing 1 + 4; the correct
  count is five.
- Re-verification: fixture regenerated blob-identical across consecutive runs.
  The FINAL committed fixture SHA-256 is
  `00ccf7c8d0bea4442653205ec74158213e0dc1ed468fc34b20f64496f71cc57f`; the
  hashes quoted in the two sections above (`c4845236…`, `9557ae1f…`) are
  superseded historical values retained per this file's append-only rule.
  `npm run typecheck`; `npm test` (381); `npm run build`;
  `cargo check --locked`; `cargo test --locked` (64);
  `cargo clippy --locked --all-targets` clean on every new file; targeted
  `rustfmt --check` clean; `npm run e2e` (25); `git diff --check` clean.

### RUNNER-CONFIG-001 third review correction (append-only update)

Date: 2026-07-26. Fix for the PR #73 third-round findings.

Append-only correction first: the previous correction changed the word "Four"
to "FIVE" IN PLACE inside the first correction's clippy bullet, then described
that same text as still saying "Four". Editing an earlier record is exactly
what this file forbids, and it left the two sections contradicting each other.
The original wording has been restored; the correct count is recorded here
instead. Five clippy warnings on the new files were addressed: one adopted
`manual_range_contains` in `seed.rs` plus four `neg_cmp_op_on_partial_ord`
sites carrying an explicit `#[allow]`.

- The `-0` guard was a FALSE NEGATIVE. `expectExactJson` ran against the
  imported fixture and against `JSON.parse(JSON.stringify(regenerated))`, but
  `JSON.stringify(-0)` is `"0"` — a negative zero is erased the moment the
  fixture is written, so the assertion could never observe one. It now runs on
  the LIVE builder output before any round trip, with explicit negative
  controls (`-0`, `NaN`, `±Infinity`, nested and array positions) proving the
  guard is not vacuous. Verified by mutation: injecting `min: -0` into an axis
  case and REGENERATING the fixture (so the artifact matches byte for byte)
  still fails on `Object.is(value, -0)`, where the previous arrangement passed
  silently.
- The Rust axis comparison bypassed the `exact-v1` rules with a bare
  `actual == expected`, so `-0.0 == 0.0` would have been accepted there too.
  All numeric comparisons now route through one `assert_exact_leaf` helper,
  and a test asserts that the helper actually PANICS on `-0` (either side),
  `NaN`, `±Infinity`, and a genuine drift mismatch.
- `pub key: &'static str` did not mean "whitelisted": `'static` only promises
  the text outlives the program, and every string literal satisfies it, so a
  caller could construct an axis naming any key at all. The doc comment
  claiming otherwise was wrong. `DiscoveryAxis.key` is now a closed `AxisKey`
  enum whose only constructor from text is `AxisKey::parse`, making a
  non-whitelisted axis unrepresentable rather than merely rejected;
  `discovery_axis_keys()` is derived from `AxisKey::ALL` so the string
  whitelist cannot drift from the enum. A test locks that non-whitelisted
  keys — including the deliberately excluded `feePct`/`slipPct`/`sizePct` —
  fail to parse.
- Deep-clone regressions extended to `exitRules` and to in-place mutation of
  NESTED rule objects (a shallow array copy survives an append but not an
  element mutation), on both the caller-input and candidate-to-candidate
  paths.
- Re-verification: fixture regenerated blob-identical
  (`00ccf7c8d0bea4442653205ec74158213e0dc1ed468fc34b20f64496f71cc57f`,
  unchanged — this round altered guards and types, not fixture content);
  `npm run typecheck`; `npm test` (382); `npm run build`;
  `cargo check --locked`; `cargo test --locked` (66);
  `cargo clippy --locked --all-targets` clean on every new file; targeted
  `rustfmt --check` clean; `npm run e2e` (25); `git diff --check` clean.

### RUNNER-STORE-001 implementation record (append-only update)

Date: 2026-07-26. Implementer: Claude. Branch:
`feat/runner-store-001-run-job-store`.

- Added `src-tauri/migrations/0003_discovery_runner.sql` and
  `src-tauri/src/db/discovery.rs` + `discovery_tests.rs`. `discovery_runs` and
  `discovery_jobs` had been schema-only since 0001 with no writer anywhere, so
  0003 is the first migration to give either table data-carrying structure.
- Schema (D5): `discovery_jobs.candidate_index` plus
  `UNIQUE(discovery_run_id, candidate_index, segment)`; a nullable
  `validation_records.discovery_run_id` plus a PARTIAL unique index on
  `(run, strategy, dataset)` so runner assessments are unique per run while
  manual saves keep PERSIST-001's unconstrained append-only behaviour; and a
  partial unique index on an expression identical for every `running`/`paused`
  row, which enforces the one-non-terminal-run rule in the DATABASE instead of
  a check-then-act that could race.
- Store (D5/D6): the fixed transition table (`idle -> running`;
  `running -> paused|completed|failed|cancelled`; `paused -> running|cancelled`;
  terminal never resumes), with `idle` deliberately holding no global slot.
  `commit_candidate_assessment` writes both segment summaries and trades, the
  append-only record with its run linkage, BOTH job rows, run progress, and
  the strategy lifecycle in ONE transaction. `write_backtest_result` and a new
  `insert_validation_record_for_run` take `&Connection`, so a caller-owned
  transaction passes itself in — the Resolution's "never append a record and
  patch job state later".
- Fail-closed by construction rather than by validation where possible:
  `Segment` is a two-variant enum so a Test job row cannot be built; the
  lifecycle update is derived from `record.gate_passed` and
  `best_strategy_id` from the stored assessments, so a caller cannot record a
  verdict or a winner the evidence does not support; a commit is refused
  unless the run is `running` and the queued pair exists and matches the
  committed summaries' identity.
- Crash recovery pauses orphaned `running` runs, requeues only in-flight jobs,
  never touches `done` rows, never auto-resumes CPU work, and is idempotent.
- Test discipline note, carried forward from the PR #73 review rounds: the
  obvious atomicity test (delete a job row, assert nothing was written) is a
  FALSE NEGATIVE, because that guard fires before any write. It was verified
  as such by mutation — flipping the transaction's drop behaviour to `Commit`
  left it passing. The real proof is
  `a_failure_after_the_writes_rolls_everything_back`: its failure is the
  per-run uniqueness rule firing on the record INSERT, after both summaries
  were upserted and their trades replaced, and it asserts the surviving rows
  still hold the PREVIOUS commit's values. That test DOES fail under the same
  mutation. The weaker case is retained under the name
  `a_broken_job_pair_is_rejected_before_anything_is_written` so it cannot be
  mistaken for atomicity evidence.
- `db/discovery.rs` carries a module-level `#![allow(dead_code)]` because no
  non-test build calls the store yet. RUNNER-EXEC-001 MUST remove it when it
  wires the commands, or genuinely dead code will stop failing.
- Documented in `docs/discovery-runner-store-contract.md`.
- Verification: `npm run typecheck`; `npm test` (382, unchanged — this slice
  adds no frontend surface); `npm run build`; `cargo check --locked`;
  `cargo test --locked` (86, +20); `cargo clippy --locked --all-targets` clean
  on the new files; targeted `rustfmt --check` clean; `npm run e2e` (25);
  `git diff --check` clean. The 4 pre-existing `backtest.rs`/`score.rs` clippy
  warnings from RS-CORE-002/005 remain proposed, not fixed here.

RUNNER-STORE-001 is Done pending merge. The only newly unblocked slice is
RUNNER-EXEC-001 (worker pool, commands, pause/resume/cancel, single writer,
versioned events); the frontend UI remains blocked behind it.

### RUNNER-STORE-001 review correction (append-only update)

Date: 2026-07-26. Fix for the PR #74 review findings. Supersedes the
verification counts recorded above (86 -> 94 Rust tests).

Three blockers:

- `transition_run` accepted `running -> completed`, giving callers a second
  path to a terminal "completed" that skipped D6's `best_strategy_id`
  derivation entirely. That transition is now ABSENT from the table, so
  completion exists only as `complete_discovery_run`. That function also now
  refuses while any job is still `queued`/`running`: "completed" never
  resumes, so allowing it mid-queue would freeze unfinished work behind a
  terminal state and derive the winner from a partial set of assessments.
- The candidate commit did not check job status, so a late-arriving result
  could flip an already `failed` or `skipped` row to `done` and wipe its
  `error_message` — destroying exactly the evidence D5 requires a failure to
  carry — or rewrite a `done` checkpoint. The pre-check now requires both rows
  to be `queued`/`running`, and the UPDATE carries the same restriction.
- Each migration and its `schema_migrations` row now commit in ONE
  transaction. SQLite DDL is transactional, so without it a migration failing
  partway (0003 has several statements) left half a schema behind AND no
  version row, and every retry then died on "duplicate column name" —
  permanently unupgradeable. This was latent in 0001/0002 and only became
  reachable with 0003's multi-statement body.

Same round:

- `fail_candidate_jobs` used `status != 'done'`, so it overwrote `skipped` and
  replaced an earlier `failed` row's evidence with a later reason. It is now
  restricted to unfinished rows.
- `start_discovery_run` accepted two candidate indexes sharing one
  (strategy, dataset). Enumeration deduplicates by strategy hash, so that can
  only come from a caller building the queue wrong; it now fails at enqueue
  instead of after the second candidate's backtests have run, when the commit
  would hit 0003's per-run assessment uniqueness rule.
- `validation_records.discovery_run_id` was `ON DELETE SET NULL`, which would
  silently erase the run provenance of an immutable audit record. It is now
  `ON DELETE RESTRICT`: a run that produced records cannot be deleted.

Test-integrity finding (the one worth carrying forward):

- The rollback test only covered summaries/trades, because its failure point
  (the record INSERT) precedes the job rows, lifecycle, and progress writes.
  A new test injects the failure at the LAST write via a trigger on the
  progress update, so every earlier write must be undone; it is
  mutation-verified against a commit-on-drop transaction.
- Re-running that mutation ALSO revealed that the blocker-2 pre-check had
  silently downgraded the previous rollback test into a guard test: with the
  earliest rejection moved ahead of any write, it passed under the mutation.
  It has been renamed `re_committing_a_done_candidate_is_rejected_before_any_write`
  and documents why. **A guard added upstream of a failure point can quietly
  invalidate a test that depended on reaching it** — re-run the mutation after
  adding guards, not just after writing the test.
- That pre-check also shields 0003's per-run uniqueness index from the store
  API, so it is now asserted directly at the schema level; it remains the last
  line of defence if a job row is manipulated.

Re-verification: `npm run typecheck`; `npm test` (382, unchanged);
`npm run build`; `cargo check --locked`; `cargo test --locked` (94, +8);
`cargo clippy --locked --all-targets` clean on the changed files; targeted
`rustfmt --check` clean; `npm run e2e` (25); `git diff --check` clean.

### RUNNER-STORE-001 self-review correction (append-only update)

Date: 2026-07-26. Findings from an adversarial self-review of PR #74 after the
review corrections above. Supersedes the Rust test count (94 -> 96).

Two real defects, both found by probing rather than by re-running the suite:

- `select_best_strategy` documented "highest FINITE-score gate passer" but
  filtered only on `score IS NOT NULL`. SQLite stores NaN as NULL yet keeps
  +/-Infinity, so an infinite score outranked every genuine candidate — a probe
  confirmed a `9e999` row beating a legitimate 5.0. The commit path's validator
  rejects non-finite scores, but this selector is public and documented as
  finite-only, so it now filters with `score * 0 = 0` (true only for finite x).
- A run whose every job FAILED could be marked `completed` with no winner,
  which is indistinguishable from a clean run where nothing passed. D5 requires
  an engine/system failure to fail the run WITH evidence, so
  `complete_discovery_run` now refuses while any job is `failed` and directs
  the caller to `failed` instead.

One hardening: `transition_run` was a check-then-act (read status, then write)
while `start_discovery_run` and `complete_discovery_run` were already
transactional. It now shares one transaction, so the state machine does not
depend on caller discipline.

One probe that did NOT become a fix, recorded so it is not "found" again:
`skip_remaining_jobs` and `fail_candidate_jobs` appear to operate on terminal
runs, but completion requires zero `queued`/`running` jobs and both functions
only touch those states, so on a completed run they are provably no-ops; and
skipping after a cancel is D5's documented flow, not a bug. Manufacturing a
guard there would have added a rule the contract does not ask for.

Open item for RUNNER-EXEC-001: `discovery_runs` has no run-level failure
evidence column, so `transition_run(.., Failed)` records the state but not the
reason. Per-job `error_message` carries the detail today. If EXEC needs a
run-level reason it should add the column in its own migration rather than
having this slice add speculative schema.

Re-verification: `npm run typecheck`; `npm test` (382, unchanged);
`npm run build`; `cargo check --locked`; `cargo test --locked` (96, +2);
`cargo clippy --locked --all-targets` clean on the store files; targeted
`rustfmt --check` clean; `npm run e2e` (25); `git diff --check` clean. Both
new tests were mutation-verified: reverting either fix fails its test.
