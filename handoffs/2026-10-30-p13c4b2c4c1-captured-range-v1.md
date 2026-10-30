# Handoff: Capture the selected original campaign range

Date: 2026-10-30 (UTC+8)
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/captured-campaign-range
Status: Resolved; #240 verified delivery.

## Scope and order

After verified #239 (`affce56`), implement the two private integer scalars required
by [contract §2](../docs/plans/research-original-market-consumer-v1.md#2-one-preparation-one-complete-reservation).
Extract fromMs/toMs with checked integer access from the exact selected frozen
campaign binding during existing selection, then retain them in the sealed
graph. No extra parsing, SQL, JSON work, enumeration, public constructor or new
capture refusal. Exact dataset/range agreement remains a later post-reservation
market semantic check; existing lineage/warmup behavior stays intact.

This small preparatory PR precedes the complete observer/reservation/producer
controls4c-2. Actual4c and every later trial/file/candle/availability/action/cost/
actual-owner/registry gate remain open. No RegisteredSource or source authority.

## Intended files and controls

Production: source_rows/selection.rs and source_rows/graph.rs only. Existing
selection/tests.rs, graph/tests.rs and graph/compare_tests.rs verify literal
original bounds, safe-integer
precision, the selected instrument when it is not first, coherent range changes,
caller state/recheck, unchanged usage and preservation of mismatched historical
capture acceptance. No producer, fixture, schema, dependency, frontend or runtime
activation change. Tracked task/previous delivery receipts accompany the code.

Run focused checks then fresh exclusive GUID TEMP/TMP full locked Rust before
Node-heavy checks, locked all-target check/Clippy/scoped format and frontend
checks. Close handles and verify owned parent/leaf/every descendant/no-reparse
before cleanup. Independent review, six completed exact-head/base jobs, normal
expected-head merge/readback and clean-main/full-tree equality gate delivery.

## Implementation and independent review

Selection retains the exact two i64s with checked access on the already frozen
selected binding; private campaign_from_ms/campaign_to_ms getters feed the sealed
graph. No other production body changed. Existing literal manifest controls and
the 135-field recheck coverage include the new scalars. Two new tests cover zero/
MAX_SAFE_INTEGER on a distinct second instrument, exactly one original config
parse/enumeration, coherent campaign rehash, original pretty bytes, deliberately
mismatched dataset acceptance and caller savepoint/transaction/writer/OS lease.
The test fixture accepts explicit original table inputs only inside its existing
private test module; market producers and frozen fixtures remain unchanged.

A different reviewer session approved correctness/scope without findings. The
initial getter name triggered Clippy's from_* convention; the final private name
above removes that new warning without adding a lint allowance. Final full Rust
and checks were repeated after this naming refinement.

## Local verification

- 118 focused source-row controls pass. Final fresh exclusive GUID TEMP/TMP
  cargo test --locked passes1135/one existing ignored (297/836/2;9.78/97.03/5.59s),
  before Node-heavy checks. All prior lineage/warmup/resource controls remain.
- Locked all-target check/Clippy, scoped rustfmt/diff checks pass; only the five
  existing backtest330/342, score416/428 and file_commands156 warnings remain.
- Frozen fixture/six package hashes and six pre-code pins against verified #239,
  unchanged producer/schema/dependency/old-fixture scope, links and append-only
  previous handoff receipt pass. The two intended private source files change
  without rewriting the design's immutable pre-code pins.
- Vitest1242/80 files (maxWorkers4;5.15s), typecheck and build111 modules(0.827s)
  pass. Sandboxed esbuild spawn EPERM was resolved by the identical frontend
  command with escalation, without source/config/dependency changes.
- Final full Playwright run passes98/no retries (2.0m), using the existing default
  single worker. First local run was97 pass/one initial-navigation timeout in
  annualization-samples; unchanged exact-case rerun with trace passes (24.2s).
  Its navigation takes22.99s, fill61.97ms; console has only Vite debug/React info
  and no page error. The mock route/import600daily-bars/holdout/run/popout flow
  verifies the same 180-day warning in both metrics views. Trace-frame inspection
  confirms rendered app/popout with no framework overlay. Browser plugin was
  unavailable; the existing repository Playwright workflow supplies evidence.
  This supports a startup-timing issue, not a proven root cause; no frontend
  or test timeout/assertion was modified. Preserve the initial failure evidence.
- Full Rust roots close test processes/handles before exact canonical parent/
  GUID/every-descendant/no-reparse cleanup:42files/63directories in each full
  run. The focused root is empty at cleanup. All three Playwright-managed ports
  are released; trace/frame output remains outside the repo. Local logs/manifests
  remain under ignored test-results/captured-range.

All local gates pass. Six exact-head/base jobs and normal merge/readback remain
delivery gates; metadata preparation alone does not complete actual4c.

## Resolution — PR #240 delivery (2026-10-31)

Verified [#240](https://github.com/yoyoCadence/AlphaFactorForge/pull/240): head `914a22725253de04a236255f329dff1402da3d92`, base `affce56aba95c41f0990b291705bf8f383fd7d3a`, merge `8d12f6ac7274328f45468b6a7ed2b75807f25426`, tree `98340cf104d4b9d7fe1e422f662b860cda009db1`. Workflow41068450301 passes all six exact-head/base jobs once:1135 Rust/one existing ignored,1242 Vitest/80 files,98 E2E/no retries,audit0,native smoke and policy restoration/owned cleanup. Normal expected-head merge returned HTTP502; subsequent read-only PR lookup confirmed MERGED and exact identities before any further mutation. GitHub final receipt and clean-main/full-tree equality verified.

Whole-consumer4c-2 then began locally. Original-writer setup independently exposed v1 stale seed claims before the new observer ran; unpublished checkpoint `d682a8d` preserves that work. An additive design/data correction now precedes its continuation; this metadata delivery never closes actual4c or source/live parents.
