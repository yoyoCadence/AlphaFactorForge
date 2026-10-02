# Research noise simulation, engine for recalibration (`research-noise-simulation-v2`)

> **Status: P12e-5, engine only (2026-10-02).** No runtime caller. It is the
> simulation engine that
> [`confirmation-recalibration-plan-v1`](plans/confirmation-recalibration-plan-v1.md)
> runs on. **No new statistic is implemented and no diagnostic or acceptance
> cell of that plan has been run**: the only selectable statistic is the
> baseline, the fixture holds tiny cases that are not plan cells, and the
> plan's seeds (20261005, 20261117) have not been used.
> Implementation: `alpha-factor-forge/src-tauri/src/discovery_core/noise_simulation_v2.rs`;
> runner: `alpha-factor-forge/src-tauri/examples/noise_simulation_v2.rs`.
> [`research-noise-simulation-v1`](research-noise-simulation-v1.md) and its
> two declared acceptance runs are unchanged and stay in the test suite.

## 1. What changes from v1

The noise (v1 §3), the simulated family life (v1 §4) and the definition of a
rejecting simulation are the same. v2 adds what the plan needs:

| | v1 | v2 |
| --- | --- | --- |
| statistic | fixed: `research-confirmation-statistics-v1` | declared; today only that one (§3) |
| what is judged | the family rate | every confirmation **and** the family (§5) |
| rule | observed rate ≤ nominal + tolerance | none, a point screen, or the 95% Wilson upper bound (§5) |
| limit | nominal + `tolerancePpm` | nominal × `limitMultiplierPpm` |
| scenario | null only | null, or a declared shift on the strategy's returns (§4) |
| long runs | one report | checkpoints: running counts at declared prefixes (§6) |

In the null scenario with the baseline statistic, v2 counts exactly what v1
counts (tested on every v1 fixture case).

## 2. Declaration (strict JSON)

All fields are required; unknown fields are rejected; integers are JSON
integers.

| Field | Domain | Meaning |
| --- | --- | --- |
| `contractVersion` | `"research-noise-simulation-v2"` | exact |
| `statistic` | `"research-confirmation-statistics-v1"` | the confirmation statistic under test (§3) |
| `noiseModel` | `"ar1-uniform-sum"` | v1 §3 |
| `autocorrelationPpm` | integer `[0, 900000]` | v1 §3, including its supported range |
| `effectMillionths` | integer `[0, 1000000000]` | shift of the strategy's returns, in millionths; `0` is the null scenario (§4) |
| `bars` | integer `[2, 1000000]` | as v1 |
| `candidatesPerConfirmation` | integer `[1, 1024]` | as v1 |
| `priorTrials` | integer `[0, MAX]` | as v1 |
| `blockLength` | integer `[1, MAX]`, `L² ≤ bars` | as v1 |
| `bootstrapSamples` | integer `[1, 1000000]` | `B` |
| `simulations` | integer `[1, 1000000]` | `N` |
| `seed` | integer `[0, MAX]` | as v1 |
| `check` | object (§5) | how the rates are judged |
| `checkpoints` | array of at most 16 integers | strictly increasing, each in `[1, simulations − 1]` (§6) |
| `allocation` | a [`research-alpha-allocation-v1`](research-alpha-allocation-v1.md) declaration | every scheduled confirmation is simulated |

Rejection order (part of the contract): not an object → first unknown field
in sorted order → the fields in table order (`check` and `allocation` by
their own order, prefixed `simulation.`) → `blockLength² > bars` → the last
confirmation's family count above `MAX` → a rule together with a non-zero
`effectMillionths` → a limit that is not a whole number of ppm below 1000000
(§5).

## 3. Statistic

`statistic` names the contract of the confirmation calculation each simulated
confirmation runs. The engine calls that calculation unchanged and reads only
its public result: per test, the bootstrap extreme count and whether the null
is rejected.

Today the only value is the plan's baseline **V1**. The plan's candidates S1
and S2 are added in P12e-6, each with its own committed draft contract; until
then any other name is refused.

## 4. Scenarios

- **Null** (`effectMillionths = 0`): v1's noise bit for bit; nothing is added.
  Every tested null hypothesis is true, so a rejection is a false positive.
- **Shifted** (`effectMillionths > 0`): `effectMillionths / 1e6` (one IEEE
  division) is added to every bar of the strategy's returns; the benchmark is
  untouched. Both tests then have a true effect, and a rejection is a
  detection. A rule is refused in this scenario — only counts are reported.

The recalibration plan's **power** of a cell is
`confirmations[0].counts.netReturnRejectingSimulations` of a shifted run.

## 5. Checks

`check` is `{"rule": "none"}`, or `{"rule": "point-screen" |
"wilson-upper-bound", "limitMultiplierPpm": m}` with `m` an integer in
`[1000000, 10000000]`.

Each scheduled confirmation is checked against its own share `α_k`, and the
family against the sum of the schedule. For a nominal rate `a` ppm:

```text
limit l = a × m / 1e6      must be a whole number of ppm, and below 1000000
```

A declaration whose limits are not whole ppm is refused, not rounded
(`16666 × 1.2` is refused; `25000 × 1.2 = 30000` is accepted).

With `x` rejecting simulations out of `N`:

```text
point-screen        within  iff  x·10⁶ ≤ l·N
wilson-upper-bound  within  iff  x·10⁶ < l·N  and  (l·N − x·10⁶)²·10000 ≥ 38416·l·(10⁶ − l)·N
```

The second rule says that the upper end of the **two-sided 95%** Wilson
interval (`z = 1.96 = 49/25`, so `z² = 38416/10000`) is at most the limit. It
is the recalibration plan's §7 rule; at `N = 20000` it admits at most 552
simulations for a 3% limit and 1134 for a 6% limit (tested).

`status` is `MEASURED` when the rule is `none`; otherwise `WITHIN_LIMITS` only
when **every** confirmation and the family are within their limits, else
`EXCEEDS_LIMITS`. A family within its limit does not excuse a confirmation
above its own. There is no `PASS`.

Every check also reports the Wilson interval in whole ppm, rounded outwards,
computed in exact integers:

```text
bounds = ( N(1250x + 2401) ∓ sqrt( 2401·N·(2500·x(N − x) + 2401·N) ) ) / ( N(1250N + 4802) )
```

`withinLimit` under the Wilson rule is the same statement as
`wilsonUpperPpm ≤ limitPpm` (tested for every count at several sizes). The
limits are accepted simulation limits, not the nominal alphas.

## 6. Checkpoints

Simulation `s` depends on the seed and on `s`, never on how many simulations
follow. So a run declared with `simulations = M` produces exactly the counts
a longer run had after its first `M` simulations.

`checkpoints` lists the prefixes at which the running counts are recorded.
Each entry reports `simulations`, `familyRejectingSimulations` and, per
confirmation, the counts of §7 — including the extreme-count totals, which
change with any change to the data, the resampling or the statistic even when
nothing is rejected.

**The prefix the plan's reports declare.** Every diagnostic and acceptance
declaration of the recalibration plan carries exactly one checkpoint, at

```text
4096 / bars  simulations      16 at 256 bars, 8 at 512, 4 at 1024
```

so every cell costs the same to re-check. The normal test suite re-runs each
committed report's declaration cut at that checkpoint and compares the counts.
It is a guard against drift in the code, the fixtures or the toolchain; the
full report is reproduced only by the runner (§8).

## 7. Report

camelCase JSON: `contractVersion`, `statistic`, `allocationId`, `status`,
`simulations`, `effectMillionths`, `checkRule`, `limitMultiplierPpm` (`null`
without a rule), `family`, `confirmations`, `checkpoints`.

- A **check** (`family`, and `check` of each confirmation): `nominalPpm`,
  `limitPpm`, `count`, `ratePpm` (floored), `wilsonLowerPpm`,
  `wilsonUpperPpm`, `withinLimit` (`limitPpm` and `withinLimit` are `null`
  without a rule).
- A **confirmation**: `confirmationNumber`, `alphaPpm`, `familyTests`,
  `check`, and `counts`: `rejectingSimulations` (it rejected at least one
  null), `netReturnRejectingSimulations`,
  `benchmarkExcessRejectingSimulations`, `netReturnExtremeTotal`,
  `benchmarkExcessExtremeTotal` (sums of the bootstrap extreme counts over
  every candidate and simulation).

## 8. Runner

```text
cd alpha-factor-forge/src-tauri
cargo run --release --locked --example noise_simulation_v2 -- [--timing-only] <runs.json>
```

`<runs.json>` is one declaration, or `{"runs": [{"id", "declaration"}]}`. The
reports are printed to stdout in input order; timings go to stderr. Runs
execute in parallel; a report does not depend on that or on the build
profile. `--timing-only` prints durations and the amount of work and nothing
about any outcome, so the cost of a grid can be measured before any of its
results is seen.

It is a Cargo example, not a product binary: no desktop or service build
includes it.

## 9. Fixture and reference

[`research-noise-simulation-v2.json`](../alpha-factor-forge/fixtures/rs-core/research-noise-simulation-v2.json)
holds five small hand-written declarations. Their `expected` reports come from
an independent TypeScript/BigInt reference
(`src/parity/noiseSimulationV2Fixture.ts`,
`npm run fixtures:noise-simulation-v2`), which finds the Wilson bounds by
bisection on the score-test inequality rather than by the closed form above.
A Vitest fails if the fixture stops matching; the Rust tests must reproduce it
exactly.

Cases: a point screen where one confirmation exceeds its share while the
family is within its limit; the Wilson rule on correlated noise; a shifted run
without a rule; no rejection within a point screen; and the same run under
the Wilson rule, where five simulations without a rejection are not enough
evidence.

## 10. Limits

- Everything in v1 §9 still applies: easy noise, size not general validity,
  one configuration is one data point.
- The engine does not know the plan. It will run any valid declaration; that
  a run is a declared cell, uses the right seed and happens once is the
  record-keeping of plan §9, enforced by commits and review.
- A checkpoint re-check covers a few simulations per report. It detects
  drift; it does not re-establish a rate.
- Rust only. The reference in `src/parity` is test support.
