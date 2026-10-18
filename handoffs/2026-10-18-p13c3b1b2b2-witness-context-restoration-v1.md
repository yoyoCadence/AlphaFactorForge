# Handoff: actual witness context and atomic proof-only restoration

Date: 2026-10-18
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b1b2b2-witness-context-restoration
Status: implemented and verified; authoritative delivery receipt is in the PR containing this note

## Summary

P13c-3b-1b-2b-2 follows merged #200
(`3d83d57003d02abf8eb0a2592b2b95716e2cfa3a`). New strict archival APIs derive
context from complete original witnesses and actual transactional registry
rows. Checked combined reads/lookup, monotonic union and explicit witness-only
restoration preserve original bytes/audits, permanent stops and last-good data.

## Review notes

- Hydrate both blob tables after combined bounded type/length/namespace checks;
  verify all original SHA/UTF-8/kind codecs and scalar/interval projections.
  Old supplied-context compatibility IO still refuses every witness row.
- Check actual current registry ID, event chain/indexes, canonical family keys,
  every pinned/upgraded protocol and original restrictive evidence. Reconstruct
  every complete original source, then compare ALL original events against
  actual current identities/payloads/batch membership, including foreign origins
  and other families. Missing events retain their original origin; missing
  protocol HWM retains source origin. Unknowns never replace invalid proof.
- Historical prefixes/counts come from original anchors; current incorporation
  is a separate obligation. Actual checked alpha context includes alternative
  declarations and all reservation/conflict/provenance identities. Generic
  protocol roots never replace an older exact eligible admission anchor.
- Ordinary union validates duplicate representations before replay, derives
  only required permanent gap stops and completes ordinary conflict union.
  Explicit restoration accepts only witness snapshots plus derived markers and
  validates the MATERIALIZED old graph. No auto-completion of missing old
  conflicts, events, blobs or indexes; no nested trial import or receipt fallback.
- Both write paths share the existing difference insertion mechanism and
  strictly reload before their one commit. Late trigger side effects, capacity,
  corrupt metadata/bytes/projections and lost restrictive facts roll back.
  New modules isolate responsibilities; old transfer/root files are not reformatted.

## Verification

Twenty new real-SQLite regressions cover actual managed source snapshots,
current growth, original lookup, source loss/explicit restoration, exact old
bytes/audits, replay/reopen, same-head eligible/generic protocol roots, complete
foreign-origin accounting, unknown stops, lost restrictive evidence, corrupt
bytes/types/namespace/columns, exact 10,000-row/64 MiB and unknown-origin caps,
independent concurrent connections and late insert/final-reload rollback.

Normal conflicting imports deliberately withhold the source checkpoint when
any original event is missing. The foreign-origin regression explicitly models
a retained matching checkpoint in a partially restored registry, while keeping
the actual receiver's chain/indexes valid. It proves that a matching last event/
checkpoint cannot substitute for the complete foreign-event accounting; it does
not relax or change the importer. The initial fixture incorrectly expected that
checkpoint from a conflicting import; the corrected case retains it explicitly.

Four executable mutants were assertion-caught: omitted foreign-origin events,
omitted same-head protocol gaps, auto-completion during materialized restoration,
and skipped final strict reload. All original source bytes were restored before
the final suite. Scoped formatting and seven new local document links pass.
The restored full suite passes **867 Rust** (297 library, 568 desktop, 2 service;
one ignored) and **1242 Vitest / 80 files**. Typecheck/build, locked all-target
cargo check/Clippy and scoped rustfmt pass; only the five existing warnings
remain. Desktop tests take 151.22 seconds locally; no timeout was changed.
No calibration or reserved seed was run; existing two-test/report/P12 policy
and AI Train boundaries are unchanged.

## Next action

After exact-head CI/review/base checks and merge, implement whole-graph binding
v5 observation/adoption (3b-1b-3), preserving exact identity coverage and the last
good workspace binding. Complete archive v5 transport and private final admission
capture remain later. No production execution or observer is activated here.

[Contract](../docs/research-witnessed-confirmation-storage-v1.md),
[implementation order](../docs/plans/research-witness-context-restoration-plan-v1.md),
[parent plan](../docs/plans/research-source-witness-storage-plan-v1.md).

## Resolution (2026-10-18 — implemented and locally verified)

Complete the agreed actual-context/strict-IO/restoration slice with the twenty
regressions and four assertion-caught mutants above. All original mutations
were restored. The delivery PR containing this note records the verified final
head, all six CI jobs, fresh base/review checks, exact merge SHA and clean main
tree receipt; it is the authoritative delivery status after local verification.
Binding observation/adoption remains the next separate task.
