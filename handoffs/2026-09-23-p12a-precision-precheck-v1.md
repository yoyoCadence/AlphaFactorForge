# Handoff: P12a statistical precision precheck

Date: 2026-09-23
Repo: yoyoCadence/AlphaFactorForge
Branch: `feat/p12a-precision-precheck` (from main `7aefaa9`, P11 merged via PR #114)
PR: [#115](https://github.com/yoyoCadence/AlphaFactorForge/pull/115) (ready for review; merge is maintainer-owned)
Status: implementation and local verification complete; review pending. P12 parent phase stays In Progress.

## Summary

P12 (research feasibility & trial ledger) is too large for one session, so it is
split into P12a–d in `tasks.md`. P12a adds the pure Rust contract
`research-precision-v1`: given the trial family size, alpha, a relative Monte
Carlo error limit, the declared bootstrap sample count and a sample cap, it
returns `ELIGIBLE` / `NOT_ELIGIBLE` with every failing reason. It closes the
"AlphaBTC precision counterexample is blocked" half of the P12 acceptance; the
"trial counts cannot be reset" half needs the ledger (P12b).

## Scope and decisions

- **Exact arithmetic.** Alpha and the error limit are integer ppm; all decisions
  are `u128` integer inequalities, so boundaries (2918/2919, 72974/72975) cannot
  drift with float rounding.
- **Three checks**, reported in fixed order: Holm resolution
  (`m/(B+1) ≤ α`), Monte Carlo relative standard error at the strictest Holm
  threshold `α/m`, and whether the larger requirement fits
  `maxBootstrapSamples`.
- **No product defaults.** Alpha and the error limit must be declared by the
  plan. The fixture's 20% error limit is a test choice. AlphaBTC's fixed 1,000
  samples are not copied (plan §2/§4.5).
- **Strict parsing** in the `discovery-config-v1` style: unknown fields,
  float-literal counts, negatives and values above 2^53−1 are rejected with
  path-qualified messages in a fixed order.
- **Rust only.** The consumer will be the backend confirmation/admission path;
  the fixture is language-neutral for a future TypeScript reader.
- **Not wired.** No runner, command, migration, dependency or UI change;
  `priorTrials` is caller-supplied until the ledger exists.

Files: `alpha-factor-forge/src-tauri/src/discovery_core/precision.rs` (new),
`discovery_core/mod.rs` (+1 line), `alpha-factor-forge/fixtures/rs-core/research-precision-v1.json`
(new), `docs/research-precision-v1.md` (new), plus status updates in `tasks.md`,
`CHANGELOG.md`, `docs/plans/active-plan.md`, and
`docs/autonomous-research-capability-registry.md`.

## Required Action / Decision

1. Review the contract, especially: whether the Monte Carlo precision criterion
   (relative SE at `α/m`) is the check you want before P12b builds on it, and
   the reason names/order.
2. P12b needs a spec before code: ledger schema (migration), trial-kind
   classification, where the ledger lives relative to the workspace (plan §4.4
   places the Test registry outside it; decide whether the trial ledger follows),
   and how a restore/re-import unions rather than resets counts.

## Review Notes

- Growing the family can only raise requirements (tested for priorTrials
  0 → 1,000,000), which is the property the ledger will rely on.
- Requirements above 2^53−1 are reported as `null`; the budget reason is then
  always present because the cap itself is bounded by 2^53−1.
- `ELIGIBLE` is a statement about sampling capacity only, not about any
  candidate.

## Verification

- `cargo test --locked` (CARGO_TARGET_DIR outside OneDrive): **385 passed**
  (87 lib + 296 bin + 2 service smoke), including 12 new precision tests.
- `cargo clippy --locked --all-targets`: only the 5 pre-existing warnings; none
  in the new file. `rustfmt --check` on the new file passes (repo-wide fmt drift
  is CI-RUSTFMT-001, untouched).
- `npm run typecheck` pass, `npm test` **973/973**, `npm run build` pass.
- Playwright not rerun: no frontend file changed.
- Fixture expectations were derived independently with Node BigInt, not by
  running the Rust implementation.
