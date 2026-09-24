# Research walk-forward feasibility (`research-walk-forward-v1`)

Status: P12c-1 pure Rust planning contract. Fold execution and runtime admission
remain P12c-2 and P12d, respectively.

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
