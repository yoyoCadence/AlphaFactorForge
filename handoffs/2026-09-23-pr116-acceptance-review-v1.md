# Handoff: PR #116 / P12b trial-ledger specification acceptance review

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p12b-trial-ledger-spec`
Reviewed head: `84f2118ae69c928fde8163ebb3bdb5e58c62c4c7`
PR: [#116](https://github.com/yoyoCadence/AlphaFactorForge/pull/116)
Status: Needs four bounded specification corrections before acceptance. P12b implementation remains unstarted.

## Summary

The proposed registry location, broad instrument family, register-before-enqueue ordering, and fail-closed intent follow the maintainer's stated decisions. Four contradictions in the immutable event and restore contracts would prevent the specification from meeting its own acceptance cases. This review covers the documentation contract only; it does not assert a runtime implementation exists.

## Required Action / Decision

1. **R1 (blocking): Export/import omits rows required by its own foreign key.** §8.1 exports families and event payloads only (lines 227–230), while §10 requires every imported `trial_events.batch_id` to reference `trial_batches(batch_id)` (lines 269–289). A fresh registry cannot import even one event under the proposed schema without synthesizing batch rows, and a synthetic batch cannot reproduce the original `prior_trials`/`planned_trials` retry receipt. Define batch export/import and validation, or remove this foreign key and specify how historical retry receipts work. Add a fresh-registry export/import fixture with `foreign_keys=ON` and a same-batch retry after import.

2. **R2 (blocking): Count watermarks do not establish event inclusion.** §7 accepts a registry with the same `registryId` whenever its global/family counts are at least the stored watermarks (lines 205–216). Example: workspace last saw events A and B; an older copy of the registry with only A receives C; it now has the same ID and count two, so the workspace accepts it although B was lost. The replacement case likewise accepts an import from the old registry based on the number of imported events, without proving that the specific events the workspace saw are present. Persist and verify an append-only event-set commitment, sequence/hash-chain checkpoint, or equivalent inclusion evidence at binding and replacement. Add same-count divergent-copy and replacement-with-equal-count-but-missing-event acceptance cases. This is the core "trial counts cannot be reset" invariant, not just tamper resistance.

3. **R3 (blocking): Reproduction identity cannot be verified from the immutable event.** §4.2 requires split/embargo and seed to match (lines 117–119), but §4.3's `eventPayload` contains neither (lines 125–133). The event table stores only that payload as its immutable evidence, so an imported original cannot be checked against the proposed reproduction rule. Include canonical split/embargo and seed (or their frozen hashes) in every applicable payload and validate those fields on import; similarly make the benchmark identity needed for the §4.2 whitelist explicit. Add mismatched-seed and mismatched-split checks after export/import.

4. **R4 (blocking): A batch retry can skip payload conflict detection.** §6.1 says an existing `batch_id` returns the stored prior/planned values directly (lines 169–175), while `batch_id` is only a hash of sorted idempotency keys (§10, lines 269–275). Retry the same keys with changed strategy hash or kind: the batch ID is unchanged, so the direct return bypasses §4.3's `idempotency_conflict` rule. Require exact event payload/ID comparison before replaying a batch receipt, or bind the batch identity to the complete ordered payload set while still rejecting reuse of an idempotency key with changed content. Add a whole-batch same-keys/different-payload case in addition to A4's single-event case.

## Review Notes

- §15's six maintainer choices are policy decisions; they do not resolve R1–R4. They should remain visible for approval after the contract is corrected.
- `originRegistryId` is part of `eventId` (§4.3). This makes independently registered otherwise-identical events in two registries get different IDs; specify whether that is a deliberate conflict or whether identity must survive registry reconstruction. The present claim that the same event always has the same ID across registries requires that qualification.
- Existing P03b `requestId` is reserved before command execution; P05 `attempt_key` contains the later `run_id`. The proposed use of `requestId` for new registrations is consistent with that ordering.
- No source code, migration, or dependency changed in this PR. The PR's six CI jobs all succeeded on the reviewed head; they do not exercise the proposed ledger behavior.

## Verification

- Confirmed local head equals remote PR head and the worktree was clean at review start.
- Read the five-file `origin/main...HEAD` diff, relevant P05/P06 contracts, migrations `0007`/`0008`, and P03b/runner command paths.
- `git diff --check origin/main...HEAD` passed.
- GitHub Actions run `35859403151`: typecheck, test, build, cargo-check, native-smoke, e2e all succeeded on `84f2118`. No local code suites were rerun for this documentation-only PR.
