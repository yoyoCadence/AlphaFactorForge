# Handoff: JSON float round-trip and identity audit

Date: 2026-10-04
Repo: yoyoCadence/AlphaFactorForge
Branch: `audit/numeric-json-roundtrip` (from main `f0459b0`; rebase after PR #153)
Status: In Progress; audit only, parser/identity repair is a separate decision.

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

Pending. This task does not run or revise the final statistical acceptance.
