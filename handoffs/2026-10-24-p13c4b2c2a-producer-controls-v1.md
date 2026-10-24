# Handoff: actual original producer conformance controls

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b2c2a-producer-controls
Status: Resolved; merged as PR #222 with six exact-head/base jobs

## Summary

From merged #221 (`bf0d69c`), implement only
[frozen first code](../docs/plans/research-original-producer-projections-v1.md#5-first-code-slice-actual-producer-controls-before-consumer-code).
The exact inline StrategyDef construction moved to private
candidate_strategy_def; its call stays in the same runner start-loop position,
inside the same lock and immediately before the unchanged repository call.
All fields, serialization/evaluation order and producer/repository policies are
preserved. Only that helper is formatted; the legacy runner file is not rewritten.

Six new tests compare the nine pre-code literal candidates against actual helper,
engine, run_lineage, repository and history writers. Every format has a fresh
migrated file SQLite DB and actual isolated lease/owner. Root allocation uses
bounded create_dir/AlreadyExists skipping, never opening/removing a preexisting
PID/counter directory. Handles close before removal of the newly created root.
Only declared dataset/run reference rows are seeded; no candles, jobs, artifacts,
results, registry events, providers, protected execution or statistical sampling.

## Actual evidence and remaining boundaries

All **six focused tests pass**: v1 ordered axes (3 candidates / 1 hypothesis),
v1 original winning-base dedup (4 / 2), v2 fixed DSL (1 / 1), v3 walk-forward
(1 / 1), existing-library conflict preservation and intended numeric-hash refusal.
Every fresh DTO/stored strategy field, exact input/engine/hypothesis JSON, scalar
projection and foreign reference matches #221's unchanged independent fixture.
Repeated complete hypothesis content returns the same IDs without new rows.

The conflict control preserves whitespace/reversed key order, name/source/
lifecycle and a deliberately contradictory saved dsl_json/params claim. This
characterizes the unchanged writer and is not semantic consumer acceptance.
The numeric control preserves the intended identity, observes the frozen legacy
reparse and refuses before any strategy/total_changes/mutation_seq write.
No fixture value, identity, numeric policy, schema/dependency/workflow, engine,
history, resume or execution behavior is changed.

The first compile found a test-module name collision with tests::history;
the actual research history import now has an explicit alias. An initial focused
run used invalid run status queued; test reference setup now uses idle. Another
test mistakenly repeated full attempt registration: attempt keys are unique;
only complete hypothesis registration deduplicates, as the original contract
states. The corrected repeat tests register hypotheses directly. Setup/repeat
failure logs remain under normal TEMP. These were test harness corrections,
not production changes or fixture compatibility repairs.

## Required verification / next action

Six frozen assertion mutants, exact restoration of both runner/engine files,
full isolated locked Rust before Node, all-target check/Clippy/scoped helper/test
format, frontend verification and six exact-head CI precede delivery. Preserve
caller/base/head/review checks and clean-main verification. After these controls
deliver, refine bounded sealed-graph consumer APIs and whole parse/enumeration/
scratch budgets before separate semantic implementation. No source builder or
runtime admission follows from producer conformance. Original numeric/cost/
action/file/candle/trial/live-owner/registry obligations and parent phases remain
incomplete.

## Resolution (focused/mutant evidence)

Six focused cases pass with all nine unchanged independent candidate controls.
Six temporary mutants each exited 101 with an assertion panic / FAILED and no
compiler error: full DSL wrapper (15.16 s), wrong DSL kind (14.80 s), v1 default
engine contracts (14.70 s), candidate hypothesis hash (13.70 s), first base only
(13.58 s), reversed axis order (18.70 s). Per-mutant logs and restorable source
backups remain in normal TEMP. Exact finally restoration was independently
read back:

- runner mod.rs SHA-256:
  `c6bd080032e6445ab10ac3153ec2a9a932c571b446e13df63518177a61d86bd3`
- execution.rs SHA-256:
  `d050bd6f7cccbb968e6b2f003ab568813cedddecdb7d71e089e83c750c593139`

execution.rs has no final Git diff. The actual extracted helper and new test file
pass isolated/scoped rustfmt checks; existing runner/tests modules are not
globally formatted. Fresh isolated full locked Rust is running before Node,
followed by check/Clippy/scoped format. Logs stay outside its owned TEMP root;
delivery and final cleanup are recorded after completion.

## Resolution (2026-10-25 local verification)

Full isolated `cargo test --locked` passes **1046 tests / one ignored**
(297 library / 747 desktop / 2 service); test times 10.27 / 92.12 / 5.68 s
after 22.49 s compilation. All-target locked check passes in 17.56 s;
Clippy passes in 7.99 s with only the five previously recorded warnings
(backtest:330/342, score:416/428, test file_commands:156). Scoped helper/new-test
rustfmt passes. Frontend: **1242 Vitest / 80 files**, typecheck and build pass.
Rust preceded Node. execution.rs remains unchanged; both restored source
checksums above were read back after mutation work. The frozen JSON fixture
remains unchanged.

The exact newly owned full-check root
`aff-p13c4b2c2a-full-11676c4d5b474f848667cfd98f3121f3` was checked against
normal TEMP, traversed without reparse points and removed (43 regular files /
64 directories). Producer-control roots close handles and clean themselves.
Verification/mutant logs and backups remain outside the removed root. No old
PID/counter roots were reused or deleted.

Local implementation is complete; draft publication, six exact-head/base jobs,
fresh reviews/main, expected-head merge and clean-main tree verification remain
delivery requirements. The next slice refines bounded sealed-graph consumer
APIs and whole-operation budgets before semantic implementation.

## Resolution (2026-10-25 delivery)

[PR #222](https://github.com/yoyoCadence/AlphaFactorForge/pull/222) merged as
`754b08a2a3a1010a68795f9e7cdae82db0d00b08`. Tested head
`6c04c39a078d148c287ece660e8bbe20c005094c`, base
`bf0d69c5090b6b6861df106b6f7d67c31d2da2f9`; CI run **40143396281**.
All six completed job logs show the exact head/base checkout. Rust **1046 plus
one ignored** (297/747/2; 32.30/281.71/5.79 s), including all six new controls;
Vitest **1242 / 80 files**, E2E **98 without retries**, production audit **0**.
Native render/SQLite invoke/metrics/event round trip, exact hosted-policy
restoration and owned process/TEMP cleanup pass.

Fresh main remained unchanged; no reviews/threads and mergeable before ready
and expected-head merge. GitHub merged/closed was read back. Local main
fast-forwarded to the merge; its full tree equals the tested head and worktree
is clean. Final PR body contains this receipt. Next is the separately scoped
bounded sealed-graph consumer contract; parent source/admission phases remain
incomplete.
