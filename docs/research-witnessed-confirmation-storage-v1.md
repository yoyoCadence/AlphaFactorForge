# Checked source-witness context and registry IO

P13c-3b-1b-2b-2 implements the remaining storage part of the
[durable witness plan](plans/research-source-witness-storage-plan-v1.md).
The [implementation order](plans/research-witness-context-restoration-plan-v1.md)
preserves the schema-7 physical contract and existing statistical policy.

## Actual context in one transaction

Strict witnessed APIs accept original inventory/proofs and no caller-supplied
historical counts or graph context. Combined preflight includes both physical
blob tables, original types/individual and joint lengths, cross-table namespace,
all document/interval projections and permanent unknown origins. Hydration
verifies exact SHA, original UTF-8, length and strict logical-kind codec.

A borrowed registry transaction fixes the actual registry ID, current event
chain/indexes, canonical family keys, pinned protocols and every upgrade's
document/scalar agreement. Current family/protocol/evidence domains are checked;
oversized current payload/key/protocol fields are refused before hydration.
The chain is checked once per context, rather than once per witness. Each
witness's complete original archive is independently reconstructed and ALL its
original event payloads/identities/batch membership are compared with current
rows, including other batches, families and foreign origins. Protocol gaps
also survive an unchanged event head. Lost original quarantine/origin-conflict
evidence fails, even when uncertainty is present.

Each missing original event contributes its actual original origin. An
unincorporated protocol high-water mark contributes the archive's origin,
because upgrade rows have no origin field. Read-only observation requires an
actual permanent confirmation-unknown or origin-conflict stop for every gap.
It does not write markers. Actual current alpha declarations, alternative budget
claims, reservations, conflicts and provenance come from the checked alpha loader.

Historical prefixes/counts are independently verified original archival facts;
they are not proof that the receiver contains every original event and are not
a current workspace-prefix/admission fence. The separate whole-source coverage
check prevents such a shortcut. Same-head generic protocol roots remain archived
facts; eligible admission still requires its exact original two-test anchor.
Metadata/artifact/publisher claims retain their existing authentication boundary.

## Strict IO and explicit restoration

`read_witnessed_confirmation_history` and original-origin lookup use one deferred
transaction, strict materialized graph validation and exact projections. The
existing supplied-context schema-6 compatibility APIs keep refusing witness rows.
There is no production binding/transport observer switch in this slice.

`append_witnessed_confirmation_history` performs monotonic archival union in one
immediate transaction. It validates incoming original representations before
duplicate replay, appends derived permanent uncertainty for gaps, completes only
ordinary union conflicts, inserts the checked difference, then strictly reloads
the materialized result. First bytes/audits and every restrictive row survive.

`restore_confirmation_source_witnesses` accepts only immutable original witness
snapshots. It reads original old rows/projections, validates the MATERIALIZED
old-plus-witness graph without completing old conflicts/events/blobs/indexes,
and may append only new witnesses plus derived permanent uncertainty. It cannot
repair corrupt or missing old restrictive evidence. A strict final reload must
equal the proposed inventory before the transaction's one commit. Capacity,
duplicate metadata, bad proof, late insert or reload failure rolls back every
addition. Concurrent independent connections serialize under SQLite's immediate
transaction; retry preserves the first stored bytes/audit and one logical ID.

No nested trial import, receipt-derived count, known-empty assertion, clearing
API or execution permit is introduced. Missing actual original snapshots stay
incomplete. Binding v5 observation/adoption, complete archive v5 transport and
private final admission capture remain subsequent tasks. Witnesses never enter
AI Train context, generation, tuning or ranking.

Executed verification and delivery are recorded in the
[handoff](../handoffs/2026-10-18-p13c3b1b2b2-witness-context-restoration-v1.md).
