# Handoff: P13c-4b-2c-2b-2 original lineage semantic observation

Date: 2026-10-25
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b2c2b2-lineage-semantics
Status: Local implementation and verification complete; exact-head CI/merge delivery pending.

## Summary

From merged #225 (`bca7d5a`), implement the [frozen semantic contract](../docs/plans/research-original-lineage-semantics-v1.md) without changing original producer/parser/identity/persistence behavior or any committed fixture. One sealed graph entry prepares once and returns only immutable complete historical lineage observations. Borrowed ID indexes resolve actual attempt references independently of SQL row order. All candidates, original cost bits, stored DSL projections, exact input/engine catalogs and winning-base shared hypotheses must agree.

Whole post-preparation W32 MiB / C64 MiB / O4 MiB accounting precedes every new payload copy/encoding/hash/output. The private no-output measurements reserve the unchanged core identity and research canonical helpers; no alternate encoder/hash/parser or authority is introduced. Existing P+E preparation limits remain separate. Final result retains only config hash and ordered candidate IDs/hashes/base IDs.

## Verification

- 31 original_lineage tests pass (16 added semantic cases), including four frozen complete outputs/work counts and nonmonotonic library references; each sole semantic entry prepares/enumerates once.
- Real captured W/C exact resource preflight and one-over tripwires match the independent recipes; intentionally poisoned stored definitions remain semantic refusals after exact resource acceptance.
- Real O exact boundary succeeds through actual original candidate/repository/history writers: 1101 candidates, 4194304 string bytes; 4194305 refuses before payload work. Only synthetic job/event tokens are seeded; no candles, execution, registry expenditure or protected inputs.
- Six targeted assertion mutants caught: ignored saved DSL, current engine replacement, first equal-hash base, candidate hash substituted for base document, per-item W/C reset and output copy before O preflight. Each source restoration was byte-exact (audited pre-comment/check-conversion semantics SHA-256 `869589933f6f05df9b2248721289647f412adeffaa8c9c5903edbdc5cee27223`). Full isolated locked Rust passes **1077 tests, one ignored** (297/778/2; 31.59/465.62/3.45 s). Locked all-target check (59.32 s), Clippy (54.40 s; only the five existing backtest/score/file-command warnings), scoped format and diff checks pass. Full four-worker Vitest passes1242/80 files (6.30 s), typecheck and production build pass. Exact-head six CI jobs/reviews/expected-head merge/readback/clean-main tree comparison remain delivery gates.

The exclusively created full-check root `aff-p13c4b2c2b2-full-39cbdfc70cd04681a1fe62c1b78f2def` was removed after success (42 files/63 directories). Exact resolved parent/leaf and every traversed entry were checked before recursive deletion; no reparse points. Logs remain outside that root in normal TEMP under `aff-p13c4b2c2b2-*`.

## Test setup findings

Initial test setup incorrectly used a top-level candidatesCap key; the frozen format uses caps.candidates. Negative tests initially attempted immutable history UPDATEs and were correctly stopped by SQLite triggers. Corrected only the owned disposable forged-history setup: three named immutability guards are removed there explicitly, while actual-writer positive controls retain all guards. Raw scalar source/hash domains and shared candidate-bound membership can also refuse during graph capture, so semantic negatives now use capturable valid-domain contradictions (manual source, fixed-length wrong digests, single-candidate parent projections). Original focused failures remain in local logs; no production policy/trigger/schema changes. The first early-output mutant hit both the intended payload assertion and a destructor assertion during unwinding (Windows exit 0xc0000409). The guard now resets its flag but skips a second assertion while panicking; the rerun exits101 with the intended assertion. The uniquely created aborted-test root `aff-lineage-preparation-7220-1` was verified by creation interval, SQLite owner PID7220, exact 1101-attempt/3672+901 recipe, absent original process and no reparse points, then removed; no pre-existing PID/counter root was reused or deleted. Six final mutation logs are in the normal TEMP under `aff-p13c4b2c2b2-mutant-*.log`.

## Local scheduling observation

The first default-worker Vitest run overlapped all-target checking and timed out only the unchanged 50000-iteration R3 reference case (5192 ms against its existing 5000 ms limit); 1241 other tests passed. No timeout/test/fixture/runtime setting was changed. A complete four-worker run after Clippy passes all1242 in6.30 s, supporting local CPU scheduling contention as the cause; default-worker CI remains required. Both logs are retained.

## Remaining obligations

This result proves historical consistency only. It grants no RegisteredSource, live owner/registry/admission, numerical execution, numericCostsHash, warmup/file/market/trial authority or research PASS. Parents remain incomplete. After delivery promote/refine P13c-4b-2c-2b-3 envelope/mode producer controls before further source adapter code; v2/v3 params/mixed-mode families remain unsupported until that extension delivers.
