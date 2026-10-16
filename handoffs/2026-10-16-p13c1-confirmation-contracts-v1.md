# Handoff: P13c-1 pure confirmation documents and overlap

Date: 2026-10-16
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c1-confirmation-contracts
Base: merged design #187 (`da259f09572d5f4c54e83c4ebfbedd1de26b216c`)
Status: Merged as PR #188; P13c-2 continues the ordered pure-contract work.

## Summary

The [bounded wire contract](../docs/research-confirmation-contracts-v1.md)
was written before implementing the pure module. It freezes three checked,
immutable `{id, body}` records with version-separated canonical identities.
There are no database, file, network, Tauri, reservation or execution calls.

- Validation binds the supplied complete registered enumeration, original
  artifact references, canonical instrument/family, split/time support, costs,
  both stage seeds/settings and the predeclared selection rule. Test inherits
  every common field and exactly the sealed selection's original candidates.
- Selection binds original report bytes and the supplied reservation/seal.
  It verifies the full v2 report, recomputes whole-family Holm decisions using
  integer extreme counts, requires both tests to reject and takes ascending
  candidate indexes up to the frozen limit. Empty selection is retained but
  cannot create Test. Display floats are never rehashed into new identities
  or used to choose candidates; legacy numeric interpretation is unchanged.
- Operations retain original origin/key/request and exact batch/workspace/
  family/allocation/stage/selection bindings. Exact replay differs from an
  altered body using the same original origin/key; no ownership epoch or
  transport replacement mints authority.
- Overlap uses canonical instrument and actual half-open interval support.
  Dataset/source/revision/timeframe/role metadata cannot exempt intersections;
  touching endpoints do not overlap. Canonical sets are sorted, disjoint and
  already merge touching members.

## Verification

The independently authored fixture has fixed canonical identities, an existing
family/allocation/reservation identity, and a hand-calculated m=8 report with
adjusted numerators `[8,14,60,150]` over denominator 800. Rust and the existing
TypeScript encoder reproduce those constants. These are synthetic structural
examples, not a calibration claim or a bootstrap run.

Twenty-four Rust regressions cover missing/extra/substituted enumeration,
all inherited Test bindings, whole-family count and tie order, report bytes,
sealed-selection/replay tampering, strict nested original JSON, exact and
one-over capacities (4 MiB, 4096 candidates, 8192 report rows, 128 intervals),
safe integers and market-time boundaries. The overlap oracle compares all
65,025 pairs of nonempty subsets of an eight-millisecond domain using set
intersection, independently of the implementation's linear interval scan.

Three executable mutants were caught by assertions: inclusive touching
endpoints, batch-only Holm adjustment and omitted Test-parent equality.
All mutated source files were restored byte-for-byte. The strict stage test
also caught Serde's unit-variant extra-field acceptance during development;
the empty struct variant now refuses it without changing the wire shape.

Local `cargo test --locked`: 692 passed (204 library + 486 desktop + 2 real
service smoke), one existing ignored test. `npm test`: 1191 passed / 74 files.
Typecheck, production build, all-target Cargo check and Clippy passed (five
existing warnings; none in the new module). Final-head CI
results are recorded with the delivery PR, including browser and native smoke.
No calibration simulation, reserved final-seed run or threshold change.

## Remaining authority and next task

The checked pure API only proves consistency with its supplied context. The
host must authenticate the actual complete registered source and original
artifact/snapshot/calendar/cost/report bytes, prove real support coverage,
authenticate a durable seal, and union persistent history. A deliberate
regression demonstrates that an invented, consistently truncated context is
not detectable here; this API never claims completeness or execution rights.

P13c-2 is next: a pure Train-only context allowlist, protected/nested/notes/
selection/hash and mixed/stale provenance refusal, and exposure-time checks.
P13c-3 schema/transfer/binding design remains separate. No production caller
or real-market statistical confirmation before the design's policy and
manual-mediation gates. P12e's exact synthetic tested set remains unchanged.

## Resolution (2026-10-16)

[PR #188](https://github.com/yoyoCadence/AlphaFactorForge/pull/188) merged as
`70dcc424985d5cb42e40d8dbdc57b4cfbdcaf805`, after all six jobs succeeded on
final head `1c244dbdba784a2d07cbf337230609e8ae884d9e` in
[CI run 38913482233](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38913482233).
CI verified 692 Rust tests (one ignored), 1191 Vitest, 98 E2E and real native
SQLite/invoke/event/metrics smoke with cleanup. Main's merge tree matches the
verified head. P13c-2 proceeds separately from this baseline; the
[Train-only context contract](../docs/research-train-context-v1.md) preserves
the distinction between pure consistency and host authentication/authority.
