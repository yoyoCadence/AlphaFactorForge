# Handoff: P13a alpha reservation evidence/history

Date: 2026-10-15
Repo: yoyoCadence/AlphaFactorForge
Branch: feat/p13a-alpha-reservation-history
Baseline: `4c04b5a0e169b85785779a610d330532cbfb7ad6` (merged PR #180)
Status: Merged as PR #181 after six green final-head CI jobs (see Resolution)

## Scope and authority

The maintainer authorized continued development, PR publication and merging.
P12's closure audit staged this independently verifiable P13a task in Next;
it moved to In Progress before source edits. This applies the already adopted
2026-10-01 alpha-allocation §9.1 decisions, without settling the still-open
policy for confirmation outside P12e's exact calibrated set.

The bounded plan is pure record integrity and history review: verify identity
and family/declaration/number/share bindings, deduplicate exact replay, block
independent records competing for a confirmation number, and project amounts
only after a contiguous history is validated. It introduces no registry schema,
runtime caller, command, UI, data exposure or statistical PASS.

## Implementation

[`research-alpha-reservations-v1`](../docs/research-alpha-reservations-v1.md)
defines the record and review contracts. The Rust core owns strict text
parsers, a checked opaque record and a hash over every body field. The body
includes a source registry ID and stable operation key so independent
reservations of the same batch/number remain distinct. Reassigning the same
source operation to another body is also a conflict. These are integrity
identifiers, never authenticated authority or evidence that alpha was spent.

The review preserves every distinct checked record and stable conflict groups.
Conflicts return BLOCKED and no allocation; exact replay does not spend twice;
consistent exhaustion returns the existing allocator's NOT_ELIGIBLE report.
Wrong family/declaration/share or a gap fails closed. Failed and unfinished
reservations cannot be removed by a status filter because this input has no
such filter. A deliberately truncated valid history is tested as a limitation:
the future registry must prove completeness and synchronize reservation.

The hand-authored synthetic golden record's ID was computed using the existing
TypeScript canonical encoder and SHA-256, before running the new Rust module.
Rust reproduces it and Vitest independently guards the committed expected ID
`3d49ae12d1965c2dfc10364c94bf2018326e162569ceea473c098508654ff0af`.
No production TypeScript path imports the new module or changes numeric policy.

## Verification

The ten new Rust cases pass, including the original equal-share/duplicate
confirmation-1 counterexample, unequal schedules, replay, source operation
conflicts, arrival-order independence, exhaustion, missing/mismatched evidence,
every hash-covered field, duplicate/unknown/missing keys and integer bounds.
Full verification: **624 Rust (180 library + 442 desktop + 2 service), 1
existing ignored; 1184 Vitest / 73 files**. Typecheck, production build,
`cargo check --locked --all-targets` and targeted rustfmt pass. Three executable
mutants (conflict threshold raised, body omitted from hash, prefix gaps allowed)
are caught by failing tests; the original source is restored and all ten new
cases pass again. Clippy passes with only the five existing warnings in
unchanged modules; two new test-only clone warnings were removed and the ten
cases rechecked. Final-head CI results follow. No rendered UI changed; the PR
CI runs existing E2E and native smoke.

## Remaining work and risks

P13 is not complete. Next is a bounded **P13b registry persistence design**:
external append-only budget/reservation/conflict storage, binding rollback
protection and transfer union, with explicit crash/atomicity acceptance before
schema or runtime edits. Subsequent slices own synchronized ledger/precision
reservation, frozen batches/selection, exposure identity and overlap,
recovery/execution, prompt boundaries and UI/native acceptance.

The pure reviewer cannot detect omitted history, authenticate an origin, prove
frozen batch contents, reserve alpha or keep conflicts permanent across a
caller that deletes records. Those are explicit future registry obligations.
The calibration plan's unsupported-configuration questions stay open. No
statistic, acceptance seed, sample count, threshold or historical result changed.

## Resolution — 2026-10-15, CI and merge

[PR #181](https://github.com/yoyoCadence/AlphaFactorForge/pull/181) final head
`86ddf606599c6c1e3ea3b2272bccbd519085ddb6` passed all six jobs in
[CI run 38735635919](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/38735635919):
624 Rust (180 + 442 + 2, 1 ignored), 1184 Vitest and 98 E2E, plus native
smoke. No review threads were open. Merge
`5aef090ac181af8478661576973443ddae7ac5e5` has an identical tree; local main
fast-forwarded cleanly. The [delivery receipt](https://github.com/yoyoCadence/AlphaFactorForge/pull/181#issuecomment-6192284099)
retains the checked scope and limitations. P13b's
[persistence plan](../docs/plans/research-reservation-registry-plan-v1.md)
is the next bounded task.
