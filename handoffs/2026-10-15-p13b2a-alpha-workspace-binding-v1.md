# Handoff: P13b-2a alpha-aware workspace binding

Date: 2026-10-15
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13b2a-alpha-workspace-binding
PR: [#184](https://github.com/yoyoCadence/AlphaFactorForge/pull/184)
Status: Implementation verified; delivery, final-head CI and merge status are maintained on PR #184.

## Summary

From merged P13b-1 #183 (`9ab7fd8`), workspace binding v3 now retains checked
alpha budget, reservation and permanent-conflict identities alongside the
existing event prefix and §23 evidence. Equal counts no longer hide replaced
alpha evidence. Missing or malformed evidence leaves the last good workspace
binding untouched. [Contract](../docs/research-alpha-workspace-binding-v1.md).

The storage reader validates all canonical content/IDs/family/share bindings
and affected-family conflict copies before observation; raw SQL ID selection
would be insufficient. Budget identities contain both family and allocation
ID so the same declaration on another family cannot substitute for it.
Alternative declarations inside permanent budget conflicts also remain in
the observed budget set, preserving future transfer/rebinding evidence.
Prefix, restrictive evidence and alpha are read in one registry transaction.

## Compatibility and boundaries

- v1 keeps unknown restrictive/alpha evidence; v2 keeps its original
  restrictive evidence and unknown alpha. Neither legacy binding can be
  written directly by the new writer. Verified adoption observes the actual
  registry and writes complete v3, including nonempty alpha history.
- Strict v3 rejects missing/null/unknown alpha fields, invalid IDs, unsorted
  or duplicate sets and duplicate struct fields. No declaration ID alone,
  row count or derived blocked status substitutes for saved identities.
- Total alpha identities: 30,000 (up to three per storage row, including
  embedded budget declarations); complete encoded binding: 4 MiB, including
  legacy reads. Excess evidence is refused, never truncated/defaulted.
- Existing adoption failure stops workspace opening. Valid alpha conflicts
  continue to allow adoption and ordinary exploration; no trial quarantine
  is created. The old admission fence still ends its read transaction before
  returning and is not an atomic reservation permit.
- Complete transfer/qualification refusal of legacy exports remains P13b-2b.
  A trial-only export with a proven prefix cannot erase saved alpha. This
  watermark cannot prove completeness of an unseen registry, settle legacy
  manual exposure or detect jointly restored workspace/registry evidence.

## Verification

**648 Rust passed (180 library + 466 desktop + 2 service), 1 existing ignored;
1184 Vitest / 73 files; typecheck/build pass.** Thirteen new regressions cover
checked growth and conflicted-family reopen/exploration, missing/changed
budgets (including same allocation ID on another family), alternative budget
identities retained inside conflicts, valid-hash
same-count reservation substitution, missing/replaced conflicts, unchanged
IDs with damaged bodies, lost affected-family copies, old trial-only
replacement, strict formats/legacy upgrade/capacity, and real workspace DB
reopen preserving its binding JSON byte-for-byte after alpha rollback.

All-target check, Clippy (five pre-existing warnings), focused rustfmt and
diff/link checks pass. Normal tests replay declared prefixes only; the final
calibration seed was not rerun. No new dependency, migration or UI behavior.

Actual isolated service smoke: the new service writes v3 and stops with exit
0. The preserved baseline service (schema-4-capable, executable SHA-256
`f6e25d4400c50157f881baf5d5cce4ccd5d52602caaa48553ec5e750f5b64458`)
refuses it with exit **1**, `registry_state_invalid` / unknown `alpha` field.
No endpoint remains, both service PIDs exit, owned temp workspace/registry
removed. Evidence under OS temp `aff-p12-closure-20261015/`:
`binding-downgrade-result.json`, new/old stderr, stop log,
`verify-alpha-binding.ps1` and `p13b2a-*` verification logs. Only the debug
temp registry seam was used; the actual per-user registry was untouched.

## Next

P13b-2b: complete alpha-aware export/import union, validate before writes,
deduplicate by identity, preserve independent conflicting records and budgets,
recompute permanent stops, refuse alpha-unknown qualification transfers, and
test duplicate/two-hop transfer plus atomic failure. Then P13c must establish
the frozen batch/exposure/atomic final-fence/precision/reservation/recovery
contract before any production confirmation. No statistical policy extension,
data reveal or qualification PASS is provided by this slice.

## Resolution (2026-10-16 — merged binding delivery)

PR [#184](https://github.com/yoyoCadence/AlphaFactorForge/pull/184) merged as
`13649f4b68beacb55dc99afb97482ac710ee1370` after all six checks passed on
`be259d1af555645480dae45589704ace01cacf57`, run `38760421014`:
648 Rust (1 ignored), 1184 Vitest and 98 E2E. The authoritative
[delivery receipt](https://github.com/yoyoCadence/AlphaFactorForge/pull/184#issuecomment-6195986765)
records the final-head evidence. P13b-2b follows with complete transfer and
rollback-aware legacy-alpha provenance; runtime confirmation remains P13c.
