# Handoff: PR #140 frozen v2 and final declarations acceptance review

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e7-final-acceptance`
PR: [#140](https://github.com/yoyoCadence/AlphaFactorForge/pull/140)
Reviewed head: `759ecde26afc39d45f355f79b85985314b08374f`
Base: `e9e73e818aae65715c836d9c003055d53799fba5` (merged #139)
Status: P12e-7a accepted; no merge-blocking finding. The frozen statistic remains NOT YET ACCEPTED statistically, with no supported configuration. Final seed 20261117 was not executed, including prefixes. P12e-FINDING-1 remains open and P13 blocked. PR remains draft/unmerged by this review.

## Summary

The wrapper preserves the selected S2 computation and adds the declared R3
constraint for every candidate. The common parser still enforces each
contract's own name and v1's field/rejection rules. The engine dispatches v2
to that wrapper without changing data streams or confirmation seeds.
Exactly six null-scenario final declarations match plan sections 3 and 7.
This accepts the freeze/declaration slice, not the unexecuted calibration.

## Answers to the screenshot's three questions

### 1. Final method's power — accept reuse, explicitly as diagnostic evidence

Accept the proposed reading of plan section 5: the already recorded S2-R3
power is also the frozen method's **diagnostic power**, since v2 runs the
same computation with the same R3 lengths. Do not add power cells to the
fixed six-cell final acceptance in section 7.

The existing first-confirmation net-return rejection counts are:

| Bars | phi = 0 | phi = 0.3 | Simulations | Seed |
| --- | --- | --- | --- | --- |
| 512 | 3067 | 3116 | 4000 | 20261005 |
| 1024 | 3083 | 3118 | 4000 | 20261005 |

Keep this provenance visible in the final-method record. These are the
same data used to choose the method; they are not an independent post-
selection power estimate, not 20,000-simulation final-seed results, and not
evidence that the one-count ranking difference establishes superior power.
Power at 256 bars was not measured and must stay marked as unmeasured,
even if final size acceptance later supports that length. A future
independent power study or a new 256-bar power run requires its own prior
declaration; it must not silently enlarge section 7.

### 2. Shared S2 code — accept, with the shared path treated as frozen

Sharing `confirmation_candidates::bootstrap_s2` is reasonable and ensures
the method frozen here is the method that was measured. There is no reason
to copy the algorithm in this PR. From this freeze onward, that shared code
path is part of v2's immutable statistical definition, despite its location
in a file named `confirmation_candidates`.

New S2 experiments should use a separate named path/version. A numerical or
statistical behavior change in the shared path, comparison, normalization,
constant rule or block rule changes v2 too; use a new contract version and
the plan's amendment/redeclaration process. The old fixtures and diagnostic
reports are evidence to preserve, not expected values to regenerate so an
unversioned change passes. Sample-based fixture/prefix equality is useful
regression protection; equality of two wrappers sharing one function alone
does not prove historical immutability.

### 3. Length holes — confirmed; input admissibility is separate from support

The length restriction is mathematically consistent. The intersection of
`L = round(cuberoot(n))` and `4L^2 <= n` gives **36-42, 64-91, and n >= 100**.
For example, R3 is 5 at 92-99 bars and requires at least 100 bars for its
twice-wide block. No change to R3 or the wider-block condition is needed.

An independent integer-interval calculation verifies those small ranges.
For every L >= 6, `(2L-1)^3 - 32L^2` is positive: it is 179 at L=6 and its
increment is `24L^2 - 64L - 30 > 0`, so every integer n in the corresponding
R3 interval satisfies the wider-block condition. The Rust tests also cover
the actual evaluators up to 400 bars and large R3 boundaries.

P13 should distinguish a length that the algorithm refuses from a legal
length without calibration evidence. **100 is not a calibration minimum**;
neither 100 bars nor arbitrary larger lengths become supported by this
freeze. Even after final acceptance, support comes only from the tested
configuration set in plan section 7. Do not silently select a different L,
pad the input, or extrapolate calibration to fill a hole. Behavior at legal
but untested lengths remains the separate P13 decision in plan section 11.

## Verification

- `cargo test --locked --lib`: **168 passed, 1 ignored**, 42.49 s excluding
  compilation. Includes v1 regressions, v2/S2 parity, R3 boundary and mixed-
  candidate checks, all existing diagnostic prefixes, and the six selected
  S2-R3 diagnostic prefixes replayed through v2.
- `npm test`: final full rerun **1072 passed across 64 files**; typecheck
  passes. The initial run, concurrent with Rust compilation, had **1071
  passes and one 5-second timeout** in the exhaustive BigInt R3 scan (11.263
  s), with no numerical assertion mismatch. The unchanged failed file
  passed in isolation (scan 483 ms), and the unchanged entire suite passed
  after Rust finished (scan 750 ms). No timeout or source setting was
  changed. This is consistent with contention during compilation and is
  recorded as a non-blocking local test observation.
- An independent reconstruction verifies every field of the six final
  declarations and the exact Wilson maxima **552 / 1134** (the next counts
  fail). It parses declarations and integer inequalities only, without
  simulating them.
- A recursive scan of every JSON file under `fixtures/` finds final seed
  20261117 only in those six declared seed fields. The acceptance fixture
  has no reports. Inspection of the Rust/TS tests confirms final declarations
  are parsed/checked only, not passed to the simulator.
- Seven old engine fixture cases are unchanged. Existing v1, draft S2 and
  diagnostic fixtures/reports are unchanged; the cited diagnostic power
  counts and their 4000/20261005 provenance were independently checked.
- Changed Rust files pass `rustfmt --check`; the PR range passes
  `git diff --check`.
- All six exact-head CI jobs pass in
  [run 37126987054](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37126987054):
  test, typecheck, build, cargo-check, e2e and native-smoke. Logs confirm
  **566 Rust (168 + 396 + 2), 1 ignored**, and **1072 Vitest**. CI library
  runtime is **16.43 s**; desktop 43.55 s, service 5.67 s.

Full diagnostics, mutation checks, local desktop/service tests, build and
Playwright were not repeated. No unexecuted final declaration or final
prefix was simulated. The capability-registry historical wording follow-up
from PR #139 is resolved in this PR.

## Required action / next step

After this head is merged, proceed with **P12e-7b** on exactly the six
committed declarations, using the release runner once on seed 20261117.
Commit unedited reports, then add their declared prefix checks; record all
three checks per cell, point estimates and Wilson intervals, the supported
tested configurations and measured run/CI timing. Keep the existing failed
v1 record. Close or retain the finding according to section 7; P13 remains
blocked until that acceptance records a supported tested set.

Only this review handoff and `tasks.md` are updated locally. No product
change, commit, push, GitHub review submission, draft conversion, merge or
final-seed execution is performed by this review. NUMERIC-JSON-001 remains
separate and open.
