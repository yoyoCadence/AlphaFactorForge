# Handoff: P08 ETF daily market semantics

Date: 2026-09-20
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p08-etf-market-semantics
PR: https://github.com/yoyoCadence/AlphaFactorForge/pull/110
Status: Published as draft PR #110; not merged

## Scope and preflight

User authorized execution of `docs/plans/active-plan.md`, which requires one phase
per session. P00–P07 were already complete; P08 is the first eligible phase.
The repository uses `tasks.md`; its former `task.md` must not be recreated.
Started from clean P07 branch, fetched origin, confirmed it was contained in
`origin/main`, fast-forwarded local main to `bfb4216`, and created a P08 branch.
Existing pure TS/Rust boundaries, calendar types, metrics, and tests match the plan.

Small scope clarification: P06 prose assigned intraday ETF sessions/timezone
boundaries to P08, whereas active-plan §4.1 specifies first-release daily ETFs.
Preserved daily session labels, added explicit evidence bounds, corrected the
stale prose and introduced no timezone dependency. P09/P10 own real calendars;
P18/P20 own actual execution/observation instants. No objective or acceptance
criterion was expanded or relaxed.

## Implementation and files

- `src/core/market-data/etf.ts` ↔ `src-tauri/src/discovery_core/etf.rs`:
  `etf-semantics-v1`, bounded calendars, currency and cost profiles, ex-date
  receivables/payment, split position/order transformations, position valuation,
  point-in-time signal prices and semantic eligibility reasons.
- `src/core/metrics/etf.ts` ↔ `discovery_core/etf_metrics.rs`:
  `etf-metrics-v1`, actual elapsed-time CAGR/Calmar with explicitly configured
  daily annualization; reuse the existing metrics for the remaining fields.
- `fixtures/rs-core/etf-semantics-v1.json`, TS/Rust parity tests: 55 independently
  authored normal/rejection cases, plus non-finite and legacy-contract checks.
- `docs/etf-semantics-v1.md`: API usage, responsibilities and limitations.
  README, task/roadmap, market contracts, capability registry, parity doc,
  architecture doc, active-plan execution note and changelog synchronized.

No database migration or dependency changes. No existing hash, golden fixture,
metrics version, dataset, discovery computation, Tauri command or UI behavior
changed. New contracts are opt-in dependencies for later phases; they do not
grant an existing ETF snapshot or strategy automatic qualification.

## Acceptance evidence

| P08 criterion | Implementation / shared examples |
| --- | --- |
| Calendar, holiday, halt | P06-derived session grid, explicit evidence bounds; `holiday_weekend_early_close`, `suspension_is_not_a_gap`, `calendar_not_extrapolated`, timezone and Taiwan cases |
| Ex-date / payment date | `accrueDividend` / `payDividend`; known/unknown/early/repeated payment, sale-before-payment; entitlement remains fixed |
| Split / no artificial gain | `applySplit` / `valueEtfPosition`; forward/reverse splits, pending limit/stop orders, receivable unaffected, total equity conservation |
| Causal prices | `splitAdjustedSignals`; future-announced and unobserved events excluded, effective-date bar untouched, original input immutable |
| Annualization | `computeEtfMetrics`; one/two elapsed years with identical bar count, population volatility, total loss, invalid/stale series; legacy daily 365/CAGR unchanged |
| Native currency / costs | USD/TWD/USDT mismatch rejection, explicit buy/sell commission/slippage/tax; missing payment/completeness/user cost confirmation returns degraded |
| TS/Rust parity | Both read the same 55 expected scenarios, exact discrete fields/session dates, existing finite tolerance for calculations |

## Verification

All commands run in `alpha-factor-forge/` (Rust commands in `src-tauri/`):

- `npm.cmd run typecheck`: pass; also performed by build.
- `npm.cmd test`: **969 passed**, 53 files (58 P08 tests).
- `npm.cmd run build`: pass.
- `cargo check --locked --all-targets`: pass.
- `cargo test --locked`: **337 passed** = 71 pure-core + 264 application + 2
  native service smoke tests. Includes unchanged golden/parity, DB/migration,
  ownership and runner regression coverage.
- `cargo clippy --locked --all-targets`: pass, only the 5 pre-existing warnings
  (`backtest.rs` ×2, `score.rs` ×2, `file_commands.rs` ×1).
- `npm.cmd run e2e`: **78/78 passed**. Used a hidden, task-owned Vite process on
  localhost:5188, verified its listening PID/command line, stopped after testing.
- `git diff --check`: pass. Reviewed scope, no debug artifacts or temporary
  workarounds in tracked changes.

Native desktop UI smoke was not rerun: P08 introduces no desktop command/event
or UI path. Browser mock tests do not claim native bridge coverage; actual
service-process tests, Rust compile/link and existing integration tests passed.
No real ETF download was performed: provider credentials, real holiday/company
event evidence and adapter integration are P09/P10. All new inputs are synthetic
fixtures; no production research DB/accounts were touched. Existing Rust tests
use their own temporary workspaces. Vite logs are outside the repository under
`%TEMP%/aff-p08-validation`.

## Remaining boundaries and next phase

P08 provides pure daily primitives, not a paper ledger or a new selectable UI
backtest model. P18 must compose them with causal fills, cash/lot/tick constraints,
source-defined same-date action ordering and durable atomic event application.
Unknown payment dates are unpaid and degraded until source evidence is revised;
reverse splits retain fractional entitlement without inventing cash-in-lieu.
Callers must supply completed raw bars and evaluate each signal at its own cut.
Semantic eligibility is necessary but does not replace snapshot/statistical/Test
qualification. Total-return signal adjustment and intraday ETF sessions are not
implemented. The original plan's overall P00–P22 acceptance is not claimed done.

Stop here per plan. P09 requires new authorization and a Tiingo account; do not
silently skip to P10 or start another task.

## Publication

The only configured remote is GitHub. The user was offered the choice of the
existing GitHub draft PR or a supplied GitLab URL; no GitLab remote is available.
Use a Chinese draft PR on the existing remote, never merge. Final commit/PR
verification is appended after publication.

## Resolution — publication (2026-09-20)

- Implementation commit: `6b7147e5bba66554a528613f14d285aab69b6e7c`.
- Re-fetched origin; main remained `bfb4216`. Rebase was a no-op. Post-rebase
  typecheck, 72 relevant TS parity tests, all 71 Rust core tests and all-targets
  check passed. The full suite results above remain applicable.
- Pushed `feat/p08-etf-market-semantics`; created and verified Chinese **draft
  PR #110**, base `main`, head `feat/p08-etf-market-semantics`. Not merged.
- The environment's GitHub token was invalid even without proxies. No connector
  was installed. Chrome verified no existing same-head PR; push succeeded using
  the existing OS-keyring CLI login with environment token overrides omitted
  for that process only. The now-working CLI created the PR; Chrome retains
  its page as the deliverable. No global auth/settings changes or secret output.
- Remote CI is separate from local verification and may still be pending;
  consult the PR checks for the latest result. This documentation follow-up
  records the published result and introduces no runtime changes.
