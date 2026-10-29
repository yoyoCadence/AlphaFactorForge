# Handoff: actual original provenance producer controls

Date: 2026-10-29 (UTC+8)
Repo: yoyoCadence/AlphaFactorForge
Branch: test/original-provenance-producers
Status: Local implementation/verification complete; exact-head CI/merge delivery pending.

## Summary

From verified #236 (`78c8ae3`), implement only [contract §4](../docs/plans/research-original-market-source-projections-v1.md#4-first-code-after-delivery-actual-provenance-controls4b). One private cfg(test) provenance child directly checks existing record_hash/record_raw and artifact/revision APIs against the frozen fixture. Six groups cover nine original hashes,12 actual registrations/retries,13 staged refusals, raw SQL/file projections, revision chain1/5/7 and both lookup directions, controlled missing/corrupt-file retries, immutable storage and source/package pins. Backlog → Next → In Progress preceded code. Only a four-line test declaration changes the original provenance file; production bodies/all fixtures stay unchanged.

## State and owned IO

Complete snapshots cover nine market-related tables including datasets/candles/sqlite_sequence, storage types and REAL bits, all existing created_at fields, and complete directory/file-byte inventories. Rejected/comparison records and all four media suffixes remain original. Exact successful retry preserves all state; missing-file retry recovers exact bytes without a DB row; corruption refuses without changing the row/file. All IO stays in exclusively created PID/counter roots, skipping pre-existing names without reuse/deletion. Before recursive cleanup the helper verifies canonical absolute parent/leaf, containment, symlink/reparse metadata and every descendant; SQLite handles are dropped first.

The source-pin group strips only this fixed four-line cfg(test) declaration and permits the separately planned fixed snapshot declaration (`snapshot_projection_controls.rs`, module projection_controls) when that later test-only slice lands. No runtime code is ignored. All eight frozen producer source bodies and registry package remain pinned. Snapshot itself is unchanged in this slice.

## Verification and delivery

First focused compilation is running. Then six specified source mutants must individually fail behavioral tests, excluding source-pin tests, and restore exact source bytes after every run. Full fresh exclusive GUID TEMP/TMP locked Rust precedes Node-heavy checks, followed by locked all-target check/Clippy/scoped format/source/fixture/link/append-only receipt/scope checks and Vitest/typecheck/build. No manual server is needed. Record owned cleanup evidence; never reuse/delete stale PID roots.

All six completed exact-head/base CI jobs, fresh reviews/main, expected-head normal merge, GitHub readback, clean-main/full-tree equality and final PR receipt complete delivery. Stop afterward. No manual user test needed. #236 Resolution is appended to the previous handoff without replacing original notes.

## Focused controls and restored behavioral mutants

All6 focused controls pass (0.40s after1m45s compile), including actual missing/corrupt-file behavior and verified owned cleanup. Six temporary mutants then fail the specified behavioral test with cargo101/panic/FAILED and no compiler errors: omitted availableAt, ignored accepted, erased revisionOf, erased requestScope, exact retry after fork check and bypassed retry artifact verification/recovery. Source-pin tests are excluded. Four fail exact frozen identities, the ordering mutant hits the real fork refusal on a previously registered child, and the artifact mutant fails exact missing-file recovery state. Each mutation is restored immediately byte-for-byte; restored provenance SHA256 `e9407f8840376b1065b7d8eac62649d5fca2a34befb9944699fea8bafdde2210`. Normal TEMP retains `aff-p13c4b2c4b-producer-*` driver/backup/six logs/results.

The source audit passes: only the four-line test declaration differs in the original provenance file, all old fixtures/eight source pins match, owned cleanup guards/links/append-only #236 receipt/five-file scope/scoped rustfmt/diff checks pass. Full locked Rust is running before Node-heavy checks in exclusively created `aff-p13c4b2c4b-full-89edf20c32eb4822bf7ac0b3133badd9`; no stale root is reused.

## Full local verification and owned cleanup

After exact mutant restoration, fresh isolated `cargo test --locked` passes1127 tests/one existing ignored (297/828/2;10.37/101.54/1.46s;147.33s total), before Node-heavy checks. Locked all-target check17.84s and Clippy11.70s pass with only the five existing backtest330/342, score416/428 and file_commands156 warnings. Scoped rustfmt/source/fixture/link/append-only receipt/five-file scope/diff checks pass. Vitest1242/80 files (maxWorkers4;6.64s), typecheck and production build pass.

Full-suite root above was removed only after exit0 and exact resolved TEMP parent/GUID verification plus breadth-first per-entry containment/no-reparse checks:42 files/63 directories including the root. New test-owned artifact roots clean themselves only after complete safety checks and SQLite handle drop, including mutant panic paths. No stale root was reused/deleted; normal TEMP retains driver/backup/logs/results outside the removed root. No manual server started; existing service tests own/stop helpers. Local implementation is complete; six exact-head/base CI and verified merge/readback/clean-main delivery remain gates. Only actual provenance4b may close after delivery; snapshot and other market/source/live parents stay incomplete.

This is original writer conformance with synthetic bytes, not raw-to-candle/market fact authentication. Snapshot producer controls are next after verified4b delivery on a later explicit resume; bounded whole-market consumer and trial follow. File/candle/availability/action/cost originals, numeric compatibility and final actual-owner/registry admission retain their own gates. Market4/source/live parents remain incomplete; RegisteredSource/numericCostsHash remain unavailable.
