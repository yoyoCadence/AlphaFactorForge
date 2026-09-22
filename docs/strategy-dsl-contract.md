# Executable Strategy DSL (`strategy-dsl-v1`)

Status: implemented in P11 (2026-09-22). This contract is pure and versioned;
it does not call an AI provider and does not add an automatic research loop.

Owning implementations:

| Concern | TypeScript | Rust |
| --- | --- | --- |
| Schema / validation | `alpha-factor-forge/src/core/strategy-dsl/{schema,validator}.ts` | `alpha-factor-forge/src-tauri/src/discovery_core/dsl.rs` |
| Evaluation | `alpha-factor-forge/src/core/strategy-dsl/evaluator.ts` | `alpha-factor-forge/src-tauri/src/discovery_core/dsl.rs` |
| Shared acceptance | `alpha-factor-forge/fixtures/rs-core/strategy-dsl-v1.json` | same authored fixture |
| Runner admission | `discovery-config-v2` | `discovery_core/config.rs` |

## Executable surface

`strategy-dsl-v1` uses only the unambiguous single-series intersection already
implemented by both cores:

- Indicators: `EMA`, `SMA`, `WMA`, `RSI`, `ROC`, `ATR`, `STDDEV`, `HIGHEST`,
  `LOWEST`, plus raw `CLOSE`, `OPEN`, `HIGH`, `LOW`, `HLC3`.
- Sources: `CLOSE`, `OPEN`, `HIGH`, `LOW`, `HLC3`, `VOLUME`.
- Arithmetic: `ADD`, `SUB`, `MUL`, `DIV`, `ABS`, `MIN`, `MAX`, `CLAMP`.
- Comparison: `GT`, `LT`, `GTE`, `LTE`, `CROSS_UP`, `CROSS_DOWN`.
- Logic: `AND`, `OR`, `NOT`.
- Causal time operations: `SHIFT`, `RISING`, `FALLING`; every offset is a
  positive historical lookback. There is no future or negative shift.
- Constant: `CONST`.

MACD and Bollinger Bands exist in both indicator libraries but produce more
than one series. They are deliberately not executable in v1 because the DSL
does not yet define an output selector. The larger historical design whitelist
is not an execution promise; each addition requires a contract bump or a
backward-compatible, both-language parity extension.

## Admission rules

The root must contain exactly `version`, `name`, `params`, `entry`, and `exit`.
`version` is `strategy-dsl-v1`; entry and exit must type-check as boolean.

Validation rejects before execution:

- unknown fields, indicators, operators, sources, parameter references, or
  contract versions;
- wrong operator arity or numeric/boolean operand types;
- non-finite constants/defaults and constants outside ±1e9;
- period/lookback defaults outside integer `[2, 400]`;
- AST depth over 8 or total entry+exit node count over 64;
- code/IO/network/import-like tokens anywhere in the JSON payload.

Parameter entries are either a finite fixed number or an exact
`{type,min,max,default}` object. An `int` spec requires safe-integer bounds and
default. Evaluation uses the frozen fixed value/default; P11 does not tune DSL
parameters.

Warm-up numeric values are `NaN`; comparisons involving them are false.
Division by zero yields `NaN`. Crosses require finite current and previous
values. Both evaluators derive and return the maximum causal signal lookback.

## Runner envelope

Existing `discovery-config-v1` remains byte-for-byte params-only. A fixed DSL
candidate uses `discovery-config-v2`, pins `strategyDsl: strategy-dsl-v1` and
`enumeration: discovery-enumeration-v2`, and uses
`presetVersion: discovery-dsl-preset-v1`:

```jsonc
{
  "mode": "dsl",
  "dsl": {
    "version": "strategy-dsl-v1",
    "name": "SMA cross",
    "params": { "fast": 10, "slow": 30 },
    "entry": { "op": "CROSS_UP", "args": [
      { "ind": "SMA", "src": "CLOSE", "len": "$fast" },
      { "ind": "SMA", "src": "CLOSE", "len": "$slow" }
    ]},
    "exit": { "op": "CROSS_DOWN", "args": [
      { "ind": "SMA", "src": "CLOSE", "len": "$fast" },
      { "ind": "SMA", "src": "CLOSE", "len": "$slow" }
    ]}
  },
  "slPct": 2,
  "tpPct": 4,
  "feePct": 0.05,
  "slipPct": 0.02,
  "sizePct": 100,
  "fillMode": "nextOpen",
  "direction": "long"
}
```

The base's `axes` must be empty. Risk, costs, sizing, fill mode, and direction
are outside the DSL and frozen for the candidate. The config parser validates
the DSL before enumeration; execution validates it again. An admitted DSL is
persisted as `strategy_def.type = dsl` with `dsl_json` and parameter schema,
and its P05 hypothesis/attempt lineage is created in the same enqueue
transaction before worker execution.

The `validate_strategy_dsl` Tauri command returns the structured validation
report for preview. `generate_strategy_dsl` and all provider/AI behavior remain
P15; no P11 path automatically approves or queues model output.
