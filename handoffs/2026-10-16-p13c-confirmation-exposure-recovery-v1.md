# Handoff: P13c confirmation, exposure and recovery design

Date: 2026-10-16
Branch: docs/p13c-confirmation-exposure-recovery
Base: merged #185 (`6a95837f4cebc12e881ad536d4c66ba2cc696279`)
PR: [#187](https://github.com/yoyoCadence/AlphaFactorForge/pull/187)
Status: Design delivered; the PR records final-head CI and merge status.

## Scope and decisions

The next task from the task board is now specified in the
[P13c contract](../docs/plans/research-confirmation-exposure-recovery-plan-v1.md).
This is a design slice, not a runtime implementation or statistical-policy
extension. It preserves the active plan, P12 and adopted P13 alpha decisions.

- Freeze the complete candidate/Validation batch, its selection rule and Test
  region before protected evaluation; freeze the exact sealed Validation
  selection before Test. Each stage spends its own scheduled share.
- Occupancy is canonical instrument + actual half-open time support across
  datasets/sources/revisions/timeframes/roles. Legacy/manual uncertainty stays
  separate from unknown trial/alpha history and cannot become known-unseen
  through import, rename or self-attestation.
- Pin verified backend input bytes and workspace epoch/mutation/reference
  observations. Workspace write guard precedes the registry mutex/IMMEDIATE
  transaction. Current trial count, actual-share P12a, alpha spend, exposure
  claim and final admission receipt share one registry commit, then the
  owner-checked workspace enqueue commits. No reverse workspace callback.
- Persist original origin/key before admission. Retry/replacement recovers
  that exact receipt. Failure never refunds spend; no automatic re-execution
  after execution_started. Only durable sealed output is replay/projectable.
- Require an exposure-aware successor to P13b transfer/binding before runtime;
  v4 alpha history is insufficient. Train-only context assembly rejects mixed
  or protected-derived evidence. Manual/raw/chart/Holdout/worker/export bypass
  mediation and the explicit statistical policy gate must precede production
  activation, even if execution code has been implemented behind gates.

## Delivery sequence

P13c-1 is Next after this design merges: define bounded strict pure
batch/selection/operation documents and interval overlap, with independent
identity/overlap cases and tampering/refusal tests. No IO, schema, reserve,
execute, provider or reveal caller.

P13c-2 (Train-only context), P13c-3 design and separately reviewed persistence /
binding-transfer slices, P13c-4 admission, P13c-5 dormant execution and P13c-6
manual mediation/UI remain Backlog in contract order. P12e support remains
its exact simulated tested set. Real-market qualification or per-declaration
simulation policy needs its separate decision/evidence contract. No final
calibration-seed run is authorized or performed.

## Verification

Checked the plan against the active-plan split/exposure rules, provider §7,
P12 campaign/snapshot/admission APIs, workspace ownership/mutation helpers,
P13a reservation identity and P13b atomicity/transfer contracts. Reviewed
stage separation, current-family Holm count, source/union rollback, original
origin retry, manual-read/admission race and every commit/execution/seal crash
boundary. Twelve relative file links, conflict-marker refusal, five interval
examples and diff checks passed; the two added primary contract links were
also checked to exist. No unsupported calibration simulation was run.

No product source, migration, dependency, fixture or test setting changes.
Baseline #185 has six green CI jobs, 668 Rust (1 ignored), 1184 Vitest and
real native SQLite/event/metrics acceptance. The design PR must also complete
the unchanged six-job CI before its expected-head guarded merge.
