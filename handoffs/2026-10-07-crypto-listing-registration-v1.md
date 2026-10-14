# Handoff: CAMPAIGN-LISTING-001 repair

Date: 2026-10-07 (Asia/Taipei)
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/crypto-listing-registration`
Baseline: `77c1bda6ffb57365b2b9003dd89f3a155123a93e`
PR: Pending implementation and verification.
Status: In progress; maintainer authorized repair, PR and merge.

## Summary

The #126–145 acceptance review reproduced a formal Binance BTC snapshot whose
immutable instrument has no listing date. Native campaign Add correctly refused
it, and there was no supported registration entry. The accepted repair adds the
bounded service CLI in [the contract](../docs/crypto-listing-registration-v1.md).
It preserves source evidence in a new instrument revision; re-fetch builds a new
snapshot. Old rows, snapshot identity and missing-listing refusal remain intact.

## Required action and scope

- Strict versioned settings and bounded byte-preserving, operator-reviewed
  Binance source evidence are validated before workspace acquisition.
- Register through the existing owner epoch and transaction, preserving other
  instrument metadata and rejecting contradictory known listing assertions.
- Verify exact retry, immutability, ownership, bounds and source integrity in
  Rust tests; validate actual CLI fetch and native campaign UI/reopen.
- Publish one scoped repair PR, verify its final-head CI and merge under the
  maintainer's existing authorization.

No schema, credentials, automatic listing inference, UI metadata editor or
numeric-policy rollout belongs to this repair. A source checksum proves byte
integrity; it does not authenticate an operator's listing assertion.

## Verification

Pending implementation checks and controlled live/native acceptance. The
source used is the official USDT announcement linked in the contract, with a
short reviewed capture and explicit UTC conversion rather than a candle-derived
date. The native harness must clean its owned processes and temporary paths.

## Resolution

Pending. Append implementation, exact verification, PR/head and remaining
acceptance limits when acted on; preserve the original review evidence.
