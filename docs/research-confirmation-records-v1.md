# Pure confirmation history records v1 (P13c-3a-1a)

Implements the first bounded codec slice of the
[schema/transport contract](plans/research-confirmation-storage-transfer-plan-v1.md#2-acyclic-immutable-records).
Fields, versions and bounds remain exactly that contract. There is no IO,
schema, registry mutation, execution, provider or production caller.

## Checked scope

- `freeze_confirmation_exposure` / stored parser require checked P13c-1
  batch, original operation and P13a alpha reservation. All origin/key,
  batch/family/allocation/instrument/alpha references and the **exact** frozen
  stage support must match. A Validation share/support cannot authorize Test.
- `freeze_manual_exposure` / stored parser accept only manual or legacy-unknown
  claims, retaining mandatory nullable seal/evidence references. They create
  no known-unseen state or read permission. A supplied `afterSealId` is only a
  reference at this stage; its actual seal/containment is checked in 3a-1b.
- `freeze_confirmation_event` / stored parser require checked batch and
  operation, bind the operation/batch, started workspace and sealed engine,
  and validate all variant IDs/counts/mandatory nullables. Admission/start/
  report/artifact existence, transitions and complete seals remain 3a-1b and
  the later host's responsibility; parsed events never schedule work.
- `freeze_conflict_declaration` / stored parser check the closed kind, two
  sorted distinct claim IDs, 0–2 family targets and 1–2 canonical
  instrument targets. Families must derive from the supplied instruments.
  The returned **declaration** cannot stop anything. Only 3a-1b's full graph
  may validate the actual relation and exact complete derived targets; a
  self-asserted declaration is not validated conflict evidence.

Checked records have private fields and no Deserialize. Immutable getters
provide IDs/documents and exposure geometry only. The two new P13c-1 operation
getters and batch allocation getter expose existing immutable bindings without
changing the earlier wire shape or identities.

Every constructor reads original JSON directly, rejects unknown/missing/
duplicate fields and float/exponent counts, enforces lowercase hash versions,
safe integer/time bounds, closed tagged variants and the existing canonical
interval rules. Each raw body/stored document is at most 4 MiB UTF-8; stored
output must fit too. IDs use the existing version + zero + canonical-body
encoding. Parsers recompute IDs rather than trusting the wrapper.

Unknown enum strings, extra variant fields, absent nullable fields, changed
parent/reference/geometry and over-capacity inputs fail closed with static
error codes. No rejected value or parser diagnostic enters an error message.
Original protected reports and their floats are not parsed or rewritten by
this slice. Hashes/labels and synthetic fixture IDs prove no authenticity.

## Acceptance and next step

Independent Node-authored constants are checked with Rust and the existing
TypeScript encoder. Cover both stages and all manual/legacy/event variants,
wrong/changed supplied bindings, contained/touching/disjoint supports, strict
original wire parsing, field/identity tampering and exact/one-over bounds.
Explicitly retain the partial-reference boundary: a structurally valid
declaration/event is not a valid complete graph, admission or execution right.

P13c-3a-1b adds checked admission/P12a proof, original report/binding blobs and
the complete dependency/conflict graph. Storage follows only after both pure
slices merge. Statistical policy, final seed and manual mediation are unchanged.
