# Handoff: PERSIST-001 design proposal — persist the validation-run record (reviewer decisions needed)

Date: 2026-07-19
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/handoff-persist-001-proposal
PR: (this handoff PR)
Status: open question — implementation must not start until a Resolution records the decisions below

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

(Reviewer: record D1–D5 decisions here, then implementation may start.)
