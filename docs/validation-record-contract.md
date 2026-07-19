# Validation Record Contract (PERSIST-001)

Status: adopted Phase B foundation, 2026-07-19. Implements the PR #64 handoff Resolution (`handoffs/2026-07-19-persist-001-validation-record-proposal-v1.md`), revised Option C — the Resolution overrides the original proposal.

## Purpose

One completed validation assessment produces one **immutable, append-only** row in `validation_records` (migration `0002_validation_records.sql`) plus the refreshed Train/Validation `backtest_summary` rows. The summaries remain the mutable "current/latest" view (their key upserts on re-runs); the record is the historical decision audit snapshot that survives every re-run. There is NO update or delete path in v1.

## Schema (0002)

`validation_records(id, strategy_id FK, dataset_id FK, record_version, gate_passed CHECK(0/1), score, record_json, created_at)` with the D3 invariant locked at the DB layer: `CHECK ((gate_passed = 0 AND score IS NULL) OR (gate_passed = 1 AND score IS NOT NULL))`. The Rust command validates the finite score and JSON contents first; the CHECKs are the second line of defense. Multiple records per strategy × dataset are expected (one per assessment), indexed by `(strategy_id, dataset_id, created_at)`.

## The immutable snapshot (`validation-record-v1`)

`record_json` is self-contained: strategyId + strategyHash, datasetId + datasetHash, the embargo derivation (VAL-003), the complete split plan (VAL-001), JSON-safe Train and Validation metric snapshots, the complete benchmark record, the full encoded GateVerdict, the full ScoreBreakdown (or null on gate fail), the testedCombinations evidence, and the contract versions (`backtest-execution-v1`, `benchmark-suite-v1`, `gate-v1`, `score-v1`-or-null). Candidate trades/equity stay in their own tables; benchmark equity/trades are never stored.

The benchmark snapshot (`bench-record-v1`) records the interval, inclusive validation bar range, startEquity, inherited costs, each deterministic benchmark's exact strategy config (null only for Buy & Hold, whose behaviour the benchmark contract fixes) with metrics-only snapshots, and the FULL Random Entry evidence including the `netReturns` distribution. The same snapshot is duplicated into the Validation summary's `benchmark_result_json` (latest view) — deliberate, for historical immutability.

## Summary-row semantics (D3)

- Train row: `gate_passed` / `score` / `score_breakdown_json` / `benchmark_result_json` all null.
- Validation row: benchmark record and `gate_passed` required; gate fail ⇒ `score` and `score_breakdown_json` null; gate pass ⇒ finite `score` + full breakdown required.
- Test rows are never created; the Test segment is never read, executed, or persisted.
- The TS composer (`services/validationRecord.ts`) expresses the outcome as a discriminated union (`AssessmentOutcome`), so "gate failed but has a score" is unrepresentable.

## JSON discipline

All encoding goes through the shared `services/metricsCodec.ts` (extracted from the report exporter so exactly one codec exists): metrics encode as finite-or-null values plus explicit METRIC-001 statuses; encoded GateVerdict criteria carry `valueStatus` for non-finite values; `assertJsonSafe` recursively rejects any unencoded non-finite number BEFORE serialization; every payload survives stringify → parse → deepEqual.

## Atomicity (D5)

ONE Tauri command `save_validation_record` persists the whole bundle — Train summary + trades, Validation summary + trades, record — in one SQLite transaction; any failure rolls everything back. The bundle is fully validated BEFORE the transaction opens (segments exactly train/validation, identity match, gate/score consistency, Phase B nulls on the Train row, `record_version` matching the JSON envelope). Typed read paths: `list_validation_records` (newest first, optional strategy scope) and `get_validation_record`; the dev mock client mirrors the same validation and append semantics.

## Non-goals

- UI / Results Explorer wiring; lifecycle promotion/rejection; the discovery runner state machine (a future `discovery_run_id` linkage arrives via its own migration if needed).
- Test-segment execution or persistence; broad persistence refactors beyond this schema.
