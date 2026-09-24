# Research walk-forward feasibility (`research-walk-forward-v1`)

Status: P12c-1 planning, P12c-2a fixed-candidate execution and P12c-2b
runner persistence are implemented. Campaign admission remains P12d.

| Concern | Location |
| --- | --- |
| Planner and precheck | `alpha-factor-forge/src-tauri/src/discovery_core/walk_forward.rs` |
| Outer split | `alpha-factor-forge/src-tauri/src/discovery_core/split.rs` (`validation-split-v1`) |
| Product boundary | `docs/plans/active-plan.md` §4.4–4.5 |

## Question and boundary

Given a declared bar count, embargo and inner-fold requirements, can the
existing outer 60/20/20 split supply enough **Train** bars for every fold?
The planner derives the outer split itself and returns only its Train range.
It never reads candle values, gives Validation/Test to a candidate search, or
runs a backtest. `ELIGIBLE` means only that the declared windows fit; it is
not strategy evidence or a confirmation `PASS`.

## Declaration

The JSON entry point accepts exactly these fields, with no defaults:

| Field | Domain | Meaning |
| --- | --- | --- |
| `contractVersion` | `"research-walk-forward-v1"` | exact version |
| `totalBars` | integer `[0, 2^53−1]` | oldest-to-newest dataset length |
| `embargoBars` | integer `[0, 2^53−1]` | same declared gap in outer split and every inner fold |
| `minimumTrainBars` | integer `[1, 2^53−1]` | smallest initial fitting span |
| `foldValidationBars` | integer `[1, 2^53−1]` | exact size of each inner validation span |
| `foldCount` | integer `[2, 128]` | bounded number of inner folds |

Malformed input is an error, including float JSON literals, unknown fields,
zero-length declared spans and fewer than two folds. P12d must freeze and
justify the per-campaign bar thresholds before admitting a campaign; this
contract does not invent a fixed AlphaBTC threshold or infer one from results.
`minimumTrainBars` must account for the strategy's warm-up and the campaign's
declared minimum evaluation history when P12d wires this contract.

## Windows and feasibility

The existing `validation-split-v1` planner removes two outer embargo gaps and
allocates the remaining bars 60/20/20. Failure to form that split returns
`NOT_ELIGIBLE` with `insufficient_outer_history`. Otherwise, let `T` be its
Train bar count, `M` the declared minimum training bars, `F` the fold count,
`E` the embargo, and `V` the exact inner validation length:

`requiredTrainBars = M + F × (E + V)`.

If `T < requiredTrainBars`, the result is `NOT_ELIGIBLE` with
`insufficient_train_history` and **no partial folds**. The requirement is
calculated in `u128`; its JSON report field is `null` when it exceeds the
JavaScript safe-integer bound. Such a requirement cannot fit any valid outer
Train span and is always ineligible.

When enough bars exist, the surplus `T − requiredTrainBars` extends the
first training span. Each fold uses an expanding training range from the first
outer Train bar, then exactly `E` embargo bars, then `V` validation bars. The
next fold's training span may include earlier inner validation bars; all
remain within the development-only outer Train range. Inner validation spans
are disjoint; the final one ends on the last outer Train bar. All ranges use
zero-based inclusive `from`/`to`/`count` indexes. No window includes either
outer embargo or any outer Validation/Test bar.

## Authored boundaries

With `E=2`, `M=20`, `F=3`, `V=10`, the requirement is `56` Train bars.
`totalBars=96` gives 55 outer Train bars and `NOT_ELIGIBLE`;
`totalBars=97` gives 56 and three folds with inner validation ranges
`22..31`, `34..43`, `46..55`. Tests also pin an insufficient outer split,
zero embargo, malformed declarations, and a requirement above the safe-integer
bound. Runtime fold execution, trade-count evidence, cost/parameter stability,
campaign admission and statistical confirmation remain open.

## P12c-2a fixed-candidate fold execution (2026-09-25)

`discovery_runner::execution::execute_candidate_walk_forward` accepts an
already enumerated candidate, its immutable dataset candles, and an explicit
`WalkForwardPlan`. It returns serializable `walk-forward-evidence-v1` material
for P12c-2b to persist. There is no production runner caller yet.

Before any backtest, execution verifies the candidate's strategy hash and
costs against the resolved discovery config, the dataset identity and candle
count, the derived embargo against the declaration, and the initial Train
minimum against the signal lookback. A valid but infeasible plan returns its
`NOT_ELIGIBLE` report with an empty `folds` array; it never executes a
subset of folds.

For an eligible plan, each fold runs the unchanged costed backtest twice:
once on the training prefix ending at that fold's Train boundary, and once
on the prefix ending at its inner validation boundary. Both executions use
the declared segment's inclusive `from`/`to` indexes, the existing execution
model and the configured starting equity. Signal construction sees only the
corresponding prefix; the whole routine never reads a candle after the outer
Train endpoint. Every fold records its window, encoded metrics (including
explicit non-finite statuses) and closed trades. The evidence also binds the
candidate index, strategy ID/hash, dataset ID/hash/interval, starting equity,
fee/slippage, derived embargo and execution / metrics contract versions. It
contains no Gate, Score or confirmation `PASS`.

This is **fixed-candidate evaluation**, not per-fold parameter fitting or
optimization. P12c-2b must add a versioned run declaration and persist the
evidence in the immutable candidate artifact with an auditable attempt link;
P12d must freeze campaign thresholds and decide how this evidence affects
admission. Neither existing `discovery-config-v1/v2` nor its result artifact
changes in P12c-2a.

## P12c-2b versioned run and immutable evidence (2026-09-25)

`discovery-config-v3` retains v2's candidate and execution fields and adds a
required `walkForward` object with exact keys `minimumTrainBars`,
`foldValidationBars`, and `foldCount`. The first two are safe integers at least
1; folds are an integer in `[2,128]`. The run contracts additionally pin
`walkForward: research-walk-forward-v1` and
`walkForwardEvidence: walk-forward-evidence-v1`. Older v1/v2 envelopes reject
this field and keep their previous behavior and `candidate-result-v1` output.

For each v3 candidate, the runner derives `totalBars` from the verified dataset
and `embargoBars` from that candidate's used-signal lookback plus the frozen
holding allowance. Before creating a run, it checks that `minimumTrainBars`
covers the lookback and the P12c-1 report is `ELIGIBLE` for **every** enumerated
candidate. An invalid or short plan fails before run/attempt rows are written.
Resume repeats that preflight from the stored raw config and verified dataset.

The worker executes the normal outer Train/Validation assessment and the
P12c-2a fixed-candidate folds. The fold executor reads only outer Train candles.
Its complete evidence is embedded as `walkForward` in `candidate-result-v2`,
whose `attemptKey`, strategy/dataset identity and content-addressed artifact
reference link it to the completed attempt. The existing artifact store stages
and hashes the file before the candidate transaction commits its reference,
jobs, summaries, record and progress. A failed fold execution or artifact write
fails the run without a completed candidate checkpoint. No database migration
or new UI flow is needed.

The declared lengths are caller-provided bounds, not P12d's campaign policy.
Fold evidence is not used by Gate, Score, qualification or confirmation in this
slice; P12d must decide those rules and freeze an admissible campaign protocol.
