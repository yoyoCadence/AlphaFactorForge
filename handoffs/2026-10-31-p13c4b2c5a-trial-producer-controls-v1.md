# Handoff: actual original trial/split producer conformance

Date: 2026-10-31 (UTC+8)
Repo: yoyoCadence/AlphaFactorForge
Branch: test/original-trial-producers
Baseline: verified #243 (`095d861`)
Status: Independent final review approves; all local checks/cleanup pass, delivery pending.

## Scope and agreed implementation

Implement ordered5a against the immutable [contract](../docs/plans/research-original-trial-projections-v1.md).
Call unchanged original enumeration/trial_split_identity/candidate report/
candidate_split_hashes and private ledger preparation/real registration for
11 cases/41 candidates, six accepted controls,24 refusals and nine primitives.
Add only fixed cfg(test) module declarations to runner mod.rs/trial_ledger.rs
and their new test modules/helpers, plus task/append-only handoff updates.
No runtime body/schema/older fixture/dependency change or source constructor.

Every fresh-sequence case uses an exclusive empty sibling registry beneath an
owned OS-temp root. Preserve complete table/schema/value/storage-type/REAL-bit/
sqlite_sequence metadata, actual writer total_changes, artifacts, caller
transaction and lease. Release writer mutex guards before producer calls. Test
reversed-input replay, unchanged state, subset/superset conflict and changed-key
idempotency precedence. Known-family protocol2-to3 appends only the upgrade and
retains original protocol2/event/batch/receipt, then refuses downgrade without
writes. The grid's null family is not a protocol-upgrade proof.

The complete runner control obtains VerifiedDataset only from the original
loader over the actual frozen four-bar dataset. Retain the real workspace ID;
independently reframe the fixed declared namespace rather than rewrite birth
metadata. Register against actual source/snapshot inputs before any historical
attempt backfill. Pure counterfactual counts never construct VerifiedDataset.
No worker execution/benchmark attestation/AI/generation/Test-seed activity.

## Review and verification gates

A separate reviewer inspected original APIs and approved this bounded plan,
including fresh ordering, current binding/head handling, true writer counters,
conflict precedence and protocol distinction. Focused assertions and meaningful
behavioral mutants precede one fresh full isolated Rust run, locked all-target
check/Clippy, required frontend checks, bounded scope/source/link/fixture checks
and final independent review. Close handles and verify exact containment and
no-reparse paths before owned cleanup. Six exact-head/base CI jobs, normal
expected-head merge, GitHub readback and clean-main/full-tree equality complete
delivery. Consumer API/work design and every source/live parent remain open.

## Focused implementation evidence

Eight fixed controls now pass, covering all11/41 original pure identities/hashes/
nullable candidate reports, nine split primitives,17 original ledger preparations
and isolated fresh registrations/replays,24 actual validation refusals, complete
subset/superset/idempotency precedence, known-family2-to3 and the real four-bar
runner path. The latter builds the actual original calendar/instrument/provenance/
snapshot and imports the frozen candles, then uses the original verified loader,
actual scheduled strategy/run IDs, real ownership epoch and workspace birth ID.
Its expected namespace is independently reframed without ledger identity helpers.
Missing binding and four late lineage defects refuse without writes; actual
returned head/admission equal fresh original reads, and input-order event links
persist through the original history writer inside a caller savepoint whose
rollback restores all original SQL rows. Registry-only registration leaves the
workspace transaction/binding/lease intact.

Fresh registration permits exactly its four original tables and exact
n+2+2*known-family writer changes; every other registry row/table/schema remains
unchanged. Replays/refusals compare all schema/table cells/storage types/REAL
bits/sqlite_sequence and the real writer counter. Protocol upgrade appends
exactly one row, preserves every prior event/batch/receipt/base protocol and
refuses downgrade without writes. Complete artifact bytes and empty directories
are preserved; six exact SQLite files are captured logically rather than read
through Windows SHM locks. A zero-byte locked ownership file is represented by
its actual zero length. Initial focused harness lock-read failures and corrected
passing logs remain ignored; no runtime change was needed. Focused4 is8/8 in1.05s.

Independent early review requested real epoch, returned watermark/admission and
missing-binding assertions; all are incorporated. Nine actual behavioral mutants,
fresh full checks and final delivery review are the remaining local gates.

## Behavioral and full verification

All nine deliberate original-body mutants compile and fail runtime assertions:
holding identity, candidate lookback guard, numeric member sort, Diagnostic
effective status, returned input order, missing protocol upgrade, premature
membership conflict, missing strategy binding and stale returned head. No checksum
or test mutation supplies the evidence. One initial upgrade mutant failed type
inference and is explicitly excluded; its compiling replacement fails the actual
upgrade assertions. Every target's exact original bytes/SHA are restored and
independently checked. The final production diff remains only five cfg(test)
declaration lines across two old files; execution.rs and all old fixtures/schema/
dependencies are unchanged. Scoped format/append-only/lifecycle/490-link checks pass.

Fresh isolated full Rust passes1166 tests/one existing ignored:297 library in9.76s,
867 desktop in99.01s and two service smokes in5.53s. Locked all-target check passes
in16.49s. Clippy's new constant-assertion style warning was corrected to its
equivalent const block; final focused8/8 in1.07s and all-target Clippy in7.14s pass,
with only the five pre-existing warnings (backtest330/342,score416/428,file_commands156).
No production body changed for that test-only style fix. Independent Node still
verifies every frozen projection and19 historical source/nine package pins.

Vitest1242/80 files passes in4.05s, typecheck passes, and build passes111 modules
in829ms. Sandbox Vite/esbuild spawn EPERM was reproduced, logged and resolved by
rerunning identical commands escalated; it caused no source edit. Existing finite
project Playwright harness is the recorded fallback without a Browser plugin;
E2E, managed-server release, exact owned-root cleanup and final independent review
remain pending before commit/PR. No user manual operation is required.

Final local E2E passes98/no retries in2.0m; managed ports5173/5174/5175 are all
released. All22 explicit owned roots were checked against the exact OS-temp
parent, expected leaf/canonical descendants/no-reparse points and removed after
all compiler/test/browser processes exited:980 files/126 directories including
roots. The full Rust root contained43 files/64 directories; all per-case Harness
roots had already closed ledger/workspace/lease and cleaned themselves. The one
initial excluded compile-control root was explicitly recovered as the empty
owned aff-trial-mutant-hinw_mcv directory, created10:28:25UTC; no wildcard delete
or unrelated path was used. Ignored logs, manifests and initial-failure evidence
are retained. Only separate final review and normal exact-head CI/merge/readback
remain; no additional implementation or manual test is outstanding locally.

The separate final reviewer approves after independently checking1166 Rust/one
ignored,1242 Vitest,98 E2E,check/Clippy,nine behavioral mutants/exact restoration,
all22-root cleanup and the seven-file/five-cfg-line scope. No unresolved material
issue or requested code change remains. Fresh main/origin/main are095d861 and
same-head PR search is empty before publication. Only exact-head CI and normal
merge/GitHub final receipt/clean-main full-tree verification remain.
