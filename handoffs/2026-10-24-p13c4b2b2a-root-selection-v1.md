# Handoff: P13c-4b-2b-2a bounded original root selection

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b2b2a-root-selection
Baseline: merged #215 (`731aa5e`).
Status: implementation/local verification complete; PR/CI delivery pending.

## Summary

Implement only the [frozen selection slice](../docs/plans/research-source-graph-adapters-v1.md#2-first-code-slice-bounded-root-selection).
The private adapter re-freezes original campaign roots, selects the exact
admission instrument, parses the original config under the unchanged legacy
policy, checks dataset equality and derives every ordered candidate key.
Performance-only concurrency parsing is independent of current hardware.

Before the existing Cartesian enumerator can clone strategies/base IDs, checked
raw products and the frozen B/Q scratch formula enforce byte/node ceilings.
An in-memory serializer counting sink retains no output or identity encoding.
All strings/keys preserve existing strategy-v2/dataset-content-v2 prefixes;
original root bytes remain immutable. The output has private fields/read-only
getters, no Debug/serde/default/factory or runtime caller.

## Independent Evidence

Thirteen new cases plus all 49 earlier workspace/JSON/root tests cover six saved
manifests (3/9/1/4/4/4), authored v2 DSL/v3 walk-forward/legacy long-float controls,
second-instrument selection, identity/version/membership/dataset refusals,
deterministic concurrency, raw over-cap/all-pruned input, exact B/Q sizing,
4096/4097 products, joint exact/one-over/overflow and pre-Cartesian refusal.

The new source-selection-v1 fixture was frozen independently of the adapter:
existing TypeScript parseDiscoveryConfig/enumerateCandidates generated v2/v3
manifests. For the long-float reference, DataView assigned f64 bits
`3f6e43cfc21ad9ff` to both strategy and benchmark fee before reference enumeration;
the stored configJson retains original token `0.0036944444444444438` in both places.
This characterizes the existing legacy reader, not original execution or repair.
Original/dataset-a/second-instrument declaration IDs came from an independent
type-tagged binary encoder, including the existing known original control.

The temporary reference script's initial sandbox spawn EPERM was resolved by
same-command escalation. Its first legacy reference input failed the existing
benchmark-cost equality check; both test fees were aligned before freezing output.
Initial Rust fixture include/module paths were corrected after compile errors.
No existing fixture, parser, identity policy, dependency or runtime changed.

Four explicit runtime-failed mutants were caught, never compile failures: first
instrument selection, omitted dataset equality (both gates), omitted scratch
preflight and partial manifest. The test-only enumerator tripwire prevents a
broken preflight mutant from allocating the oversized product. Exact original
source restored after each; selection.rs SHA-256:
`a085b966955c97e29fada648191e33b381451dd0a0201aeebb7adadb79e13bc3`.
Both temporary reference/mutation drivers were removed.

## Full Verification / Windows Temp Evidence

The first full locked Rust run passed library 297 + one ignored but failed five
existing desktop cases: three campaign/admission expected counts/statuses and two
alpha-transfer declarations observed Replayed instead of Stored. All new selector
cases passed. Preserve TEMP `aff-p13c4b2b2a-rust.log` as the failed receipt.

Those existing fixture helpers use PID+counter names and attempt directory removal
while database fields may still be live. Default TEMP contained alpha/admission
roots for PID 9880 created at 06:37, before this slice; existing SQLite files kept
the older timestamps. This supports stale fixture/PID reuse, not an adapter-driven
ledger write (selection has no ledger/SQL path). No production/test source changed
between runs. A fresh command-local TEMP/TMP root then passed full locked Rust:
**1011 pass** (297/712/2), one existing ignored, 10.39 / 85.69 / 1.35 seconds.
This controlled isolation result does not replace a permanent fixture cleanup fix.

All-target locked check/Clippy/scoped format pass with five existing warnings.
Vitest 1242 / 80 files, TypeScript and production build pass. Preserved receipts:
TEMP `aff-p13c4b2b2a-{rust-isolated,check,frontend}.log`, focused/boundary logs and
four `aff-p13c4b2b2a-mutant-*.log` files. The unique verification root's 170 regular
files were removed only after exact absolute-root/parent and no-reparse checks;
three successful log receipts were copied out first. Original failure evidence
and pre-existing default-TEMP data were preserved.

## Limits / Next Step

No graph SQL, schema/dependency/workflow changes, source/RegisteredSource builder,
file/candle/registry/IO authority, spend, command/UI or execution. Counting sink
implements Write only for bounded in-memory sizing. Scratch limits are not a
whole-process memory claim. Missing original execution/cost/action/source evidence
remains unsupported; parent 4b-2b/4b-2/4b/P13c-4 stay incomplete.

After fresh main/rebase and post-rebase focused/scope/link checks, publish the
draft, verify six exact-head CI jobs/reviews/base, merge with expected head and
verify clean main. Only the already-frozen eleven-table graph 4b-2b-2b follows.
Keep Windows temp fixture isolation/cleanup as a separate Backlog investigation.
