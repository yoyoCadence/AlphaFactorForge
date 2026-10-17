# Handoff: P13c-3a-2a confirmation schema/provenance

Date: 2026-10-17
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3a2a-confirmation-schema
Status: Merged as PR #193; checked graph storage is next

## Summary

From merged #192 (`d873917`), implement the schema/provenance half of the
already agreed P13c-3a-2 storage contract. The separate 3a-2b slice still owns
checked graph reads/writes, column/interval projections and original-origin
lookup before binding v5 can depend on persistence.

[Schema contract](../docs/research-confirmation-schema-v6.md) and
[parent plan](../docs/plans/research-confirmation-storage-transfer-plan-v1.md).

## Changes and Review Notes

- Append registry migration 0006 with the ten exact tables, deferred references,
  required indexes, closed SQL variants, original UTF-8/BLOB lengths, safe
  IDs/time/index domains and joint capacities. UPDATE/DELETE and changed-row
  INSERT OR REPLACE are refused, including when recursive_triggers is OFF.
  Text ID checks reject BLOB/NUL substitutes; SQL shapes alone are not graphs.
- Only genuinely new migration-0001 creation writes the immutable birth key.
  Existing registries get permanent confirmation-unknown markers for the local
  origin and every prior import/genesis/checkpoint origin. Oversized backfill
  rolls back schema 6 and leaves its migration unapplied.
- Legacy v3/v4 archival imports preflight and retain source/carried origins in
  their existing transaction, including alpha-reservation origins and empty
  files. Duplicate imports retain first-observed rows without clearing markers.
- Public legacy exporters refuse any confirmation table contents. Internal
  trial/alpha projections stay transaction-scoped for the future v5 writer.
  Old transport-mechanics tests use explicit cfg(test)-only projections; new
  public tests check omission refusal for every table. There is no production
  bypass flag. Existing alpha-specific refusal keeps its earlier priority.
- A legacy v4 qualification request cannot establish confirmation provenance
  and refuses before writes, after the existing alpha-provenance checks.
  Ordinary archival import remains available. New error code:
  `confirmation_history_unknown`; it is a transport restriction, not a permit.

## Verification

Final local verification: **762 Rust** (259 library + 501 desktop + 2 service;
one ignored), **1234 Vitest / 77 files**, typecheck, production build, all-target
cargo check and Clippy (only five existing warnings). Fifteen new SQL tests
cover all append/replacement
triggers, deferred-FK commit/rollback, independent connection/reopen, fresh/old
birth, all known-origin sources, migration failure, text/BLOB/NUL domains,
exact individual/combined capacities and required index columns. An injected
late confirmation-marker failure checks trial/alpha/import-log rollback.

Large row/interval test fixtures temporarily omit only the aggregate-capacity
trigger while preparing pre-limit rows, then restore its exact saved SQLite
definition before asserting the last allowed insert, overflow refusal and
idempotent duplicates. This avoids repeating the same aggregate scan thousands
of times; the actual boundary checks run with all production triggers active.

No bootstrap, reserved final seed, supported statistical-policy adjustment,
UI or runtime confirmation/recovery capability is added.

## Next Step / Limits

After final-head six-job CI and merge, P13c-3a-2b must revalidate the complete
pure graph and all stored columns/intervals before exposing checked snapshots
or append/union helpers. SQL constraints and the birth marker never establish
unseen manual data, authentic publisher context or live admission rights.
Exercise maximum-sized valid union throughput with the aggregate SQL capacity
backstops as part of that writer's tests. Complete v5 transport remains 3b-2;
public old-format export is deliberately unavailable once new history/unknown
markers would be omitted. Keep production policy/manual mediation gates.

## Resolution (2026-10-17)

[PR #193](https://github.com/yoyoCadence/AlphaFactorForge/pull/193) merged as
`44870effa7290202a1d878dd4e47a794fa861415` using final head
`e13237f43de060a34c7355321aac94fbde0b00a8` after all six jobs succeeded in
[CI run 38985924072](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38985924072).
CI confirmed 762 Rust (one ignored), 1234 Vitest, 98 first-pass E2E and real
native render/SQLite/metrics/events with exact browser-policy restoration and
owned-process/temp cleanup. No review/thread blockers at the final check.
Main was fast-forwarded and its tree matches the CI-verified head.

The continuous authorized loop proceeds with P13c-3a-2b from this baseline.
SQL shapes/provenance alone grant neither a checked graph nor live rights.
