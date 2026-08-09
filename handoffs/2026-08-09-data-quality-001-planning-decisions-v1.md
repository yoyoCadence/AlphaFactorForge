# Handoff: DATA-QUALITY-001 planning decisions

Date: 2026-08-09
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/data-quality-001-handoff
PR: pending
Status: implemented in PR #94; the rule-1 structural exception below is adjudicated and accepted

## Summary

This handoff records the agreed planning direction for `DATA-QUALITY-001` before another agent writes the detailed backlog specification or changes code. The task must add matching TypeScript/Rust market-data admission validation and fail closed when a runner encounters invalid data that was stored before the new admission rules existed.

## Required Action / Decision

### 1. Use an explicit product admission range for epoch-millisecond timestamps

Accept timestamps only inside this half-open UTC interval:

```text
2000-01-01T00:00:00Z <= timestamp < 2100-01-01T00:00:00Z
```

Shared decimal constants:

```text
MIN_MARKET_TIMESTAMP_MS = 946_684_800_000
MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE = 4_102_444_800_000
```

This is a product plausibility boundary, not the TypeScript or Rust language limit. The implementation must also independently require a finite integer and successful date representation in both runtimes. The lower bound rejects common epoch-seconds-as-milliseconds mistakes; the upper bound rejects microsecond/nanosecond values and implausible future data. Use the same inclusive-lower/exclusive-upper semantics, constants, and boundary fixtures in TypeScript and Rust.

### 2. Route the existing chrono/JavaScript-safe timestamp regression through stored-invalid-data coverage

Repurpose the existing `chrono_invalid_js_safe_timestamp` test so it writes an invalid candle directly to SQLite, bypassing the new import admission path. This simulates legacy or externally corrupted data. Assert that `load_verified_dataset` fails closed before execution and does not improperly advance or mutate run state, jobs, progress, emitted events, or coordinator controls.

This does not replace admission coverage. Add a separate table-driven atomic-import mutation case for the timestamp invariant so a newly imported invalid dataset is rejected and any previously committed dataset remains unchanged. Together the two tests prove both boundaries:

```text
new invalid input -> import admission rejects atomically
already-stored invalid input -> runner load rejects fail closed
```

Every other `DATA-QUALITY-001` invariant must receive equivalent TypeScript/Rust parity fixtures and atomic-import mutation coverage: positive OHLC prices, non-negative volume, and `low <= open/close <= high`.

### 3. Keep the next session in planning mode until the specification is approved

The next planning agent should first refine the existing `DATA-QUALITY-001` entry in `docs/improvement-backlog.md` with the exact invariants, error semantics, TS/Rust ownership, test matrix, atomicity expectations, and runner fail-closed behavior. Stop after the tracked specification is complete and request confirmation.

After approval, a separate coding agent should start from the latest `origin/main`, move the task to `In Progress`, implement it on a focused branch, run the full relevant verification suite, and open a Chinese draft PR. Do not combine planning, implementation, and review into the same session.

## Scope Guardrails

- Preserve the durable dataset hash definition; validation must not silently redefine dataset identity.
- Do not change the intentionally accepted unknown-interval fallback. That remains the separate `INTERVAL-CONTRACT-001` decision.
- Do not bundle `BUG-SWEEP-CONTEXT-001`, `STRATEGY-VALIDATION-001`, or `RUNNER-UI-001` into this task.
- Reject invalid imports atomically and prove that an existing valid dataset remains intact after every mutation case.
- Keep frontend/core validation pure TypeScript and backend validation in Rust; match observable acceptance and error classification across both sides.

## Recommended Follow-up Order

1. Finalize and approve the `DATA-QUALITY-001` backlog specification.
2. Implement and merge `DATA-QUALITY-001`.
3. Continue with `BUG-SWEEP-CONTEXT-001`.
4. Continue with `STRATEGY-VALIDATION-001`.
5. Start `RUNNER-UI-001` only after all listed blockers have landed.

## Verification

Documentation-only decision record. No application code or runtime behavior was changed in this handoff.

## Resolution (added when acted on)

The backlog specification was written on 2026-08-09 and is appended to
`docs/improvement-backlog.md` as `DATA-QUALITY-001`, implementing all three
decisions above: the adjudicated admission constants, the repurposed
stored-invalid-data regression, and the planning/implementation split. No
application code was changed.

Two conditions found while grounding the specification, both recorded in it:

1. The decision text refers to refining an "existing `DATA-QUALITY-001` entry"
   in `docs/improvement-backlog.md`. No such entry existed — the task lived only
   in `tasks.md` and the audit handoff — so the specification was added as a new
   section in the audit addendum, in the adjudicated execution order after
   `METRIC-002`.
2. `load_verified_dataset` is called at `discovery_runner/mod.rs:348` **before**
   any run row is inserted, so the repurposed regression cannot assert a
   persisted failed run as the current test does. `start` returns `Err` and the
   correct fail-closed assertion is that no run row, job, progress record, event,
   or coordinator control was written at all.

Still pending: maintainer approval of the specification, after which a separate
coding-agent session implements it from the latest `origin/main`.

## Implementation Resolution (2026-08-09, PR #94)

The specification was approved and merged as PR #93, and implemented on
`fix/market-data-admission-validation` as PR #94 — commit
`fix(data): validate market data at dataset admission`. All three planning
decisions above were implemented as adjudicated: the admission range constants
were used verbatim, the chrono regression was repurposed into stored-invalid-data
coverage, and planning, implementation, and review stayed in separate sessions.

### Adjudicated exception — rule 1 is unreachable at the import mount point

**Decision: accept the implementation as written. Do not reorder the validation
sequence.** Maintainer adjudication, 2026-08-09.

`timestamp_not_integer` (rule 1) cannot be reported by the quality gate at
`repositories.rs` `import_dataset_with_candles`, because:

- `db::Candle.timestamp` is an `i64`, so a non-integral timestamp cannot exist
  at that call site; and
- `identity::normalize_dataset_candles` / `verify_dataset_identity` already
  reject non-JavaScript-safe-integer timestamps *before* the quality gate runs.
  The TypeScript side is identical: `normalizeDatasetCandles` runs its own
  `Number.isSafeInteger` check first.

The specification's acceptance criterion — "every reachable rule id (1, 2, 4–7)
has an atomic-import mutation case … the call returns `Err` naming that rule" —
had assumed rule 1 was reachable there. It is not.

The exception is accepted on these grounds:

1. **No safety gap and no write path.** Invalid data still fails closed at that
   mount point; only the *identity* of the rejecting rule differs. Nothing is
   admitted that would otherwise be rejected, and no row is written.
2. **The atomicity guarantee is still proven.** The rule-1 row stays in the
   table-driven mutation test and asserts all three required properties:
   rejection, zero rows written, and the previously imported dataset still
   byte-identical. Only the rule-id assertion is replaced with the rejection
   actually observed, carried explicitly as
   `RejectedBy::IdentitySafeInteger` rather than left implicit.
3. **Rule-id classification is owned elsewhere.** Rule 1 remains fully reachable
   and covered through the validator's own API, where timestamps arrive as
   arbitrary numbers: the shared parity fixture
   (`market-data-quality-v1.json`) carries four rule-1 rejection rows —
   fractional, `2^53`, NaN, and infinite — and both runtimes are asserted to
   classify each identically. Atomic-import mutation cases prove *atomicity*;
   the validator and parity tests prove *classification*. Splitting the two
   concerns is the correct division, not a concession.
4. **Reordering would cost more than it buys.** Making the quality gate report
   rule 1 at import would require running it *before*
   `verify_dataset_identity`, which would: invert the deliberate error
   precedence (a tampered payload should report its identity mismatch first,
   which is exactly why mount point 4 is specified as identity-first); change
   the reported candle index from normalized order to raw input order; and blur
   the identity/quality boundary this task exists to keep clean. The
   asymmetry between mount points 3 and 4 would be unjustifiable.

`tasks.md` and `docs/market-data-quality-contract.md` §5 already describe this
condition; those descriptions stand as written and need no revision.

### Second implementation finding — accepted with no further change

`atomic_dataset_import_rolls_back_dataset_when_a_candle_write_fails` bound its
injected SQL trigger to `NEW.timestamp = 2`. Once step 8 moved
`identity_candles()` into the admissible range, that trigger would no longer
fire, while the new admission gate would still satisfy the test's `is_err()`
assertion — the test would have kept passing for the wrong reason. The trigger
was moved to the new second candle's timestamp and the test now asserts the
error contains `injected candle failure`, proving the rollback comes from the
in-transaction candle write rather than from pre-transaction admission.

**Decision: accepted as implemented; no further change required.**

### Verification

703 Vitest + 145 Rust + 50 Playwright E2E, typecheck, production build,
`cargo check --locked`, `cargo test --locked`, and
`cargo clippy --locked --all-targets` with only the four pre-existing
`backtest.rs` / `score.rs` warnings. Fixture regenerated twice with matching
SHA-256 digests. `src/core/hashing/index.ts`, `src-tauri/src/identity.rs`,
`identity-v2.fixture.json`, `src-tauri/migrations/`, `e2e/`, and
`package-lock.json` are all absent from the diff, and `identity.rs`'s timestamp
`1`/`2` hashing tests pass unmodified.

`INTERVAL-CONTRACT-001` remains an open, separate decision.
`BUG-SWEEP-CONTEXT-001` is the next task.
