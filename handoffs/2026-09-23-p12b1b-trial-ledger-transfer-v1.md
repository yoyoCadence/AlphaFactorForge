# Handoff: P12b-1b trial-ledger export and import

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12b1b-trial-ledger-export-import` (from merged PR #117, `17d3d6c`)
Status: Local implementation and verification complete; review pending. P12b-2 and P12d remain open.

## Summary

The host-agnostic registry now exports a complete `trial-ledger-export-v1` JSON Lines snapshot and imports an event union in one SQLite transaction. It preserves batches and all historical receipts, appends new events to the destination's own chain, and records verified source checkpoints. No runtime command calls these methods yet.

## Decisions and scope

- Export includes all families, batches, events, receipts, and known origin checkpoints in foreign-key order. The header's `bodySha256` covers exact body bytes. The snapshot comes from one read transaction and refuses a broken local event chain.
- Import checks the entire file before writing: canonical JSON, version, counts, family/event/batch IDs, exact batch members, receipt membership and effective counts, all source chains, reproduction identity, and benchmark whitelist. It reconstructs each batch through the registry's own `prepare_batch`, so missing or altered event fields cannot pass by recalculating hashes.
- A valid source file unions under `BEGIN IMMEDIATE`. Matching events are skipped and do not increase counts. Key/batch/protocol/receipt conflicts quarantine only affected families; other families continue. The source registry's own event chain becomes origin checkpoints. Divergent origin checkpoints are recorded in `registry_conflicts` and are not accepted. An incomplete origin prefix due to quarantine is not written.
- **Fail-closed benchmark boundary:** PR #117 uses an opaque in-process proof for non-effective benchmark registration, but the proof is not persisted or exported. Import therefore refuses a file containing `effective = false` benchmark events with `import_integrity_failed`. An unverified, counted benchmark can import. P12b-2 must define persisted portable execution provenance before cross-registry benchmark exemptions can be supported. This is the conservative branch of the reviewed §4.2 rule.
- The registry schema and workspace schema are unchanged. There is no frontend, service, or desktop command change.

## Required Action / Decision

1. Review the fail-closed benchmark boundary and decide whether P12b-2 should add portable evidence to the event/export contract. Never accept a non-effective imported benchmark on public params hash alone.
2. After this slice is accepted, P12b-2 wires workspace binding, register-before-enqueue, claim checks, and legacy backfill; P12d later enforces admission.

## Verification

- `cargo test --locked` passed after final validation: 423 tests (88 library, 333 desktop binary, 2 service smoke).
- Targeted transfer tests cover A13–A16, A21, A25, A26, A29, exact reproduction, quarantine permanence, source checkpoint divergence, mixed-family import, and rejection of non-effective benchmark imports.
- `cargo check --locked --all-targets` and targeted rustfmt check passed; `cargo clippy --locked --all-targets` reported only the five pre-existing warnings outside this slice.
