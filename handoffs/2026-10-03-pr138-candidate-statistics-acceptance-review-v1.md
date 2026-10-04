# Handoff: PR #138 candidate statistics acceptance review

Date: 2026-10-03
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12e6a-candidate-statistics`
PR: [#138](https://github.com/yoyoCadence/AlphaFactorForge/pull/138)
Reviewed head: `7a988aa949a34694c7ed7bc8b8e4bd9e700e81dc`
Base: `50b20e5a561d2b369d48679f1a7d893b44b84cfd` (merged #137)
Status: Independently re-reviewed and accepted at `276765a` (2026-10-03); R1 and R2 closed, with no new merge-blocking findings. PR remains draft/unmerged by this review. No diagnostic or final-acceptance seed was used. P12e-FINDING-1 remains open and P13 blocked. See the independent re-review Resolution below; the original findings are retained.

## Summary

The draft formulas match the amended plan: S1 uses the variance of drawn
blocks, S2 uses the stated flat-top/Bartlett ratio and wider block domain.
The shared extraction preserves v1's fixture and both original acceptance
reports. The normal-size fixtures, exact rational p-values, Holm arithmetic,
engine dispatch and draft-only runtime boundary are covered by passing
tests. The 6a/6b split preserves the planned order and is sensible.

Two additional probes expose behavior that should be settled before freezing
the draft for diagnostics: multiplying squared quantities can silently
overflow/underflow despite finite inputs and variances; and the promised S1
constant-series behavior does not hold for ordinary decimal constants.
Neither finding is evidence about the unexecuted diagnostic grid's size or
power, and neither changes v1's recorded 6.45% failure.

## Required Action / Decision

### R1 — P2: Keep the square-free comparison valid over its accepted numeric domain

Location: `alpha-factor-forge/src-tauri/src/discovery_core/confirmation_candidates.rs:46`
(`scaled_at_least`), with the S1 resampled variance and S2 flat-top
intermediates at lines 208/243. Same comparison in the TypeScript reference;
draft §2 claims the comparison for finite a/s and non-negative p/q.

The decision forms `(a*a)*p` and `(s*s)*q` without checking representability.
It can turn unequal finite, representable square-root products into
`Infinity >= Infinity` or `0 >= 0`, counting a resample as extreme when it
is not. A finite observed `B(L)` does not protect these products.

Direct examples, all inputs finite and square-root products representable:

| a | s | p=q | squared comparison | a·sqrt(p) >= s·sqrt(q) |
| --- | --- | --- | --- | --- |
| 2^400 | 2^401 | 2^800 | true (Inf >= Inf) | false |
| 2^-400 | 2^-399 | 2^-800 | true (0 >= 0) | false |

End-to-end probe through the real Rust public evaluator, not a replacement
implementation: n=16, L=2, B=799, seed=17, alpha=25000 ppm, familyTests=2,
candidateIndex=0, benchmark all zeros. Returns are the following eight
values repeated twice:

```text
[0.75, 0.375, 1.0, 0.25, 0.625, 0.875, 0.375, 0.5]
```

| Exact positive scale | S1 extreme count (each test) | rawP | rejectsNull |
| --- | --- | --- | --- |
| 1 | 0 | 1/800 | true |
| 2^400 | 384 | 385/800 | false |
| 2^-400 | 799 | 800/800 | false |

These powers of two avoid decimal conversion/multiplication-rounding as a
confound; the input sums and block variances are still finite. The evaluator
silently returns a changed decision. An assertion comparing the scale-1 and
scale-2^400 rejection results fails with true versus false. The S2 smooth
fixture input also changes extreme counts under these exact scales when its
correction is active, because it uses the same comparison.

Required before merge: use a comparison that handles exponent range while
remaining deterministic, or explicitly bound/refuse inputs/intermediates
outside the supported numeric range. Check non-finite resampled totals,
drawn-block variances and flat-top estimates too. Detect underflow rather
than treating an arithmetic zero as a genuine zero variance. Synchronize
the draft and reference, and add meaningful large/small-scale regressions;
the small dyadic grid and factor-2 test do not cover this boundary. This is
scoped to the new candidates; do not broaden it into a v1/runtime refactor.

### R2 — P2: Make S1's declared constant-series behavior match the implementation

Location: `docs/research-confirmation-candidates-draft-v1.md:100`, with
`block_variance` centering at `confirmation_candidates.rs:29` and the
resampled centering at line 62.

The draft, PR description and handoff say a constant series has every
resample extreme, rawP=1, and is never rejected. With the same declaration
as R1, sixteen returns of **0.1** and a zero benchmark produce:

```text
S1: extremeCount = 0, rawP = 1/800, rejectsNull = true (both tests)
```

Sixteen returns of 0.125 instead produce the declared 799 / 800/800 / false.
The 0.1 case is independent of the JSON parser: the probe directly constructs
`vec![0.1; 16]` in Rust. Summing gives S=1.6000000000000003;
`2*(S/16)` is 0.20000000000000004, whereas each two-bar block sums to 0.2.
Consequently B(2) is about 3.85e-34 rather than zero. The drawn-block
variance has the same centering residual; S*-S is zero, so no resample is
extreme. An assertion of the draft's 799 extreme resamples fails with 0.

Required before merge: explicitly settle this edge policy. If constant
series must get p=1, implement that behavior for equal input bars regardless
of decimal representation and cover it in both references/fixtures. If the
literal IEEE arithmetic and its representation-dependent outcome are the
intended experimental candidate, correct the unconditional promise and pin
the actual behavior before diagnostics. Do not quietly add an arbitrary
epsilon later after seeing results. A declared draft change can still be
made before P12e-6b.

## Answers to the screenshot's two questions

1. **Keep the 6a -> 6b split and order, after R1/R2 are resolved.** Review and
   freeze the draft contracts first; commit the entire diagnostic declarations
   (seed 20261005, 4000 per cell, V1/S1/S2 x R3/R4 x six cells and declared
   prefixes) before running release diagnostics; commit all reports/failures
   and the deterministic selection. Keep seed 20261117 unused until P12e-7.
   No matrix expansion, new candidate or early final acceptance is requested.
   Existing v1 reports remain untouched; no calibration finding is closed by
   merely implementing the candidates.
2. **Yes, open a separate JSON float round-trip/identity audit task.** Current
   serde_json 1.0.150 features, independently inspected with `cargo tree`, do
   not include `float_roundtrip`. The example literal
   `0.0036944444444444438` parses to bits `3f6e43cfc21ad9ff`, while standard
   Rust parsing and Node JSON.parse produce `3f6e43cfc21ad9fe` (one ulp).
   A serialize/deserialize round-trip changes the f64 bits, and passing the
   before/after Values to this project's real `canonical_bytes` produces
   different bytes. This is more than a formatting observation.

   The real product's `src-tauri/src/identity.rs:115` parses definition JSON
   before canonical hashing; lines 147–159 parse and verify stored strategy
   definitions. Thus the affected class of operation exists in the repo.
   This review does **not** establish that any existing user record is bad
   or quantify affected product paths. The current fixture's decision
   counters remain exact; do not generalize that fact to all inputs.

   Track as **NUMERIC-JSON-001**: inventory float-bearing read/rehash paths,
   reproduce end-to-end identity and persistence round-trips, compare Rust
   and TypeScript bits/identities, then evaluate enabling float_roundtrip
   with parsing cost and old identity compatibility. Do not change features
   globally or regenerate established fixtures/hashes within this PR.
   This existing parser issue is separate from PR138-R1/R2 and does not
   itself add a blocker to this candidate implementation. Any parser change
   that alters calibration evidence must be declared before affected runs.

   Primary source:
   [serde_json 1.0.150 feature definition](https://github.com/serde-rs/json/blob/v1.0.150/Cargo.toml#L59)
   specifies the f64 -> JSON -> f64 round-trip purpose of float_roundtrip;
   it is distinct from arbitrary_precision.

## Verification

- Independent local `cargo test --locked --lib`: **155 passed, 1 ignored**;
  23.28 s excluding compilation. Includes the candidate/reference tests,
  v2 dispatch, v1 statistics fixture and both original acceptance reports.
- `npm test`: **1046 passed, 61 files**; `npm run typecheck`: passed.
- [Exact-head CI run 37021783753](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37021783753):
  all six jobs passed. Backend logs confirm **553 passed (155 library +
  396 desktop + 2 service), 1 ignored**; Vitest logs confirm 1046. Full
  desktop/service suites, build and e2e here are CI evidence, not local reruns
  in this review.
- Additional actual Rust evaluator probes for R1 and R2 failed their
  assertions, not compilation. Constants/scales and parser/canonical-byte
  results above were reproduced independently; Node agrees on the original
  parser bits and on the direct finite comparison counterexamples.
- The probe used only small fixed inputs and seed 17, not a declared
  diagnostic/acceptance cell. No plan seed was used, and no diagnostic or
  final-acceptance report was generated.
- Temporary example source was removed. Product code/tests/fixtures and
  Cargo features remain unchanged. Only review handoffs and task records
  persist locally; no commit, push, GitHub review submission or merge.

## Resolution (2026-10-03)

R1 and R2 were acted on in PR #138, on top of the reviewed head `7a988aa`,
before any diagnostic run. This handoff, its task board lines and the PR #136
re-review Resolution were committed unchanged first (`ac66a6b`).

- **R1.** The candidates now normalize each test series by an exact power of
  two (largest magnitude in `[1, 2)`), so no decision depends on the overall
  scale, and they refuse — with an error naming the candidate — any product
  or quotient that would overflow or underflow, including the squares of the
  comparison, `B(ℓ)`, `v*`, the flat-top estimate and normalization itself.
  An arithmetic zero is only a zero when an operand was zero. The review's
  probe series gives identical counts and rejections at scales 1, 2^±400,
  2^±1000 and 2^±52 for S1 and S2, in Rust and in the TypeScript reference;
  the direct comparison counterexamples are refused. Draft §2 states the
  normalization and the range rule; §5 lists the refusal.
- **R2.** An explicit, representation-independent rule: a test whose bars
  are all equal reads no variance; it is rejected when the value is positive
  and never when it is zero or negative — the limit of the studentized
  statistic and v1's behaviour. Sixteen bars of 0.1 and of 0.125 now give
  the same result; zero and negative constants give `rawP = 1`. No tolerance
  was introduced; near-constant series remain decided by the arithmetic and
  the draft says so. Fixture cases cover decimal, positive, zero and
  negative constants for both candidates and both references.
- **Answers.** The 6a → 6b order is kept and seed 20261117 stays unused;
  NUMERIC-JSON-001 stays a separate task and was not acted on here.

Every non-constant fixture result and the engine fixture are unchanged. 556
Rust (1 ignored) and 1052 Vitest pass locally; 21 mutation checks are caught.

## Resolution (2026-10-03) — independent re-review at `276765a`

Reviewed immutable head `276765a90409c8bc56a2018e3cc0a2cb43b272a5`,
against the original reviewed head `7a988aa` and the unchanged PR base
`50b20e5`. **R1 and R2 are resolved; this head is accepted for merge.**
No new merge-blocking correctness or contract finding was identified.

- **R1 closed.** The actual Rust evaluator regressions now reproduce the
  original binary-exact probe with identical extreme counts and decisions
  at scales 1, 2^±400, 2^±1000 and 2^±52. Every existing fixture also keeps
  its extreme counts at 2^±400, including S2 with its correction active.
  Normalization reads the exponent from bits, handles subnormal inputs,
  and refuses a non-zero normalized value outside the declared normal
  range. Products and quotients in the comparison and variance calculations
  distinguish genuine zero factors from underflow; the original direct
  overflow/underflow counterexamples return `OutOfRange`. The error names
  the candidate, and the draft/reference state the same supported range.
- **R2 closed.** An explicit rule now handles exactly equal bars per test,
  without consulting rounded variance or adding a tolerance. Sixteen 0.1
  and sixteen 0.125 bars both give zero extreme resamples; zero and negative
  constants give all resamples extreme. Both S1 and S2 regressions pass.
  Precisely, a positive constant produces `rawP = 1 / (B + 1)` and still
  goes through the shared exact Holm/alpha comparison: it does not bypass
  family size or Monte Carlo resolution. The fixture also verifies that a
  constant net-return test leaves a varying benchmark-excess test active.
- **Design decisions accepted.** Keep the six declared choices, including
  fixed sum order, the square-root-free comparison within its explicit
  range, per-test constant handling, S2's non-shrinking correction, and the
  shared v1 report/Holm scaffolding. The seven original non-constant
  candidate fixture cases are unchanged in full. v1's fixture and both
  original acceptance reports pass independently; no runtime integration
  or dependency/identity change was introduced by these repairs.

Independent verification:

- `cargo test --locked --lib`: **158 passed, 1 ignored**, 25.53 s excluding
  compilation; includes candidate regressions and both v1 acceptance runs.
- `npm test`: **1052 passed across 61 files**; `npm run typecheck` passes.
- `rustfmt --check` for the candidate module/tests and `git diff --check`
  for the repaired commit range pass.
- All six jobs for this exact head pass in
  [CI run 37084566037](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37084566037):
  test, typecheck, build, cargo-check, e2e and native-smoke. The logs confirm
  **556 Rust (158 + 396 + 2), 1 ignored**, and **1052 Vitest**. Local full
  desktop tests, build, Playwright, timing measurements and mutation checks
  were not repeated; the relevant remote jobs and author evidence cover
  those checks separately.

Proceed with **P12e-6b after merge**: commit the diagnostic declarations
before executing the release runner, retain every outcome, and record the
selection and failures under the existing plan. This re-review used neither
diagnostic seed 20261005 nor final seed 20261117 and establishes no new
calibration claim. NUMERIC-JSON-001 remains a separate open audit; any later
change that affects declared diagnostic arithmetic must be recorded before
execution (or as a plan amendment after execution). P12e-FINDING-1 remains
open and P13 blocked until the planned acceptance is completed.

Only this handoff and `tasks.md` were updated by the re-review. No product
changes, commit, push, GitHub review submission, draft conversion or merge
were performed.

## Resolution — 2026-10-04, NUMERIC-JSON-001 audit

The separately scheduled audit is complete: six-case frontend/default/feature
evidence and real backend hash, SQLite save/read and artifact tests reproduce
the one-ULP identity mismatch and demonstrate why a global parser feature
change would invalidate a legacy interpretation. A release microbenchmark
measures 1.445× median parsing cost on its declared array. Product parser,
identity versions, established fixtures and user data remain unchanged.
See the [audit report](../docs/numeric-json-identity-audit-v1.md) and
[handoff](2026-10-04-numeric-json-audit-v1.md). NUMERIC-JSON-002 (P2) owns the
separate parser-policy/legacy-compatibility repair design; this resolution
closes the audit only and does not claim the numeric problem is fixed.
