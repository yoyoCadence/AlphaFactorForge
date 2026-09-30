# Handoff: P12d-2d-1 campaign commands and typed client

Date: 2026-09-30
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12d2d1-campaign-commands` (from merged PR #129, `3a92e9f`)
PR: [#130](https://github.com/yoyoCadence/AlphaFactorForge/pull/130) (draft)
Status: PR #130 merged and accepted on 2026-09-30; P12d-2d-2 UI continued on `feat/p12d2d2-campaign-ui`.

## Summary

P12d-2d was split in two after the maintainer decided the UI must author
campaigns. This slice adds the backend command surface and the typed client
the UI will use; it changes no UI. Contract:
[`docs/research-campaign-declaration-v1.md`](../docs/research-campaign-declaration-v1.md)
("P12d-2d-1").

## Maintainer decisions (2026-09-30)

1. The UI authors campaigns (not only starts declared ones).
2. Freezing saves: `freeze` stores the declaration; runs start from the saved
   list by `campaignId`.
3. Multi-instrument campaigns in the first UI, one run per instrument,
   started one at a time.

## What changed

- `db::campaign`: `freeze_and_store_campaign` (owner write transaction,
  idempotent), `get_campaign` and `list_campaigns` (each row re-frozen; a row
  that no longer reproduces its ID is an error), a shared `store_campaign`
  used by both freezing and the P12d-2c enqueue, and a serializable
  `StoredCampaignAdmission`.
- Runner: `start_stored_campaign_for_request` loads and re-freezes a stored
  campaign; a campaign start records its request outcomes as `campaign.start`.
- `research-command-v1`: `campaign.freeze` and `campaign.start` (mutating,
  exact payloads); `ServiceProxy::campaign_freeze` / `campaign_start` for the
  connected mode.
- New `commands/campaign_commands.rs`, registered in `main.rs`:
  `list_market_instruments`, `list_market_snapshots`,
  `preview_research_campaign`, `freeze_research_campaign`,
  `list_research_campaigns`, `start_campaign_discovery`,
  `get_campaign_admission`.
- Frontend: `campaigns` client and types in `tauri-client/commands.ts`;
  whitelist entries in `services/researchCommand.ts`; new
  `tauri-client/campaignCommands.test.ts` pinning every wrapper's command
  name and argument keys against the Rust signatures and handler list.
- Docs: campaign contract section; runtime contract §2 note.

## Required Action / Decision for P12d-2d-2

1. Add `campaigns` to the `dataClient` seam and a `?mock=1` implementation
   (mock snapshots/instruments, preview, freeze, list, start, admission).
2. Authoring UI: pick instruments from `listSnapshots` (prefill snapshot,
   dataset hash, interval and bounds; listing bounds from `listInstruments`),
   sample policy with rationale, sampling; preview shows the backend ID and
   per-instrument resolution; freeze saves; start one instrument's run with a
   `discovery-config-v3` whose walk-forward block equals the sample policy.
3. Decision view for a campaign run (`admission`): status, ordered reasons,
   precision, snapshot, frozen costs — never shown as a PASS.
4. Playwright coverage through the mock; real-host smoke by the operator.
5. There is still no UI to fetch data or build snapshots; the form only
   offers snapshots created by the service CLI adapters (P07/P09/P10).

## Verification

- `cargo test --locked`: **490 passed (101 + 387 + 2)** = 488 + 2 envelope
  tests (freeze saves once and is idempotent by request and content; strict
  payloads; runs start from the saved list, replay by request ID, outcome
  command `campaign.start`, listing shows the run's decision, unknown campaign
  `NotFound`; a row that no longer re-freezes is an error).
- `npm test`: **984 passed (55 files)**, including 9 new client pin tests;
  the existing whitelist pin covers the new commands. A mutation that renamed
  `campaignId` to `campaign_id` in the client was caught.
- `npm run typecheck`, `npm run build`: pass.
- `cargo check --locked --all-targets`: pass; clippy shows only the five
  existing warnings; rustfmt applied to the new Rust files; `git diff --check`
  pass.
- No UI change, so Playwright was not rerun locally (CI runs e2e).

## Resolution — PR #130 acceptance (2026-09-30)

Reviewed the merged `8269c7e` against its base `3a92e9f`: owner-checked,
idempotent declaration storage; re-freezing on list/get; strict command
payloads; stored campaign start and request outcome; embedded/connected Tauri
commands and the typed client argument names. No blocking defect was found in
this command slice. All six jobs on the PR head passed in
[CI run 36714893574](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/36714893574).

The next slice is implemented on `feat/p12d2d2-campaign-ui` from the merged
main. Its separate [handoff](2026-09-30-p12d2d2-campaign-ui-v1.md) records
the UI behavior, validation and remaining real-host smoke.
