# Handoff: P12e-3 noise false-positive simulation, plus the host-test startup wait

Date: 2026-10-02
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e3-noise-simulation` (from merged PR #134, `b5a3438`)
PR: [#135](https://github.com/yoyoCadence/AlphaFactorForge/pull/135) (merged as `1764c48`)
Status: Merged (2026-10-02) after the acceptance-review R1 and R2 fixes (see Resolutions). **One of the two declared acceptance runs exceeds its tolerance; the maintainer decided on 2026-10-02 to revise the statistic, starting with a calibration plan (P12e-4). P12e stays open and P13 stays blocked.**

## Summary

Two things in one PR, as the maintainer asked on 2026-10-02:

1. **P12e-3** — `research-noise-simulation-v1`
   ([contract](../docs/research-noise-simulation-v1.md)): a pure, seeded
   simulation that runs a trial family's whole confirmation life on noise
   with no edge, using the production P12e-1 statistics and P12e-2 allocator
   unchanged, and compares the family false-positive rate with a tolerance
   declared beforehand. No runtime caller; never a `PASS`.
2. **CI-HOST-STARTUP-001** — the host tests' wait for an in-process service
   to publish is now its own finite limit and explains a miss.

PR #134 (P12e-2 and its review fix) was merged as `b5a3438`.

## Result of the declared acceptance runs

Declared and committed first (`a0cd10a`), then run once: 2,000 simulations,
two confirmations of 2.5% each (nominal 5%), 256 bars, block length 6, 799
bootstrap samples, tolerance one percentage point, seed 20261002.

| Run | Family false positives | Limit | Status |
| --- | --- | --- | --- |
| independent noise | 97 / 2000 = 4.85% | 120 | `WITHIN_TOLERANCE` |
| AR(1) 0.3 noise | 129 / 2000 = 6.45% | 120 | **`EXCEEDS_TOLERANCE`** |

The correlated run is 1.45 points (about three standard errors) above
nominal. Even on independent noise the first confirmation alone rejected in
3.05% of simulations against a 2.5% share. So the P12e-1 bootstrap test
rejects true nulls somewhat too often at this sample size, and clearly too
often under moderate autocorrelation. Nothing was changed after seeing this:
seed, sizes, block length and tolerance are the committed ones.

### Exploratory, after the fact — NOT acceptance

To give the decision something to stand on, one labelled diagnostic was run
with a different seed (1001), same sizes, varying only the block length. It
is not in the fixture or the tests and changes nothing above.

| Block length | Independent | AR(1) 0.3 |
| --- | --- | --- |
| 6 | 99 (4.95%) | 131 (6.55%) |
| 11 | 112 (5.60%) | 125 (6.25%) |
| 16 (= √256, the contract's maximum) | 123 (6.15%) | 133 (6.65%) |

Reading, with the caution that these are single draws with a standard error
near half a point: the second seed agrees with the declared runs, and a
longer block does **not** repair the correlated case — it only makes the
independent case worse (fewer blocks per resample). The cause is therefore
unlikely to be the block-length choice alone; it looks like the unstudentized
centered statistic at a few hundred bars and small tail probabilities.

## Decisions

- **Maintainer decisions of 2026-10-02:** noise = independent + AR(1) 0.3;
  acceptance = nominal + one percentage point over 2,000 simulations; the
  host-test wait fix ships in this PR rather than a separate one.
- **Design choices made in this slice — please confirm in review:**
  1. **Nominal = the schedule's sum**, the alpha actually handed out (equal
     to the total in the acceptance runs).
  2. **The family rate gates; per-confirmation rates are reported only.**
  3. **Noise innovations are sums of four uniforms** — exactly reproducible
     across Rust and TypeScript (no `ln`/`cos`), but light-tailed and
     homoscedastic, i.e. easier than real returns (contract §9).
  4. **Returns and benchmark are independent series**, so both nulls are true
     at their boundary.
  5. **The acceptance configuration** (contract §7) was fixed from a timing
     measurement that printed no outcomes: smallest family (one candidate,
     no prior trials) as the least conservative setting; block length
     `round(256^(1/3)) = 6`; 799 samples so no Holm threshold is rounded by
     the p-value grid.
  6. **Statuses are `WITHIN_TOLERANCE` / `EXCEEDS_TOLERANCE`**, deliberately
     not `PASS`/`REJECT`.
  7. **`SplitMix64` and `mix64` in `confirmation.rs` became `pub(super)`** so
     the simulation keys its streams the same way; no behaviour change.
  8. **Acceptance reports come from Rust**, because the BigInt reference is
     too slow at this size; small cases are cross-checked in full and the
     Vitest re-derives every derived field of the acceptance reports from
     their counts.

## What changed

- `discovery_core::noise_simulation` (new, pure) with tests in
  `noise_simulation/tests.rs`; `discovery_core::confirmation` visibility only.
- Fixture `fixtures/rs-core/research-noise-simulation-v1.json`: four small
  cases (reference-produced) and the two acceptance runs.
- Reference `src/parity/noiseSimulationFixture.ts`, guarding Vitest,
  generator `scripts/generate-noise-simulation-fixtures.ts`,
  `npm run fixtures:noise-simulation`.
- `discovery_runner/tests/host.rs`: `published_service` / `await_publication`
  replace `wait_for_published` at its four call sites; three tests of the
  helper.
- Docs: the new contract; result pointers in
  `research-confirmation-statistics-v1.md` §9/§10,
  `research-alpha-allocation-v1.md` §11 and the capability registry; task
  board; Resolutions on the P12e-2 handoff and the PR #134 review handoff.

## CI-HOST-STARTUP-001

- The startup wait is `AFF_TEST_SERVICE_STARTUP_SECS` (default 30 s) instead
  of the shared 10 s `TEST_TIMEOUT`; every other wait and every production
  timeout is unchanged, and there is no retry.
- A service that exits before publishing is joined and reported at once with
  its own error. At the deadline the message lists the workspace directory's
  files and whether an endpoint exists; the wait then continues for a grace
  period so a late publication is timed and the service stopped rather than
  left holding the workspace (the old failures also printed "temp dir not
  removed").
- Evidence gathered: locally a service publishes in 0.06–0.12 s, so the CI
  misses were startups more than 80 times slower than normal, not a uniform
  4x slowdown. **30 s is a candidate, not a proven fix**; if it is missed
  again the message will now say how long startup took and how far it got.
- Not done: no change to test concurrency, and the root cause on the runner
  is still unknown.

## Required Action / Decision

1. **P12e-FINDING-1 (maintainer).** The correlated acceptance run exceeds its
   tolerance. Options to weigh, none chosen here:
   - change the P12e-1 statistic (for example a studentized bootstrap or a
     variance correction) as a new contract version, then declare and run a
     new acceptance simulation;
   - keep P12e-1 and require more bars, and/or require a
     `WITHIN_TOLERANCE` simulation of each confirmation's own declared values
     (with an autocorrelation assumption) before it may run;
   - accept a documented inflation and lower the alpha a family may declare.
   Whatever is chosen, a new acceptance run needs its declaration committed
   before it is executed, and this result stays on record.
2. **P13** must not describe a P12e-1 confirmation as controlling its
   false-positive rate on serially correlated returns until 1 is settled.
3. **Review:** the eight design choices above.

## Verification

- `cargo test --locked`: **532 passed (136 library + 394 desktop + 2
  service), 1 ignored** = 515 + 14 new library tests + 3 new host-harness
  tests. The ignored test only prints the acceptance reports.
  - simulation: small cases reproduce the TypeScript reference exactly,
    including the generated noise; both acceptance runs reproduce their
    committed reports and are checked against the declared values written
    out in the test; strict parsing, rejection order and every integer
    domain; directly constructed declarations re-checked; noise mean,
    variance and lag-one correlation; distinct streams; reproducibility and
    seed sensitivity; the whole schedule simulated with the allocator's
    shares and a growing family; family-rate bounds; more untested family
    members never raise the count; exact tolerance boundary (3/8 at 75,000
    vs 74,999 ppm); no `PASS`.
  - host harness: early exit reported with the service's own error; late
    publication timed and the service stopped; a service that never
    publishes fails after a finite wait with the evidence.
- `npm test`: **1017 passed (59 files)**, 7 new.
- Mutation checks, each caught by a failing test: `<` for `<=`; family
  counted per confirmation; benchmark sharing the returns stream; no
  warm-up; family not growing; one bootstrap seed for all simulations; first
  share for every confirmation; autocorrelation ignored; nominal taken from
  the total; data stream ignoring the candidate; block rule unchecked; a
  confirmation counting tests instead of simulations.
- `npm run typecheck`, `npm run build`, `cargo check --locked --all-targets`:
  pass. Clippy: the five existing warnings only. rustfmt on the new Rust
  files; `git diff --check` pass.
- Cost: the two acceptance runs take about 25 s in a debug build on the
  development machine (they run in parallel), so `cargo test` and the CI
  `cargo-check` job are that much slower.
- Manual: `AFF_TEST_SERVICE_STARTUP_SECS=0` on a real host test produced the
  late-publication message and left no temp directory behind; a non-numeric
  value fails with a clear message.
- No UI change; Playwright not rerun locally.

## Resolution (2026-10-02) — PR #135 acceptance review: R1, R2 and the maintainer's decisions

Published as draft PR [#135](https://github.com/yoyoCadence/AlphaFactorForge/pull/135).
The [acceptance review](2026-10-02-pr135-noise-simulation-acceptance-review-v1.md)
reproduced both declared results and found two P2 defects, fixed on this
branch on top of the reviewed head `37e5a4c`.

**R1 — warm-up versus the accepted coefficient range.** A series starts at
zero, so the first kept bar has `1 − φ^130` of the stationary variance. With
the coefficient allowed up to 0.999999 that was 0.013%: not the stationary
AR(1) the contract promised.

- `autocorrelationPpm` is now `[0, 900000]` (`NOISE_MAX_AUTOCORRELATION_PPM`);
  at the maximum the shortfall is about 1.1 ppm. The warm-up stays a fixed 64
  bars because changing it would change the declared acceptance runs; their
  declarations and reports are byte-for-byte unchanged (at 0.3 the shortfall
  is about 1e-68, so this cannot explain the 6.45%).
- Contract §3 states the bias and the supported range; a larger coefficient
  is refused and needs a new contract version.
- Tests: boundary 900000 accepted, 900001 / 990000 / 999999 refused; the
  analytic shortfall at 0.3, 0.9, 0.99 and 0.999999; measured over 40,000
  independent series, the first kept bar has the stationary variance at 0.9
  and about 0.013% of it at 0.999999. A fifth small fixture case sits at the
  maximum, and the TypeScript reference refuses the same range.

**R2 — a service left unmanaged after deadline plus grace.** The handle was
dropped, and the service could still publish and own the workspace later.

- Every failure path that leaves a live service now calls
  `supervise_failed_startup`: the workspace lock is taken at once when it is
  free (so a service that starts later is refused as `NotOwner`), and a
  cleanup thread stops the service if it publishes after all, joins it,
  releases the lock and removes the directory. `wait_for_startup_cleanup`
  lets a test wait for that.
- Unchanged: the wait is finite, the original failure message and the
  production timeouts are kept, nothing is retried.
- Tests: a real service held back until after deadline plus grace is refused
  and leaves no endpoint, directory or lock, with nothing but the helper
  cleaning up; a real service already starting when the wait fails is brought
  down either way; the never-published and late-publication tests now also
  assert the clean end state. Dropping the handle again fails three tests;
  fifteen consecutive runs were clean.
- Still true: an in-process `service::run` cannot be cancelled, so a service
  that hangs without ever publishing or exiting keeps its thread (and the
  fence) until the test process ends.

**Corrections to what this handoff said above.**

- "rejects true nulls somewhat too often at this sample size, and clearly
  too often under moderate autocorrelation" overstated the evidence. The 95%
  Wilson interval of 129/2000 is 5.45%–7.61%: above the nominal 5%, but it
  contains 6%, so the true rate is not proven to exceed the limit. The
  independent first confirmation (61/2000, 2.38%–3.90%) contains its 2.5%
  share and does not by itself show miscalibration on independent noise. The
  run still fails the threshold declared beforehand, and that record stays.
- The exploratory block-length table is a hypothesis only. It does not rule
  out the block choice or show that the cause is the unstudentized statistic.
- "The acceptance runs take about 25 s" is the local debug build; in CI the
  whole library suite took 8.17 s.

**Maintainer decisions (2026-10-02), answering Required Action 1.**

1. Take the first direction: revise the statistic. A new calibration plan is
   submitted first; then a studentized bootstrap and a serial-correlation
   variance correction are compared. A longer sample may become a condition
   of applicability after it is verified; lowering alpha is not, on this
   evidence, a way to close the finding.
2. The 6.45% failure stays on record; P12e stays open; P13 stays blocked.
3. The new acceptance checks each confirmation and the whole family, so a
   conservative later batch cannot hide a loose first one.
4. The simulations stay in the normal test suite. The 30 s startup wait
   remains a candidate: cleanup first (R2), then collect startup timings and
   failure causes.

Next task: **P12e-4**, the recalibration plan (plan only), on the task board.

Verification after the fixes: `cargo test --locked` **535 passed (137 library
+ 396 desktop + 2 service), 1 ignored** = 532 + 1 warm-up test + 2 cleanup
tests; `npm test` **1019 passed (59 files)**; `npm run typecheck`,
`npm run build`, `cargo check --locked --all-targets` pass; clippy the five
existing warnings only; rustfmt on the simulation files; `git diff --check`
pass. The twelve engine mutation checks were run before R1 and not repeated
(R1 changed one domain bound).

## Resolution (2026-10-02) — merged; the calibration plan is written

- PR #135 was re-reviewed and accepted at `0125e6c` and merged as `1764c48`.
- Required Action 1 has its first step:
  [`confirmation-recalibration-plan-v1`](../docs/plans/confirmation-recalibration-plan-v1.md)
  (P12e-4, [handoff](2026-10-02-p12e4-recalibration-plan-v1.md)) — plan only,
  nothing implemented or run. P12e-FINDING-1 stays open and P13 blocked until
  that plan's final acceptance (P12e-7).
- Timing correction: "in CI the whole library suite took 8.17 s" was one run;
  the next took 19.17 s. They are suite totals that vary with the runner.
