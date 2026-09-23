# Handoff: PR #117 / P12b-1a trial-ledger core acceptance review

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12b1-trial-ledger-core`
Reviewed head: `0cd5e0a902c91aae01c7cdf8e06da58674de762f`
PR: [#117](https://github.com/yoyoCadence/AlphaFactorForge/pull/117)
Status: R1–R3 resolved locally; awaiting updated PR CI and merge. P12b-1b/P12b-2 remain separate.

## Summary

The new registry module has the intended append-only schema, content-bound IDs, batch idempotency checks, hash chain, and a current admission count distinct from historical receipts. Three edges in the implemented opening/classification/replay contract still need correction. This review does not expect export/import or runtime wiring from P12b-1a.

## Required Action / Decision

1. **R1 (blocking): A claimed benchmark can avoid the effective-trial count without evidence of the actual run.** In `research/trial_ledger.rs` lines 606–620, a `Benchmark` event needs only `dataset_hash`, an allowed `benchmark_id`, and the published constant `benchmark_params_hash`. The input's `strategy_hash`, `seeds_hash`, `split_hash`, and execution evidence may be null or arbitrary; `kind.effective()` then stores zero. The in-tree A5 test's `random-entry-v1` input has `seeds_hash: None` (lines 1429–1440, 1698–1713) and is accepted as non-effective, although spec §4.2 requires the random-entry contract's explicit seed and pairing rules. A caller can likewise claim the expected `smaCross` params hash while passing a tuned strategy hash, and the registry cannot associate the claim with a frozen benchmark definition. The core should require trusted, auditable benchmark identity from the actual backend-generated benchmark run, or conservatively count an unverified run as effective. Add tests for missing/wrong random-entry seed or pairing and for a tuned strategy accompanied by the expected benchmark hash.

2. **R2 (blocking): Exact replay of a pre-pin, non-effective batch can fail after the family's first effective trial.** §3 freezes `protocol_json` only on the first effective event. Register a benchmark-only batch with `tests_per_trial = 1` (family protocol remains null), then an effective batch for the same instrument with `tests_per_trial = 2` (pins 2). Replaying the original benchmark batch with identical events and protocol now hits the unconditional pinned-protocol comparison in lines 1027–1037 and returns `family_protocol_mismatch`, instead of the same event IDs and receipt required by §4.3/§6.1 and A3. Keep exact retries idempotent across a later pin, while still rejecting genuinely new effective trials under a mismatched protocol. Add this exact three-step regression, including a crash-before-workspace-write variant.

3. **R3 (blocking against spec §2.1): Windows mapped network drives pass the volume check.** `is_unsupported_volume` (lines 714–722) checks UNC string prefixes and known OneDrive environment roots only. A mapped SMB drive can be represented as a drive-letter path such as `\\?\\Z:\\ledger` after Windows path canonicalization; it matches neither test and is opened in WAL mode at lines 937–940. The accepted spec requires refusing network volumes because SQLite file locking there is unreliable. Detect remote drive type (or another equivalent volume property), not just UNC syntax; test a mapped-drive/remote-volume case in addition to the current UNC case. This is an inference from the code and the [Rust canonicalize documentation](https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html), [Windows final-path documentation](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew), and [Windows drive-type documentation](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew).

## Review Notes

- The §16 decision to check payload conflicts before batch-membership conflicts matches A27/A28; no finding there.
- Letting a registry with a broken chain append more events while returning `registry_chain_broken` for every admission is fail-closed for qualification. The future import/recovery path must not clear that state without proving the chain.
- The missing export/import, workspace binding, runner wiring, and final P13 confirmation fence are already assigned to later slices and are not findings against this PR.

## Verification

- Local branch and remote PR head matched; worktree was clean at review start.
- Read the PR's 11-file diff, full new registry schema, the registration/opening paths, relevant benchmark contracts and test cases, and spec §2/§4/§16.
- `git diff --check origin/main...HEAD` passed.
- Independently ran `cargo test --locked --bin alpha-factor-forge a5_only_whitelisted_frozen_benchmarks_are_not_effective`: 1 passed. The passing test uses a seedless `random-entry-v1` benchmark, illustrating R1 rather than resolving it.
- GitHub Actions run `35870467685` completed successfully on this exact head in all six jobs: typecheck, test, build, cargo-check, native-smoke, e2e. Passing CI does not exercise the three counterexamples above.

## Resolution (2026-09-23)

R1–R3 were fixed on `feat/p12b1-trial-ledger-core` for PR #117:

- **R1:** A benchmark exemption now needs opaque, in-process evidence minted by the Rust deterministic or random-entry executor. The evidence is compared with every event input field. Random-entry commits to its explicit seed and candidate closed trades. A missing seed is rejected; a changed seed, pairing, or strategy identity invalidates the evidence. An otherwise whitelisted benchmark without evidence is recorded as an effective trial. Import in P12b-1b must reject or quarantine non-effective benchmark events without independently verifiable run provenance; it must not trust the public params hash alone.
- **R2:** A complete replay of a non-effective batch registered before the family protocol was pinned bypasses the later pin while returning its original receipt. New batches and effective replays still enforce the pin. A restart after registry commit and before workspace write is covered.
- **R3:** Registry opening probes the Windows volume mount point and drive type before enabling SQLite WAL, refusing remote or indeterminate volumes in addition to UNC paths and known sync roots. A mapped-drive-shaped path is covered with an injected remote-volume probe.

Local verification: all 25 trial-ledger tests pass; full `cargo test --locked` passes (88 library, 321 desktop binary, 2 service smoke). The original review result is superseded by this resolution; final merge status is recorded in the PR.
