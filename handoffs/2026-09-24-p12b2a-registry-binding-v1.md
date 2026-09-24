# Handoff: P12b-2a registry binding primitives

Date: 2026-09-24
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12b2a-trial-ledger-binding` (from merged PR #118, `ecb9a4f`)
Status: PR #119 merged; P12b-2b runtime integration implemented locally for review.

## Summary

This slice implements the registry side of the workspace binding contract in spec §7.2–7.3. A bound workspace can open only an existing registry; binding checks compare the last observed chain prefix against one consistent registry snapshot. It makes no migration or runtime behavior change.

## Decisions and scope

- `TrialLedger::open_existing` refuses a missing directory/file and a blank SQLite file before any migration; `open` remains the first-use path for an unbound workspace. An existing registry still gets normal containment, volume, schema, and chain checks.
- `LedgerBinding` is the strict `{registryId, seq, chainHead}` value stored under `app_settings.trial_ledger_binding`. Malformed stored values fail rather than defaulting to an unbound workspace. `write_workspace_binding` does not own a transaction so P12b-2b can write it atomically with attempts.
- `check_binding` verifies the current registry chain in a read transaction. A shorter chain is `registry_rolled_back`; a changed prefix at the saved sequence is `registry_diverged`; a broken current chain is `registry_chain_broken`. A different registry ID needs the exact imported origin checkpoint and its event; otherwise it is `registry_replaced`.
- A valid check returns the current head to save. The P12b-2b caller must backfill existing attempts before first binding, and save an updated head in the same workspace transaction as new attempts. These APIs alone do not enable qualification or claim.

## Required Action / Decision

1. P12b-2b: add workspace migration `0009` for `trial_event_id` and its frozen-column trigger; implement legacy backfill and pre-P05 unknown-history reporting before first binding.
2. P12b-2b: isolate test registries, attach the ledger to both desktop and service workspace open paths, register before enqueue, save the returned binding with attempts, and check non-NULL/existing event at claim. Do not leave a new runtime path able to run an unregistered attempt.
3. Keep the P12b-1b fail-closed rule for non-effective benchmark imports until portable execution provenance is designed and persisted; do not infer it from the public params hash.

## Verification

- Six isolated Rust tests pass: missing/blank registry, valid existing reopen, same-prefix/rollback/divergence, imported replacement checkpoint, post-open chain damage, and strict/transactional workspace binding storage.
- `cargo test --locked`: 429 Rust tests (88 library, 339 desktop binary, 2 service smoke), all pass. `cargo check --locked --all-targets` and targeted rustfmt pass; clippy reports only the five existing warnings outside this slice.

## Resolution (2026-09-24)

PR #119 merged P12b-2a. P12b-2b now adds migration `0009`, backfills legacy attempts, binds desktop/service startup, registers discovery candidates before enqueue, and checks registry membership before claim. The new workspace/runtime handoff records the implementation and remaining P12d admission boundary: [P12b-2b handoff](2026-09-24-p12b2b-trial-ledger-runtime-v1.md).
