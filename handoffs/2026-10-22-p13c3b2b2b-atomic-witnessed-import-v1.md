# Handoff: atomic complete witnessed v5 union

Date: 2026-10-22
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b2b2b-atomic-witnessed-import
Baseline: merged #205 (`1e6e4e7`).
Status: implementation/local verification complete; exact-head PR/CI delivery follows.

## Summary

The complete v5 importer independently verifies original source bytes before
the receiver mutex. One IMMEDIATE transaction holds the actual destination
snapshot while identical trial/alpha/confirmation apply and final checks run
first privately, then once on the receiver after owned cleanup. The outer-v5
SHA is audited; legacy public omission/provenance guards stay intact.

The qualification-named path checks coverage only: source/current/proposed
unknown trial families or raw witness gaps, alpha unknowns and confirmation
unknowns refuse before receiver DML. Fully accounted conflicts remain blocked
evidence and can travel. No workspace binding/file write, admission, command,
provider, execution or recovery is activated.

## Review notes

All original independent claims, three-kind blobs, first document bytes/audits
and permanent stops survive replay/multiple hops/replacement. Required new
conflicts/gap markers are monotonic. A late actual-only SQL failure must roll
back every table/checkpoint/import receipt, even though SQLite's connection
`total_changes()` can increase for rolled-back writes. Preflight failure must
leave that counter unchanged. Local non-effective benchmarks keep their actual
local-history meaning; no portable proof is fabricated or newly required.

The next stage is bounded P13c-4 design/implementation for actual authenticated
owner/count/share/file observations, workspace epoch guard and registry-first
enqueue. Statistical policy, two-test discipline and reserved seeds stay frozen.

## Verification

Targeted integration and boundary tests, deliberate mutants, full Rust/frontend
checks, exact-head CI and guarded merge are being recorded before delivery.
See the [contract](../docs/research-confirmation-import-v5.md) and
[adopted implementation plan](../docs/plans/research-confirmation-transport-implementation-v1.md).

## Mutation evidence

All four deliberate versions failed assertions (exit 101), not compilation.
After each run the exact original source SHA was restored; final restoration
also matched. No mutant or verification script is part of the product tree.

- Running the proposal on the real transaction instead of the private copy
  preserved the rolled-back persistent hash but changed `total_changes` from
  14 to 41. The proposed-only gap test caught this first-write violation.
- Omitting source provenance before the mutex caused the held-mutex test's
  bounded channel wait to expire. Its guard is always released before joining.
- Replacing the outer archive audit SHA with zero failed the exact file digest
  assertion.
- Omitting expected summary/graph equality accepted a valid actual-only trigger
  addition. The regression expected refusal and full persistent rollback.

The importer additionally compares final trial head/restrictive evidence and
alpha identity/provenance sets with the privately checked proposal. Seven new
relative documentation links resolve.

## Resolution (local verification)

Nineteen new real-SQLite tests cover managed/manual/empty/unknown and conflicting
history, exact original blob bytes, noncanonical first document/audit retention,
full outer-file audit SHA, self/replay/two-hop/replacement/reopen binding coverage,
foreign-origin trial coverage with both older/newer original witnesses, concurrent
independent writers and all legacy omission guards. Source/current/proposed
provenance refusal and missing-source-proof tests check no receiver changes;
held-mutex tests prove source validation/provenance precedes that mutex.

All four exact joint row/original-byte/interval/unknown limits accept replay and
reject overflow before receiver DML (`total_changes` unchanged). Fixture setup
retains every original SQL guard; cached statements and a bounded test-only page
cache avoid unnecessary preparation/index-cache churn. Actual non-effective
local benchmarks remain valid receiving history. Corrupt actual chain/scalars/
intervals refuse. Actual-only late SQL failure and a valid unexpected trigger
addition both preserve every original table/audit/checkpoint/binding on refusal.

- Locked full `cargo test`: **936 passed**, one existing ignored; 297 library,
  637 desktop and 2 service (31.82 / 470.53 / 5.74 s). All 19 new cases pass;
  the existing Windows host-restart case also passes on this full run.
- Locked all-target `cargo check` / `cargo clippy`: pass (28.57 / 37.96 s).
  Only five existing warnings (four core, one old file-command test).
- Scoped formatting of seven applicable files: pass; legacy unformatted files
  were not globally reformatted.
- `npm test`: **1242 passed / 80 files**; typecheck and production build pass.
- Four mutants failed assertions and were exactly restored before full checks.
  Seven new relative links and `git diff --check` pass.
- Full-test PID 22696 has no remaining owned preflight/confirmation-fixture
  directories. Verification logs/scripts are owned temporary artifacts, excluded
  from the product; final PR records their cleanup and delivery receipt.

Continue the Next P13c-4 design refinement immediately after six exact-head CI
jobs, fresh reviews/base, expected-head merge and clean-main verification.
