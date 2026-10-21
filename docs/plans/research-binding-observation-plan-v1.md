# Whole-graph binding v5 observation and adoption

Date: 2026-10-21
Scope: P13c-3b-1b-3, from merged #201 (`28e4218`).
Authority: [durable source-witness plan §7/8](research-source-witness-storage-plan-v1.md).

1. Keep exact v1–v4 readers and their 4 MiB bound; represent confirmation
   evidence as unknown. Activate the existing strict original-text v5 codec,
   its 8 MiB bound and canonical writer. Never default missing evidence.
2. In the existing deferred binding transaction, verify the event prefix,
   complete actual trial/alpha/provenance context and strict witnessed graph
   including original bytes/projections. Derive all nine exact confirmation
   identity sets; witness SHA IDs use the existing blobs set.
3. Require all saved restrictive evidence and confirmation identities to remain
   covered. Valid loss/substitution is evidence rollback; invalid or incomplete
   proof/graph is invalid state. Ordinary checks/adoption never write proofs or
   uncertainty markers. Preserve valid restricted exploration and permanent stops.
4. Persist only the complete verified v5 observation inside the existing owner
   checked workspace transaction. Retain the exact last-good binding on every
   failed check/adoption, including incomplete experimental legacy history.
5. Exercise actual managed witnesses, all identity sections, valid substitution,
   corrupt/incomplete graphs, legacy adoption, concurrent/reopened connections,
   original-text strictness/bounds and failed workspace writes. Catch deliberate
   mutants by assertions. Preserve the real #201 binary before edits to prove
   old workspace-reader refusal; schema-6 refusal of schema 7 was already
   demonstrated with the real old binary in #200 and its unchanged schema guard
   remains covered by the normal suite.

Run the relevant locked Rust/frontend checks and all six final-head CI jobs,
then open/verify/merge the scoped PR with expected-head protection. Complete
archive v5 transport and private final admission capture remain separate tasks.
No policy expansion, calibration, reserved seed or production execution change.
