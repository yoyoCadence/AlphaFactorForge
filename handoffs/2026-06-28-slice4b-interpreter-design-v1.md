# Handoff: Slice 4b — code-mode safe interpreter (design proposal)

Date: 2026-06-28
Repo: yoyoCadence/AlphaFactorForge
Branch: (design only — no implementation branch yet)
PR: (none yet)
Status: Design proposal — awaiting decisions before Mode B

## Summary

Slice 4b adds a `code` strategy mode: the user manually writes entry/exit
expressions (e.g. `rsi < 30 && price > ema`), evaluated by a self-written SAFE
interpreter. NO `eval` / `new Function` / dynamic execution (STRATEGY_DISCOVERY
§0.3). AI must never reach code mode.

## Non-negotiable invariants

- No `eval` / `Function` / dynamic import / string-to-code anywhere.
- Parse failure or any non-whitelisted token -> reject the whole expression
  (don't execute; surface the error).
- AI never reaches code mode (AI only emits DSL/blocks; `source` isolates it).

## Architecture — three pure stages (`src/services/exprInterpreter.ts`)

```
expr string -> [Tokenizer] -> tokens -> [Parser] -> AST -> [Evaluator] -> boolean
                  reject bad chars       reject bad syntax / over-caps   whitelist only
```

1. Tokenizer: numbers / identifiers / operators / parens; throw on any other char.
2. Parser: recursive descent with operator precedence -> AST; enforce depth/node caps.
3. Evaluator: per candle, evaluate the AST against that bar's whitelisted variable
   values -> boolean.

## Whitelist (everything else is rejected)

| Category | Contents |
|---|---|
| Variables | same as blocks operands: `price/open/high/low/volume`, `maFast/maSlow/ema/rsi`, `macd/macdSignal/macdHist`, `bbUpper/bbMid/bbLower`, plus each `prev*` (prior bar, e.g. `prevRsi`) |
| Operators | arithmetic `+ - * /`, compare `> < >= <= == !=`, logical `&& || !`, unary minus, parens `( )` |
| Constants | numeric literals |
| Forbidden | function call `x(`, property `a.b`, index `a[`, assignment `=`, strings, `?:`, keywords (`while`/`for`/…) -> reject |

AST node kinds: `Num` / `Var(whitelisted name)` / `Unary` / `Binary`. No loops, no
functions, no IO.

## Caps (DoS / blow-up guard)

Expression length (~500 chars), AST depth (~32), node count (~256); exceeding any -> reject.

## Integration

- `strategy.ts`: add `'code'` to `mode`; add `entryCode`/`exitCode` string fields (legacy has defaults).
- `strategySignals.ts`: `buildCodeSignals()` — reuse `resolveSeries`, build per-bar
  var map (incl. `prev*`), evaluate; `buildSignals` dispatches `'code'`;
  `i < 1 -> false`, `NaN -> false` (as blocks).
- `BacktestPanel.tsx`: add a 程式碼 (code) tab — entry/exit textareas + live error
  hint + whitelisted-variable list; label manual-only.
- `strategyRecord.ts`: `type` already follows `mode` (saves as `code`).

## Tests (CI-required)

- Security: `eval(...)`, `x()`, `a.b`, `a['b']`, `x=1`, `while`, strings -> all
  rejected (error, not executed); grep the module for no `eval`/`Function`.
- Correctness: precedence; `prevRsi`; `NaN -> false`; `rsi < 30` (code) equals the
  equivalent blocks rule.
- Caps: over-long / over-deep / too-many-nodes -> rejected.

## Proposed slicing (keep slices small)

- 4b-1: interpreter + `buildCodeSignals` + tests (pure logic, CI-verifiable, no UI).
- 4b-2: BacktestPanel code tab (UI). (Both touch BacktestPanel, so sequence the PRs.)

## Open decisions (needed before Mode B)

1. Variable/operator set as tabled above? (esp. `==` / `!=` / `prev*` — legacy has them.)
2. Split into 4b-1 / 4b-2? (recommended.)
3. Cap numbers (length / depth / nodes) — accept the defaults above?

## Resolution

Pending decisions.
