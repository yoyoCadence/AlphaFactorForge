# Handoff: JSON float round-trip and identity audit

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `audit/numeric-json-roundtrip` (from main `f0459b0`; rebase after PR #153)
Status: Audit implementation complete; verifying and publishing. Parser/identity
repair remains a separate compatibility decision.

## Scope and plan

The maintainer requested ongoing autonomous task-board work with PRs and merges.
NUMERIC-JSON-001 is the next independent audit while native campaign and Tiingo
operator acceptance remain open. The PR #138 review already bounded its scope:
inventory float-bearing product read/rehash paths; pin Rust/TypeScript bits,
hashes and persistence evidence; evaluate parsing cost and old identity
compatibility before proposing a fix. It explicitly excludes a global parser
feature change or regeneration of existing hashes/calibration evidence.

1. Trace strategy saves/reads, discovery resume, canonical research JSON,
   campaign persistence, dataset and artifact boundaries; distinguish verified
   bytes from parse-and-rehash behavior.
2. Preserve a small numeric fixture and use real product hash/SQLite functions
   on isolated in-memory data to expose behavior, without touching user data.
3. Compare default serde_json with a command-only `float_roundtrip` build of
   an audit example, including repeatable release parsing timings.
4. Record compatibility risks and a bounded repair proposal. Run relevant
   checks, publish one audit PR, merge only after final-head CI passes.

## Findings / verification

The [audit report](../docs/numeric-json-identity-audit-v1.md) inventories product
boundaries, preserves default and roundtrip-enabled release probe outputs and
proposes a versioned repair. Six-case frontend evidence pins exact bits,
canonical bytes and strategy hashes. Three real backend tests confirm:

- The default parser changes the long decimal by one ULP; the frontend hash
  is refused by the verified SQLite save, with no write. The old backend
  hash saves and reads with the exact original text and verifies under the
  current policy. A feature flip would change that row's interpreted hash.
- Reconstructing canonical research JSON changes its numeric bits/hash,
  while the actual artifact store still verifies and returns original bytes.
- The other five controls agree. Feature-enabled parsing matches the frontend
  for all six inputs. No frequency or whole-product equivalence is claimed.

Release microbenchmark: default median **46.178 ms**, roundtrip **66.737 ms**
for 200 parses of the same 12,288-number array, approximately **1.445×** on
this machine/document; all three samples are recorded, not used as thresholds.
Both comparisons use command-only Cargo options; Cargo.toml/lockfile and
established identities/statistical fixtures remain unchanged. No user data
was read or edited and no full statistical acceptance was rerun.

Confirmed finding **NUMERIC-JSON-002 (P2)**: identity correctness and legacy
compatibility need a separate repair design. Recommended direction: bind the
new correctly rounded policy to versioned identities/configs, preserve and
explicitly verify legacy interpretation, retain original evidence and use an
additive re-freeze for conversions. A global feature flip or silent rehash is
not adopted. Native transport and full runner-resume counterexamples remain
repair acceptance work, not claims made by this audit.

## Verification / publication

After rebasing onto merged PR #153 (`d1bf8e2`): **1083 Vitest / 65 files;
582 Rust (170 library + 410 desktop + 2 service), 1 ignored**. Typecheck,
production build, all-target check and targeted rustfmt/diff checks pass;
clippy retains the five existing warnings. No rendered UI changed; local
Playwright not rerun, existing full e2e/native suites remain CI gates.

[PR #154](https://github.com/yoyoCadence/AlphaFactorForge/pull/154), base main,
head `audit/numeric-json-roundtrip`; audit commit `8efdf63` after rebase,
verification record `de49dc6`. The PR records final-head CI and merge status;
merge requires all six jobs to pass on its final head.

## Resolution — 2026-10-04, CI and merge

Final head `a310be5b6c8c54a7a8e93c444c4740dea7e1259b` passed all six jobs
(typecheck, test, build, cargo-check, e2e, native-smoke) in
[CI run 37212880656](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37212880656).
PR #154 was marked ready and merged on that head as
`78912188d634e0c3bd70f916e91cdaff02197ceb`. Audit done; NUMERIC-JSON-002
remains the separately scoped compatibility design/repair task.

## Resolution — 2026-10-07, bounded manual repair

NUMERIC-JSON-002a implements and locally verifies the manual params/blocks/code
save/load boundary with hash-covered definition/policy markers, isolated raw-token
binary64 parsing and verified policy-preserving edit preparation. Legacy rows keep
their original parser, JSON, identities and provenance; explicit copies add a new
row with an immutable parent. Original audit fixtures remain unchanged. Full
1131 Vitest / 600 Rust (1 ignored) / 85 E2E, typecheck/build/all-target check and
real isolated Tauri 18-case numeric plus legacy/copy/rejection smoke pass.
[Contract](../docs/manual-strategy-numeric-policy-v1.md),
[repair handoff](2026-10-07-manual-strategy-numeric-policy-v1.md).

PR delivery/CI/merge are pending at this record. The parent NUMERIC-JSON-002
remains open for discovery/runtime request/dataset/artifact versioning; this
manual slice does not close the audit's remaining transport/replay boundaries.

## Resolution — 2026-10-07, final-head CI and merge

[PR #166](https://github.com/yoyoCadence/AlphaFactorForge/pull/166) was marked
ready and merged at 20:56 Asia/Taipei on the verified final head
`a7f0783481681057ea1b545d3231edaa49021f2d`. All six checks passed in
[CI run 37623643276](https://github.com/yoyoCadence/AlphaFactorForge/actions/runs/37623643276).
Merge commit `0dafef5811282c5b711ae8e7b6725c8d8186046d` has the same tree
`335bec9e77b6796d34a71bee9c469e835654bc38` as that head; local main was
fetched and fast-forwarded. NUMERIC-JSON-002a and PR146-165-DOC-R1 are delivered.
The parent NUMERIC-JSON-002 and outstanding operator acceptance remain open.
The requested previous-20 review is recorded in
[PR126–145 acceptance](2026-10-07-pr126-145-acceptance-review-v1.md).
