# Handoff: P13b reservation registry persistence design

Date: 2026-10-15
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13b-reservation-registry-plan
Baseline: `5aef090ac181af8478661576973443ddae7ac5e5` (merged PR #181)
Status: Design complete; publication and final-head CI pending

## Scope

The maintainer requested continued development and autonomous PR/merge work.
The P13b design task was already in Next, and moved to In Progress before
the tracked design artifact was added. This is a documentation slice, with
no SQL, source, test, dependency or runtime permission change.

The [persistence plan](../docs/plans/research-reservation-registry-plan-v1.md)
turns the adopted per-family budget/replay/conflict decisions into concrete
storage, binding/transfer and crash/atomicity requirements. P13b-1 owns
append-only storage and serializable local writes; P13b-2 owns rollback and
complete transfer; P13c must first specify frozen batches, exposure and the
integrated final-fence/reservation protocol before production execution.

## Important implementation constraints

- Use the same external trial-registry database; retain the existing family
  identities and chain. A process mutex does not serialize another process.
- Preserve conflicting reservation/budget bodies and permanent alpha-family
  stops. Do not use a unique number constraint or replace-on-conflict write
  that discards the second record. A source operation key is checked across
  families as well as within P13a's selected family.
- Extend binding observations by identity sets, so an equally sized restored
  history with different records cannot pass. Unknown old alpha evidence is
  not empty history. P13-aware transfer needs the complete new format.
- The final count, actual allocated alpha, frozen-count/cap precision check
  and reservation commit belong in one IMMEDIATE registry transaction.
  Reuse transaction-scoped helpers, not the returning `fence_admission` call.
- Registry spend commits before workspace enqueue; every crash/retry keeps
  the original spend and operation identity. No implicit refund/reallocation.
- Alpha conflicts block confirmation; do not reuse trial-family quarantine
  to block all exploration. No existing trial-only behavior changes here.

## Verification

Cross-checked the plan against registry migrations 0001–0003,
`trial_ledger.rs`, `trial_ledger_binding.rs`, `trial_ledger_transfer.rs`,
alpha-allocation §9, trial-ledger §23 and the P12d final-fence requirement.
P13a's independently reproduced identity, original conflict counterexample
and completeness-limit regressions are the implementation baseline.
`git diff --check` and relative-link verification run before publication;
full six-job final-head CI remains required before merge.

## Limits and next step

This plan changes no statistical policy, seed, threshold or accepted set.
Unsupported-configuration policy, real-market confirmation, exposure overlap,
legacy manual exposure and prompt filtering remain explicit P13c decisions.
P13 remains In Progress. After this PR merges, start P13b-1 as an isolated
implementation PR; do not bundle binding/export or runtime reveal into it.
