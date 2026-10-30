# Handoff: Bounded original market consumer contract

Date: 2026-10-29 (UTC+8)
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/original-market-consumer-contract
Status: Locally verified design/data; independent review approved, delivery CI pending.

## Scope

Backlog → Next → In Progress precedes this refinement of the [producer contract](../docs/plans/research-original-market-source-projections-v1.md#5-separate-next-code-actual-snapshot-controls). Freeze a single sealed-graph market entry, exact original projections and supported historical consistency boundary, joint preparation and complete post-preparation warmup/market resource accounting before code. Independent family/market controls must bind actual producer originals rather than synthetic SQL market success rows. Production code/schema/dependencies/old fixtures remain unchanged in this design.

The source graph captures selected snapshot components, not every successor anywhere in the DB. It contains no candles or raw files and version-only action/cost references. Neither current provenance head nor original bar observability/candle/coverage/action/cost authentication follows. The later file/candle/availability and final current-owner/registry gates remain mandatory; RegisteredSource/numericCostsHash/sourceEvidenceHash remain unavailable.

## Delivery

Validate independent Python/Node fixtures, exact schemas/scalar bindings/resource arithmetic, source/package pins, links, append-only #238 receipt and scope. No local runtime suite is rerun solely for documentation/unconsumed data. All six completed exact-head/base CI jobs, fresh reviews/main, expected-head normal merge, GitHub readback and clean-main/full-tree equality complete this slice. The user authorizes continuous development/PR/merge cycles; append this verified receipt in the next code slice and continue in order. Market/source/live parents remain incomplete.

## Local verification — 2026-10-30

Standalone Python json/hashlib/struct reproduction preserves all 122944 fixture
bytes exactly: LF SHA256
`d6768ea8cf25db6dfff1250e1f7808ca06f3a2121de1eca174b2a8a58ddeb9a8`.
Separate Node built-ins verify six source-package pins, six pre-code consumer
pins, all four complete family/config/input/hypothesis/campaign binary identities,
original document/hash/scalar manifests, joint preparation and W/C/A/Q/O, five
coherent refusals and four exact/+1 recipes. Strict duplicate-free/finite JSON,
relative links, append-only prior handoff and diff checks pass. GitHub readback
confirms #238 merged with the exact recorded head/base/merge commit; fresh fetch
leaves this branch at the same baseline. Production code and prior fixtures are
unchanged. Independent review and delivery CI remain required before merge.

## Independent review — 2026-10-30

A separate reviewer session checked the original producer rules/campaign binary
codec, captured row manifests, work formulas, old warmup Q bound, complete source
membership and capability limitations; recommendation: approve after the final
verification receipt and six delivery jobs. No correctness blockers were found.
Its sole publication note requested the fixture packaging SHA, now recorded
above. Actual Rust implementation and its mutation/real-capture controls remain
the next separately gated task.
