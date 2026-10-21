# Handoff: complete canonical witnessed export v5

Date: 2026-10-21
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13c3b2a-witnessed-export-v5
Status: Implemented and verified; authoritative delivery receipt is in the PR containing this note.

## Scope

From merged #202 (`f4b0cd7`), split P13c-3b-2 into export then strict input/
atomic union under the existing frozen format. New backend-only complete
exporter reads the strict witnessed graph and existing internal v4 trial/alpha
projection in one deferred transaction. It emits canonical schema-7 v5 with
all nine counts, prescribed section/identity ordering, three original blob
kinds/text/SHA, permanent unknowns, body-byte digest and final newline.

Bounded serialization enforces actual encoded file/line ceilings while writing,
including escaping. Separate embedded-v4/header/final-file limits remain exact.
The writer never publishes private partial output, mutates registry evidence
or changes old v3/v4 omission/qualification guards. It exposes no command,
importer, provider call, live admission or execution capability.

## Verification

Nine new regressions passed in the focused archive run. Independent Node
wire oracles reuse a real complete four-event source and freeze exact bytes/
hashes. Real SQLite cases cover nine nonempty sections/all three blob kinds,
old guards, exact original text/audits, corrupt/incomplete graph refusal,
independent concurrent trial/alpha/confirmation snapshots and reopening.
Private small ceilings test the same production bounded serializer at exact/
one-over file/header/projection/line limits, including JSON escaping.

Four deliberate mutants were caught by assertions and restored byte for byte:
skipped SQL projection checks, omitted witnesses, disabled encoded-line ceiling,
and wrong body hash. Scoped rustfmt, deterministic fixture regeneration and diff
checks pass. Complete Rust/frontend suite and exact-head delivery follow below.
One initial test used a newer rusqlite convenience method unavailable here;
the fixture now reads SQLite's native `total_changes()` to prove read-only IO.
No production guard or timeout was relaxed. No new calibration/reserved-seed
experiment; committed-report reproduction remains in the normal suite.

## Next action

Finish full verification, current-main rebase/rechecks, draft PR, six exact-head
CI jobs and expected-head guarded merge. Then immediately continue P13c-3b-2b:
strict original input/source validation and one-commit trial/alpha/confirmation
union with every original branch/stop, two-hop/replay, capacity and late rollback.
Final owner/count/share/files/authentication and execution remain later.

[Export contract](../docs/research-confirmation-export-v5.md),
[bounded implementation order](../docs/plans/research-confirmation-transport-implementation-v1.md).

## Resolution (2026-10-21 - implemented and verified)

The restored full locked suite passes **889 Rust** (297 library, 590 desktop,
2 service; one ignored), **1242 Vitest / 80 files**, typecheck/production build,
locked all-target check/Clippy and scoped rustfmt. Only the five existing
Clippy warnings remain. Nine new regressions/four assertion-caught mutants
passed; original source was restored exactly before full verification. The
independent fixture regenerates byte-identically; seven new local links and
diff checks pass. No timeout, statistical policy or AI boundary changed.

The containing PR records fresh-main rebase/relevant checks, six exact-head
CI jobs, fresh base/review audit, guarded merge and clean-main tree receipt.
Strict original v5 input/source validation and atomic union is Next, immediately
after this delivery; the user explicitly requested the PR/merge/task loop.
