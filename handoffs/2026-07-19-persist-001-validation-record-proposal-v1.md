# Handoff: PERSIST-001 design proposal — persist the validation-run record (reviewer decisions needed)

Date: 2026-07-19
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/handoff-persist-001-proposal
PR: #64
Status: resolved — D1 Option A REJECTED; revised Option C mandated (see Resolution). PERSIST-001 stays out of In Progress until this handoff merges, then implements the Resolution.

## Summary

The Phase B pure-function layer is complete (VAL-001→003, BENCH-001/002, GATE-001, METRIC-001, SCORE-001), but nothing it produces survives a session: the embargo derivation, split plan, benchmark outputs, GateVerdict, and ScoreBreakdown are all designed "for recording" yet never recorded. PERSIST-001 persists one validation run's full audit trail. Persistence is a declared high-risk area, so this follows the SCORE-001 governance pattern: Mode A proposal → reviewer Resolution → implementation.

**Key finding (already verified in code):** the persistence stack for per-segment results ALREADY exists end to end — `backtest_summary` has `gate_passed` / `score` / `score_breakdown_json` / `benchmark_result_json` columns (0001_init.sql:84-87), the Rust `BacktestSummary` DTO carries them and `insert_backtest_summary` INSERTs + UPSERTs them (repositories.rs:99-105, 276-313), and the TS interface declares them optional (tauri-client/commands.ts:73-76). Only two gaps exist: (1) nothing populates these fields; (2) the RUN-level record (embargo derivation + split plan + GateVerdict + configs) has no defined home.

## Required Action / Decision

Answer D1–D5 (accept the recommendation or state an adjustment), then append a `## Resolution`. Implementation follows the Resolution wherever it differs.

### D1 — Home for the run-level record (the main decision)

The derivation/plan/verdict/config bundle describes one validation RUN, not one segment row. Options:

- **Option A (recommended): one `discovery_runs` row per validation run.** `config_json` = the run record (below); `status: 'completed'`; `best_strategy_id` = the strategy. The table exists precisely for run-level records (schema-only since Phase A), so no migration; semantics stay honest. Cost: one new narrow Rust command + repository fn + in-memory tests (the FEAT-002 pattern). `discovery_jobs` rows stay deferred to the runner slice — v1 linkage is by the summary rows' `(strategy_id, dataset_id, segment)` unique key recorded inside the run record.
- **Option B: duplicate the run record into the validation summary row's `benchmark_result_json` envelope.** Zero Rust work (TS-only), but the column name becomes misleading, the record duplicates per dataset row, and run-level data pretends to be segment-level.
- **Option C: migration `0002` adding a dedicated table/column.** Cleanest names, but touches the migration path (highest-risk area) for something Option A already expresses without schema change.

### D2 — `benchmark_result_json` content (validation summary row)

Recommended shape (JSON-safe, versioned):

```jsonc
{
  "version": "bench-record-v1",
  "benchmarks": [ { "id": "buyHold", "metrics": { /* finite-or-null + nonFinite statuses, reusing the schema-2 report codec */ } }, ... ],
  "randomEntry": { "runs": 200, "seed": 7, "candidatePercentile": 97.5, "netReturns": [ ... ] }
}
```

- Per-benchmark METRICS ONLY — never equity/trades (size; trades already have their own table for the candidate).
- Keep the full `netReturns` distribution (~200 numbers ≈ 4 KB): it is the audit evidence behind the percentile and cheap to store. Alternative if rejected: store only `{ runs, seed, candidatePercentile }` and accept that re-audit requires re-simulation from the recorded seed.

### D3 — Column semantics for the Phase B summary fields

- `gate_passed` (0/1) and `score` + `score_breakdown_json`: set on the **validation** segment row ONLY; train rows keep them null (Gate/Score are validation-segment judgments by contract). `benchmark_result_json` also validation-row-only.
- The FULL `GateVerdict` (per-criterion values + thresholds + config) lives in the run record (D1), not in a summary column — it is a run-level judgment with run-level config.
- Test rows: none exist and none may be created (v1 never executes Test).

### D4 — Run-record shape and non-finite discipline

```jsonc
{
  "version": "validation-run-v1",
  "strategyId": 12, "strategyHash": "…", "datasetId": 3, "datasetHash": "…",
  "embargo": { /* EmbargoDerivation breakdown (VAL-003) */ },
  "splitPlan": { /* ValidationSplitPlan (VAL-001) */ },
  "gate": { /* GateVerdict, non-finite values encoded finite-or-null + status */ },
  "score": { "formulaVersion": "score-v1", "value": 2.65 },  // full breakdown stays in score_breakdown_json
  "testedCombinations": { "n": 100, "basis": "lineage-final-unique" }
}
```

- Every JSON payload passes through explicit non-finite encoding (the METRIC-001 `nonFinite.ts` vocabulary); `ScoreBreakdown` is already JSON-safe by contract; `GateVerdict` gains a small serializer because criterion values may be non-finite.
- All shapes carry a `version` string so later migrations of the JSON payloads are detectable.

### D5 — Scope and sequencing

- v1 delivers: a pure TS composer (`buildValidationRecord` + summary-field population via the existing `metricsToBacktestSummary` seam), the D1 persistence path (Option A: one Rust command + repo fn + tests), and unit tests — **unwired from UI** (the current UI only saves `full`-segment runs; wiring belongs to the runner/Results-Explorer slices, consistent with every engine-first slice so far).
- Lifecycle transitions (`candidate → validated/rejected` from `gate_passed`) are NOT part of this slice — the Resolution for SCORE-001 assigns promotion policy to the runner.

## Review Notes

- Option A introduces the first writer to `discovery_runs`; the runner slice later takes ownership of that table's lifecycle states (`running`/`paused`/…) — v1 only ever writes `completed` rows.
- The existing `insert_backtest_summary` UPSERT (`ON CONFLICT(strategy_id, dataset_id, segment)`) already gives idempotent re-saves of a re-run validation; the run record in Option A would append a new run row per execution (audit trail, not upsert) — confirm this asymmetry is intended.
- Nothing here reads or executes the Test segment; the `segment` CHECK constraint's `'test'` value remains unused.

## Verification

Proposal only — no code. Current baseline on `main`: 284 vitest + 25 Playwright e2e green (post-SCORE-001, PR #63).

## Resolution (added when acted on)

Date: 2026-07-19. Decider: Codex (reviewer), delivered as a PR #64 comment by @yoyoCadence (the original zh-TW comment on PR #64 is the source text; transcribed here in the repo's doc language). **Implementation authority: this Resolution > the original proposal. The proposal's D1 Option A is REJECTED — starting implementation on it is forbidden.**

> 驗收結論：目前設計不通過。文件格式與看板更新正常、CI 全綠，但不可依 D1 Option A 開始實作。

### Mandated execution order

1. Append these D1–D5 decisions to this Resolution (done — this section).
2. After the Resolution is appended, this handoff PR proceeds through normal review/merge.
3. PERSIST-001 implements the REVISED Option C: migration + immutable validation record.
4. PERSIST-001 must not move to In Progress before this Resolution is appended.
5. This slice wires no UI, changes no lifecycle, and starts no discovery runner.

### Blocking findings

1. **A single validation record must NOT go into `discovery_runs.config_json`.** `discovery_runs` is the discovery BATCH parent (`discovery_jobs.discovery_run_id` implies one run → many strategies/datasets/segments) and `config_json` is that batch's INPUT config. One completed run row per strategy × dataset validation would conflate "discovery batch" with "single candidate assessment", block the future runner from owning the run → jobs state machine, make `config_json` carry outputs as well as inputs, and misuse `best_strategy_id`. D1 Option A rejected.
2. **The original proposal cannot form an immutable audit trail.** `backtest_summary` upserts on `(strategy_id, dataset_id, segment)` — re-runs overwrite. An append-only run record that merely references that key would read the NEW summary after a re-run; and the proposed record stored only the score value (no full ScoreBreakdown, no Train/Validation metrics snapshots, no full benchmark record), so a historical judgment could not be reconstructed.
3. **Writes must be atomic across the whole validation bundle.** One transaction must save (1) Train summary + trades, (2) Validation summary + trades, (3) the immutable validation record — any failure rolls back everything; no half-saved results.

### D1 — Run-record home (final)

**Revised Option C: new migration `0002_validation_records.sql`; `discovery_runs.config_json` is never used for this.** Minimum schema:

```sql
CREATE TABLE validation_records (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy_id    INTEGER NOT NULL REFERENCES strategy_def(id),
    dataset_id     INTEGER NOT NULL REFERENCES datasets(id),
    record_version TEXT    NOT NULL,
    gate_passed    INTEGER NOT NULL CHECK (gate_passed IN (0, 1)),
    score          REAL,
    record_json    TEXT    NOT NULL,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_validation_records_identity
    ON validation_records(strategy_id, dataset_id, created_at);
```

Rules: append-only (no update/delete command in v1); multiple historical assessments per strategy/dataset are expected; `record_json` is a self-contained immutable decision snapshot; `backtest_summary` remains the "current/latest materialized view"; `discovery_runs.config_json` forever stores discovery input config only; if the future runner needs linkage it adds a nullable `discovery_run_id` via its own migration — do not pre-guess it here. The migration runner already supports appended migrations; the risk is controlled with upgrade/idempotency tests, not by polluting existing column semantics to avoid a migration.

### D2 — Benchmark record (final)

Keep the full Random Entry `netReturns` distribution. Every validation's benchmark record includes at least: `version` + benchmark contract version; interval, validation range, startEquity; inherited fee/slippage costs; per deterministic benchmark its id, EXACT strategy/config (Buy & Hold may be null + explicit contract), and JSON-safe metrics with non-finite statuses; the full Random Entry result (runs, seed, candidateNetReturn, candidatePercentile, netReturns). No benchmark equity/trades. `benchmark_result_json` may stay on the Validation summary as the latest view, but the immutable `validation_records.record_json` MUST embed the same benchmark snapshot — the duplication is deliberate, for historical immutability.

### D3 — Phase B summary semantics (final)

- Train row: `gate_passed`, `score`, `score_breakdown_json`, `benchmark_result_json` all null.
- Validation row: benchmark record required; `gate_passed` required; Gate fail → `score` and `score_breakdown_json` MUST be null; Gate pass → finite `score` and full `score_breakdown_json` required.
- Test row: creation forbidden.
- The composer expresses Gate pass/fail as a discriminated union so a "gate failed but has a score" state is unrepresentable.

### D4 — Immutable validation record (final)

`validation-record-v1` must be self-contained, at least: strategyId + strategyHash; datasetId + datasetHash; embargo derivation; complete split plan; Train metrics snapshot (JSON-safe); Validation metrics snapshot (JSON-safe); complete benchmark record; complete GateVerdict; full ScoreBreakdown or null on Gate fail; testedCombinations evidence; formula/benchmark/gate/execution contract versions. Storing only `score: { formulaVersion, value }` and relying on the mutable summary breakdown is forbidden.

JSON discipline: extract `reportExport.ts`'s private metrics encoder into a shared `metricsCodec.ts` (no second drifting codec); Gate criterion non-finite values use the METRIC-001 status vocabulary; ScoreBreakdown, though JSON-safe by contract, still passes boundary validation; before `JSON.stringify`, recursively check every nested number and THROW on any unencoded Infinity/−Infinity/NaN (never let JSON silently null it); every payload must survive stringify → parse → deepEqual. The record is a "decision audit snapshot": benchmark equity/trades excluded; candidate trades/equity stay in their existing tables; the record keeps enough metric/evidence/contract versions to reconstruct the judgment.

### D5 — Scope, API, atomicity (final)

v1 deliverables: pure TS `buildValidationRecord` composer; shared JSON-safe metrics codec; `0002_validation_records.sql`; Rust DTO/repository; ONE atomic `save_validation_record` Tauri command; typed TS save wrapper; typed `list/get_validation_records` read path; mock client parity; focused TS + Rust tests; board/contract docs updates.

The atomic command validates BEFORE opening the transaction: Train/Validation summaries match the record's strategyId/datasetId; segments are exactly `train` and `validation`; Gate pass/fail agrees with score nullability; `record_version` matches the JSON envelope version.

Out of scope: UI/Results Explorer wiring; lifecycle promotion/rejection; discovery runner state machine; Test execution/persistence; broad persistence refactors beyond this schema.

### Acceptance checklist (from the reviewer)

Migration / Rust:
- fresh DB applies 0001 + 0002 in order; an existing 0001 DB upgrades preserving data; migration re-run is idempotent; FKs and the gate_passed CHECK are enforced.
- multiple records append for the same strategy/dataset; no update/delete path.
- if Validation or record write fails after Train succeeded, the whole transaction rolls back; on success both summaries + trades + record commit at once.
- list/get read back the exact record JSON.

TypeScript:
- the Gate pass/fail discriminated union cannot express an illegal score state; Train Phase-B fields all null; Validation Phase-B fields per D3.
- benchmark record embeds exact configs and full Random Entry evidence; metrics/Gate non-finite values explicitly encoded; a missed nested non-finite fails closed; full record JSON round-trips deepEqual.
- the Test segment is never read, executed, or persisted; typed Tauri invoke argument names exactly match the Rust command.
