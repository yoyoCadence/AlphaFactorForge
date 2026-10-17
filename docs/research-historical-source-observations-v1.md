# Original historical trial source observations

P13c-3b-1b-1 implements the bounded read-only
[reconstruction plan](plans/research-historical-observation-plan-v1.md) after
merged #195. The backend primitive has no runtime caller, IO, SQL mutation,
import/adoption or execution authority. Persistence and the binding observer
remain separate tasks; the runtime still writes v4 and uses schema 6.

## Inputs and exact checks

`transfer::reconstruct_historical_source` takes original complete trial v3
archive bytes, an immutable format-checked v5 `BindingSnapshot` and typed
`RegisteredSource`. Original input is at most 8 MiB before decoding. Reuse
complete v3 header/body hashes, canonical records, event chain, batch members,
protocols and complete payload/kind validation. Every failure is a static
`registry_state_invalid` diagnostic that omits rejected input.

Reproduction references resolve only inside that original archive. Existing
imports use the same resolver-based validator and retain their actual receiver
SQL fallback. The new primitive has no receiver parameter or fallback. Existing
refusal of nonportable exempt-benchmark provenance also remains in force; this
is not permission to reinterpret effective flags or change trial counting.

Match original registry ID, sequence and chain head exactly to the v5 snapshot.
Derive all source trial quarantines, origin conflicts and pinned/upgraded
protocol high-water marks from source rows and require exact binding equality.
An event head alone cannot freeze protocol upgrades made without new events.
Valid restrictive source snapshots remain observable; observation is no permit.

Validate registered-source domains/order through the existing pure validator,
now public to the backend. Locate its actual original batch and family, and
match every original candidate index/strategy hash and workspace, with complete
membership in both directions. Equal lengths cannot hide substituted indices,
hashes, workspaces or families. A Test subset cannot become the original source.

## Derived facts and claims

Count actual effective events across the entire original source family and
registered batch, derive target tests per trial from its pinned/upgraded
protocol and complete-candidate count from verified membership. Receipt
family-before totals and current receiver counts are not used for these facts.

`HistoricalSourceObservation` is private to the ledger implementation and has
read-only getters for original archive SHA, prefix, historical trial facts and
`source_claims`. Artifact hashes/bytes, campaign/run/engine linkage and physical
source availability are not recorded by the trial schema. Those metadata
remain supplied domain-checked claims, explicitly tested as such. The primitive
does not authenticate a publisher, prove unseen completeness or mint a fresh
`AdmissionCount`, owner fence, alpha share or execution/recovery capability.

A source prefix with four trials remains four after the receiver has five.
Two divergent original branches can each remain verifiable against their own
exact original binding after actual union records an origin conflict. Neither
needs to become the receiver's current primary branch. Protocol upgrades with
the same event head require the original matching restrictive evidence.

## Extension boundary

This primitive proves observations against the supplied original snapshot. It
does not save that snapshot or extend blob kinds/schema/wire formats. Durable
context must preserve these actual original bytes and source claims with their
correct binding/dependencies before the v5 runtime observer can activate.
Rebuilding an old source from receiver-primary rows or copying admission
receipt counts is insufficient. Public legacy omission/provenance guards stay
active; a private trial projection is no known-empty alpha/confirmation claim.

P13c-3b-1b-2 owns the next storage/dependency/downgrade design and its bounded
implementation slices. Whole-graph observation/adoption and actual old-build
schema/binding refusal remain 3b-1b-3. P13c-4 still requires authenticated actual
source/owner/current counts and actual-share precision in the final transaction.
No statistical policy, calibration or reserved final seed changes.
