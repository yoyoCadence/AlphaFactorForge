# Handoff: PR #172 annualization display repair

Date: 2026-10-14（Asia/Taipei）
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/metrics-annualization-notes`
Base: `63bc2df`（PR #175，six green CI jobs）
Status: Merged as PR #176; native acceptance remains separate.

## Scope

Fix acceptance R1/R2: every displayed full/in/out column uses its completed
result's actual bar count and the same existing bars-per-year basis as the engine.
Exactly one year is not short. The main table, floating panel and native metrics
window share sample metadata and note rendering. No core metric, saved record,
schema, interval fallback, pinned codec or fixture contract change.

Saved summaries only report an annualization duration when their own persisted
benchmark range or complete dataset bounds/count establish it. Otherwise the UI
says the annualization sample was not recorded; it does not infer counts from
timestamps or attach an older validation record's split to a newer summary.

## Verification

Passed: 1,183 Vitest tests (72 files), typecheck, production build and all
97 Playwright E2E tests. Coverage includes day/hour one-year boundaries,
600-bar full / 420-bar in / 180-bar out Holdout, JSON wire round-trip and
sample isolation, expanded views and saved full-year summaries. After the
final wording adjustment, all six affected annualization/metric-display
E2E cases passed again. Connected Chrome also rendered the short-Holdout
note in the main and expanded views without console errors.
The review's original three failures remain in its patch as historical evidence;
only the two metrics cases belong to this PR. Native acceptance is a separate
task; Windows automation currently reports its native pipe unavailable.

## Resolution

Implementation complete locally; publication and exact-head CI pending.

### Delivery resolution (2026-10-14)

Published and merged as [PR #176](https://github.com/yoyoCadence/AlphaFactorForge/pull/176).
Final head `e9d498ccf0aa677c70d8cf9866d6ef4666f78f85`; all six jobs of CI
run `38571582140` succeeded. Merge `a8e73de70278b92d6ae87e725877ef73978b2bbe`.
The original acceptance patch is retained; this resolves R1/R2.
