# Handoff: P13c-2 pure Train-only generation context

Date: 2026-10-16
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c2-train-context
Base: merged #188 (`70dcc424985d5cb42e40d8dbdc57b4cfbdcaf805`)
Status: Merged as PR #189; P13c-3 continues with storage/transport design.

## Summary

The [wire contract](../docs/research-train-context-v1.md) fixes the bounded
allowlist before implementation. Scope binds one Train-only data slice and
its actual time window. Approved origins contain typed integer summaries or
finite hypothesis/failure codes, with aggregate and per-input provenance.
There are no generic report maps, free-form notes or provider calls.

Assembly requires the exact complete origin set, rejecting hand-picked
subsets and stale scope bindings. It checks current known protected exposure
against the whole Train window and rejects protected digest references even
through recognized prefixes. Stored contexts are reassembled against current
observations; a cached record cannot waive newly seen/unknown history.
Only explicit Train scope/fact fields enter the provider DTO and context ID.
Static error codes never echo rejected field names, values or parser text.

This is a pure module with no runtime caller, IO, migration, reservation,
execution or provider integration. Existing statistical settings, calibration
support, final seed and legacy numeric parsing remain unchanged.

## Verification

The committed independent Node encoder generates fixed scope/origin/context
identities without importing the Rust or TypeScript implementation. Its
output regenerates byte-for-byte; Rust and the existing TypeScript encoder
reproduce those values. The positive fixture includes all three fact kinds.

Sixteen new Rust tests cover all scope bindings, complete origin membership,
aggregate/nested mixed provenance, every nested object/typed scalar escape,
free-form notes/selection/hash refusal, digest prefix aliases, stale scope,
new/unknown exposure on replay, whole-window half-open overlap, integer and
exact/one-over bounds, and static error non-disclosure. Four executable mutants
were assertion-caught: skipped nested provenance, ignored fresh exposure,
accepted origin subsets and missing digest normalization. Mutated source was
restored byte-for-byte.

Local `cargo test --locked`: 708 passed (220 library + 486 desktop + 2 real
service smoke), one existing ignored test. `npm test`: 1197 passed / 75 files.
Typecheck, production build, all-target Cargo check and Clippy passed; five
existing warnings, none in the new module. Targeted rustfmt and relative file
links pass. Final-head CI results are recorded with the delivery PR, including
the unchanged browser/native smoke lanes.
No provider invocation or protected-data/calibration execution was performed.

## Host responsibility and next task

The host must authenticate the original Train-only data, actual support,
scope, complete approved origin set and derivation/approval from Train alone.
It must obtain a complete synchronized protection observation at every use.
The pure module checks consistency; a `train_only` label, hash or fresh
in-memory wrapper is not proof of authenticity, complete lineage or permission
to release data. A regression explicitly shows self-consistent invented or
truncated evidence cannot be detected here. No production caller is enabled.

P13c-3 design is next: exact append-only exposure/receipt schema, identities,
capacities, migration/legacy behavior and successor complete binding/transfer
contracts before separately reviewed persistence and transfer slices. Runtime
and real-market confirmation remain behind the existing policy/mediation gates.

## Resolution (2026-10-16)

[PR #189](https://github.com/yoyoCadence/AlphaFactorForge/pull/189) merged as
`331a8807a3c2ef2c23a167d8dc143ad2320de604` after all six jobs passed on
final head `fda7e1abb98315c6de6a1f742e90972e4b5726c8` in
[CI run 38918654971](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38918654971).
CI confirmed 708 Rust tests (one ignored), 1197 Vitest, 98 E2E and real native
SQLite/invoke/event/metrics smoke with cleanup. The merged tree matches the
verified head. P13c-3 follows from that merged main with an
[exact storage/transport contract](../docs/plans/research-confirmation-storage-transfer-plan-v1.md)
before migration or runtime changes; the Train context boundary stays pure.
