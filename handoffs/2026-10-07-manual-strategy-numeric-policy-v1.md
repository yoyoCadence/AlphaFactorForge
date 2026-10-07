# Handoff: versioned manual strategy numeric policy

Date: 2026-10-07
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/versioned-strategy-numeric-policy`, from main `060f3d6`
Status: Resolved — merged PR #166 on verified head, six CI checks successful.
Task: NUMERIC-JSON-002a; maintainer authorized repair, PR publication and merge.

## Change and reason

The default Rust JSON parser interprets `0.0036944444444444438` one ULP away
from JavaScript, causing verified manual strategy saves to reject a legal
frontend identity. New params/blocks/code definitions now carry hash-covered
`manual-strategy-definition-v1` / `json-f64-roundtrip-v1` markers. An isolated
raw-token parser resolves their numbers correctly; unmarked legacy definitions
retain the default parser, original text and original identities.

The new async typed `prepare_saved_strategy({ strategyId })` verifies a stored
row and returns its interpreted definition as a JSON string. The editor checks
source id/hash/policy and ignores stale preparation responses. Run artifacts
capture the source row, so saving uses the provenance that produced the result.
Changed or legacy definitions visibly use **另存新版本**, with the original row
as immutable parent. Repeated marked saves retain the original JSON, lifecycle
and parent. A missing, unverifiable, self or conflicting parent refuses the copy
before writes. The mock rejects legacy decimals it cannot prove equivalent to
Rust instead of guessing the old parser's interpretation.

The implementation adds only serde_json `raw_value`; no global float parser
feature, migration, core hashing change or historical fixture rewrite occurs.
The [compatibility contract](../docs/manual-strategy-numeric-policy-v1.md)
records explicit dispatch and rejection rules. PR146-165-DOC-R1 is also resolved
by appending PR #165's final merge/CI evidence to its existing handoff.

## Verification

- Full Vitest: **1131 passed / 70 files**; typecheck and frontend build pass.
- Full Rust: **600 passed** (170 library + 428 desktop + 2 service), **1 ignored**;
  `cargo check --locked --all-targets` passes.
- Shared additive fixture: params/blocks/code × six independent audit decimals,
  exact frontend bits/canonical bytes/hash → migrated SQLite save/read/prepare.
  Original six-case audit, discovery/validation/seed/calibration contracts remain
  unchanged. Parser tests cover nested values, strings, signed zero, unsupported
  markers, duplicate keys, nonfinite numbers and depth; repository tests cover
  refused writes, unresolved legacy rows and source lineage.
- Real isolated Tauri smoke: **18 marked cases passed**; legacy source keeps hash
  `strategy-v2:b317bbc5d4a1d39ce62186dffe173c716a65e58222c9e205bf3a3a5acee1b1a3`
  and bits `3f6e43cfc21ad9ff`; new rounded bits are `3f6e43cfc21ad9fe`.
  Unknown policy, conflicting parent and wrong invoke argument name refuse;
  rejected writes leave all rows unchanged. Rust event-plugin round trip passes.
  Owned app/WebView2 processes and three temporary directories were cleaned.
- Full browser E2E: **85/85 passed** on the clean rerun, including the long-decimal
  save/load/same-hash rename/explicit copy and delayed preparation regressions.
  Owned Vite PID `44136` was verified through port owner/CommandLine and stopped;
  the port was released. Final-head CI results will be verified before merge.
  The first E2E attempt was interrupted when concurrent Rust compilation caused
  Windows Vite `fs.watch` EBUSY on a target executable; the owned Vite exited.
  Restarting after Rust builds completed permits a clean full rerun. No runtime
  or CI configuration was changed to hide that local failure.

Ignored local evidence: `output/numeric-policy-*.log` and
`alpha-factor-forge/node_modules/.cache/numeric-policy-native-smoke/`.

## Remaining scope and next step

NUMERIC-JSON-002 remains open for discovery configuration/admission/resume,
idempotent numeric request transport, dataset import and research artifact
replay. This slice repairs the confirmed manual save/load boundary; it does not
claim a global numerical rollout. The single immutable parent schema refuses
multiple copy origins for an existing destination hash; load that existing row
instead. O1–O4, P12 native campaign acceptance and P13 runtime reveal remain
separate. After verified PR merge, complete the requested #126–#145 review.

## Resolution — 2026-10-07, final-head CI and merge

[PR #166](https://github.com/yoyoCadence/AlphaFactorForge/pull/166) was marked
ready and merged at 20:56 Asia/Taipei on the verified final head
`a7f0783481681057ea1b545d3231edaa49021f2d`. All six checks passed in
[CI run 37623643276](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37623643276).
Merge commit `0dafef5811282c5b711ae8e7b6725c8d8186046d` has the same tree
`335bec9e77b6796d34a71bee9c469e835654bc38` as that head; local main was
fetched and fast-forwarded. NUMERIC-JSON-002a and PR146-165-DOC-R1 are delivered.
The parent NUMERIC-JSON-002 and outstanding operator acceptance remain open.
The requested previous-20 review is recorded in
[PR126–145 acceptance](2026-10-07-pr126-145-acceptance-review-v1.md).
