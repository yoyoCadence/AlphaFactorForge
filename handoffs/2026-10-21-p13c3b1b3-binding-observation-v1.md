# Handoff: whole-graph binding v5 observation and adoption

Date: 2026-10-21
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b1b3-binding-v5-observation
Status: Implemented and verified; authoritative delivery receipt is in the PR containing this note.

## Summary

From merged #201 (`28e4218`), activate the strict original-text workspace v5
reader/canonical writer and observe complete actual confirmation/witness
history in the existing deferred binding transaction. Legacy formats retain
unknown confirmation evidence and their 4 MiB bound; v5 retains its 8 MiB bound.
The existing owner-checked adoption transaction persists only verified complete
observations and preserves exact last-good JSON/audit on failure.

## Scope and decisions

- Reuse the shared immutable pure snapshot codec and its nine exact-set subset
  check. Immutable backend confirmation evidence comes only from that codec
  or strict content/projection checked witnessed graph IO in the same TX.
- Keep prefix/replacement priority and all trial/alpha/provenance coverage.
  All three blob kinds, including witness SHA IDs, share the existing blobs set.
  Valid loss/substitution is evidence rollback; invalid/incomplete proof/graph
  is invalid state. Ordinary observation/adoption has no proof/marker writes.
- Keep restricted/conflicted/manual/unknown exploration available, permanent
  stops represented, and incomplete legacy managed histories refused until
  explicit verified witness restoration. No receipt/current-primary fallback.
- Old-reader binary was built from exact merged #201 before source edits:
  SHA-256 `8b123abe674fd5fb44a76599b6685f4d0dc61d76ca73db8922c886f6c1efdd30`.
  Schema remains 7; reuse #200's actual schema-6 refusal receipt for that
  unchanged guard and additionally verify actual old binding-reader refusal.
- Complete v5 archive transfer, final private admission capture, metadata/file
  authentication and live owner/count/share checks remain later tasks.

## Verification

Thirteen new real-SQLite/format regressions cover actual managed proofs, all
nine nonempty identity sections, valid loss/equal-sized substitution, standalone
witness/unknown loss, corrupt bytes/scalars/interval projections, incomplete
legacy adoption then explicit restoration, v1–v4 actual adoption, permanent
stops, original duplicate fields/maps, exact raw limits, independent concurrent
snapshot/reopen and owner/late workspace rollback.

Early fixture corrections used the existing `chart` access kind and supported
UTC interval domain, actual instrument/snapshot pairing and managed/manual
overlap (manual/manual overlap is not a conflict). An older v2 fixture now
removes the new confirmation field when constructing its exact legacy shape.
No production guard or timeout was relaxed to accommodate those fixtures.

Final suite, deliberate mutants and real-binary delivery receipts will be
appended below after verification. No calibration/reserved seed or policy change.

## Next action

Complete scoped verification, fetch/rebase/current-base checks, draft PR,
six exact-head CI jobs and expected-head guarded merge. Then start P13c-3b-2
complete archive v5 transport/atomic union from that merged baseline.

[Contract](../docs/research-binding-observation-v5.md),
[implementation order](../docs/plans/research-binding-observation-plan-v1.md),
[parent authority](../docs/plans/research-source-witness-storage-plan-v1.md).

## Resolution (2026-10-21 - implemented and verified)

All thirteen new regressions pass. Four executable mutants were caught by
expected assertions: removed exact-ID coverage, skipped strict SQL projection
verification, omitted witness SHA identities, and defaulted unknown legacy
confirmation to empty. Every mutation was restored byte for byte before the
final suite. The restored full locked suite passes **880 Rust** (297 library,
581 desktop, 2 service; one ignored) and **1242 Vitest / 80 files**. Typecheck,
production build, locked all-target cargo check/Clippy and scoped rustfmt pass;
only the five existing warnings remain. Library/desktop test times were
32.70/106.52 seconds locally; no timeout was changed. Eight new relative links
and diff checks pass.

Actual preserved #201 service refuses a v5 workspace with exit 1 and the
unsupported-binding-version guard, publishes no endpoint and preserves exact
binding SHA/audit (`6ab1875bd4de08f3cd215e391d99b0f21eb3e190fbf4bb59cc3145847362613d`).
The new service creates/reopens/stops successfully; final listener PIDs
28580/30756, ports 57712/57722, were verified against their actual executable.
Windows harness quoting/exit-code collection was corrected without changing
runtime code. All owned service processes/endpoints and the checked absolute
`aff-p13c3b1b3-8941420b33f349b783911476e747b914` temp namespace were removed;
the prior test-registry environment was restored. Registry schema remains 7,
with #200's real old schema-6 refusal receipt retained.

The delivery PR records fresh-main rebase/relevant re-verification, all six
exact-head CI jobs, reviews/base checks, guarded merge SHA and clean main tree
receipt. Complete archive v5 transport is Next after that delivery; production
execution/final admission and all policy/AI boundaries remain unchanged. No new
calibration/reserved-seed experiment was run; committed-report reproduction
remains in the normal suite.
