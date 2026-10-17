# Handoff: P13c-3b-1b-2 durable source-witness design

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c3b1b2-source-witness-design
Status: Design locally verified; delivery CI/merge pending

## Summary

From merged reconstruction #196 (`ea8387a`), specify durable self-contained
original source witnesses before activating binding observation. A complete
original trial snapshot supplies historical facts across reopen, receiver growth
and source forks; receipt totals and current primary rows do not replace it.
[Plan](../docs/plans/research-source-witness-storage-plan-v1.md).

## Decisions / Implementation Order

- Canonical closed `research-confirmation-source-witness-v1`: original trial v3
  text plus complete typed registered-source claims, no binding reference or
  authoritative count fields. Exact raw outer SHA is the third logical blob kind.
- Self-contained derived trial anchors and checked host observations tied to
  witness SHA. Admissions require exact original binding anchor/source matches.
  Freeze-only sources can retain proof roots without inventing historical alpha
  fields. Statistics/binding orphan rules remain strict.
- Keep eligible historical context at two tests per trial. Generic other source
  protocol facts never become admission/report support. Match same-head histories
  by exact original anchor. Align pure graph quarantine/HWM coverage with the
  existing binding rule, while retaining actual current maps and live stops.
- Additive schema 7 with an eleventh witness table; retain old blob contents,
  checks/FKs and birth metadata. Add immutable/anti-REPLACE/namespace guards and
  extend all joint capacities/legacy omission checks. Original witness 8 MiB;
  existing 10,000 rows/64 MiB/interval/unknown limits remain unchanged.
- Pre-witness managed histories retain permanent uncertainty; missing proofs
  are incomplete state, never receipt-derived known history. Explicit verified
  witness-only restoration validates materialized old-plus-proof history without
  auto-repairing missing restrictive witnesses. All late failures roll back.
- Proof roots do not ingest trials. Known current history must account for
  their event/protocol facts or retain actual permanent stops on unincorporated
  history; read observation rejects unmarked gaps, archival writes may add
  uncertainty atomically, and qualification imports reject it before writes.
- Revise the unimplemented archive v5 plan explicitly to schema 7/third blob
  kind, keeping nine counts and existing binding v5 fields. No shipped wire
  implementation or old migration is changed by this documentation PR.
- Ordered PRs: witness codec/source anchors; pure graph integration; additive
  SQL/backend context/restoration; original binding observer/adoption/downgrade;
  complete transport. Each waits for predecessor final-head CI and merge.

## Verification / Limits

Documentation only; runtime/schema/fixtures remain identical to verified #196
(805 Rust, 1236 Vitest / 78 files, 98 first-pass E2E and native smoke). All 15
local Markdown targets across the four related plan/handoff documents exist;
task-plan links, `git diff --check` and runtime-tree equality pass. The five
changed files are Markdown only. Final-head six-job CI and merge remain the
delivery gate before codec/anchor implementation.

Source inspection confirmed the two-test historical admission guard, existing
stats/binding direct-use rule and existing quarantine high-water-mark coverage;
the plan preserves those boundaries instead of widening policy to fit generic
proof data. Metadata absent from trial rows remains source claims. No IO/runtime
observer, publisher/file authentication, current count/owner/share authority or
execution/recovery permission is added. Witness data never enters AI context,
generation/tuning/ranking. No bootstrap/calibration/reserved seed was run.
