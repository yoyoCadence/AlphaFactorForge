# Handoff: PR #139 diagnostic grid acceptance review

Date: 2026-10-03
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e6b-diagnostics`
PR: [#139](https://github.com/yoyoCadence/AlphaFactorForge/pull/139)
Reviewed head: `4a21b52a2e78e81455917abbebd0541dd4acb7fc`
Base: `13100ce8c28fb61db577667fbd511a9ce9a47718` (merged #138)
Status: Accepted at `4a21b52`; no merge-blocking findings. All 54 complete diagnostic reports independently reproduced, as well as the S2-R3 selection. Local checks and all six exact-head CI jobs pass. PR remains draft/unmerged by this review. Final seed 20261117 remains unused; P12e-FINDING-1 open, P13 blocked.

## Review notes

- Commit `41d68eb` contains only the 72 diagnostic declarations and their
  generator, with no reports. The later fixture preserves every original
  top-level field (`plan`, `phase`, `description`, `screen`, `size`, `power`).
  An independent reconstruction from the plan tables verifies all 72 full
  declarations, including statistics, block lengths, integer power shifts,
  family/schedule, seed, sample count, check rule and checkpoint.
- An independent calculation, without importing the PR's selection helper,
  reconstructs all six pairs, all ten failed cells, the lowest passing
  tested lengths, exactly 18 required power reports, and **S2-R3**. The
  selection follows the predeclared order: smallest qualifying tested
  length, then the larger minimum of the two noise models' first-confirmation
  net-return power, then the fixed candidate/rule tie order.
- At 512 bars, the ranking values are S2-R3 **3067**, S1-R3 **3066**, and
  S2-R4 **3061** out of 4,000. This implements the plan's selection metric;
  the record correctly makes no claim that these differences establish
  superior power. All 256-bar correlated cells failed the point screen.
  No new false-positive-control or supported-configuration claim follows.
- The new Rust test actually re-simulates all 54 declared prefixes and
  checks their counts and echoed fields. The normal suite intentionally
  does not verify all 4,000 simulations per report. Full replay in this
  review therefore also checks the counts after the checkpoint, including
  those that choose between the nearly tied power results.
- Product/statistic implementations, runtime callers, dependency settings,
  existing v1 acceptance data and final-seed declarations are unchanged.
  NUMERIC-JSON-001 remains a separate audit.

## Prefix-cost decision

Keep all 54 prefix checks and the original `4096 / bars` checkpoint. The
reported 16.6 s single-thread / 1.25 s on 16 threads is an observed cost
above an estimate, not grounds to remove reports or shorten declarations
after seeing the results. The parallel test retains the required coverage.

The exact-head [CI run 37117837930](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37117837930)
has **21.16 s** library tests, **51.53 s** desktop tests and **5.71 s** service
tests. The prefix test completed 10.44 s after the library suite's start;
that is a completion-time bound, not an isolated benchmark. The cargo-check
job log spans approximately **6 min 45 s**, including 2 min 17 s check-build
and 2 min 36 s test-build time. These measurements are acceptable for this
slice. P12e-7 should retain all six additional acceptance prefixes and record
the resulting CI timing; do not infer a fixed cross-machine performance
guarantee from either local measurement.

## Verification

- `cargo test --locked --lib`: **159 passed, 1 ignored**, 30.10 s excluding
  compilation; includes all diagnostic prefixes and both v1 acceptance runs.
- `npm test`: **1060 passed, 62 files**; `npm run typecheck` passes.
- Changed Rust test file: `rustfmt --check` passes.
- `git diff --check 13100ce..HEAD` passes.
- All six jobs pass for the reviewed head: test, typecheck, build,
  cargo-check, e2e and native-smoke. CI logs confirm **557 Rust
  (159 + 396 + 2), 1 ignored**, and **1060 Vitest**.
- Independent complete release replay: pending. Inputs are exactly the
  36 declared size runs and the 18 declared power runs with committed
  reports, each with seed 20261005 and 4,000 simulations. No unrequired
  power declaration or final-acceptance seed is executed.

## Next step

After this PR is accepted and merged, proceed with **P12e-7**. Freeze the
selected S2/R3 method and commit all six final-acceptance declarations before
running seed 20261117 once with 20,000 simulations per cell. Include the
256-bar cells even though diagnostics failed there. Report the supported
tested configurations solely from final acceptance; keep the finding open
and P13 blocked unless the plan's acceptance supports a declared set.

Minor documentation follow-up: the capability-registry row still retains
P12e-5's historical phrase that diagnostics and acceptance have not run,
before its new P12e-6b result. Qualify the older phrase as the state at
P12e-5 so the current status is unambiguous. This does not change the
diagnostic result or block this review.

Only review records are edited. No product changes, commit, push, GitHub
review submission, draft conversion, merge or final-acceptance run is
performed by this review.

## Resolution (2026-10-03) — full replay completed and head accepted

**Accept PR #139 at `4a21b52a2e78e81455917abbebd0541dd4acb7fc`.** No
merge-blocking finding remains. The full replay pending above completed
successfully; all **54 / 54 entire reports**, not just their checkpoints,
equal the committed reports in every JSON field.

The replay used the unchanged release runner:

```text
cd alpha-factor-forge/src-tauri
cargo run --release --locked --example noise_simulation_v2 -- target/pr139-review-runs.json
```

The temporary input contains `{ "runs": [{ "id", "declaration" }, ...] }`
from every committed size declaration and precisely those power declarations
whose ids have committed reports. It was checked against the fixture again
after execution. The output is
`alpha-factor-forge/src-tauri/target/pr139-review-output.json`; both files
are ignored build/verification artifacts. Compilation took 7 min 53 s on
this cold release target; the 54-run replay took **799.729 s wall on 16
threads**. No declaration was shortened or otherwise altered, and no
additional or final-seed run was made.

An independent selection calculation from the newly simulated full reports
again ranks S2-R3 (512, 3067), S1-R3 (512, 3066), S2-R4 (512, 3061), before
the three pairs requiring 1024 bars. The ten screen failures, required 18
power reports and all recorded pairs are unchanged.

The prefix-cost decision above is confirmed: retain all existing prefixes,
accept the documented measurement above the original single-thread estimate,
and record the additional six-report cost in P12e-7. The capability-registry
wording follow-up is non-blocking. Proceed with freezing the unchanged S2/R3
method and committing final declarations after merge; no claim of calibrated
false-positive control or supported configurations is accepted at this
diagnostic stage.

The remote head was rechecked and still equals the reviewed commit. Only
this handoff and `tasks.md` were changed locally by the review; the PR remains
draft/open. No product changes, commit, push, GitHub review submission,
draft conversion, merge or final acceptance were performed.
