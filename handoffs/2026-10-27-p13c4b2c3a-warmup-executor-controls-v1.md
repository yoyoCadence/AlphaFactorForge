# Handoff: actual original warmup/embargo executor controls

Date: 2026-10-27
Repo: yoyoCadence/AlphaFactorForge
Branch: test/original-warmup-executor-controls
Status: Resolved; delivered by #231. User resumed2026-10-27 for3b bounded consumer design.

## Summary

From verified #230 (`5745c35`), implement only [3a](../docs/plans/research-original-warmup-projections-v1.md#4-first-code-actual-executor-conformance-3a). One private cfg(test) child calls the actual unchanged CandidateStrategy::parse then embargo for all47 frozen standalone inputs and all23 candidates in eight earlier families. Eight focused groups pin22 standalone successes/25 staged refusals, exact three-field derivations, repeat determinism and unchanged input Value/bytes. The parent's only edit is the four-line test-module declaration. No production helper, public bridge, new analyzer, DB/candles/full execution or source capability is added by these controls.

The new test oracle pins the design fixture and both earlier fixture packages using the frozen LF-packaging checksums; decoded strategy JSON remains exact. Family checks retain complete case membership, candidate indexes/hashes/modes and the original holding allowance. Existing source/identity/numeric/fixture/budget behavior remains the baseline. V3 minimumTrainBars20 is still below params lookback22; producer arithmetic does not establish executable folds, authenticated candles or historical warmup suitability.

## Verification in progress

Initial focused `cargo test --locked warmup_controls -- --nocapture` passes all8 groups (compile1m15s/tests0.01s). Seven temporary production-source mutants must assertion-fail and restore exact source bytes before the full isolated locked suite. Logs/driver/backups are retained in normal TEMP under `aff-p13c4b2c3a-*`. Full Rust precedes Node-heavy checks; locked all-target check/Clippy, scoped format, typecheck/Vitest/build and all six exact-head/base CI jobs remain mandatory.

## Remaining scope and session boundary

After verified3a delivery, refine3b-design's sealed graph API and complete-operation analysis/clone/serialization/node/output budget plus independently frozen varied-candidate/real-boundary controls. Consumer3c follows a separate verified design delivery. Warmup parent3 and every market/trial/file/action/cost/registry/owner/final admission gate remain incomplete; RegisteredSource/numericCostsHash stay unavailable. Fresh reviews/main, expected-head merge, GitHub readback and clean-main/full-tree equality complete this delivery. Stop after this PR merges; no next task starts in this session.

## Assertion mutants and source restoration

All seven mutants fail at actual assertions in warmup_controls.rs with cargo exit101, panic/FAILED evidence and no compiler error: params cross increment, entry-only lookback, unused-period inflation, omitted DSL offset, DSL clamp400, omitted holding allowance, bypassed final safe ceiling. The final refusal mutant is caught at refuse-embargo-one-over; all others fail the literal three-field equality. Every original byte is restored: embargo.rs SHA256 `9dfd483315c3f69f64ee02a352a873d86ddd5fa3dc3e60a5c6ddcd0240e47f89`, dsl.rs `afa785b95235b2ddb39a3d576bdf076e19a769901674c850cb160d7fdb6b54ed`, execution.rs (including the intentional test declaration) `f94ed64e1409e50e3ea4d386bb3026a44b47257e4e87c435390b8e6bc5417f9e`. After removing only those four declaration lines, the entire execution file matches the prior baseline under newline normalization; both core files match baseline. All fixture files are unchanged; warmup fixture LF-packaging SHA remains `b67268643162eca6da5c97b056b61d5461c1a31ae0ea02372d8b395a8f31ee06`. Driver/logs/backups remain in normal TEMP. Full isolated suite is run only after this restoration.

## Full local verification and owned cleanup

Complete isolated `cargo test --locked`, after exact source restoration and before Node-heavy checks, passes1098 tests/one ignored (297/799/2;11.84/101.31/3.45 s; compile26.20 s). Locked all-target check15.22 s/Clippy9.93 s, scoped rustfmt for the new test module and scoped declaration/diff checks pass. Only the five existing backtest330/342, score416/428 and file_commands156 warnings remain. Vitest1242/80 files (four workers,6.49 s), typecheck and production build pass. No timeout/limit/parser/identity/fixture/production-body change.

The exclusively created root `aff-p13c4b2c3a-full-630b62195bfa4b3f919b20a922b70b5c` was removed after successful suite completion (43 files/64 directories). Exact resolved TEMP parent/GUID leaf and every traversed entry were checked before recursive deletion; no reparse points. No stale PID/counter root was reused/deleted. Logs/driver/backups remain outside that removed root. Existing full service smoke owns/stops its helper; this slice starts no manual server. Only six completed exact-head/base jobs and verified merge/readback/clean-main delivery remain.

## Resolution: #231 delivery and resume (2026-10-27)

[PR #231](https://github.com/yoyoCadence/AlphaFactorForge/pull/231) delivered head `1e130e593241391f719261dddc1f2d8491312cd2` against base `5745c355994952fddb0bdc30c2e5617aa7fae05b`, merge `f53a228499517344a924d40e762ed248bf525cc9`. Workflow40421775557: all six completed logs confirm that exact pair; Rust1098/one existing ignored (297/799/2;16.63/318.45/5.59 s), Vitest1242/80, E2E98/no retries, production audit0, native rendered/SQLite/metrics/Rust event roundtrip plus exact hosted browser policy restoration/owned cleanup pass. All eight new controls appear in the successful Rust log. Fresh reviews/threads empty, main unchanged/mergeable; expected-head merge without bypass, GitHub closed/merged readback and clean local main/full-tree equality (`67028ce0febe09b3425c428c8b9d4aecab61f6e2`) verified, then final PR receipt read back. Stopped at that boundary. Current resume starts design/data3b only;3c and all parent/source/admission gates stay incomplete.
