# Handoff: P06 acceptance review — four reproducible gaps

Date: 2026-09-20
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p06-market-foundation`
Reviewed commit: `aac5a77` (implementation `838690f`, baseline `a0f738e`)
PR: #108 (reference from the implementation handoff; remote state not rechecked)
Status: Resolved — all four findings fixed and regression-tested; included in the P06 PR #108 update

## Summary

P06's baseline suites pass, but four focused counterexamples fail. Three allow a snapshot to carry
`status = ok` despite missing provenance, future availability, or changed candle content; the fourth
breaks revision retry idempotency. These are defects in the new foundation APIs, even though their
production consumers are scheduled for later phases.

This review makes no implementation fix and does not advance to P07. The temporary diagnostic tests
were removed after execution; their exact bodies are preserved below.

## Required Action / Decision

1. **P1 — Require source evidence before admitting a snapshot.**
   `alpha-factor-forge/src-tauri/src/market/snapshot.rs:302` accepts an empty
   `provenance_ids` list, and `component_events` only checks elements that exist.
   With the existing valid BTC fixture and `request(1, vec![])`, the result is
   `Created(status = Ok)`, has no snapshot sources, and is qualification eligible.
   Reject or record a blocking event for an empty source set before admission. Cover both historical
   and forward-observed requests, including the guarantee that no snapshot row is inserted.

2. **P1 — Compare forward-observed availability with the snapshot cut.**
   `alpha-factor-forge/src-tauri/src/market/snapshot.rs:412` checks only whether
   `available_at` is present. The existing observation has availability on July 31 and retrieval on
   August 1, but a forward-observed snapshot with an as-of of July 15 04:00 UTC is accepted as
   `Created(status = Ok)`. Thus evidence unavailable at the cut can be labeled forward-observed.
   Parse and enforce the applicable availability/as-of relation before admission; an unknown time or
   a time after the cut must fail closed. Add before/equal/after-cut cases. This finding does not
   require implementing the future paper scheduler.

3. **P1 — Verify the actual candle payload at snapshot admission.**
   `alpha-factor-forge/src-tauri/src/market/snapshot.rs:191` loads metadata and line 198
   loads only timestamps; the snapshot copies `dataset_hash` without verifying OHLCV against it.
   After normal import, changing close from 100.5 to the still-plausible 100.25 leaves every timestamp
   unchanged and still produces `Created(status = Ok)` under the old dataset hash.
   SQLite does not make the old candle table immutable. Reuse
   `identity::verify_dataset_identity` and the existing candle plausibility gate on the complete
   stored payload before recording a snapshot (including the Existing path).
   The existing discovery runner already checks stored payloads this way; no hash-contract change is needed.
   Add hash mismatch, invalid candle, and metadata/count mismatch regression cases.

4. **P2 — Make an identical revision retry idempotent.**
   `alpha-factor-forge/src-tauri/src/market/provenance.rs:198` rejects an already-revised
   target before the `record_hash` lookup at line 207. After recording A and revision B,
   repeating the exact B observation and bytes returns
   `revision target 1 is already revised by record 2`, instead of `(B, false)`.
   Resolve an identical existing observation before rejecting a genuinely different child/fork.
   Verify retry creates no new provenance row and a different revision of A still fails.

## Review Notes

- The branch and implementation scope match the supplied completion report; the initial worktree was clean.
- The UTC daily-bar convention, absent real ETF calendars, lack of network downloads, and deferred
  Tauri/UI/discovery consumers are explicitly documented phase boundaries. They are not counted as defects here.
- Shared authored fixtures and immutable new-table triggers are present.
- No claim is made that current user-facing backtests already exercise the new faulty APIs.
- Do not treat the implementation's Done declaration as acceptance until the four findings are resolved.

## Verification

Baseline, before inserting temporary tests:

- `npm.cmd test`: 52 files, **909 passed**.
- `npm.cmd run typecheck`: passed.
- `npm.cmd run build`: passed.
- `cargo test --locked`: **297 passed** (68 lib + 227 desktop binary + 2 service smoke).
- `cargo check --locked --all-targets`: passed, no warnings emitted.
- Rust commands used `CARGO_TARGET_DIR=C:/tmp/aff-target`.

Focused reproduction:

- Temporarily inserted the four tests below in the existing `snapshot.rs` `tests` module,
  immediately before `fn codes`, reusing its normal-import helpers.
- Ran `cargo test --locked --bin alpha-factor-forge market::snapshot::tests::review_ -- --nocapture`.
- **0 passed, 4 failed**, each at its expected admission/idempotency assertion.
- Removed only the temporary test block afterward; source diff returned to empty.
- Playwright, clippy, native Tauri UI, and remote PR/CI state were not rechecked in this review.

## Exact diagnostic tests

These assertions describe the required behavior and fail on the reviewed commit (before the Resolution below).
Place them inside the existing `market::snapshot::tests` module to reproduce:

```rust
#[test]
    fn review_empty_provenance_must_not_qualify() {
        let mut conn = memory_db();
        let outcome = build_snapshot(&mut conn, &request(1, vec![])).unwrap();
        assert!(matches!(outcome, SnapshotOutcome::Blocked { .. }),
            "missing provenance was admitted: {outcome:?}");
    }

    #[test]
    fn review_future_availability_must_not_qualify_as_forward_observed() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        // observation availability is July 31; request as-of is July 15.
        let raw = record(&conn, &store, &observation(), b"bars");
        let candidate = SnapshotRequest {
            kind: SnapshotKind::ForwardObserved,
            ..request(1, vec![raw])
        };
        let outcome = build_snapshot(&mut conn, &candidate).unwrap();
        assert!(matches!(outcome, SnapshotOutcome::Blocked { .. }),
            "future availability was admitted: {outcome:?}");
    }

    #[test]
    fn review_changed_candle_content_must_not_acquire_a_snapshot() {
        let mut conn = memory_db();
        let (store, _guard) = fresh_store();
        let raw = record(&conn, &store, &observation(), b"bars");
        // Still plausible OHLC, but no longer the dataset-content-v2 payload.
        conn.execute("UPDATE candles SET close = 100.25 WHERE dataset_id = 1", []).unwrap();
        let outcome = build_snapshot(&mut conn, &request(1, vec![raw]));
        assert!(matches!(outcome, Err(_) | Ok(SnapshotOutcome::Blocked { .. })),
            "changed candle content was admitted: {outcome:?}");
    }

    #[test]
    fn review_revision_retry_must_return_the_existing_record() {
        let conn = memory_db();
        let (store, _guard) = fresh_store();
        let first = record(&conn, &store, &observation(), b"original");
        let revision = RawObservation {
            retrieved_at: "2024-08-05T00:00:00Z".into(),
            revision_of: Some(first),
            ..observation()
        };
        let second = record(&conn, &store, &revision, b"revised");
        let retried = provenance::record_raw(&conn, &store, &revision, b"revised");
        assert!(matches!(retried, Ok((id, false)) if id == second),
            "identical revision retry failed: {retried:?}");
    }
```

## Resolution

2026-09-20, local changes on `feat/p06-market-foundation`, based on `64e72d7`.
The fixes are recorded in the commit containing this Resolution; local verification below is separate from PR CI.

- **Finding 1:** an empty source list now records `missing_source` / `refetch_range` and
  returns Blocked. Historical, forward-observed, and demo requests are covered; no snapshot or
  source-link row is inserted.
- **Finding 2:** forward-observed sources require a parseable availability instant at or before
  the cut. Future availability records `availability_after_cut`; absent/unparseable availability
  uses `availability_unknown`. Comparisons preserve submillisecond precision and account for
  timezone offsets. Tests cover unknown/before/equal/after, a one-microsecond excess, and an
  existing snapshot requested with an earlier cut.
- **Finding 3:** snapshot admission reads all stored OHLCV, including rows outside declared bounds,
  and reuses identity verification followed by the existing plausibility gate before any write or
  Existing return. Tests cover modified prices, count/bounds, an extra row outside the bounds,
  and invalid negative volume with a matching hash. Failed checks leave snapshot/source/event
  counts unchanged and never repair stored candles.
- **Finding 4:** provenance identity is calculated before the fork check. Identical retries return
  the original id and still verify/store the raw artifact; different content or observation metadata
  remains a rejected fork. A retry of a revision that itself has since been revised is also idempotent.

Five permanent regression tests were added to `market/snapshot.rs` and `market/provenance.rs`.
Contract docs now describe the two additional storage event codes and the fifth use of the existing
candle-quality gate. No migration, hash preimage, dependency, or frontend path changed.

Verification after the fixes:

- Focused market tests: **24 passed**.
- `cargo test --locked --quiet`: **302 passed** (68 lib + 232 desktop + 2 service smoke).
- `cargo check --locked --all-targets`: passed, no warnings.
- `cargo clippy --locked --all-targets`: only the existing 4 core warnings and 1 file-command warning.
- `npm.cmd test -- --reporter=dot`: **909 passed**; typecheck and build passed.
- `git diff --check`: passed.
- Playwright and native UI were not rerun: the fixes affect backend admission/storage only.

The four reported acceptance gaps are closed locally. These checks do not establish real-market
coverage or implement the deferred snapshot consumers. Publication was authorized on 2026-09-20;
the scoped changes belong to existing draft PR #108. Review PR CI for the updated head before merge.
