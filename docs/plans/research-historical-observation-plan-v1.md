# Original historical trial observations: implementation plan

Date: 2026-10-17
Parent: P13c-3b-1b, after merged codec #195 (`9d05e9b`)

## Problem and bounded slice

Checked confirmation storage needs independently supplied historical source,
prefix and trial-count observations. Workspace reopen loses caller context;
trial union can skip conflicting incoming family rows/checkpoints. Current
receiver counts can also exceed the original count. A receipt cannot witness
its own numbers, and requiring the original branch to become primary loses
restrictive history.

Before designing persistence, implement one read-only backend primitive from a
complete original trial v3 snapshot plus the existing immutable v5 binding and
typed registered source. It returns historical observations, never the current
admission count/capability. It performs no IO, locking, SQL mutation, import,
binding write, provider call or execution. Existing archive versions/schema and
runtime reader/writer remain unchanged.

## Validation and derivation

1. Bound original archive bytes to 8 MiB before decoding; reject overflow,
   never truncate. Reuse the complete v3 header/body/record/hash/chain/member/
   protocol parser. Wrap failures in static diagnostics without rejected data.
2. Reuse full event-kind/payload validation, with reproduction references
   resolved exclusively inside that source archive. Refactor the existing
   import validator into a shared resolver-based helper: imports retain their
   existing receiver fallback; historical reconstruction supplies no fallback.
   Keep the existing refusal of nonportable exempt-benchmark provenance.
3. Match original registry ID, exact sequence and chain head to the strict v5
   binding. Derive source trial restrictive evidence from source rows:
   quarantined families, conflicted origins and all pinned/upgraded protocol
   high-water marks must exactly match the binding. No receiver-primary lookup.
4. Reuse the pure registered-source domain/order checks (make that existing
   validator available to this backend). Match the named actual batch/family,
   complete candidate-index/strategy-hash membership and workspace from its
   source events. A subset, duplicate, wrong index/hash/workspace/family or a
   missing batch fails, even when lengths match. Artifact/campaign/run/engine
   metadata not recorded by the trial schema remain supplied historical claims;
   this primitive must not claim to authenticate them or establish file presence.
5. Count effective source events in the entire actual family and original
   batch, derive target tests-per-trial from the source pinned/upgraded protocol,
   and derive complete-candidate count from checked source membership. Construct
   `HistoricalTrialObservation` with that original prefix. Ignore audit receipt
   family-before totals as authority. Do not recompute using current receiver
   growth, reinterpret Test subsets as a new original source, or mint freshness.
6. Return immutable access to the original archive SHA, checked supplied source,
   prefix and recomputed historical observation. These are archival facts against
   that supplied snapshot, not publisher authentication, unseen-data proof or an
   actual current registry transaction permit.

## Verification and limits

Use real registered batches/protocols and source snapshots; demonstrate original
counts after receiver growth, independent/conflicting source branches without
replacement of receiver primary rows, source protocol upgrades with unchanged
event heads, complete membership failures, every prefix/evidence mismatch,
closed reproduction, kind/provenance refusal, corrupted/rehashed input and
original byte boundaries. Audit receipt family-before changes must not change
derived counts. Static failures must not disclose protected sentinels. Existing
import resolver behavior, all tests/checks and no-new-Clippy-warning discipline
must remain intact; executable mutations must fail by assertions and restore.

This proves that an original snapshot can supply independent trial observations;
it does not persist that snapshot or define a new witness blob. P13c-3b-1b-2
must freeze the durable format, bounds, graph dependencies, migration/legacy
handling and complete transfer before implementing storage. Existing public
v3/v4 omission/provenance guards stay active; using the private trial projection
as a proof input is never a known-empty alpha/confirmation assertion.

Whole-registry observation/adoption and actual old-build refusal remain
3b-1b-3. P13c-4 still owns authentic complete source/owner/current count and
actual-share precision inside the final synchronized transaction. No statistical
policy, bootstrap, calibration or reserved final seed changes.
