# Handoff: DATA-QUALITY-001 planning decisions

Date: 2026-08-09
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/data-quality-001-handoff
PR: pending
Status: ready for backlog specification; implementation not started

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

Pending backlog specification and user approval.
