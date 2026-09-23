# Handoff: PR #116 / P12b trial-ledger specification acceptance review

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p12b-trial-ledger-spec`
Reviewed head: `84f2118ae69c928fde8163ebb3bdb5e58c62c4c7`
PR: [#116](https://github.com/yoyoCadence/AlphaFactorForge/pull/116)
Status: Re-review at `af1a7c7`: R1–R4 closed; one new admission-count finding (R5) needs correction. Six §15 policy decisions recorded below. P12b implementation remains unstarted.

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

## Resolution

### Resolution — R1–R4 addressed in the spec (2026-09-23, same branch, PR #116)

Documentation only; `docs/trial-ledger-v1.md` revised. No code, migration or command.

- **R1 (export omits batches):** §8.1 export now carries `batch` (identity + sorted `[idempotencyKey, eventId]` members), `event` (with its `batchId`), `receipt` and `originCheckpoint` records. §8.2 validates every batch/receipt reference before writing and inserts in foreign-key order (families → batches → events → receipts → checkpoints) under `foreign_keys = ON`, so nothing is synthesized. Receipts moved to `batch_receipts (batch_id, receipt_registry_id)` (§6.3) and are explicitly not a count source; a retry after import returns the origin receipt. Acceptance A21.
- **R2 (count watermark ≠ inclusion):** §7 replaces counts with a per-registry append-only hash chain (`chain_seq = sha256(chain_{seq-1} || eventId)`, genesis bound to `registryId`). The workspace binding is `{registryId, seq, chainHead}`; same ID requires the chain value at `binding.seq` to match (`registry_diverged` otherwise). A replacement registry is accepted only through a verified `origin_checkpoints` row for the old registry at the bound seq whose chain matches — checkpoints can only come from §8.2 imports that recompute them from genesis. Acceptance A22 (same-count divergent copy), A23 (equal-count replacement missing an event), A24 (valid replacement), A30 (chain edited).
- **R3 (reproduction identity not in payload):** §4.3 payload now always contains `splitHash`, `seedsHash`, `benchmarkId`, `benchmarkParamsHash` (null when not applicable, always present). §4.2 requires the five identity fields to be equal and non-null; §8.2 re-validates reproduction and benchmark rules on import. Legacy events with unrecoverable split/seed cannot be reproduction targets. Acceptance A25, A26.
- **R4 (batch retry bypasses conflict check):** `batchId` now hashes the family plus every `[idempotencyKey, eventId]`, so it binds full content. §6.1 always compares each key's existing `eventId` first (step 3) — mismatch → `idempotency_conflict`, overlap with another batch → `batch_conflict` — and only then may replay a receipt (step 4). Acceptance A27 (whole batch, same keys, changed payload) and A28 (partial overlap).
- **Review note (`originRegistryId`):** removed from the hashed payload; kept as a non-identity `origin_registry_id` column, so the same registration written independently in two registries gets one `eventId` and dedupes on union. Acceptance A29.

Self-verification: a small Node simulation confirmed that `[A,B]` vs `[A,C]` chains share length but differ at seq 2 (A22), that a replacement lacking the seq-2 checkpoint is refused (A23) and a full one accepted (A24), and that changing one event's payload changes the `batchId` (A27). Stale references (`§8.3` quarantine, count watermarks, A12 wording) were swept. `git diff --check` passes; relative links resolve.

§15's six policy questions are unchanged and still need the maintainer's decision. P12b implementation has not started.

Status: R1–R4 addressed in the specification; awaiting re-review.

## Re-review — `af1a7c7` (2026-09-23)

The remote PR head and clean local branch match at `af1a7c79eb40b4b5d9b5112d9a12924cea259069`. The documentation diff closes R1–R4 and the `originRegistryId` review note:

- R1: full export contains batches, events and receipts; import validates references and inserts in foreign-key order. A21 covers import into an empty registry and receipt replay.
- R2: workspace binding checks a prefix hash-chain checkpoint, including same-count divergence and verified replacement. A22–A24/A30 cover the cases.
- R3: immutable payload includes split, seed and benchmark identity hashes; registration and import revalidate the reproduction/benchmark rules. A25–A26 cover mismatches.
- R4: batch identity includes event IDs, and registration compares every existing idempotency key before replaying a receipt. A27–A28 cover changed payload and partial overlap.

**R5 (blocking): A replayed receipt can be stale when used for a new precision admission.** §6.3 returns the original `(priorTrials, plannedTrials)` on retry, including after an import (lines 235–243), while §11 tells P12d to build the P12a plan from `register_batch`'s result (line 431). Example: family initially has zero trials; batch A registers one and crashes before its workspace enqueue/admission; batch B adds ten trials; retry A returns its old receipt `(0,1)`. A P12a plan built from that return uses family size 1 although the registry now has 11 effective trials. The receipt is correct as a historical retry answer but not an authoritative *current* count for a newly executed admission. Specify a separate current family count read and when it must be taken (and how its freshness is fenced against later admission/confirmation), or restrict receipt replay to replaying an already persisted decision. Add a deterministic A31-style crash/retry/intervening-batch case. This does not reopen R1–R4.

### §15 policy decisions for v1

1. **Accept one canonical instrument per family.** Interval, date range, dataset, workspace and protocol stay outside the key; a family pins one protocol at its first effective event. State the guarantee as *per instrument*, not per economic asset.
2. **Defer cross-venue aliases to a versioned follow-up.** Economic equivalence across spot/futures, quote currencies and venues needs its own governed mapping and merge rules. Do not imply cross-venue trial accounting is solved in v1; a multi-instrument or cross-venue confirmation stays unsupported rather than using an implicit alias.
3. **Count every diagnostic in v1.** Without evidence that its result was hidden from selection, an exemption would be an avoidable undercount.
4. **Block qualification for unknown pre-P05 history.** Keep `legacy_trials_unknown`; do not accept a manually estimated count as verified history. A future recovery rule needs its own recorded evidence and contract.
5. **No manual quarantine release in v1.** A conflicting family stays ineligible. Define an append-only, reviewable resolution process in a later contract version before allowing release.
6. **Accept `%LOCALAPPDATA%\\com.alphafactorforge.evidence\\` for the registry.** It keeps SQLite on local storage and outside the Roaming workspace. A moved/restored workspace whose registry is missing must remain ineligible until a verified export/import restores the evidence.

These are the requested policy choices, not a claim that the PR is accepted or that implementation has begun. They are recorded in the local §15 edit and scope/limitations; R5 still needs correction.

Verification: `git diff --check 84f2118..af1a7c7` passed. GitHub Actions run `35863487620` completed successfully on this head in all six jobs (typecheck, test, build, cargo-check, native-smoke, e2e). No local code suites were rerun for this documentation-only correction.
