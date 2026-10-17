# Handoff: P13c-3b-1a pure binding v5 foundation

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b1a-binding-codec
Status: Locally verified; delivery CI/merge pending

## Summary

From merged storage #194 (`bb16fcf`), expose the existing original-text v5
binding-blob codec as an immutable pure snapshot, canonical bounded JSON and
exact per-section confirmation-ID coverage. This is the first bounded part of
P13c-3b-1, followed by whole-registry observation/adoption.
[Contract](../docs/research-binding-snapshot-v5.md),
[parent plan](../docs/plans/research-confirmation-storage-transfer-plan-v1.md).

## What Changed / Responsibilities

- `confirmation_history/binding_snapshot.rs`: private checked snapshot,
  read-only trial/alpha/confirmation getters, recursive sorted JSON and
  allocation-free sorted subset traversal for all nine confirmation sets.
- `admission.rs`: reuse its existing strict decoder; bound original input
  before decoding on either entry point and permit serialization of its
  already checked internal types. No statistical arithmetic changes.
- Independent Node byte/genesis fixture and Rust/TypeScript oracles. Rust
  contract tests cover original-text failures, exact capacities and exhaustive
  per-section subset relations; workspace SQL verifies dormant integration.
- Root board/plan explicitly split 3b-1a and 3b-1b without reordering transport
  or later admission tasks. Append #194's actual merge/CI receipt.

## Boundary / Required Next Work

The runtime continues its existing v1-v4 behavior and 4 MiB bound. The new
pure API accepts v5 only, up to 8 MiB; it never observes registry rows or
converts legacy unknown evidence to empty. Format validity and confirmation-ID
coverage prove no graph existence, historical authenticity, freshness or permit.
Canonical output is for a new binding; stored original blob bytes never change.

Before 3b-1b activates the writer/observer, specify durable independent
historical context. The caller-supplied GraphContext used by checked storage
does not survive workspace reopen, and incoming conflicted trial branches may
not survive as primary rows/checkpoints. Receipts cannot independently witness
their own counts/source metadata; current larger receiver counts cannot replace
historical counts. Preserve valid restrictive contradictory/unknown history
and last-good bindings. This PR adds no witness payload/schema or partial
runtime observer. P13c-4 still owns actual live source/count/owner authority.

## Verification

**794 Rust** (270 library + 522 desktop + 2 service; one ignored),
**1236 Vitest / 78 files**, typecheck, production build, all-target cargo check
and Clippy pass; only the five existing warnings remain. Scoped rustfmt and
`git diff --check` pass. The independent fixture regenerates byte-identically
(SHA-256 `2f94f0e0b0f5e0fdecfb3f958da6ec0158a1842ba1b361062f9ee64732989efb`).

Eleven new pure Rust tests cover all nine sets against the independent
32-by-32 bit-mask oracle, full original/canonical JSON round trip, every
closed object level (including budget entries), duplicate/unsorted dynamic
family maps, all ID domains, safe-integer/genesis boundaries, exact/one-over
8 MiB original input, 10,000 joint confirmation IDs, 30,000 alpha identities
and separate 10,000-origin capacities. One real workspace-SQL regression
retains rejected v5 bytes, verifies runtime v4 output and unchanged legacy
4 MiB refusal. Two new TS oracles verify original/canonical hashes and genesis.

Four executable mutations failed by assertions (not compile errors): omit
unknown origins from coverage, accept a substituted ID, normalize original
JSON through Value before strict decoding, and ignore trailing original-byte
padding. Both edited source files were restored, then the full suite passed.
Logs are local TEMP `aff-binding-mutant-*.log`, `aff-p13c3b1a-{rust,check,clippy}.log`.

Final head must pass all six CI jobs and merge before continuing 3b-1b.
No bootstrap, calibration, final reserved seed or protected execution was performed.
