# Handoff: RUNNER-001 design proposal — discovery job runner program (reviewer decisions needed)

Date: 2026-07-19
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/handoff-runner-001-proposal
PR: (this handoff PR)
Status: open question — implementation must not start until a Resolution records the decisions below

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

(Reviewer: record D1–D7 decisions here, then implementation may start with the approved slice order.)
