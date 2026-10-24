# Handoff: original producer projection contracts and controls

Date: 2026-10-24
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/p13c4b2c2-projection-contracts
Status: Design/fixture locally verified; exact-head delivery required

## Summary

From merged #220 (`59a19b6`), refine the ordered original projection task in the
[plan](../docs/plans/research-original-producer-projections-v1.md). The actual
runner embeds its fresh StrategyDef producer in start. The repository preserves
existing library rows on hash conflict, including their JSON bytes; it does not
certify the separate saved dsl_json. Fresh serialization controls and later
historical semantic agreement must therefore stay distinct.

The first code will extract only that exact pure private producer and add actual
producer/storage conformance controls. Actual engine/run_lineage/history writers
must agree with independent pre-code literals before any consumer adapter relies
on them. No source/semantic consumer or authority is added here.

## Frozen controls

New unconsumed original-producer-projections-v1 fixture has four cases: v1
ordered two-axis family (3 candidates / 1 shared hypothesis), cross-base dedup
(4 / 2 with equal base strategy hashes but different winning attribution), v2
fixed DSL (1 / 1), and v3 walk-forward (1 / 1). Complete fresh DTOs, exact
definition/DSL/params/input/engine/hypothesis JSON and hashes are literal.

Independent Python JSON/hashlib/struct authored these from existing committed
cross-language enumeration/selection inputs and source-frozen field rules,
without importing/executing project implementations. It independently verified
all existing candidate identities and seeds before use. Numeric number-3 keeps
the known intended/legacy bits and repository refusal, without a new policy.
Initial authoring incorrectly looked up raw config key version; the schema has
envelopeVersion. No fixture was written by that failed authoring attempt. The
corrected generator uses the actual key; independent verification follows.

## Required action / remaining evidence

Verify the independent literal fixture, sources/catalog/numeric references and
links/scope/lifecycle, then design/data-only PR with six exact-head CI and clean
main delivery. Only the frozen 4b-2c-2a private helper/conformance code enters
Next. That code must use actual repository/history writers under fresh migrated
file SQLite and owned leases, without start/enqueue/execution/registry/provider
activity. Six assertion-caught/restored mutants and full verification precede
its delivery. Consumers and bounded parsing/enumeration budgets need subsequent
refinement after actual producer controls pass. Green CI on an unconsumed
fixture does not claim those future controls ran.

Original cost/action/file/candle/trial/live-owner/registry/numeric obligations
and parent phase incompleteness remain unchanged. This branch adds no runtime,
schema, dependency or workflow changes and spends no protected/statistical input.

## Verification

Independent binary strategy-v2 / seed-v1 checks match all committed candidate
identity/seed references used for the four cases. A second strict JSON pass
checks decoded duplicate keys/non-finite tokens, all nested exact field sets,
config/engine/hypothesis checksums, candidate/reference order, complete input
fingerprints, fresh DTO fields and nested DSL/params projections. The frozen
two-axis/dedup/equal-base-hash and v2/v3 invariants pass. DSL seed is the literal
256232906 in both cases; no random stream is executed. Numeric intended/legacy
bits and full identities match the existing audit/default/roundtrip reports.

Source review covers the actual inline StrategyDef block, hash-conflict
repository behavior, engine/config catalogs, run_lineage, complete hypothesis
content/registration and the unresolved numeric boundary. Fixture inputs use
envelopeVersion; nine-field hypothesis documents / 12-field input fingerprints /
11-field fresh DTOs / nine-field engines retain the exact original names.
Fresh per-case file databases and explicit conflict cases avoid conflating
fresh serialization with a preexisting preserved row.

The fixture is unconsumed by current runtime/tests, confirmed by source search.
No local runtime/statistical suite is rerun for design/data-only artifacts;
unchanged baseline is #220's six-job-verified 1040 Rust + one ignored,
1242 Vitest / 80 files and 98 E2E without retries. Actual producer conformance
remains mandatory in the next code PR. Link/scope/task checks and six-job delivery
are completed separately below.

## Resolution (local design/data checks)

All fixture/source/vector/numeric-reference/link/scope/task-lifecycle and
diff --check checks pass. Plan tables match every literal config/engine/hypothesis
checksum. The temporary independent authoring script was removed after checking;
no generator or project dependency is added. Only six intended design/data/task/
handoff paths are in scope. Delivery still requires fresh-main rebase, relevant
artifact rechecks, exact-head six-job CI and expected-head merge/clean main.
