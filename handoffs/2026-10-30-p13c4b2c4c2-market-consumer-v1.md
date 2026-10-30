# Handoff: Bounded original market projection consumer

Date: 2026-10-30 (UTC+8)
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/original-market-consumer
Status: Independent final review approves; local verification complete, exact-head CI/normal merge pending.

## Scope

Implement [the frozen market consumer contract](../docs/plans/research-original-market-consumer-v1.md)
as one private sealed-graph entry, with joint P/P+E and W/C/A/Q/O, original market
field/identity agreement, unchanged actual warmup bridge and one complete output.
No runtime caller, schema/dependency/producer/old-fixture change, new source
authority, file/candle check, current-successor claim or protected execution.
Actual4c closes only after complete code delivery; trial and later source/live
gates remain separate.

## Intended files and independent planning review

Narrow preparation/type/entry wiring in original_lineage.rs and reserve/finish
sharing in warmup.rs; a private market child (with local measurement/verification
children if useful) and private actual-producer/control/boundary test modules.
Preserve all existing tests. Task/receipt/handoff docs accompany this slice.

A separate reviewer checked the proposed composition against original code:

- Send joint P into existing prepare_run_plan, which gates P+E before enumeration;
  keep PreparedOriginalLineage.parse/combined as old P/P+E. Both existing lineage
  and warmup Q gates use those old nodes and must not be relaxed by market P.
- Reuse one ReservedLineage and fixed warmup reserve/finish stages. No second
  observer/preparation, callback, document factory or caller-selectable cap.
- Measure only borrowed structure before the whole reserve: even instrument-ID
  parsing allocates. Typed copies, validators/DateTime, canonical/hash, source
  sorting and final bindings belong after the joint cap.
- Exact outer keys: calendar7/instrument14/snapshot11/provenance14/coverage8.
  Typed structs ignore unknowns, including nested Suspension keys; compare the
  complete typed roundtrip+version to the original before hashing. Preserve
  optional nulls and original SQL REAL bits under the locked numeric policy.
- Calendar timezone only forbids trim-empty; valid continuous earlyCloses are
  allowed. Instrument base/quote/timezone also require trim equality; timezone
  need not equal the calendar's. No source whitelist or cost/action trim policy.
- record_raw compares available/retrieved timestamp_millis; forward cut/archive
  equality use full DateTime instants. Preserve timestamp spelling/offsets.
  revisionOf may lie outside the selected closure; raw length may be0; scope
  may be any Value. Raw path is checked but excluded from the fourteen-field hash.
- Publisher equivalence uses the original nonempty first-hyphen origin; compare
  each member with one fixed reference, not the producer's quadratic pair loop.
  Keep the captured revision/as_of; Existing retains original metadata. Cost/
  action Option presence only declares a reference, including Some("").
- T charges repeated raw text cells, P original bytes/nodes, Jd compact decoded
  values; Ji/Ni include provenance bool/null/scope. Audit Value clones/typed
  construction against4Nd+2Ni. Om is192+72*m. Immutable pre-code pins refer to
  their recorded baseline; all actual market producers/old fixtures stay fixed.
- W/C/A exact recipes reserve then refuse identity; only O exact is a complete
  success. Require joint P/E/work tripwires, complete actual families and late
  refusals, selected campaign range tests and seven assertion-caught mutants.

## Verification gates

Real migrated file-backed isolated workspace captures and actual unchanged
market/dataset/strategy/history writers precede independent-literal comparisons.
No synthesized successful market rows or expected values from the new reader.
Preserve originals, full SQL/schema/total_changes/mutation_seq, caller savepoint/
writer/physical lease and artifact inventories on every result/refusal.

After exact mutation restoration, full fresh exclusive GUID TEMP/TMP locked Rust
before Node-heavy checks; all-target check/Clippy/scoped format, frontend checks,
independent review, six exact-head/base jobs and normal expected-head merge/
readback/clean-main/full-tree equality. Verify owned cleanup containment and
no-reparse after closing all handles. No user manual server/test is required.

## Original-writer contradiction — 2026-10-31

Initial actual-producer setup catches a frozen-v1 fixture defect before the new
observer is called. The first original input fingerprint retains randomEntry
3949539066 from the old dataset, while the unchanged writer correctly produces
3642945820 for the new dataset hash and same strategy/root seed. seed-v1 includes
the dataset identity in its exact preimage (discovery_core/seed.rs), so replacing
dataset/config hashes without rederiving seeds is not a coherent original family.
The setup comparison fails; no new reader output is used as an expected oracle.
Log: ignored test-results/market-consumer/initial.log (3 pass/1 fail). The failing
test owns/closes its workspace/artifacts; its exclusive outer diagnostic root is
retained by initial-root.json. No protected execution or source admission ran.

Independent reviewer Python hashlib/struct confirms all fourteen v1 seed claims
are stale. Params seeds retain ten-digit width; each mixed family's DSL seed
grows nine-to-ten digits, increasing its original parse/scratch bytes by1. The
complete W/C/A/Q/O, node and expansion counts remain unchanged for those families.
Resource recipes also need independent seed-width accounting before code resumes.

Preserve the current implementation as an unpublished local checkpoint. Refine
an additive v2 design/data contract in its own PR from current main, retaining
v1 bytes and all original producers/runtime. Author seed frames/digests and all
affected preparation counts independently with Python and Node; verify actual
writer agreement only afterward. Complete its review/CI/merge cycle, then rebase
this implementation onto that corrected baseline and finish actual4c-2.

## Resumed after verified #241 - 2026-10-31

Additive v2 contract delivered by #241/052f8bf, full tree43cfa20. Original v1 remains historical error evidence. Consumer now uses independently corrected v2 (LF SHA a12f373f2a13580263ccb752bf6be773b812e457baa259b2646810a501aabf01); producers/runtime/older fixtures unchanged by correction. Original checkpoint d682a8d is rebased as6a6631e, preserving merged task/receipt history.

Separate preliminary production review found an original-acceptance edge: snapshot pairwise conflict logic permits identical nonblank source IDs such as -custom even though sources_share_origin returns false for their empty prefix. Fixed-reference O(m) checking must accept exact source equality OR shared origin, with real-writer positive controls for single/multiple same-source components. This is fidelity to existing producers, without new authority or producer edits. All other reviewed formulas/pre-reservation order/schema/chronology/range checks had no preliminary blocking finding; full implementation approval still requires all tests/review/delivery gates.

## Implementation and focused evidence

One private sealed-graph entry composes old preparation, one borrowed lineage
reservation, fixed warmup work, market structural counting and a complete joint
W/C/A/Q/O gate. Original P/P+E remain named old components for the unchanged
lineage/warmup clone ceilings; market originals are added to joint P/P+E before
Value materialization/enumeration. Hash/typed copies/date parsing/sort/bindings
occur after the complete reservation. The old actual warmup bridge still visits
every candidate; market output is created only after all stages succeed. Static
entry error, private Clone/Eq types and ancestor-only borrowed getters add no
runtime/source capability. No producer, schema, dependency, UI or old fixture
changes belong to this implementation.

Focused controls now pass25 main tests (23 new and2 unrelated existing matches),
plus26 existing lib tests matched by market_. This includes all four complete
original v2 families, five coherent frozen source refusals, original-writer
single/multiple identical -custom sources and complete success with two failed
members. Actual mixed4/1 warmup calls and frozen full market/lineage/derivations
remain exact. Actual campaign rehash with only a selected range contradiction
passes old warmup but refuses market after reservation; a distinct first ETH
binding cannot replace the selected second BTC range. A late failed member
reaches the original third embargo call, then withholds every output/binding.

Calendar/instrument/snapshot/dataset/coverage/provenance damages verify complete
scalars, REAL bits, declarations, source membership/order, original chronology
and static refusals. Additional coherent controls rehash all affected calendar,
instrument and snapshot documents before refusal, including unknown nested
Suspension fields, trim-invalid base, zero specification, non-object capability,
listing chronology, calendar schema, demo and unsupported price basis. Coverage
and status refusals explicitly retain agreeing original hashes. No stale hash
substitutes for these semantic checks.

Four frozen resource recipes compare every original candidate seed/strategy hash
to the v2 pre-poison manifests, then compare all P/P+E/W/C/A/Q/O literals. Exact
W/C/A reserve then reject intentional identity poison; exact O succeeds for all
1101 original warmups/candidates. Every +1 refuses before old/new payload,
warmup or bindings. Clone auditing covers typed/Value construction on both
success and post-reservation refusal. The extra Q-only over control uses actual
record_raw and a controlled extra snapshot relation, followed by fresh capture;
only Q exceeds its cap, while both capture caps and P/E/W/C/A/O fit. It asserts
no exact-Q positive and no semantic success for that damaged relation claim.

All P and P+E byte/node exact/+1 inputs are re-captured from real migrated
file-backed SQLite. Byte recipes pad the original config to1MiB to account for
P's double-config charge. Node recipes install original writers with4096 null
rules before syntax/resource shaping. Independent reviewer Python confirms
44-byte/5-node rules become20481/4097; E becomes62973/12390 and initial P nodes
20961. A controlled raw run2/admission/jobs/attempt family is appended, retaining
the original immutable attempts/hypotheses and every trigger/schema object.
Only mutable strategy definitions receive the established resource poison;
market evidence stays identical. Fresh capture usage is checked under both
original caps before the new observer is tested. Exact P reaches the P+E gate;
one-over P blocks materialization; exact P+E admits one enumeration, one-over
P+E blocks it. No successful semantic claim follows from these raw node shapes.

Each observer control checks sealed input equality, full SQL/schema/storage/REAL
bits/total_changes/mutation_seq, caller savepoint/active transaction, independent
writer contention/physical lease and complete artifact inventory. Every owned
test workspace closes handles and verifies exact parent/leaf/descendants/no
reparse before removal. Exclusive outer diagnostic roots/logs remain recorded
under ignored test-results/market-consumer until verified cleanup.

## Review refinements and retained initial failures

The first separate reviewer approved the composition and found the identical
source edge. After that session reached its usage limit, a fresh independent
reviewer found no production blocker and requested coherent-hash refusal,
successful failed-member and actual SQL-capture boundary evidence; all are now
implemented and focused checks pass. Final approval still follows mutations and
full verification. The tracked v2 and every older immutable fixture stay fixed.

Initial new tests exposed test-shaping mistakes rather than runtime fixes:
original scope may be an array, baseline suspension is empty and coverage count
is4; invalid controls were corrected to actually change their original input.
The preparation metrics helper returns the old internal error, so refusal paths
were moved to the public market entry to verify its static mapping without
changing old lineage errors. First raw run2 attempt keys were noncanonical;
capture refused them, and setup now uses run:2:candidate:i. Initial logs are kept
alongside passing final logs; no product guard/test assertion was weakened.

## Final local verification

- All nine actual-source mutants assertion-fail without checksum tests or
  compiler failures. Seven frozen mutants plus omitted joint Q and regressed
  identical-source acceptance are covered. Omitted W/C/A/O fails all four whole
  boundary cases. Final source SHA256 bytes are restored exactly and recorded
  in ignored mutants.json; a second run follows the final Clippy refinement.
- First full fresh exclusive GUID TEMP/TMP Rust passes1158/one existing ignored.
  Clippy identifies one new nonminimal boolean in suspension ordering; replace
  !previous.is_some_and(start<end) by the equivalent previous.is_none_or(start>=end)
  over i64. No lint allowance, producer or old-warning cleanup. All nine mutants
  are repeated against that final source, then full fresh isolated Rust passes
  **1158/one existing ignored (297/859/2;10.10/100.38/5.52s)** before Node checks.
  Locked all-target check3.93s and Clippy6.28s pass; only five old warnings at
  backtest330/342,score416/428,file_commands156 remain.
- Final Vitest **1242/80 files** passes (maxWorkers4;5.27s), typecheck and
  build111modules(835ms) pass. Final Playwright **98/no retries** passes (2.1m)
  with its existing single worker and managed server. Browser plugin was absent;
  the existing repository Playwright workflow is the recorded fallback. No UI,
  E2E assertion, timeout, dependency or production command was changed.
- Independent tracked Node verifier passes all14 family/1112 resource seeds,
  original/catalog/pin/identity/work checks. Approved13-file scope, scoped
  rustfmt/diff checks, links, append-only historical bodies, immutable fixtures
  and final mutant exact-byte audit pass. Data/source numeric policy is unchanged.
- Both full roots are safely removed after Rust processes/DB handles exit:
  each43files/64directories. All32 exclusive owned outer roots (including focused
  and both9-mutant runs) remove86files/158directories total. The cleanup verifies
  exact canonical parent/GUID leaf/every descendant containment/no reparse and
  post-removal absence. Initial failure logs/manifests remain in ignored
  test-results/market-consumer. Managed ports5173/5174/5175 are not listening after
  E2E. No user files, global settings or unrelated process is removed.

Six completed exact-head/base CI jobs, final independent recommendation, normal
expected-head merge, GitHub readback and clean-main/full-tree equality remain
delivery gates. Only that delivery closes actual4c. Per the agreed sequence and
continuous-cycle authorization, the next bounded task freezes original trial/
split producer evidence and missing-evidence boundaries before its consumer
code; file/candle/availability/action/cost/current-owner gates remain mandatory.

The separate final reviewer approves after checking stable final source, the
Option boolean equivalence, all nine runtime mutant assertions/source hashes,
1158 Rust/one ignored,1242 Vitest,98 E2E,check/Clippy,13-file scope, immutable
fixtures/append-only receipts and32-root verified cleanup. No unresolved
correctness/scope finding or further code change remains locally. Exact-head CI
and normal merge/readback are the remaining delivery gates.
