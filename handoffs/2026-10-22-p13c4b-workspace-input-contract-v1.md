# Handoff: P13c-4b workspace/input contract

Date: 2026-10-22
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c4b-workspace-input-contract
Baseline: merged #208 (`6db0674`).
Status: design/local verification complete; documentation-only PR/CI pending.

## Summary

The [contract](../docs/plans/research-workspace-input-observation-v1.md) separates
transaction-owned workspace consistency observations from complete source/ref
verification and bounded immutable input materialization. Only the first small
metadata reader is specified for the next code PR. It cannot authenticate a
physical owner or unseen provenance, and no runtime caller is added.

## Required action

Deliver this documentation-only refinement through exact-head CI/merge, then
promote **4b-1** only. Implement the frozen private ExpectedWorkspace and
WorkspaceObservation API, active borrowed reads, bounded/type-checked metadata,
static refusal order, positive epoch/instance checks and all mutation/rollback/
reservation/deferred-snapshot tests in contract sections 2-3. Keep old public
ownership/identity behavior untouched. Restore explicit assertion-caught mutants
before full checks. No schema/spend/activation change belongs in that PR.

## Review notes

Runtime owns a private OS lock, but loose public workspace/epoch fields are not
physical authority. Existing snapshot/source structs and trial witnesses do not
authenticate all candidate/artifact metadata or unseen acquisition. Some history
DTO JSON reads fall back to Null, and existing artifact reads are unbounded;
later contracts must freeze strict bounded replacements without rewriting
ordinary exploration paths. Future final admission still needs an actual private
owner guard and workspace IMMEDIATE before registry serialization.

The first reader's 256-byte original ID JSON cap is explicit and inclusive;
SQL typeof/length checks precede text allocation. Same-ID copies/deferred reads
remain comparison-only. Heartbeats/PID/timestamps do not change domain stamps.
Policy/managed acquisition/manual mediation authorizations remain unavailable.
No calibration/reserved seed, protected execution, UI/command/provider or new
migration. Parent 4b and P13c-4 remain incomplete through remaining acceptance.

## Verification

Check source grounding, relative links/anchors, task order and Markdown-only
scope. Source/schema/dependencies/fixtures/workflows must match #208 exactly.
Inherited code CI: 949 Rust (one ignored), 1242 Vitest / 80 files, 98 first-attempt
E2E and successful native rendering/invoke/metrics/event/policy/cleanup. Do not
attribute a redundant local source test run to this documentation refinement.
Fresh base/reviews, six exact-head jobs, expected-head merge and clean main are
required before 4b-1 begins.

## Resolution (design verification)

Source grounding checked runtime lease/startup, actual identity/ownership/state
schema and readers, snapshot resolver, history JSON fallbacks, unbounded artifact
read, pure source contracts and historical trial reconstruction. HolderKind has
exact desktop-embedded/service values; runtime IDs use 32 ASCII hex characters;
actual mutation state is an integer singleton. The new 256-byte JSON bound and
no-None/zero epoch apply only to the dormant reader; old APIs stay untouched.

The contract retains the parent's stale-artifact/ref/owner refusal requirement;
later pinning must define detection and immutable consumption without claiming
filesystem atomicity or excusing a mismatched authenticated reference. No
physical-owner or unseen capability comes from matching metadata or a copy.
Only 4b-1 follows this design; later resource/API contracts must be recorded
before their code. Parent 4b/P13c-4 remain incomplete.

Relative-link/anchor, task lifecycle/order and Markdown-only scope audits are
being finalized. No source, migration, dependency, fixture, workflow or
statistical change; #208 is the exact inherited code/test baseline. Delivery
requires six exact-head jobs and fresh reviews/base before guarded merge.

All **23 added relative links/anchors** resolve (16 in the two new documents).
Task lifecycle/order, source grounding and `git diff --check` pass. Exactly five
Markdown files are in scope; source/schema/dependencies/fixtures/workflows are
unchanged from merged #208. No redundant local source test run is claimed.
