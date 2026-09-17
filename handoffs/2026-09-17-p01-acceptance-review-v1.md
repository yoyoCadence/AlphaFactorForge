# Handoff: P01 acceptance review

Date: 2026-09-17
Repo: yoyoCadence/AlphaFactorForge
Branch: `docs/p00-contract-precheck`
Reviewed commit: `5e029d75b16572e91f9e4328adce418466dcf74a`
PR: None; local review only
Status: Resolved — R1/R2 regression checks pass; native Tauri restart/reopen remains untested

## Summary

Reviewed Plan §5 P01: reopen saved records/summaries/trades, state missing historical detail honestly, and leave calculations unchanged. The normal flows and existing verification suite pass, but two independently reproduced reader defects prevent acceptance. This note supplements the implementation-complete status in `tasks.md`; it does not authorize P02 or change the implementation scope.

## Required Action / Decision

### R1 — High: bind trade reads to the displayed result snapshot

Location: `alpha-factor-forge/src/components/ResultsExplorer.tsx:137–143`; cache reset at lines 108–110; `src-tauri/src/commands/db_commands.rs:get_trades`.

The explorer freezes summary rows at open/refresh time, but later requests trades using only `summaryId`. The persistence contract reuses that ID when the same strategy/dataset/segment is saved again and replaces its trades. A completed background run or another save between these reads therefore combines an old summary with new trades, without a stale-result warning. This also affects records' latest-summary panels. The heading about a mutable projection does not identify a mismatch within that projection.

Browser reproduction used the actual, unmodified `ResultsExplorer` and a temporary local harness that supplied controlled typed-client responses:

1. Open summaries with ID 1, net return 0.10, trade count 1; select ID 1.
2. Simulate replacement of the persisted result under ID 1 with net return 0.20 and two trades. Do not refresh the explorer.
3. Click the existing “載入 全期 交易明細（1 筆）” button and resolve `getTrades(1)` with the replacement rows.
4. Observed: summary/detail still show **10.00% / 交易數 1**, while the trade table shows **two generation-2 rows**, with no inconsistency warning.

Related code path: a pending old trade read is also allowed to populate the newly cleared cache after `load()` refreshes summaries, because `loadTrades` does not check a request generation.

Required fix: read/verify summary identity and trades consistently, and reject/disclose stale detail rather than attaching it to the displayed snapshot. Invalidate late trade responses across refreshes. A count-only check is insufficient because replacement trades may have the same count. This need not implement P05's full historical archive or change calculations. Add regressions for replacement before trade load, same-count replacement, and a late trade response after refresh.

### R2 — Medium: stop automatic retries after the first load fails

Location: `alpha-factor-forge/src/components/ResultsExplorer.tsx:118–121`, with `load()` at lines 98–115.

On failure, `data` remains null and `finally` changes `loading` back to false. The effect depends on `loading` and immediately invokes `load()` again, which clears the error. This repeats without user input or backoff while the explorer remains open. All four queries are retried on every cycle, and the failure message does not remain available for the user to inspect.

Browser reproduction used the same unchanged component with `getBacktestResults` rejecting after 250 ms and the other three reads resolving empty arrays. A single click on “展開” increased the visible query counter from **1 to 46** without clicking refresh; the button still showed disabled “載入中…”. Collapsing the explorer stopped the cycle.

Required fix: distinguish “never attempted” from “failed”, preserve the error after failure, and retry only on an explicit user action (or a deliberately bounded retry policy). Add a component/E2E regression asserting no further calls after the initial failure and that explicit refresh can recover.

## Review Notes

- Diff inspection confirms no calculation, schema/migration, or dependency change. Runner changes only initialize the newly exposed read DTO field.
- Validation ordering, Test-segment hiding, normal record/summary/trade viewing, and filter-preserved selection are covered by the passing suite. The real composer supplies the existing seeded validation records.
- Full immutable history remains P05 scope. DSL tree / benchmark deltas are already explicitly deferred in the P01 implementation handoff.
- Native Tauri restart/reopen was **not** exercised in this acceptance run. Rust integration tests and browser/mock acceptance are evidence for their respective layers, not a claim of native end-to-end verification.

## Verification

- `npm.cmd test`: **863 passed**, 49 files.
- `npm.cmd run build`: **passed**, including `tsc --noEmit` and production Vite build.
- `cargo test --locked`: **156 passed** (52 library + 104 binary); doc tests passed (0 tests).
- `cargo check --locked`: **passed**.
- `E2E_PORT=5199 npm.cmd run e2e`: **64 passed**, including both committed Results Explorer flows.
- Browser skill: normal seeded history inspected in Chrome; both defects above reproduced with controlled read responses and the unchanged production component.
- Temporary harness files and the review's Vite server are removed/stopped after review. No product source was modified.

## Resolution (added when acted on)

Pending fixes and re-verification of R1 and R2. Existing green tests alone do not close these findings.

### 2026-09-17 — fixed (Claude Code), same branch, follow-up commit to `5e029d7`

- **R1** — `get_trades` is replaced by `get_backtest_result_detail(summary_id)`, which reads the summary row and its trades in ONE transaction (`repositories::get_backtest_result_detail`, `Option`; None when the row is gone). The explorer's `loadTrades` now takes the DISPLAYED summary, compares it with the returned one column for column (`services/resultsExplorer.ts::sameSummaryRow`, all 27 persisted columns, absent ≡ null, `Object.is`), and stores one of `ok | stale | gone`: on `stale` it shows both rows' net return and trade count, says the trades belong to the newer save, and does not attach them; the displayed snapshot is untouched until the user refreshes. Every list read bumps a read generation; a detail response (or error) captured under an older generation is discarded, so a late response cannot populate the new snapshot's cache. A count-only check was never used. Regressions: Rust (`list_trades_reads_stored_rows_in_entry_order_and_reflects_replacement` now covers a same-count replacement whose id and `created_at` are unchanged), vitest (`sameSummaryRow` over every column + a composer-built replacement), Playwright (`R1: a same-count re-save between the list read and the trade read is disclosed, not attached`, driven by `?mock=1&replaceBeforeDetail=1`; `R1: a trade response that lands after a refresh is dropped`, driven by `detailDelay=1500`).
- **R2** — the read is driven by `status: idle | loading | ready | failed`; the mount effect fires only on `idle`, a failure sets `failed` and keeps the message (and any earlier data, labelled as such), and only the refresh button calls `load()` again. Regression: Playwright `R2: a failed first read stays failed until the user refreshes` (`?mock=1&explorerFailOnce=1`: the mock rejects the first `getBacktestResults` and serves every later call, so an automatic retry would have replaced the error with the empty state within the 800 ms wait; explicit refresh recovers).
- Mutation check: each of the three Playwright regressions was run against a deliberately un-fixed component (comparison forced true; generation check removed; the old `data == null && !loading` guard restored) and failed as intended, then the fix was restored.
- Re-verification: `npm.cmd run typecheck` clean; `npm.cmd test` 865 passed; `npm.cmd run build` passed; `cargo test --locked` 156 passed; `E2E_PORT=5199 playwright test --workers=1` 67 passed. Native Tauri restart/reopen still not exercised.
- Known limit, stated rather than hidden: a re-save whose row is column-for-column identical to the displayed one is indistinguishable from it (the screen is then also not wrong). A true generation marker belongs to P05's immutable attempt artifacts.

### 2026-09-17 — independent re-review of `6a3bdfd` (Codex)

**Acceptance: changes still required. R2 is closed; R1 remains open for detail requests started DURING a list refresh.** The transaction-based pair read and column comparison resolve the original replacement-before-detail case, and the existing generation check handles detail requests started before refresh. The reverse ordering remains unsafe.

#### R1 follow-up — High: prevent old-screen detail requests from joining a refresh's new generation

Locations in `alpha-factor-forge/src/components/ResultsExplorer.tsx`:

- Line 127 increments `readGenRef` when a list read **starts**; line 137 later installs the new displayed snapshot without changing that generation.
- Lines 172–185 capture the current generation but compare the result to the `displayed` argument captured at the time of the click, rather than the snapshot currently on screen when the response arrives.
- Line 671 disables the detail button only for another detail read, so it remains usable while the list is refreshing. The handler has no corresponding list-loading guard.

Consequently, a detail request started on the old screen while refresh is pending already carries the new generation. If its response arrives after the new list is installed, both checks pass against the old captured row and its old trades are cached by ID under the new visible row.

**Independent browser reproduction:** an isolated temporary page imported the production `ResultsExplorer` unchanged and supplied controlled typed-client responses. Each detail response was an internally consistent `(summary, trades)` pair; no calculation or production source was changed.

1. Initial `getBacktestResults()` returns summary A: ID 1, return 0.10, trade count 1. Open the summaries view and select it.
2. Click “重新整理”; hold the second list read pending. The old screen remains visible and its “載入 全期 交易明細（1 筆）” button is still enabled.
3. Click that detail button. Capture pair A (summary A plus one trade labelled `generation-1`) and hold its response pending.
4. Simulate persistence of summary B under ID 1 (return 0.20, trade count 2). Resolve the pending list read with B.
5. Confirm the screen now shows **20.00% / 交易數 2**. Then resolve the pending detail response with pair A.
6. Observed: the same screen still shows **20.00% / 交易數 2**, but its trade table contains **one `generation-1` trade**. No stale-result warning appears.

This is the mirror image of the existing E2E `R1: a trade response that lands after a refresh is dropped`: that test starts the detail read *before* clicking refresh, so the start-time generation increment invalidates it and cannot expose this gap.

**Required fix:** make detail admission and response acceptance belong to a displayed snapshot. A bounded solution is to disable detail actions and guard the handler during list loading; alternatively invalidate/check the actual displayed-snapshot generation when a successful refresh installs it. Preserve the existing before-refresh rejection and explicit refresh-failure behavior. Add a deterministic regression for **refresh starts → detail attempt on old screen → new list completes → old detail arrives**; assert either that no detail request is admitted during loading or that its response cannot attach to the refreshed row. Do not rely only on trade counts. No P05 archive work is required for this correction.

#### Re-verification

- `npm.cmd test`: **865 passed**, 49 files.
- `npm.cmd run build`: **passed**, including TypeScript checking.
- `cargo test --locked`: **156 passed** (52 + 104).
- `cargo check --locked`: **passed**.
- `E2E_PORT=5199 npm.cmd run e2e`: **67 passed**, including all three added R1/R2 regressions.
- Independent Browser reproduction above: **remaining R1 defect confirmed**.
- Native Tauri restart/reopen remains untested; this run does not claim native end-to-end acceptance.
- Temporary probe files removed and the review-owned server stopped after verification. Product files remain unchanged.

### 2026-09-17 — remaining R1 fixed and verified (Codex)

User explicitly authorized the fix after the `6a3bdfd` re-review. Changes are on the same branch, local and uncommitted on top of `6a3bdfd`; the earlier review additions are preserved.

- `ResultsExplorer` now blocks new detail reads while a list read is pending. `listLoading` reaches the shared `TradesBlock` through both records and summaries, disabling every unloaded-detail button. A synchronous `listLoadingRef` is set before any await/state render; `loadTrades` checks it before admission, so correctness does not depend solely on the disabled button. The same guard prevents overlapping list reads and is released in `finally` after either success or failure.
- Existing generation checks continue to reject detail responses started before refresh, and failed list reads still retain their error until explicit refresh (R2). No calculation, database command, persistence contract, or schema changed.
- Added `explorerRefreshDelay` to the existing DEV mock controls (bounded to 10 seconds, initial read immediate). Two E2E regressions exercise records and summaries separately: initial detail is available → refresh starts first → every old-screen detail button is disabled → refresh completes → the selected result's detail can load normally.
- Red/green evidence: both new tests were run before the component change and failed specifically because the detail button remained enabled. After the fix, both pass. The previous three R1/R2 regressions also pass.
- Final verification: `npm.cmd test` **865 passed**; `npm.cmd run build` **passed**, including TypeScript checking; `E2E_PORT=5199 npm.cmd run e2e` **69 passed**. Backend is unchanged from the independently verified `6a3bdfd` (156 Rust tests and cargo check passed in the preceding review); Rust was not rerun for this frontend-only fix. `git diff --check` passes.
- **R1 and R2 are closed for the reviewed browser/mock and repository behavior.** Native Tauri restart/reopen remains a separate unperformed verification, and P05's full immutable attempt history remains out of scope. This fix does not authorize P02.

### 2026-09-17 — local commit authorized

The user requested a commit. The fix, two regressions, task/changelog updates, and review notes are included with this entry in `fix(ui): block explorer detail reads during list refresh`. No push or PR is part of this action.
