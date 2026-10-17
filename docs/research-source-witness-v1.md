# Original source witness codec and trial anchor

Date: 2026-10-17
Scope: P13c-3b-1b-2a-1, from merged design #197 (`767b00c`).
Status: Pure foundation; graph/storage acceptance follows separately.
Plan: [durable witnesses](plans/research-source-witness-storage-plan-v1.md).

`SourceWitnessSnapshot` freezes the closed
`research-confirmation-source-witness-v1` document with exactly
`contractVersion`, `trialExportText` and `registeredSource`. The latter contains
the existing closed source and complete ordered candidate domains (1-4096).
Decode original JSON before normalization so duplicates cannot disappear.
Require recursively sorted compact outer JSON, no final newline, and preserve
the nested original trial text byte for byte, including its final newline.
SHA-256 identifies those exact outer UTF-8 bytes. Private fields/read-only
getters and no unchecked deserializer keep the format snapshot immutable.

The whole escaped witness must fit 8 MiB. Its nested decoded trial text also
has the original independent 8 MiB ceiling. Metadata/escaping can make a valid
inner archive too large to wrap; neither input is truncated. Static errors
disclose no rejected data. The outer codec checks source domains and format
only; even an opaque newline-terminated string can pass that layer.

`SourceTrialAnchor` is immutable domain-checked host metadata: original registry
ID, sequence/head, complete sorted quarantines/conflicted origins and complete
family protocol high-water marks. Safe integer/genesis/8 MiB metadata bounds
apply, without an invented family-count cap. Comparison against `BindingSnapshot`
is exact for every trial component. Alpha and confirmation evidence remain
outside this anchor. Generic protocol facts may exceed two tests; that does
not widen the existing two-test admission/report policy.

The backend separately verifies every nested original trial archive through
`reconstruct_original_source` / `reconstruct_source_witness`, using the existing
closed v3 parser and original-only reproduction resolver. It derives the full
anchor, actual whole-family/batch effective trials and exact complete candidate
index/hash/workspace/family membership. Receipt family-before numbers, receiver
rows and current larger counts do not supply historical facts. The existing
`reconstruct_historical_source` wrapper keeps its exact full-binding trial
comparison, error categories and original-source validation behavior.

Artifact/campaign/run/engine fields absent from trial rows remain claims.
Witness bytes, hashes and reconstruction neither authenticate those files nor
grant execution, qualification, recovery, current ownership or alpha rights.
The fixture's protocol-3 case is archival structure, not statistical support.
Witness material must never feed AI context, generation, tuning or ranking.

This slice adds no logical graph blob kind, host observation map, graph root
acceptance, migration, database writer, invoke, transport or runtime observer
switch. Existing stats/binding inventory remains unchanged. Next, 2a-2 adds
checked witness facts/dependencies/root liveness and existing quarantine/HWM
coverage parity; schema 7/backend restoration follows in its own PR.

The independent Node generator emits a real synthetic four-event chain and
two batches, a same-head protocol upgrade and an audit-only byte change. Rust
checks strict format/bounds and actual reconstruction; TypeScript independently
checks canonical bytes, hashes, genesis, events, batches and full chain.
Verification/remaining limits: [handoff](../handoffs/2026-10-17-p13c3b1b2a1-source-witness-codec-v1.md).

Follow-up 2a-2 adds [strict witnessed graph entry points](research-witnessed-confirmation-graph-v1.md)
and explicit whole-source current accounting. The foundation boundaries above
describe 2a-1; schema-7 storage/runtime integration remains a later slice.
