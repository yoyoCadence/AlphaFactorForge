# Source-witness registry schema 7

P13c-3b-1b-2b-1 implements the additive schema/provenance part of the
[durable witness plan](plans/research-source-witness-storage-plan-v1.md).
The [implementation order](plans/research-source-witness-schema-plan-v1.md)
keeps actual source/current-history reconstruction, strict combined IO and
explicit witness-only restoration in 2b-2. No runtime binding reader or writer
is activated here.

## Physical storage and immutable namespace

Append `0007_confirmation_source_witness` without changing earlier migration
bytes, the old two-kind blob table, foreign keys or the schema-6 birth meaning.
The eleventh table, `confirmation_source_witnesses`, has exactly four columns:
lowercase 64-hex `witness_sha256`, integer `byte_length` in 1..8 MiB, original
`content_bytes` BLOB of exactly that length, and text `recorded_at` audit time.
There is no foreign key to a current trial row or binding blob: a witness is
self-contained original evidence. SQL shape alone does not authenticate it.

UPDATE/DELETE and changed REPLACE inserts are rejected, including attempts to
replace only the first audit time with recursive triggers disabled. Exact
duplicates may be ignored. Both insertion directions reject an SHA already
present in the other physical blob table, preserving one logical blob kind.

Replace only the eight old document/blob capacity triggers, then add the
witness trigger. All nine insertion paths count the seven original document
tables and both physical blob tables together: at most 10,000 logical rows
and 64 MiB of original UTF-8/BLOB bytes. Original individual limits, 128,000
intervals and 10,000 permanent unknown origins remain unchanged. Duplicate
inserts do not consume new capacity; no restrictive history is discarded.
The checked-in SQL is reproducible with
`node scripts/generate-source-witness-migration.mjs` from the app directory.

## Bounded migration and provenance

DDL, trigger replacement, original provenance inspection, permanent uncertainty
and the migration receipt share the existing immediate transaction. Before
recording version 7, the migration checks bounded original document/blob
inventory, SHA/kind/length/codec and exact scalar/interval projections. Original
binding snapshots provide their actual registry IDs. Strict original-text
operation/exposure provenance getters reproduce identity and closed field
domains, returning only IDs/origins; they grant no graph or execution authority.

For managed pre-witness history, retain permanent uncertainty for the local
registry, every actual operation/exposure origin and every original binding
registry. Existing markers and all original bytes/audits remain unchanged.
Empty and manual-only registries receive no extra managed-history assertion.
Capacity, corrupt original bytes/projections or late receipt failure rolls
back schema objects and data together. No receipt supplies historical counts,
missing source proof or known-empty authority.

## Deliberate reader boundary

Public legacy v3/v4 export omission checks include all eleven tables, even an
orphan witness row. Until 2b-2 constructs checked current context, the old
experimental two-kind graph reader/writer refuses any witness row before
hydration. It never silently returns a partial inventory. Old schema-6
binaries reject the new migration version; no downgrade path is provided.

The next slice must load both blob tables, reconstruct each complete original
source, account for all current original events/protocols including foreign
origins, and invoke the strict witnessed graph. Restoration must validate the
materialized old history plus explicit original witnesses, preserve permanent
stops and perform a final reload in one transaction. Missing original proof
stays incomplete, including when an unknown marker exists.

See the [delivery handoff](../handoffs/2026-10-18-p13c3b1b2b1-source-witness-schema-v1.md)
for executed regression, mutation and old/new binary compatibility results.
