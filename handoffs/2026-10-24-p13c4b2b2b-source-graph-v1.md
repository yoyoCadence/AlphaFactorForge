# Handoff: P13c-4b-2b-2b complete typed SQL graph

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c4b2b2b-source-graph
Baseline: merged #216 (`4193eba`).
Status: corrected implementation/local verification complete; updated PR #217/CI delivery pending.

## Summary

Implement the [frozen eleven-table graph](../docs/plans/research-source-graph-adapters-v1.md#3-complete-graph-api-and-exact-record-types)
as a private child of confirmation_inputs::source_rows. Every one of the 117
graph cells has a literal SQL marker/value projection and exact typed record.
Nullable storage classes, original JSON bytes, finite REAL bits, prefixed hashes,
sentinels and ascending distinct-reference reads preserve the fixed manifest.
No runtime query/column factory, inner join or successful-only filter exists.

Capture continues the actual roots' joint byte/node usage. Original config
enumeration requires complete Train/Validation jobs and one attempt for every
candidate, with exact strategy/hypothesis/artifact, trial and dataset references.
Exact snapshot/instrument/calendar/dataset/source links and supported direct
parents are checked. Raw failed/rejected/comparison/unknown optional evidence is
preserved without promotion. Mutable summary/revision metadata does not expand
the declared closure. Recheck checks saved workspace first, validates the entire
fresh graph using the saved locator, then compares every field/member/usage.

## Independent Evidence

The frozen source-row-graph-v1 fixture was authored before the production reader:
16 root + 117 graph ordered columns and literal input cells, using the existing
saved three-candidate enumeration hashes and independently frozen dataset-a
campaign. Independent Python counting gives **6576 retained bytes / 192 nodes**,
including fixed 128 bytes. The main regression compares every ordered cell,
original byte and REAL bit. The fixture does not claim execution or file proof.

Eighteen graph tests plus all 62 prior confirmation-input cases pass (80 total).
Coverage includes all 133 retained fields, seven memberships and final counters;
consistent candidate deletion from both tables; extra/Test/duplicate/missing/
cross-linked members; all referenced singleton duplicates; every graph column's
BLOB/invalid-UTF-8 refusal; nullable marker/type/NULL/empty/JSON-null distinctions;
closed vocabularies/integer/hash/REAL domains; supported parent limits; diagnostic
exclusions; real pending visibility, full rollback, SQL errors, writer Busy,
historical deferred snapshots, matching copies and error priority.

Actual rows exercise 4096 candidates/8192 jobs, 1024 source links/provenance,
cap+1 sentinels before payload charge, exact/one-over text/JSON/depth/key/nodes,
and whole 32 MiB / 262144-node ceilings. The final node refusal precedes its sole
byte copy. Shared artifacts are charged once while candidate counts stay complete.
The large-family pure enumerator is fixture setup only, not an expected manifest;
the independently frozen three-candidate oracle remains unchanged.

Initial authoring corrected Python's Windows cp950 read to explicit UTF-8 before
fixture freeze, a test-only logical-core integer to f64 compile error, and two
failed test expectations: 29 preceding JSON copies (30 documents after padding),
and a source link moved outside the exact snapshot selector. That fresh one-link
closure is valid raw consistency; recheck against the saved two-link closure must
report changed. No new selector fallback or authenticated source claim was added.
Preserve TEMP aff-p13c4b2b2b-focused*.log and confirmation.log as receipts.

## Verification / Delivery

Four explicit mutants all failed runtime assertions (exit 101), never compile:
mutual-only membership after consistent deletion, missing artifact omitted as by
an inner join, per-table budget reset and premature caller COMMIT. Original bytes
were restored after each and in finally; graph.rs SHA-256 is
`de096168f16b39fd03e45588fb20bce71b3338547671d4ee1a98061ab0f04779`.
All one-off authoring/contract/mutation scripts were removed.

Full locked Rust passed **1029 + one existing ignored** (297/730/2),
10.10 / 93.60 / 1.30 seconds, in a fresh command-local TEMP/TMP root. First attempt
passed; no source/test changes or failed full run. Locked all-target check and
Clippy pass. Clippy's new test-only nonminimal boolean was simplified; final 80
focused cases and Clippy passed again, with only five existing warnings. Runtime
source bytes stayed unchanged. Scoped format, Vitest **1242 / 80 files**,
TypeScript, production build and git diff --check pass.

Independent static verification matched all eleven frozen manifest tables and
117 field orders to the fixture and 234 literal guarded physical columns; all
ten migrations agree. Evidence links resolve. Success/refusal/SQL-error tests
preserve caller state. No runtime caller, DML/finish/IO/registry surface exists.

Receipts remain in TEMP `aff-p13c4b2b2b-{rust,check,final-clippy,frontend,
final-focused}.log` and four mutant logs. Initial focused/Clippy failures remain
available. The unique full-verification root contained 43 regular leftover
files; exact absolute name/parent and no-reparse traversal were verified before
removing only that task-owned root. Full receipts were already outside it.

Fresh main/rebase, post-rebase focused/scope checks and six exact-head CI jobs
precede delivery. Publish a draft, verify base/reviews, merge with expected head,
read GitHub state back and verify clean main/tree equality.

## Limits / Next Step

No semantic source/RegisteredSource builder, immutable file/candle authentication,
registry/spend/physical-owner authority, execution/command/UI, schema, dependency
or workflow changes. This is a bounded historical SQL consistency observation.
Comparison/rejected sources and missing cost/action/availability evidence are
raw claims, never permission. Parent 4b-2/4b/P13c-4 remain incomplete.

After verified delivery, freeze supported original semantic adapters and
independent digest fixtures in a separate 4b-2c design PR before semantic code.
Missing action/cost/source/file/warmup evidence must remain explicitly unsupported.

## Correction after actual writer evidence (PR #218)

The original implementation followed an incorrect earlier contract: actual
run_lineage creates a shared base strategy-doc-v1 hypothesis with NULL strategy_id,
and register_run_lineage_with_events preserves/deduplicates it. The original
synthetic per-candidate fixture and green tests above did not prove compatibility.
That design error was found before merge; #217 stayed draft. Preserve the original
evidence: tested head `33d79af0725012fdff6c1a34e65c4be074870d35`, base #216,
[CI 40061917565](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/40061917565)
had six successful jobs: 1029 + one ignored (297/730/2), 32.04 / 272.51 / 5.81
seconds, Vitest 1242/80, E2E 98 without retries, production audit zero and full
native restoration/cleanup. It is not the corrected delivery result.

[Correction #218](https://github.com/yoyoCadence/AlphaFactorForge/pull/218) merged
the grounded contract first. #217 was rebased onto `f3a9d12`; only tasks.md
conflicted and retained main's correct In Progress status. Candidate strategy-v2
agreement and full closure/budget/transaction rules are unchanged. Hypothesis
claims retain strategy-doc-v1 or existing strategy-v2; nonnull strategy_id must
match its attempt, and NULL/shared base identity is preserved. Original semantic
base/content identity remains the later adapter's responsibility.

The independently revised fixture has one hypothesis 200 shared by all three
attempts; raw original cells stay claims. New literal totals are **6102 bytes /
184 nodes**. An initial mental total assertion (5902) was caught by independent
Python counting before the fixture was written; no production reader supplied
the expected counts. Canonical original base is 509 bytes; independent SHA-256
is `bf9e2f86c96299155d9da6ba031a72d85e10faa2a41f07ac59cb8bae43aed61d`.

A cfg(test)-only bridge calls unchanged actual run_lineage with controlled
fixture metadata; graph tests then call the real history registration writer.
The control proves one actual deduplicated base hypothesis/NULL strategy_id,
three attempts with exact candidate/trial links and full capture/recheck. A
separate positive case preserves matching nonnull candidate-bound references.
Wrong-domain/bare/uppercase claims and cross-strategy IDs refuse. Twenty graph
tests/all 82 confirmation-input cases pass. Whole byte/node padding was adjusted
to actual retained rows; final JSON pre-copy refusal has 25 prior copies (26
documents after padding). This replaces neither original numeric execution nor
file/registry/source proof. Corrected six-mutant/full-check receipts follow.

### Corrected local verification

All six mutants assertion-failed at runtime (101): original mutual membership,
inner-join omission, per-table budget reset and premature finish, plus forced
nonnull hypothesis identity and forced candidate-hypothesis hash equality. The
last two were caught by the actual-writer positive control. Exact restored
graph.rs SHA-256:
`d01ab1d9d157530130b60aeb03c6db3f59ca67b50e97f6aca6ca7e63a6a2df62`.
Temporary correction/mutation drivers were removed.

Fresh isolated full locked Rust: **1031 + one ignored** (297/732/2),
10.40 / 90.91 / 5.64 seconds; first corrected full run passed. All-target
check/Clippy, graph scoped format and isolated new-helper format pass; five
existing warnings only. Existing runner formatting was not rewritten. Vitest
**1242 / 80 files**, TypeScript and production build pass after Rust. Corrected
fixture/117-column/234-projection/link checks and diff --check pass. Preserved
TEMP `aff-p13c4b2b2b-corrected-{focused,rust,check,clippy,frontend}.log` and six
corrected-mutant logs. The unique verification root's 43 regular files were
removed after exact name/parent/no-reparse checks; logs remained outside it.

Update the existing #217 after fresh-main rebase and relevant rechecks; the
expected-old-head lease protects its necessary rebased push. Require new-head
six-job CI and fresh base/reviews before expected-head merge and clean main.
