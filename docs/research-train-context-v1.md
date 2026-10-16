# Pure Train-only generation context v1 (P13c-2)

This implements [P13c §8](plans/research-confirmation-exposure-recovery-plan-v1.md)
and the [provider information boundary](ai-provider-contract.md#7-資訊邊界與-p12p13-共同執行).
It has no provider, prompt template, IO, persistence or runtime caller. Exact
fields and bounds below are fixed before implementation.

## Scope and evidence

`freeze_train_scope` checks original JSON with exactly `contractVersion`
(`research-train-scope-v1`), `workspaceId` (32 lowercase hex), positive `runId`,
`campaignId`, canonical `instrumentId`, `trainDataHash`, `trainWindow` and
`engineManifestHash`. Ordinary hashes are 64 lowercase hex; `trainDataHash`
is `dataset-content-v2:<hash>` of **Train-only** data, never a full mixed
snapshot. `trainWindow` is the existing checked half-open market-time interval.
No Validation/Test region, result, selection or sealed receipt is a field.

The host must authenticate the scope, original Train-only bytes, actual input
support and every origin's derivation/approval before supplying them. It must
prove all origins, including their inclusion/order, were chosen from Train
alone. A JSON `train_only` label or self-consistent hash does not prove that.
This API checks consistency with supplied evidence; it cannot detect a host
that invents or truncates that evidence. It grants no execution authority.

`freeze_train_origin` accepts exactly `contractVersion`
(`research-train-origin-v1`), `scopeId`, `provenance`, `inputs` and `fact`.
The aggregate provenance and **every** input provenance must be `train_only`;
`validation`, `test`, `mixed`, `unknown` are recognized and refused. Each input
has exactly `provenance`, `instrumentId`, `trainDataHash`, `support`. Instrument
and Train data hash must equal the scope; every support interval is contained
in its Train window. Sets use P13c-1's canonical sorted/disjoint/merged rules.
This bounded v1 supports one instrument and one Train data slice only.

The closed tagged `fact` union is:

| `kind` | Other exact fields | Allowed values |
| --- | --- | --- |
| `summary` | `observations`, `closedTrades`, `netReturnPpm` | Positive observation count, nonnegative trade count, signed integer ppm return |
| `hypothesis` | `mechanism`, `invalidation` | Mechanism: `trend_following`, `mean_reversion`, `breakout`; invalidation: `insufficient_trades`, `costs_dominate`, `unstable_subwindows` |
| `failure` | `class` | `insufficient_history`, `no_trades`, `negative_net_return`, `unstable_subwindows` |

No free-form hypothesis, rationale, human note, arbitrary string, arbitrary
metric map, plot, candidate ID, selection, report or hash attachment exists.
The host must create/approve the integer summary under this version; this
module does not round legacy floats or reinterpret saved metrics. Further
summary/regime/sensitivity vocabulary requires an explicit contract extension.

## Assembly and fresh protection

`build_train_context` accepts a strict request with exactly `contractVersion`
(`research-train-context-request-v1`), `scopeId`, `originIds`. It also requires
the checked scope, the **entire** approved origin set in ascending unique ID
order and a current typed `ObservedProtection` input. Request origins must
equal that complete set; no subset, extra member, reordering or substitution.
Origins from another scope are refused even if their instrument is the same.

`ObservedProtection` supplies complete/unknown history knowledge, canonical
exposure footprints and known protected references. Unknown history refuses
assembly. Any protected footprint intersecting the whole Train window on the
same canonical instrument refuses it, including legacy uncertainty represented
by conservative support. Dataset/source/timeframe/role cannot reset occupancy.
Touching endpoints and other verified instruments do not intersect.

Known protected references refuse scope IDs, origin IDs and any input/scope
hash reference. A protected digest cannot be laundered by adding a recognized
hash prefix: compare digest bytes across bare SHA-256, `dataset-content-v2:`
and `strategy-v2:` forms. This deny check complements the Train-only origin
allowlist; it does not establish complete provenance by itself.

Output is exactly `{id, body}` with body fields `contractVersion`
(`research-train-context-v1`), `scopeId`, `scope`, `facts`. Each fact contains
only `originId` and its checked `fact`. Assemble those fields explicitly;
never serialize a generic report or supplied evidence object. Scope and
origin IDs use SHA-256(version UTF-8 + zero byte + existing canonical body
bytes). Context ID uses the same rule on the allowlisted context body.
Only Train evidence enters the output or its identity; observed protection
history, protected references, admission decisions and rejected input do not.

`parse_train_context` verifies stored output against the same complete origin
set and **current** protection observation, reassembling the expected body.
A previously valid context cannot bypass newly observed exposure or unknown
history. The future host must take a fresh synchronized observation at every
use; these in-memory values have no expiry/lease or persistent authority.

## Bounds and refusal

Each raw scope, origin, request and stored context is at most 64 KiB UTF-8,
checked before parsing; output must also fit. There are 1–64 approved origins,
1–8 inputs per origin, 1–128 canonical intervals per input, at most 4096
observed footprints and 4096 protected references. Counts and signed ppm
magnitudes are at most 9007199254740991. All numeric fields require integer
JSON literals; floats/exponents, unknown/missing/duplicate nested fields,
unsupported versions and unknown enum codes are refused. No truncation,
redaction or renumbering. Checked types have private fields and no Deserialize.

Failures return fixed error codes/messages without the supplied keys, values,
hashes or parser diagnostics. Rejected protected data must not leak through
provider-readable error logs. The builder itself logs nothing.

## Required evidence and next step

Commit a positive independently hashed Train fixture. Test every scope/input
binding, mixed/unknown provenance, complete-set selection, nested protected
values/notes/strings/hash attachments, protected digest aliases, stale scope,
new/unknown exposure on replay, half-open boundaries, errors and capacities.
No provider is called. No calibration, final seed or real-market policy change.
P13c-3 schema/receipt/transfer/binding design follows in its own task/PR.
