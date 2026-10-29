# Handoff: actual original market registry producer controls

Date: 2026-10-28 (UTC+8)
Repo: yoyoCadence/AlphaFactorForge
Branch: test/original-market-registry-producers
Status: Resolved — #235 verified delivery; actual registry producer4a complete.

## Summary

From verified #234 (`29952a8`), implement only [contract §4](../docs/plans/research-original-market-registry-projections-v1.md#4-next-code-slice-actual-registry-producer-controls-4a). One private cfg(test)-only registry child calls the actual existing calendar/instrument content/hash/register functions. Seven groups check frozen nine positive projections,33 phased refusals,14 registration/retry/revision calls, a coherent calendar conflict, nine direct nonfinite optional specs and four append-only SQL refusals. Production bodies, parser/numeric/identity policy, schema, dependencies, commands, workflows and all fixtures stay unchanged. Backlog ordered scope → Next → In Progress preceded implementation.

## Controls and verification

Every retained scalar/null and raw JSON text is compared to the frozen independent literals. SQL storage types and REAL to_bits are preserved in complete snapshots of both registries and sqlite_sequence, including created_at. Positive inserts preserve all prior rows; retries/refusals preserve complete snapshots. Exact input text/typed values, repeat projection determinism, old-content retry after revision2, nonfinite input bits and built-in calendar projection are checked. Source pins remove only the four-line new test declaration and compare all four frozen original files; packaging normalization affects physical CRLF only, never decoded original JSON bytes.

Initial focused filter passes all7 new tests plus10 existing producer projection tests (17 total;0.19s after58.80s compile). A corrected relative rustfmt path formats only the new test file; no legacy registry formatting changed. Six mutants run individually against the specific behavioral assertion test, deliberately excluding the source-pin test so a checksum failure cannot stand in for detected semantic regression. Each temporary source change must assert-fail without compiler errors and restore exact original bytes immediately. Driver/backups/logs/results use normal TEMP prefix `aff-p13c4b2c4a-producer-*`.

After all six restorations, full cargo test --locked must run in a fresh exclusively created GUID TEMP/TMP root before Node-heavy checks. Follow with locked all-target check/Clippy, scoped rustfmt/source/fixture/link/append-only receipt/scope checks and Vitest/typecheck/build. Owned roots require exact resolved parent/GUID and per-entry no-reparse containment checks before cleanup; no stale PID root is reused or deleted. No manual server is needed.

## Restored behavioral mutants

All six temporary mutants assertion-fail with cargo exit101 and no compiler errors: omitted calendar version, erased earlyCloses, renamed quote, omitted minNotional, accepted wrong calendar kind and latest-row return for old-content retry. Each targets exactly one behavioral test; the frozen source-pin test is excluded. Exact byte restoration is checked after each run and before the full suite, registry SHA256 `84f88230a41cd14c50d2ed0ba7dc77b5aa4bc5a2065749fd3e44368f56eda340`. Driver/backups/six logs and result JSON remain in normal TEMP. Full locked Rust is running in exclusively created `aff-p13c4b2c4a-full-5b3548548788405fb36c3b9993956032` before Node-heavy checks. Source audit confirms only the four-line declaration differs in the original registry; all old fixtures and all four original source pins match.

## Full local verification and owned cleanup

After exact mutant restoration, fresh isolated `cargo test --locked` passes1121 tests/one existing ignored (297/822/2;10.47/107.53/3.45s;149.19s total), before Node-heavy checks. Locked all-target check16.77s and Clippy11.17s pass with only the five existing backtest330/342, score416/428 and file_commands156 warnings. Scoped rustfmt/source/fixture/link/append-only receipt/five-file scope checks pass. Vitest1242/80 files (maxWorkers4;6.77s), typecheck and production build pass.

The owned full-suite root above was removed only after exit0 and exact resolved TEMP parent/GUID verification plus breadth-first one-level containment/no-reparse checks of every entry:43 files/64 directories including the root. No stale root was reused/deleted. Normal TEMP retains logs/backups/driver/results outside that removed root. No manual server started; existing service tests own/stop their helpers. Local implementation is complete; six exact-head/base CI jobs and verified merge/readback/clean-main receipt remain delivery gates. Only actual registry producer4a may close after delivery; market4 and source/live parents remain incomplete.

## Delivery and remaining work

All six completed exact-head/base CI jobs, fresh reviews/main, expected-head normal merge, GitHub merged readback, clean-main/full-tree equality and final PR receipt complete this slice. Stop after that verified merge. No manual user test is needed. The previous design handoff receives append-only #234 Resolution; its original notes remain historical.

This proves only original registry writer conformance, with synthetic calendar/spec controls. Provenance/snapshot independent producer evidence is next for a later explicit resume; bounded whole-market consumer design/code and trial follow. Missing files/candles/availability/action/cost originals, numeric compatibility and final current actual-owner/registry admission retain their gates. Market4 and source/live parents remain incomplete; RegisteredSource/numericCostsHash remain unavailable.

## Resolution — #235 verified delivery

Recorded2026-10-29 (UTC+8) on the next explicit resume; original notes above preserve commit-time status. [PR #235](https://github.com/yoyoCadence/AlphaFactorForge/pull/235) delivered head `afb5e123c488ada7672bac31ecfb62af3f20fa09` against base `29952a80a8fca5c34c81a6fcf327681c298cecdb`; workflow40691644137 completed all six jobs successfully, every completed log pinning that exact merge. Rust1121/one existing ignored (297/822/2;16.45/428.24/5.69s) includes all7 new registry controls; Vitest1242/80 files, E2E98/no retries, production audit0 and native rendered/SQLite/metrics/event-plugin roundtrip plus exact policy/process/temp restoration pass. All6 local behavioral mutants were caught without source-pin substitution and restored exactly.

Fresh reviews/threads were empty, tested main unchanged and PR mergeable. Expected-head normal merge without bypass produced `fd1660fc49e00b6935bec9a9e5168df628980295`. GitHub closed/merged readback, fast-forwarded clean main and complete tested tree `aeab9e81c761cccff8477780f4fe3ceac912740f` equality passed. Final PR receipt readback was verified; this new resume re-read it and clean main before branching. Actual registry4a is delivered; no next task started before the requested stop. This resume activates [provenance/snapshot design4b](../docs/plans/research-original-market-source-projections-v1.md) via Next → In Progress. Market4 and other source/live parents remain incomplete.
