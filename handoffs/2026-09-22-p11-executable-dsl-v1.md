# Handoff: P11 executable Strategy DSL

Date: 2026-09-22
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p11-executable-dsl`
PR: pending
Status: implementation and local acceptance complete; publication pending

## Summary

P11 adds a versioned, data-only `strategy-dsl-v1` with matching TypeScript and
Rust validation/evaluation. A fixed DSL candidate can enter the existing Rust
discovery runner through `discovery-config-v2` only after validation; the DSL,
parameter schema, hypothesis, and attempt lineage are frozen before execution.

This phase does not implement an AI provider, prompt assembly, automated retry,
or approve UI. Those remain P15, and no model output is automatically queued.

## Scope and decisions

- Executable indicators are the unambiguous single-series intersection:
  EMA/SMA/WMA/RSI/ROC/ATR/STDDEV/HIGHEST/LOWEST plus raw OHLC/HLC3.
- MACD and BBANDS remain excluded even though both cores implement them: the
  JSON DSL has no output selector, so admitting either would invent semantics.
- Validation covers exact keys, operator arity, expression types, finite
  values, parameter references, `[2,400]` integer lookbacks, causal historical
  time operations, depth 8, node count 64, and code/IO/network/import tokens.
- Existing `discovery-config-v1` remains params-only. v2 pins
  `strategy-dsl-v1` / `discovery-enumeration-v2`, requires empty DSL axes, and
  keeps costs, risk, sizing, fill mode, and direction outside the tree.
- The execution boundary validates again, derives embargo from the DSL's
  maximum causal lookback, and uses the existing split/backtest/benchmark/
  Gate/Score/persistence path. DSL complexity uses node + parameter + risk-rule
  counts with the unchanged score-v1 formula.
- No migration or dependency was added.

## Main files

- `alpha-factor-forge/src/core/strategy-dsl/{schema,validator,evaluator}.ts`
- `alpha-factor-forge/src-tauri/src/discovery_core/dsl.rs`
- `alpha-factor-forge/fixtures/rs-core/strategy-dsl-v1.json`
- `alpha-factor-forge/src{,-tauri}/**/discoveryConfig|config|enumerate*`
- `alpha-factor-forge/src-tauri/src/discovery_runner/{mod,execution,tests}.rs`
- `docs/strategy-dsl-contract.md`
- `docs/discovery-config-contract.md`, `docs/ai-provider-contract.md`,
  `docs/autonomous-research-capability-registry.md`
- `tasks.md`, `README.md`, `STRATEGY_DISCOVERY.md`, active plan and roadmap

## Verification

- `npm test`: 971/971 passed.
- `npm run typecheck`: passed.
- `npm run build`: passed.
- `cargo check --locked --all-targets`: passed.
- `cargo test --locked`: 372 passed (75 library + 295 desktop + 2 service).
- `npm run e2e`: 78/78 Playwright passed.
- `cargo clippy --locked --all-targets`: passed with the same 5 pre-existing
  warnings (four in `backtest.rs`/`score.rs`, one in `file_commands.rs`).
- `cargo clippy --locked --all-targets -- -D warnings`: not green solely
  because those same 5 existing warnings are denied; no P11 file warned.

The authored DSL fixture is consumed independently by TS and Rust and covers
exact boolean signals/analysis metadata plus invalid indicator, type, lookback,
and code-token cases. Runner tests prove a valid DSL executes through the
existing Train/Validation pipeline and that its durable DSL/params/lineage are
present in SQLite.

## Known limits / next dependency

- P11 exposes no new product UI for authoring or launching DSL envelopes.
- AI generation and explicit approval are P15; research feasibility/trial
  accounting is P12 and is now unblocked by P11.
- Adding an indicator or multi-output selector requires matching TS/Rust
  semantics and parity evidence before whitelist expansion.

## Resolution (added when acted on)

Pending commit, push, and draft PR publication.
