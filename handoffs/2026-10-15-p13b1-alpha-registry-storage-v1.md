# Handoff: P13b-1 append-only alpha registry storage

Date: 2026-10-15
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13b1-alpha-registry-storage
Baseline: `816d7dc1dff2723030de5eb73152ff26fb200145` (merged PR #182)
PR: [#183](https://github.com/yoyoCadence/AlphaFactorForge/pull/183)
Status: Implementation verified; authoritative final-head CI and merge receipt are recorded on PR #183

## Scope and implementation

The maintainer requested continued development/PRs/merges. The merged P13b
plan staged this bounded task in Next; it moved to In Progress before source
edits. [Storage contract](../docs/research-alpha-registry-storage-v1.md).

Migration 0004 adds append-only family-budget, reservation and alpha-conflict
tables to the external shared registry. Checked reads reproduce identities,
canonical text, budget/number/share and conflict-family bindings. IMMEDIATE
transactions serialize independent connections; exact retry preserves the
original number/record even after later reservations exhaust the schedule.
Changed budgets or operations retain both claims and permanently stop alpha
for the affected families. Ordinary trial exploration is not quarantined.

P13a gains read-only record accessors; record bytes/identity are unchanged.
There is no production caller, command, workspace migration or reveal.
Storage data is not a campaign/precision/fence admission permit. Explicit
row/document/aggregate-byte limits refuse oversized history without truncation.

## Safe intermediate transfer behavior

Plan §5 says old-format export must not silently drop alpha evidence. Thus
this first-storage slice adds a refusal gate to v3 export once **any** alpha
row exists. Full binding/alpha transfer remains P13b-2. Empty-alpha v3 export
keeps the unchanged schema-3 trial-only projection and existing v3 files stay
readable; legacy trial import cannot erase pinned alpha rows. This is not a
claim that legacy files prove alpha history or qualify unseen data.

## Verification

Final full suite: **635 Rust (180 library + 453 desktop + 2 service), 1
existing ignored; 1184 Vitest / 73 files**. Typecheck/production build,
all-target check and Clippy pass (five existing warnings in unchanged modules);
two new unit expression warnings were removed. All eleven new storage tests,
the independent same-number union and legacy transfer regressions pass.
Targeted rustfmt, artifact links and diff checks pass. No frontend behavior
changed; the PR's full E2E/native smoke remains required.

Final self-review also caches each validated budget ID once per family while
loading evidence. Reservation binding checks reuse that checked ID rather
than re-hashing a potentially long schedule for every record. Contract,
identity and admission behavior stay unchanged; the final code is reverified
and gets a fresh exact-head CI run before merge.

Coverage includes concurrent connections, replay/reopen/exhaustion,
same/cross-family conflicts, changed budget, continued ordinary exploration,
append-only triggers, corrupted evidence, input and aggregate resource bounds,
and rollback of a proposed record after an injected conflict-write failure.

Actual binary upgrade/downgrade smoke passes: current debug service creates
schema 4 in a fresh isolated workspace/registry, publishes its owned endpoint
and stops with code 0. Preserved pre-upgrade service (`5aef090` build) then
exits **1** with `registry_schema_newer` naming `0004_alpha_reservations`,
publishes no endpoint and leaves no service running. Temporary workspace and
registry are removed. Evidence under OS temp
`aff-p12-closure-20261015/`: `schema-downgrade-result.json`, new/old stderr,
stop log, pre-upgrade binary SHA-256 receipt and the bounded cleanup script.
Only the debug temp-registry seam was used; the real registry was untouched.

## Remaining work

P13b-2 must extend binding observations by identities and add complete alpha
transfer/union; split 2a binding first, then 2b complete transfer, before their
implementation. P13c must integrate a synchronized final count/precision/
reservation transaction, frozen batch/selection, exposure overlap and recovery
before any production confirmation. Statistical policy and calibrated set
remain unchanged; no final-seed run or qualification PASS is produced here.
