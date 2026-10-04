# Handoff: FU-9 saved campaign row status

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/campaign-row-status` (from merged PR #151, `275c43a`)
PR: [#152](https://github.com/yoyoCadence/AlphaFactorForge/pull/152)
Status: Implementation and full local verification complete; see PR #152 for final CI/merge evidence.

## Summary

The prior agent stopped after opening PR #151 for FU-7. Codex checked its
exact head `8416dce`: all six CI jobs passed in run `37203565918`, and merged
it as `275c43a` under the continuation request and screenshot's merge direction.
FU-1 through FU-8 had already landed as PRs #143–#150. FU-9 is the remaining
implementation slice in work order v2.

## Design and authorization

The maintainer-confirmed D6 and work-order §5.9 specify per-row statuses,
structured version errors, unverified raw declarations, disabled invalid
starts, retained IDs and readable historical decisions. The continuation
request and subsequent “繼續吧” approve that exact design; no new selection, migration, contract version or
database async redesign was introduced.

Valid rows expose a validated `document`. Incompatible/corrupt rows expose
`document: null`; every row carries the exact `rawDocumentJson` as display-only
data. SQLite query errors remain command errors. An unsupported top-level
version is recognized before parsing the current schema, so future document
shapes are incompatible rather than being mistaken for current-schema damage.
Current-schema structural/field/identity failures are corrupt.

## Changes and limits

- Pure Rust `CampaignError` distinguishes version and declaration failures;
  valid documents and their IDs are unchanged.
- Saved list isolates each row and retains history. Stored-campaign loading
  and starts still fail closed, including inconsistent stored version metadata.
- Historical decision reads no longer require the declaration to match the
  current build. This read cannot enqueue runs or authorize new admission.
- Typed client uses a discriminated union; mock and UI follow the same status
  boundary. Raw JSON is never parsed into start configuration by the UI.
- No migration, dependency or protocol version change. Native/connected-service
  manual smoke remains O3; browser mocks do not prove that integration.
- O1–O4 and all unrelated product/backlog items remain open. The final seed
  `20261117` was not used and final statistics acceptance was not run.

## Verification

- `npm test`: 1075 passed / 64 files.
- `npm run typecheck`, `npm run build`: pass.
- `cargo test --locked`: 578 passed (169 library + 407 desktop + 2 service),
  1 existing ignored test; campaign-focused run: 32 passed.
- `cargo check --locked --all-targets`: pass.
- `cargo clippy --locked --all-targets`: pass, five pre-existing warnings.
- `E2E_PORT=5199 npm run e2e -- --workers=1`: 83/83 passed, including mixed
  row status/reason, disabled starts, unverified raw JSON, historical decisions,
  continued valid starts and refresh. Browser runtime tools were not exposed;
  validation used the repository's Chromium Playwright harness on loopback Vite.
- `rustfmt --check` for campaign core/tests and DB module, `git diff --check`:
  pass. Pre-existing runner-test formatting preserved outside changed code.
- Sandbox process/cache permissions required escalated Rust/Vitest/Playwright
  and hidden Vite launches; no application test failed in the final full runs.

New Rust tests cover structured version errors, mixed rows, invalid direct
starts with zero run/admission/trial writes, continued valid starts, exact
historical reports and database query failures.

## Resolution

Implementation commit `0044269`; PR #152 created with base `main`, head
`feat/campaign-row-status`. After fetching/rebasing (already up to date),
typecheck and all 32 campaign Rust tests pass again. This documentation update
records the concrete PR; final-head CI and merge are checked on GitHub before
the agent reports the continuation complete. O1–O4 remain explicitly open.
